#[cfg(not(target_arch = "wasm32"))]
use super::native::{Message, Sink, StreamError};
#[cfg(target_arch = "wasm32")]
use super::wasm::{Message, Sink, StreamError};

use std::collections::HashSet;
use tokio::sync::{oneshot, broadcast};
use futures_util::SinkExt;
use serde::Serialize;
use esplora_client::Script;

#[derive(Debug, Clone)]
pub enum Event {
    Close,
    Ping,
    Subscribe(Vec<Script>),
    Unsubscribe(Vec<Script>),
}

#[derive(Serialize)]
struct TrackSPKsMessage<'a> {
    #[serde(rename = "track-scriptpubkeys")]
    track_scriptpubkeys: Vec<&'a Script>,
}

pub struct Manager {
    ws_tx: Sink,
    control_receiver: broadcast::Receiver<Event>,
    disconnect_channel: broadcast::Sender<bool>,
    close_channel: Option<oneshot::Sender<bool>>
}

impl Manager {
    pub fn new(
        ws_tx: Sink,
        control_receiver: broadcast::Receiver<Event>,
        disconnect_channel: broadcast::Sender<bool>,
        close_channel: Option<oneshot::Sender<bool>>
    ) -> Self {
        Self {
            ws_tx,
            control_receiver,
            disconnect_channel,
            close_channel,
        }
    }

    /// Handles control signals from an mpsc channel
    pub async fn start(
        &mut self,
        id: u32,
    ) {
        log::trace!("starting control loop {}", id);
        let mut active_spks = HashSet::new();
        let mut disconnect_receiver = self.disconnect_channel.subscribe();

        loop {
            log::trace!("...control loop... {}", id);
            tokio::select! {
                _ = disconnect_receiver.recv() => {
                    log::trace!("disconnect signal received! breaking control loop {}", id);
                    break;
                }

                Ok(event) = self.control_receiver.recv() => {
                    log::trace!("control event received {:?} {}", event, id);
                    match event {
                        Event::Close => {
                            log::trace!("CLOSE control received close request {}", id);
                            let _ = self.ws_tx.close().await;
                            let _ = self.close_channel.take().map_or(Ok(()), |close_sender| close_sender.send(true));
                        },
                        Event::Ping => {
                            log::trace!("websocket ping requested {}", id);
                            let message = "{\"action\": \"ping\"}".to_string();
                            let _ = self.ws_tx.send(Message::Text(message)).await;
                        }
                        Event::Subscribe(scriptpubkeys) => {
                            log::trace!("control subscribing to new addresses {:?} {}", scriptpubkeys, id);
                            let mut changed = false;
                            for scriptpubkey in scriptpubkeys {
                                changed |= active_spks.insert(scriptpubkey);
                            }

                            if changed && self.update_scriptpubkeys_subscription(
                                active_spks.iter().collect()
                            ).await.is_err() {
                                log::trace!("DISCONNECT control failed to update websocket subscription (sub) {}", id);
                                let _ = self.disconnect_channel.send(true);
                                break;
                            }
                        }
                        Event::Unsubscribe(scriptpubkeys) => {
                            log::trace!("control unsubscribing from addresses {:?} {}", scriptpubkeys, id);
                            let mut changed = false;
                            for scriptpubkey in scriptpubkeys {
                                changed |= active_spks.remove(&scriptpubkey);
                            }

                            if changed && self.update_scriptpubkeys_subscription(
                                active_spks.iter().collect()
                            ).await.is_err() {
                                log::trace!("DISCONNECT control failed to update websocket subscription (unsub) {}", id);
                                let _ = self.disconnect_channel.send(true);
                                break;
                            }
                        }
                    }
                }
            }
        }
        log::trace!("ending control loop {}", id);
    }

    async fn update_scriptpubkeys_subscription(
        &mut self,
        scriptpubkeys: Vec<&Script>,
    ) -> Result<(), StreamError> {
        log::trace!("updating websocket subscription: {:?}", scriptpubkeys);
        let message = TrackSPKsMessage {
            track_scriptpubkeys: scriptpubkeys,
        };
        let json_message = serde_json::to_string(&message).unwrap();
        self.ws_tx.send(Message::Text(json_message)).await
    }
}