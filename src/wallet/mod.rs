use crate::api;
use crate::socket::{self, WebsocketEvent};
use crate::compat;
use bitcoin::ScriptBuf;
pub use esplora_client;
use tokio::sync::{broadcast, Mutex};

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

pub mod address;
use address::{State, Tracker};

pub struct Options {
    pub hostname: String,
    pub secure: bool,
}

#[derive(Debug)]
pub enum Error {
    EsploraError(esplora_client::Error),
    Missing,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

macro_rules! impl_error {
    ( $from:ty, $to:ident ) => {
        impl_error!($from, $to, Error);
    };
    ( $from:ty, $to:ident, $impl_for:ty ) => {
        impl std::convert::From<$from> for $impl_for {
            fn from(err: $from) -> Self {
                <$impl_for>::$to(err)
            }
        }
    };
}

impl std::error::Error for Error {}
impl_error!(esplora_client::Error, EsploraError, Error);

#[derive(Debug, Clone)]
pub enum Event {
    Initializing,
    Disconnected,
    AddressReady(ScriptBuf),
    AddressEvent(address::Event),
}

impl std::fmt::Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Initializing => {
                write!(f, "Initializing wallet")
            }
            Self::Disconnected => {
                write!(f, "Lost connection")
            }
            Self::AddressReady(scriptpubkey) => {
                write!(f, "Address ready {scriptpubkey}")
            }
            Self::AddressEvent(event) => event.fmt(f),
        }
    }
}

#[derive(Clone)]
pub struct Wallet {
    pub api: api::Client,
    ws: socket::Client,
    addresses: Arc<Mutex<HashMap<ScriptBuf, Arc<Mutex<Tracker>>>>>,
    event_sender: broadcast::Sender<Event>,
}

impl Wallet {
    pub fn new(options: &Options) -> Result<Self, esplora_client::Error> {
        let api_url = format!(
            "http{}://{}/api",
            if options.secure { "s" } else { "" },
            options.hostname
        );
        let ws_url = format!(
            "ws{}://{}/api/v1/ws",
            if options.secure { "s" } else { "" },
            options.hostname
        );

        let (event_sender, _) = broadcast::channel::<Event>(256);

        api::Client::new(&api_url).map(|api| {
            Self {
                api,
                ws: socket::Client::new(ws_url),
                addresses: Arc::new(Mutex::new(HashMap::new())),
                event_sender,
            }
        })

        
    }

    pub async fn connect(&self, wait_for_connection: bool) {
        log::trace!("connecting wallet");
        let wallet = self.clone();
        log::trace!("wallet spawning event handling thread");
        compat::spawn(async move {
            let mut ws_rx = wallet.ws.subscribe();
            log::trace!("wallet spawned event handling thread");
            loop {
                log::trace!("...wallet event receive loop...");
                match ws_rx.recv().await {
                    Ok(WebsocketEvent::Offline) => {
                        log::trace!("wallet websocket offline!");
                        break;
                    }
                    Ok(WebsocketEvent::Disconnected) => {
                        log::trace!("wallet websocket disconnected!");
                        let _ = wallet.event_sender.send(Event::Disconnected);
                    }
                    Ok(WebsocketEvent::Connected) => {
                        log::trace!("wallet websocket (re)connected!");
                        wallet.init_addresses().await;
                        log::trace!("wallet initialized addresses");
                    }
                    Ok(WebsocketEvent::Error) => {
                        log::trace!("wallet websocket threw an error");
                    }
                    Ok(WebsocketEvent::AddressEvent(address_event)) => {
                        log::trace!("handling wallet ws event");
                        wallet.handle_address_event(address_event, true).await;
                        log::trace!("handled wallet ws event");
                    }
                    Err(e) => {
                        log::warn!("unexpected websocket error {:?}", e);
                    }
                }
            }
            log::trace!("wallet event loop ended");
        });
        log::trace!("wallet waiting for connection");
        self.ws.start(wait_for_connection).await;
        log::trace!("wallet connected");
    }

    pub async fn disconnect(&self, wait_for_close: bool) {
        log::trace!("disconnecting wallet");
        self.ws.stop(wait_for_close).await;
        log::trace!("wallet disconnected");
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.event_sender.subscribe()
    }

