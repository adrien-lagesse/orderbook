use serde_json::Value;

use crate::binance::LocalBook;

pub struct DepthSnapshot {
    pub last_update_id: u64,
    pub bids: Vec<(f32, f32)>,
    pub asks: Vec<(f32, f32)>,
}

impl DepthSnapshot {
    pub fn from_json_message(json_message: &Value) -> crate::Result<DepthSnapshot> {
        let last_update_id = json_message
            .pointer("/lastUpdateId")
            .and_then(|e| e.as_u64())
            .ok_or_else(|| crate::Error::SnapshotParsingError {
                symbol: "".to_string(),
                message: "Error parsing lastUpdateId".to_string(),
                body: json_message.to_string(),
            })?;

        let mut bids: Vec<(f32, f32)> = vec![];
        let arr = json_message
            .pointer("/bids")
            .and_then(|e| e.as_array())
            .ok_or_else(|| crate::Error::SnapshotParsingError {
                symbol: "".to_string(),
                message: "Error parsing bids".to_string(),
                body: json_message.to_string(),
            })?;

        for bid in arr {
            let price: f32 = bid
                .as_array()
                .and_then(|e| e.get(0))
                .and_then(|e| e.as_str())
                .and_then(|e| e.parse().ok())
                .ok_or_else(|| crate::Error::SnapshotParsingError {
                    symbol: "".to_string(),
                    message: "Error parsing bid.price_level".to_string(),
                    body: json_message.to_string(),
                })?;

            let quantity: f32 = bid
                .as_array()
                .and_then(|e| e.get(1))
                .and_then(|e| e.as_str())
                .and_then(|e| e.parse().ok())
                .ok_or_else(|| crate::Error::SnapshotParsingError {
                    symbol: "".to_string(),
                    message: "Error parsing bid.quantity".to_string(),
                    body: json_message.to_string(),
                })?;

            bids.push((price, quantity));
        }

        let mut asks: Vec<(f32, f32)> = vec![];
        let arr = json_message
            .pointer("/asks")
            .and_then(|e| e.as_array())
            .ok_or_else(|| crate::Error::SnapshotParsingError {
                symbol: "".to_string(),
                message: "Error parsing asks".to_string(),
                body: json_message.to_string(),
            })?;

        for ask in arr {
            let price: f32 = ask
                .as_array()
                .and_then(|e| e.get(0))
                .and_then(|e| e.as_str())
                .and_then(|e| e.parse().ok())
                .ok_or_else(|| crate::Error::SnapshotParsingError {
                    symbol: "".to_string(),
                    message: "Error parsing ask.price_level".to_string(),
                    body: json_message.to_string(),
                })?;

            let quantity: f32 = ask
                .as_array()
                .and_then(|e| e.get(1))
                .and_then(|e| e.as_str())
                .and_then(|e| e.parse().ok())
                .ok_or_else(|| crate::Error::SnapshotParsingError {
                    symbol: "".to_string(),
                    message: "Error parsing ask.quantity".to_string(),
                    body: json_message.to_string(),
                })?;

            asks.push((price, quantity));
        }

        Ok(DepthSnapshot {
            last_update_id,
            bids,
            asks,
        })
    }

    pub fn update_book(&self, book: &mut LocalBook) {
        book.bids
            .retain(|_, row| row.last_update_id > self.last_update_id);
        book.asks
            .retain(|_, row| row.last_update_id > self.last_update_id);

        self.bids.iter().for_each(|(price, quantity)| {
            book.update_with_bid(*price, *quantity, self.last_update_id, self.last_update_id)
        });
        self.asks.iter().for_each(|(price, quantity)| {
            book.update_with_ask(*price, *quantity, self.last_update_id, self.last_update_id)
        });
    }
}
