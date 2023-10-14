use std::collections::HashMap;

use crate::wallet::{Wallet, Options};
use bitcoin::{Txid, Transaction, BlockHash, Block, MerkleBlock, ScriptBuf};
use esplora_client::{Error, TxStatus, BlockStatus, MerkleProof, OutputStatus, Tx, BlockSummary};
use reqwest;
use delegate::delegate;

pub struct MempoolAsync {
    wallet: Wallet,
}

#[allow(clippy::inline_always)]
impl MempoolAsync {
    #[must_use]
    pub fn new(options: &Options) -> Self {
        let wallet = Wallet::new(options).unwrap();
        Self {
            wallet,
        }
    }

    #[must_use]
    pub fn from_url(url: &str) -> Self {
        let options = url_to_options(url).unwrap();
        Self::new(&options)
    }

    delegate! {
        to self.wallet.api.client {
            /// Get a [`Transaction`] option given its [`Txid`]
            pub async fn get_tx(&self, txid: &Txid) -> Result<Option<Transaction>, Error>;

            /// Get a [`Transaction`] given its [`Txid`].
            pub async fn get_tx_no_opt(&self, txid: &Txid) -> Result<Transaction, Error>;

            /// Get a [`Txid`] of a transaction given its index in a block with a given hash.
            pub async fn get_txid_at_block_index(
                &self,
                block_hash: &BlockHash,
                index: usize,
            ) -> Result<Option<Txid>, Error>;

            /// Get the status of a [`Transaction`] given its [`Txid`].
            pub async fn get_tx_status(&self, txid: &Txid) -> Result<TxStatus, Error>;

            /// Get the [`BlockStatus`] given a particular [`BlockHash`].
            pub async fn get_block_status(&self, block_hash: &BlockHash) -> Result<BlockStatus, Error>;

            /// Get a [`Block`] given a particular [`BlockHash`].
            pub async fn get_block_by_hash(&self, block_hash: &BlockHash) -> Result<Option<Block>, Error>;

            /// Get a merkle inclusion proof for a [`Transaction`] with the given [`Txid`].
            pub async fn get_merkle_proof(&self, tx_hash: &Txid) -> Result<Option<MerkleProof>, Error>;

            /// Get a [`MerkleBlock`] inclusion proof for a [`Transaction`] with the given [`Txid`].
            pub async fn get_merkle_block(&self, tx_hash: &Txid) -> Result<Option<MerkleBlock>, Error>;

            /// Get the spending status of an output given a [`Txid`] and the output index.
            pub async fn get_output_status(
                &self,
                txid: &Txid,
                index: u64,
            ) -> Result<Option<OutputStatus>, Error>;

            /// Broadcast a [`Transaction`] to Esplora
            pub async fn broadcast(&self, transaction: &Transaction) -> Result<(), Error>;

            /// Get the current height of the blockchain tip
            pub async fn get_height(&self) -> Result<u32, Error>;

            /// Get the [`BlockHash`] of the current blockchain tip.
            pub async fn get_tip_hash(&self) -> Result<BlockHash, Error>;

            /// Get the [`BlockHash`] of a specific block height
            pub async fn get_block_hash(&self, block_height: u32) -> Result<BlockHash, Error>;

            /// Get confirmed transaction history for the specified address/scripthash,
            /// sorted with newest first. Returns 25 transactions per page.
            /// More can be requested by specifying the last txid seen by the previous query.
            pub async fn scripthash_txs(
                &self,
                script: &ScriptBuf,
                last_seen: Option<Txid>,
            ) -> Result<Vec<Tx>, Error>;

            /// Get an map where the key is the confirmation target (in number of blocks)
            /// and the value is the estimated feerate (in sat/vB).
            pub async fn get_fee_estimates(&self) -> Result<HashMap<String, f64>, Error>;

            /// Gets some recent block summaries starting at the tip or at `height` if provided.
            ///
            /// The maximum number of summaries returned depends on the backend itself: esplora returns `10`
            /// while [mempool.space](https://mempool.space/docs/api) returns `15`.
            pub async fn get_blocks(&self, height: Option<u32>) -> Result<Vec<BlockSummary>, Error>;

            /// Get the underlying base URL.
            pub fn url(&self) -> &str;

            /// Get the underlying [`Client`].
            pub fn client(&self) -> &reqwest::Client;
        }
    }
}

fn url_to_options(input: &str) -> Result<Options, &'static str> {
    let mut iter = input.splitn(3, "://");
    
    let Some(scheme) = iter.next() else {
        return Err("Invalid URL");
    };

    if scheme != "http" && scheme != "https" {
        return Err("Invalid URL");
    }

    let host = match iter.next() {
        Some(host_part) => {
            let end = host_part.find('/').unwrap_or(host_part.len());
            &host_part[0..end]
        }
        None => return Err("Invalid URL"),
    };

    Ok(Options {
        hostname: host.to_string(),
        secure: scheme == "https",
    })
}