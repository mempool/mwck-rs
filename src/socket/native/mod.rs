use std::time::Duration;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::StreamExt;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

pub type Sink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
pub type Stream = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;
pub use tokio_tungstenite::tungstenite::{Message, Error as StreamError};

#[derive(Debug)]
pub enum Error {
    Timeout,
    Ws(WsError),
}

pub async fn connect(url: &str, timeout: Option<Duration>) -> Result<(Sink, Stream), Error> {
    let timeout = timeout.unwrap_or(Duration::from_millis(60_000));
    let timeout_result = tokio::time::timeout(timeout, tokio_tungstenite::connect_async(url)).await;

    match timeout_result {
        Ok(Ok((stream, _))) => Ok(stream.split()),
        Ok(Err(e)) => Err(Error::Ws(e)),
        Err(_) => Err(Error::Timeout),
    }
}
