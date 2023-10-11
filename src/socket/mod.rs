use crate::compat;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod wasm;

mod connection;
mod control;
mod message;
mod ping;

use connection::Status;
use control::Event;
pub use message::WebsocketEvent;

use tokio::sync::broadcast;
use bitcoin::Script;


#[derive(Clone)]
pub struct Client {
    manager: connection::Manager,
}

impl Client {
    pub fn new(ws_url: String) -> Self {
        Self {
            manager: connection::Manager::new(ws_url),
        }
    }

    /// Connect to the websocket and keep it alive
    /// resolves the first time the websocket successfully connects
    pub async fn start(&self, wait_for_connection: bool) {
        log::trace!("starting websocket");
        log::trace!("spawning thread to start connection");
        let mut manager = self.manager.clone();
        compat::spawn(async move {
            manager.start().await;
        });

        log::trace!("waiting for socket to finish trying to connect");
        if wait_for_connection {
            let mut rx = self.manager.subscribe_to_status();
            loop {
                let event = rx.recv().await;
                if let Ok(Status::Connected | Status::Offline) = event {
                    log::trace!("Initial websocket connection established!");
                    break;
                }
                log::trace!("socket manager state changed to {:?}", event);
            }
            log::trace!("returning from socket::start");
        }
    }

    /// Disconnect the websocket and stop trying to reconnect
    /// resolves once all websocket handling threads have been cleaned up
    pub async fn stop(&self, wait_for_close: bool) {
        log::trace!("starting websocket");
        let fut = self.manager.stop();
        if wait_for_close {
            log::trace!("waiting for websocket to close");
            fut.await;
        }
        log::trace!("returning from socket::stop");
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WebsocketEvent> {
        self.manager.subscribe_to_messages()
    }

    pub fn track_scriptpubkeys(&self, scriptpubkeys: &[Script]) {
        log::trace!("socket track_scriptpubkeys");
        self.manager.track_scriptpubkeys(scriptpubkeys.to_vec());
    }

    pub fn untrack_scriptpubkeys(&self, scriptpubkeys: &[Script]) {
        log::trace!("socket untrack_scriptpubkeys");
        self.manager.untrack_scriptpubkeys(scriptpubkeys.to_vec());
    }
}
