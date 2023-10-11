use std::{str::FromStr, sync::Arc, collections::HashSet};

use log::Level;
use tokio::sync::Mutex;
use wasm_bindgen::prelude::*;
use bitcoin::{Address, Network, Script};
use mwck::wallet::{address, Wallet, Options, Event};
use wasm_bindgen_futures::future_to_promise;

#[wasm_bindgen(module = "/main.js")]
extern "C" {
    fn init_js();
    fn onAddressEvent(address: String, tx_count: usize, balance: JsValue);
}

#[wasm_bindgen]
pub struct JsWallet {
    network: Network,
    wallet: Arc<Mutex<Wallet>>,
}

#[wasm_bindgen]
impl JsWallet {
    #[wasm_bindgen(constructor)]
    pub fn new(host: String, network_str: String) -> Self {
        let network = match network_str.as_str() {
            "testnet" => Network::Testnet,
            "signet" => Network::Signet,
            "regtest" => Network::Regtest,
            _ => Network::Bitcoin,
        };
        JsWallet {
            network,
            wallet: Arc::new(Mutex::new(Wallet::new(&Options {
                hostname: host,
                network,
                secure: false,
            }).unwrap()))
        }
    }

    #[wasm_bindgen]
    pub async fn connect(&self) {
        log::trace!("demo start connect");
        self.wallet.lock().await.connect(true).await;
        log::trace!("demo end connect");
    }

    #[wasm_bindgen]
    pub async fn disconnect(&self) {
        log::trace!("demo start disconnect");
        self.wallet.lock().await.disconnect(true).await;
        log::trace!("demo end disconnect");
    }

    #[wasm_bindgen]
    pub async fn subscribe(&self) {
        log::warn!("subscribing to events!");
        let wallet = self.wallet.clone();
        let mut event_receiver = {
            wallet.lock().await.subscribe()
        };
        let network = self.network;
        wasm_bindgen_futures::spawn_local(async move {
            let mut ready_addresses: HashSet<Script> = HashSet::new();
            loop {
                match event_receiver.recv().await {
                    Ok(Event::Initializing) => {
                        //
                    }
                    Ok(Event::Disconnected) => {
                        log::debug!("wallet disconnected");
                        ready_addresses.clear();
                    }
                    Ok(Event::AddressReady(scriptpubkey)) => {
                        log::debug!("loaded address {}", scriptpubkey);
                        ready_addresses.insert(scriptpubkey.clone());
                        let address = Address::from_script(&scriptpubkey, network).unwrap().to_string();
                        if let Some(state) = wallet.lock().await.get_address_state(&scriptpubkey).await {
                            let balance = serde_wasm_bindgen::to_value(&state.balance).unwrap();
                            onAddressEvent(address, state.transactions.len(), balance);
                        }
                    }
                    Ok(Event::AddressEvent(address_event)) => {
                        match &address_event {
                            address::Event::Mempool(scriptpubkey, _) |
                            address::Event::Confirmed(scriptpubkey, _) |
                            address::Event::Removed(scriptpubkey, _) => {
                                if ready_addresses.contains(scriptpubkey) {
                                    log::debug!("wallet event: {}", &address_event);
                                    let address = Address::from_script(scriptpubkey, network).unwrap().to_string();
                                    if let Some(state) = wallet.lock().await.get_address_state(scriptpubkey).await {
                                        let balance = serde_wasm_bindgen::to_value(&state.balance).unwrap();
                                        onAddressEvent(address, state.transactions.len(), balance);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("wallet error! {:?}", e);
                    }
                }
            }
        });
    }

    #[wasm_bindgen]
    pub fn track_address(&self, address: String) -> js_sys::Promise {
        log::warn!("tracking address: {}", address);
        let wallet = self.wallet.clone();
        let future = async move {
            match Address::from_str(&address) {
                Ok(address) => {
                    let scriptpubkey = address.script_pubkey();
                    Ok(JsValue::from_bool(wallet.lock().await.watch(&[scriptpubkey]).await.is_ok()))
                },
                Err(_) => Ok(JsValue::FALSE),
            }
        };

        future_to_promise(future)
    }
}

#[wasm_bindgen]
pub fn main() {
    wasm_logger::init(wasm_logger::Config::new(Level::Debug).message_on_new_line());
    log::warn!("MWCK WASM is loaded 😎");
}