    pub async fn watch(&self, scriptpubkeys: &[ScriptBuf]) -> Result<Vec<State>, Error> {
        log::trace!("wallet watch {:?}", scriptpubkeys);
        self.ws.track_scriptpubkeys(scriptpubkeys);

        let mut newly_synced = HashMap::new();

        {
            let mut addresses = self.addresses.lock().await;
            for spk in scriptpubkeys {
                if !addresses.contains_key(spk) {
                    let tracker = Tracker::new(spk.clone(), self.event_sender.clone());
                    let tracker_arc = Arc::new(Mutex::new(tracker));
                    addresses.insert(spk.clone(), tracker_arc.clone());
                    newly_synced.insert(spk.clone(), tracker_arc.clone());
                }
            }
        };

        for (spk, tracker_arc) in &newly_synced {
            let _ = self.sync_address_history(spk, tracker_arc).await;
        }

        let addresses = self.addresses.lock().await;
        let mut results = Vec::with_capacity(scriptpubkeys.len());

        for spk in scriptpubkeys {
            results.push(addresses.get(spk).expect("spk should exist in addresses, since we just inserted it").lock().await.get_state());
        }

        Ok(results)
    }

    pub async fn unwatch(&self, scriptpubkeys: &[ScriptBuf]) -> Result<(), Error> {
        let mut addresses = self.addresses.lock().await;

        for spk in scriptpubkeys {
            addresses.remove(spk);
        }

        self.ws.untrack_scriptpubkeys(scriptpubkeys);

        Ok(())
    }

    pub async fn get_state(&self) -> Vec<State> {
        let addresses = self.addresses.lock().await;
        let mut results = Vec::with_capacity(addresses.len());
        for tracker_arc in addresses.values() {
            let tracker = tracker_arc.lock().await;
            results.push(tracker.get_state());
        }

        results
    }

    pub async fn get_address_state(&self, scriptpubkey: &ScriptBuf) -> Option<State> {
        let addresses = self.addresses.lock().await;
        if let Some(tracker) = addresses.get(scriptpubkey) {
            Some(tracker.lock().await.get_state())
        } else {
            None
        }
    }

    async fn handle_address_event(&self, event: address::Event, realtime: bool) {
        let addresses = self.addresses.lock().await;
        match &event {
            address::Event::Mempool(scriptpubkey, _)
            | address::Event::Confirmed(scriptpubkey, _)
            | address::Event::Removed(scriptpubkey, _) => {
                if let Some(tracker_arc) = addresses.get(scriptpubkey) {
                    let mut tracker = tracker_arc.lock().await;
                    tracker.process_event(event, realtime);
                } else {
                    log::warn!("handling event for unknown scriptpubkey: {}", scriptpubkey);
                }
            }
        }
    }

    async fn sync_address_history(
        &self,
        scriptpubkey: &ScriptBuf,
        tracker_arc: &Arc<Mutex<Tracker>>,
    ) -> Result<State, Error> {
        let mut tracker = tracker_arc.lock().await;

        tracker.set_loading(true);

        let initial_state = tracker.get_state();

        log::trace!(
            "syncing address from initial state: {:?}",
            initial_state.clone().transactions.len()
        );

        let (last_txid, last_height) = initial_state
            .transactions
            .iter()
            .rev()
            .find(|tx| tx.status.confirmed)
            .map_or((None, None), |tx| (Some(tx.txid), tx.status.block_height));

        let initial_transactions = self
            .api
            .fetch_address_history(scriptpubkey, last_txid, last_height)
            .await?;

        let mut fetched_txids = HashSet::new();
        for tx in &initial_transactions {
            fetched_txids.insert(tx.txid);
        }

        for tx in initial_state.transactions.iter().rev().take_while(|tx| {
            !tx.status.confirmed || last_height.is_none() || tx.status.block_height > last_height
        }) {
            if !fetched_txids.contains(&tx.txid) {
                tracker
                    .process_event(
                        address::Event::Removed(scriptpubkey.clone(), tx.clone()),
                        false,
                    );
            }
        }

        log::trace!("processing {} transactions", initial_transactions.len());

        for tx in &initial_transactions {
            if tx.status.confirmed {
                tracker
                    .process_event(
                        address::Event::Confirmed(scriptpubkey.clone(), tx.clone()),
                        false,
                    );
            } else {
                tracker
                    .process_event(
                        address::Event::Mempool(scriptpubkey.clone(), tx.clone()),
                        false,
                    );
            }
        }

        tracker.set_loading(false);

        let _ = self.event_sender.send(Event::AddressReady(scriptpubkey.clone()));

        Ok(tracker.get_state())
    }

    async fn init_addresses(&self) {
        let addresses = self.addresses.lock().await;
        log::trace!("(re)initialising {} addresses", addresses.len());

        let spks: Vec<ScriptBuf> = addresses.keys().cloned().collect();
        self.ws.track_scriptpubkeys(&spks);

        // TODO: parallelize this
        for (scriptpubkey, tracker) in &*addresses {
            let _ = self.sync_address_history(scriptpubkey, tracker).await;
        }
    }
}
