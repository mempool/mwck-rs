use std::{
    cmp::Ordering,
    collections::{HashMap, VecDeque},
};

use bitcoin::Txid;
use esplora_client::{ScriptBuf, Tx};
use serde::Serialize;
use tokio::sync::broadcast;

use super::Event as WalletEvent;

#[derive(Debug, Clone)]
pub enum Event {
    Removed(ScriptBuf, Tx),
    Mempool(ScriptBuf, Tx),
    Confirmed(ScriptBuf, Tx),
}

impl std::fmt::Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mempool(scriptpubkey, tx) => {
                write!(f, "mempool | {} | {}", scriptpubkey, tx.txid)
            }
            Self::Confirmed(scriptpubkey, tx) => {
                write!(f, "confirmed | {} | {}", scriptpubkey, tx.txid)
            }
            Self::Removed(scriptpubkey, tx) => {
                write!(f, "removed | {} | {}", scriptpubkey, tx.txid)
            }
        }
    }
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct Balance {
    pub funded: u64,
    pub spent: u64,
}

impl Balance {
    #[must_use]
    const fn new() -> Self {
        Self {
            funded: 0,
            spent: 0,
        }
    }
}

impl Balance {
    #[must_use]
    const fn get(self) -> i64 {
        if self.funded >= self.spent {
            (self.funded - self.spent) as i64
        } else {
            -((self.spent - self.funded) as i64)
        }
    }
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct Balances {
    pub mempool: Balance,
    pub confirmed: Balance,
}

impl Balances {
    const fn new() -> Self {
        Self {
            mempool: Balance::new(),
            confirmed: Balance::new(),
        }
    }

    const fn total(self) -> Balance {
        Balance {
            funded: self.mempool.funded + self.confirmed.funded,
            spent: self.mempool.spent + self.confirmed.spent,
        }
    }
}

#[derive(Debug, Clone)]
pub struct State {
    pub scriptpubkey: ScriptBuf,
    pub transactions: Vec<Tx>,
    pub balance: Balances,
}

#[derive(Debug, Clone)]
pub struct Tracker {
    scriptpubkey: ScriptBuf,
    transactions: HashMap<Txid, Tx>,
    balance: Balances,
    queue: VecDeque<Event>,
    loading: bool,
    event_sender: broadcast::Sender<WalletEvent>,
}

// this is a dumb way to order transactions, but suffices for now
// TODO: order by dependency graph
fn partial_cmp_tx_time(a: &Tx, b: &Tx) -> Option<Ordering> {
    if (a.status.confirmed || b.status.confirmed) && a.status.block_height != b.status.block_height
    {
        match (a.status.confirmed, b.status.confirmed) {
            (true, true) => a.status.block_height.partial_cmp(&b.status.block_height),
            (true, false) => Some(Ordering::Less),
            (false, true) => Some(Ordering::Greater),
            (false, false) => {
                // should never reach here
                Some(Ordering::Equal)
            }
        }
    } else {
        a.txid.partial_cmp(&b.txid)
    }
}
fn cmp_tx_time(a: &Tx, b: &Tx) -> Ordering {
    partial_cmp_tx_time(a, b).expect("no duplicate transactions")
}

impl Tracker {
    #[must_use]
    pub fn new(scriptpubkey: ScriptBuf, event_sender: broadcast::Sender<WalletEvent>) -> Self {
        Self {
            scriptpubkey,
            transactions: HashMap::new(),
            balance: Balances::new(),
            queue: VecDeque::new(),
            loading: true,
            event_sender,
        }
    }

    #[must_use]
    pub fn from(state: State, event_sender: broadcast::Sender<WalletEvent>) -> Self {
        let mut tracker = Self::new(state.scriptpubkey, event_sender);

        for tx in &state.transactions {
            tracker.add_transaction(tx);
        }

        tracker
    }

    pub fn get_state(&self) -> State {
        let mut transactions: Vec<_> = self.transactions.values().cloned().collect();
        transactions.sort_by(cmp_tx_time);
        State {
            scriptpubkey: self.scriptpubkey.clone(),
            transactions,
            balance: self.balance.clone(),
        }
    }

    pub fn process_event(&mut self, event: Event, realtime: bool) {
        if realtime && self.loading {
            log::trace!("queuing event to process later {}", self.scriptpubkey);
            self.queue.push_back(event);
            return;
        }

        match event {
            Event::Mempool(_, tx) => {
                self.add_transaction(&tx);
                let _ = self
                    .event_sender
                    .send(WalletEvent::AddressEvent(Event::Mempool(
                        self.scriptpubkey.clone(),
                        tx.clone(),
                    )));
            }
            Event::Confirmed(_, tx) => {
                self.add_transaction(&tx);
                let _ = self
                    .event_sender
                    .send(WalletEvent::AddressEvent(Event::Confirmed(
                        self.scriptpubkey.clone(),
                        tx.clone(),
                    )));
            }
            Event::Removed(_, tx) => {
                self.remove_transaction(&tx.txid);
                let _ = self
                    .event_sender
                    .send(WalletEvent::AddressEvent(Event::Confirmed(
                        self.scriptpubkey.clone(),
                        tx.clone(),
                    )));
            }
        }
    }

    pub fn set_loading(&mut self, loading: bool) {
        if self.loading && !loading {
            log::trace!("draining the event queue {}", self.queue.len());
            self.drain_queue();
        }
        self.loading = loading;
    }

    fn add_transaction(&mut self, tx: &Tx) {
        log::trace!("add transaction {} {}", tx.status.confirmed, tx.txid);
        // we already have a copy of this transaction
        // undo the effects of that version before applying this one
        if self.transactions.contains_key(&tx.txid) {
            self.remove_transaction(&tx.txid);
        }

        for vin in &tx.vin {
            if let Some(prevout) = vin.prevout.as_ref() {
                if prevout.scriptpubkey == self.scriptpubkey {
                    if tx.status.confirmed {
                        self.balance.confirmed.spent += prevout.value;
                    } else {
                        self.balance.mempool.spent += prevout.value;
                    }
                }
            }
        }

        for vout in &tx.vout {
            if vout.scriptpubkey == self.scriptpubkey {
                if tx.status.confirmed {
                    self.balance.confirmed.funded += vout.value;
                } else {
                    self.balance.mempool.funded += vout.value;
                }
            }
        }

        self.transactions.insert(tx.txid, tx.clone());
    }

    fn remove_transaction(&mut self, txid: &Txid) {
        if let Some(tx) = self.transactions.remove(txid) {
            log::trace!("remove transaction {} {}", tx.status.confirmed, txid);
            for vin in &tx.vin {
                if let Some(prevout) = vin.prevout.as_ref() {
                    if prevout.scriptpubkey == self.scriptpubkey {
                        if tx.status.confirmed {
                            self.balance.confirmed.spent -= prevout.value;
                        } else {
                            self.balance.mempool.spent -= prevout.value;
                        }
                    }
                }
            }

            for vout in &tx.vout {
                if vout.scriptpubkey == self.scriptpubkey {
                    if tx.status.confirmed {
                        self.balance.confirmed.funded -= vout.value;
                    } else {
                        self.balance.mempool.funded -= vout.value;
                    }
                }
            }
        }
    }

    fn drain_queue(&mut self) {
        while let Some(event) = self.queue.pop_front() {
            self.process_event(event, false);
        }
    }
}
