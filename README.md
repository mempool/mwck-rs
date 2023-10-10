# Rust Mempool Wallet Connector Kit

*(**work in progress** - relies on the multi-address tracking feature from https://github.com/mempool/mempool/pull/4137)*

A utility library for efficiently syncing Bitcoin wallet history from an instance of The Mempool Open Source Project® backend.

Mwck uses websocket push notifications to discover new address transaction events, eliminating the need to constantly poll the REST API.

Aims to support both native and wasm32 targets.

## Quick start

```rust
use mwck::wallet::{address, Wallet, Options, Event};

let wallet = Wallet::new(&Options {
    hostname: "localhost:4200",
    network: bitcoin::Network::Bitcoin,
    secure: false,
});

// connect to the websocket server
wallet.connect(true).await;

// start watching two addresses
wallet.watch(&[addressA.script_pubkey(), addressB.script_pubkey()]).await;

// stop watching one of the addresses
wallet.unwatch(&[addressB.script_pubkey()]).await;

// get the current state of addressA on demand (including balance & list of transactions)
let address_state = wallet.get_address_state(addressA.script_pubkey()).await;

// get a tokio::sync::broadcast receiver
let event_receiver = wallet.subscribe();

// consume events related to the currently watched addresses
loop {
    match event_receiver.recv().await {
        Ok(Event::AddressEvent(address::Event::Mempool(scriptpubkey, tx))) => {
            // received unconfirmed tx related to scriptpubkey
        }
        Ok(Event::AddressEvent(address::Event::Confirmed(scriptpubkey, tx))) => {
            // received confirmed tx related to scriptpubkey
        }
        Ok(Event::AddressEvent(address::Event::Removed(scriptpubkey, tx))) => {
            // tx related to scriptpubkey dropped from mempool
        }
        Ok(Event::AddressReady(scriptpubkey)) => {
            // finished syncing scriptpubkey with the server
        }
        ...
    }
}
```

Also check out the `wasm_wallet_watcher` example crate.

## API

_(TODO)_

## Types/Interfaces

_(TODO)_