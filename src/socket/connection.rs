use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use super::native::connect;
#[cfg(target_arch = "wasm32")]
use super::wasm::connect;

use bitcoin::Script;
use tokio::sync::{broadcast, oneshot, RwLock};
use tokio::task::JoinHandle;

use crate::compat;
use crate::socket::control::Event;
use crate::socket::message::WebsocketEvent;
use crate::socket::{control, message, ping};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ready,        // offline, want to be online
    Connected,    // online
    Connecting,   // waiting to connect
    Disconnected, // temporarily disconnected
    Offline,      // want to be offline
}

/// utility struct for a status which must always broadcast new values
pub struct StatusUpdater {
    status: Status,
    sender: broadcast::Sender<Status>,
}

impl StatusUpdater {
    const fn get(&self) -> Status {
        self.status
    }

    pub fn update(&mut self, status: Status) {
        self.status = status;
        let _ = self.sender.send(status);
    }
}

#[derive(Clone)]
pub struct Manager {
    ws_url: String,
    status_sender: broadcast::Sender<Status>,
    event_sender: broadcast::Sender<WebsocketEvent>,
    control_sender: broadcast::Sender<Event>,
}

impl Manager {
    pub fn new(
        ws_url: String,
    ) -> Self {
        // TODO: replace these broadcast channels with intermediated watch channels?
        let (status_sender, _) = broadcast::channel(1);
        let (event_sender, _) = broadcast::channel(256);
        // TODO: replace the control broadcast channel with an intermediated mpsc channel?
        let (control_sender, _) = broadcast::channel(256);
        Self {
            ws_url,
            status_sender,
            event_sender,
            control_sender,
        }
    }

    pub fn subscribe_to_status(&self) -> broadcast::Receiver<Status> {
        self.status_sender.subscribe()
    }

    pub fn subscribe_to_messages(&self) -> broadcast::Receiver<WebsocketEvent> {
        self.event_sender.subscribe()
    }

    pub fn track_scriptpubkeys(&self, scriptpubkeys: Vec<Script>) {
        log::trace!("connection track_scriptpubkeys");
        let result = self.control_sender.send(Event::Subscribe(scriptpubkeys));
        log::trace!("sent Subscribe control event, result: {:?}", result);
    }

    pub fn untrack_scriptpubkeys(&self, scriptpubkeys: Vec<Script>) {
        log::trace!("connection untrack_scriptpubkeys");
        let result = self.control_sender.send(Event::Unsubscribe(scriptpubkeys));
        log::trace!("sent Unsubscribe control event, result: {:?}", result);
    }

    /// Executes a state machine to manage the websocket connection
    pub async fn start(&mut self) {
        let mut status = StatusUpdater {
            status: Status::Ready,
            sender: self.status_sender.clone(),
        };
        let mut close_receiver: Option<oneshot::Receiver<bool>> = None;
        let (disconnect_sender, mut disconnect_receiver) = broadcast::channel(1);
        let mut handles: Option<Vec<Option<JoinHandle<()>>>> = None;
        loop {
            log::trace!("connect loop {:?}", status.get());
            match status.get() {
                // Offline => exit
                Status::Offline => {
                    log::trace!("waiting for threads to exit");
                    if let Some(handles) = handles.take() {
                        for handle in handles.into_iter().flatten() {
                            handle.await.expect("websocket thread failed");
                        }
                    }
                    log::trace!("joined loop threads");
                    break
                }
                // Ready => Connecting
                Status::Ready => {
                    log::trace!("ready => connecting");
                    status.update(Status::Connecting);
                }
                // Connecting => Connected | Disconnected
                Status::Connecting => {
                    log::trace!("trying to connect");
                    // need a fresh oneshot channel for signalling socket closure
                    if let Some((h, c)) = self.connect(disconnect_sender.clone(), self.event_sender.clone()).await {
                        handles = Some(h);
                        close_receiver = Some(c);
                        status.update(Status::Connected);
                    } else {
                        handles = None;
                        close_receiver = None;
                        status.update(Status::Disconnected);
                    }
                },
                // Disconnected => Ready (delayed to rate-limit reconnections)
                Status::Disconnected => {
                    log::trace!("waiting for threads to exit");
                    if let Some(handles) = handles.take() {
                        for handle in handles.into_iter().flatten() {
                            handle.await.expect("websocket thread failed");
                        }
                    }
                    log::trace!("joined loop threads");
                    self.notify(WebsocketEvent::Disconnected);
                    log::trace!("reconnecting in 60 seconds");
                    compat::sleep(30_000).await;
                    status.update(Status::Ready);
                }
                // Connected => steady state until CLOSE or ERROR
                Status::Connected => {
                    let mut close_signal = close_receiver.take().expect("can never reach a Connected state without (re)initializing the close channel");
                    tokio::select! {
                        // Connected => Disconnected
                        _ = disconnect_receiver.recv() => {
                            log::trace!("event or control thread exited");
                            status.update(Status::Disconnected);
                        }
                        
                        // Connected => Offline
                        close_event = &mut close_signal => {
                            match close_event {
                                Ok(_) => {
                                    log::trace!("received request to close connection");
                                    status.update(Status::Offline);
                                }

                                Err(e) => {
                                    log::trace!("close_receiver threw an error {:?}", e);
                                    status.update(Status::Disconnected);
                                }
                            }
                            // tell threads to exit
                            let _ = disconnect_sender.clone().send(true);
                        }
                    }
                }
            }
        }
        self.notify(WebsocketEvent::Offline);
        log::trace!("ended ws connection loop");
    }

