use futures_util::stream::{SplitSink, SplitStream};
use futures_util::StreamExt;
use ws_stream_wasm::{WsMeta, WsStream};

pub use ws_stream_wasm::{WsErr as StreamError, WsMessage as Message};

pub type Sink = SplitSink<WsStream, Message>;
pub type Stream = SplitStream<WsStream>;

pub async fn connect(url: &str) -> Result<(Sink, Stream), StreamError> {
    let (_ws, stream) = WsMeta::connect(url, None).await?;

    Ok(stream.split())
}