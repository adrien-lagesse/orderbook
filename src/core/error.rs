use std::result::Result as StdResult;

use thiserror::Error as ThisError;

use crate::Spot;

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

    #[error("{symbol:?} is already tracked")]
    AlreadyTracked {
        symbol: Spot
    },

    #[error("{symbol:?} is not tracked")]
    NotTracked {
        symbol: Spot
    }
}

pub type Result<T> = StdResult<T, Error>;