    pub async fn stop(&self) {
        log::trace!("stopping websocket");
        let _ = self.control_sender.send(Event::Close);
        // wait for websocket to finish closing
        let mut rx = self.status_sender.subscribe();
        while let Ok(status) = rx.recv().await {
            if status == Status::Offline {
                log::trace!("Websocket closed!");
                break;
            }
        }
    }

    async fn connect(&mut self, disconnect_channel: broadcast::Sender<bool>, event_sender: broadcast::Sender<WebsocketEvent>) -> Option<(Vec<Option<JoinHandle<()>>>, oneshot::Receiver<bool>)> {
        log::trace!("Connecting to {}", self.ws_url);

        #[cfg(not(target_arch = "wasm32"))]
        let connection = connect(&self.ws_url, Some(Duration::from_secs(60))).await;
        #[cfg(target_arch = "wasm32")]
        let connection = connect(&self.ws_url).await;

        let (close_sender, close_receiver) = oneshot::channel();
        let last_response = Arc::new(RwLock::new(compat::now()));

        // Connect
        match connection {
            Ok((ws_tx, ws_rx)) => {
                log::trace!("Connected to {}", self.ws_url);

                let control_disconnect = disconnect_channel.clone();
                let control_receiver = self.control_sender.subscribe();
                let control_handle = compat::spawn(async move {
                    let mut manager = control::Manager::new(
                        ws_tx,
                        control_receiver,
                        control_disconnect,
                        Some(close_sender)
                    );
                    manager.start().await;
                });
                let message_disconnect = disconnect_channel.clone();
                let message_timer = last_response.clone();
                let message_handle = compat::spawn(async move {
                    let mut manager = message::Manager::new(
                        ws_rx,
                        event_sender.clone(),
                        message_disconnect,
                        message_timer,
                    );
                    manager.start().await;
                });
                let ping_controller = self.control_sender.clone();
                let ping_disconnect = disconnect_channel.clone();
                let ping_handle = compat::spawn(async move {
                    let mut manager = ping::Manager::new(
                        ping_controller,
                        ping_disconnect,
                        last_response,
                    );
                    manager.start().await;
                });
                self.notify(WebsocketEvent::Connected);
                Some((
                    vec![control_handle, message_handle, ping_handle],
                    close_receiver,
                ))
            }
            Err(err) => {
                log::warn!("Failed to connect to {}: {:?}", self.ws_url, err);
                self.notify(WebsocketEvent::Error);
                None
            }
        }
    }

    fn notify(&self, event: WebsocketEvent) {
        let _ = self.event_sender.send(event);
    }
}