use bitcoin::{
    hashes::{sha256, Hash},
    ScriptBuf, Txid,
};
use esplora_client::{AsyncClient as EsploraClient, Builder, Error, Tx};

#[derive(Debug, Clone)]
pub struct Client {
    client: EsploraClient,
}

impl Client {
    pub fn new(url: &str) -> Result<Self, esplora_client::Error> {
        let builder = Builder::new(url);
        let async_client: EsploraClient = builder.build_async()?;
        Ok(Self {
            client: async_client,
        })
    }

    /// Alternative to `esplora_client::AsyncClient::scripthash_txs`
    /// taking advantage of new mempool/electrs features
    pub async fn scripthash_txs(
        &self,
        script: &ScriptBuf,
        last_seen: Option<Txid>,
        page_size: Option<usize>,
    ) -> Result<Vec<Tx>, Error> {
        let script_hash = sha256::Hash::hash(script.as_bytes());
        let max_txs = page_size.unwrap_or(50);
        let url = last_seen.map_or_else(|| format!(
                "{}/scripthash/{:x}/txs?max_txs={}",
                self.client.url(),
                script_hash,
                max_txs
            ), |after_txid| format!(
                "{}/scripthash/{:x}/txs?max_txs={}&after_txid={}",
                self.client.url(),
                script_hash,
                max_txs,
                after_txid
            ));
        Ok(self
            .client
            .client()
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<Tx>>()
            .await?)
    }

    // TODO: make this interruptible
    /// Makes multiple requests to fetch the full transaction history
    /// of the given scriptpubkey using the REST API.
    ///
    /// Returns those transactions in chronological order (oldest first)
    ///
    /// `until_txid` and `until_height` can be used to limit the number of API requests:
    /// If either is provided, the function will only fetch as much history as necessary to find
    ///  - a transaction with the given txid.
    ///  - a transaction confirmed at or below the given blockheight.
    pub async fn fetch_address_history(
        &self,
        scriptpubkey: &ScriptBuf,
        until_txid: Option<Txid>,
        until_height: Option<u32>,
    ) -> Result<Vec<Tx>, Error> {
        let mut all_txs = Vec::new();
        let mut done = false;
        let mut found_txid = until_txid.is_none();
        let mut found_height = until_height.is_none();
        let limit_requests = !found_txid || !found_height;
        let mut last_txid: Option<Txid> = None;

        log::trace!(
            "Fetching address history until {:?} / {:?}",
            until_txid.clone(),
            until_height.clone()
        );

        while !done && (!limit_requests || !found_txid || !found_height) {
            let mut txs = self.scripthash_txs(scriptpubkey, last_txid, None).await?;

            found_txid |= limit_requests && txs.iter().any(|tx| Some(tx.txid) == until_txid);

            if let Some(last_tx) = txs.last() {
                found_height |= !found_height
                    && last_tx.status.confirmed
                    && last_tx.status.block_height < until_height;
            }

            if txs.len() == 50 {
                last_txid = Some(txs.last().unwrap().txid);
                log::trace!(
                    "...fetched +{} = {} up to  {:?}",
                    txs.len(),
                    all_txs.len(),
                    last_txid
                );
            } else {
                log::trace!("...fetched {} and done!", txs.len());
                done = true;
            }
            all_txs.append(&mut txs);
        }

        all_txs.reverse();
        Ok(all_txs)
    }
}
