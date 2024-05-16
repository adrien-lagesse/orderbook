use std::result::Result as StdResult;

use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("WebSocket Failed: {source:?}")]
    WebSocket {
        #[from]
        source: tokio_tungstenite::tungstenite::Error,
    },

    #[error("OrderBook snapshot HTTP request failed  for {symbol}")]
    SnapshotHTTPError {
        symbol: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("OrderBook snapshot failed  for {symbol}: {message}")]
    SnapshotParsingError {
        symbol: String,
        message: String,
        body: String,
    },
}

pub type Result<T> = StdResult<T, Error>;
