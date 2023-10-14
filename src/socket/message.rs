use crate::compat;
use crate::wallet::address::Event as AddressEvent;

#[cfg(not(target_arch = "wasm32"))]
use super::native::{Message, Stream};
#[cfg(target_arch = "wasm32")]
use super::wasm::{Message, Stream, StreamError};

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};
use futures_util::StreamExt;
use serde::Deserialize;
use esplora_client::{ScriptBuf, Tx};

#[derive(Debug, Clone)]
pub enum WebsocketEvent {
    AddressEvent(AddressEvent),
    Offline,
    Disconnected,
    Connected,
    Error,
}

#[derive(Deserialize)]
struct WebsocketResponse {
    #[serde(rename = "multi-scriptpubkey-transactions")]
    multi_scriptpubkey_transactions: Option<HashMap<ScriptBuf, WebsocketAddressTransactions>>,
}

#[derive(Deserialize)]
struct WebsocketAddressTransactions {
    mempool: Vec<Tx>,
    confirmed: Vec<Tx>,
    removed: Vec<Tx>,
}

pub struct Manager {
    ws_rx: Stream,
    event_sender: broadcast::Sender<WebsocketEvent>,
    disconnect_channel: broadcast::Sender<bool>,
    last_response: Arc<RwLock<Duration>>,
}

impl Manager {
    pub fn new(
        ws_rx: Stream,
        event_sender: broadcast::Sender<WebsocketEvent>,
        disconnect_channel: broadcast::Sender<bool>,
        last_response: Arc<RwLock<Duration>>,
    ) -> Self {
        Self {
            ws_rx,
            event_sender,
            disconnect_channel,
            last_response
        }
    }

    pub async fn start(&mut self, id: u32) {
        log::trace!("starting event loop {}", id);
        let mut disconnect_receiver = self.disconnect_channel.subscribe();
        loop {
            log::trace!("...event loop... {}", id);
            tokio::select! {
                _ = disconnect_receiver.recv() => {
                    log::trace!("disconnect signal received! breaking event loop {}", id);
                    break;
                }

                Some(msg) = self.ws_rx.next() => {
                    #[cfg(target_arch = "wasm32")]
                    let msg = Ok::<Message, StreamError>(msg);
                    {
                        let mut response_time = self.last_response.write().await;
                        *response_time = compat::now();
                    }
                    match msg {
                        Ok(Message::Text(text)) => {
                            log::trace!("handling websocket event {}", id);
                            self.handle_event(text.as_str());
                        }

                        Err(e) => {
                            log::trace!("DISCONNECT message error in websocket event loop {:?} {}", e, id);
                            let _ = self.disconnect_channel.send(true);
                        }

                        x => {
                            log::trace!("unexpected ws message received {:?} {}", x, id);
                        }
                    }
                }
            }
        }
        log::trace!("ending event loop {}", id);
    }

    fn handle_event(&self, json_message: &str) {
        let response: Result<WebsocketResponse, serde_json::Error> =
        serde_json::from_str(json_message);
        match response {
            Ok(message) => {
                if let Some(payload) = message.multi_scriptpubkey_transactions {
                    log::trace!("broadcasting multi-spk transactions event");
                    self.notify_spk_transactions(&payload);
                }
            }
            Err(e) => {
                log::error!("failed to parse websocket response {:?}", e);
            }
        }
    }

    fn notify_spk_transactions(&self, spk_transactions: &HashMap<ScriptBuf, WebsocketAddressTransactions>) {
        for (scriptpubkey, txs) in spk_transactions {
            self.notify_transations_for_spk(
                AddressEvent::Removed,
                scriptpubkey,
                &txs.removed,
            );
            self.notify_transations_for_spk(
                AddressEvent::Mempool,
                scriptpubkey,
                &txs.mempool,
            );
            self.notify_transations_for_spk(
                AddressEvent::Confirmed,
                scriptpubkey,
                &txs.confirmed,
            );
        }
    }

    fn notify_transations_for_spk(
        &self,
        event: impl Fn(ScriptBuf, Tx) -> AddressEvent,
        scriptpubkey: &ScriptBuf,
        txs: &[Tx],
    ) {
        for tx in txs {
            log::trace!(
                "broadcasting websocket event involving scriptpubky {} and tx {}",
                scriptpubkey,
                tx.txid
            );
            let _ = self.event_sender.send(WebsocketEvent::AddressEvent(event(
                scriptpubkey.clone(),
                tx.clone(),
            )));
        }
    }
}