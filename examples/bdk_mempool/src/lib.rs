use bdk_chain::{BlockId, ConfirmationTimeAnchor};
use esplora_client::TxStatus;

pub use mwck;

mod async_ext;
pub use async_ext::*;

const ASSUME_FINAL_DEPTH: u32 = 15;

fn anchor_from_status(status: &TxStatus) -> Option<ConfirmationTimeAnchor> {
    if let TxStatus {
        block_height: Some(height),
        block_hash: Some(hash),
        block_time: Some(time),
        ..
    } = status.clone()
    {
        Some(ConfirmationTimeAnchor {
            anchor_block: BlockId { height, hash },
            confirmation_height: height,
            confirmation_time: time,
        })
    } else {
        None
    }
}
