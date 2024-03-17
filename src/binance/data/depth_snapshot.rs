use anyhow;
use anyhow::Context;
use serde_json::Value;

use crate::binance::LocalBook;

pub struct DepthSnapshot {
    pub last_update_id: u64,
    pub bids: Vec<(f32, f32)>,
    pub asks: Vec<(f32, f32)>,
}

impl DepthSnapshot {
    pub fn from_json_message(json_message: &Value) -> anyhow::Result<DepthSnapshot> {
        let last_update_id = json_message
            .get("lastUpdateId")
            .context("No last update ID in JSON object")?
            .as_u64()
            .context("Last Update ID is not u64")?;

        let mut bids: Vec<(f32, f32)> = vec![];
        let arr = json_message
            .get("bids")
            .context("No bids in JSON object")?
            .as_array()
            .context("Bids are not in an array format")?;

        for bid in arr {
            let price: f32 = bid
                .as_array()
                .context("Bid is not an array")?
                .get(0)
                .context("No first index in bid")?
                .as_str()
                .context("Failed to parse bid.price_level as str")?
                .parse()
                .context("Failed to parse bid.price_level as f32")?;

            let quantity: f32 = bid
                .as_array()
                .context("Bid is not an array")?
                .get(1)
                .context("No second index in bid")?
                .as_str()
                .context("Failed to parse bid.quantity as str")?
                .parse()
                .context("Failed to parse bid.quantity as f32")?;

            bids.push((price, quantity));
        }

        let mut asks: Vec<(f32, f32)> = vec![];
        let arr = json_message
            .get("asks")
            .context("No asks in JSON object")?
            .as_array()
            .context("Asks are not in an array format")?;

        for ask in arr {
            let price: f32 = ask
                .as_array()
                .context("Ask is not an array")?
                .get(0)
                .context("No first index in ask")?
                .as_str()
                .context("Failed to parse ask.price_level as str")?
                .parse()
                .context("Failed to parse ask.price_level as f32")?;

            let quantity: f32 = ask
                .as_array()
                .context("Ask is not an array")?
                .get(1)
                .context("No second index in ask")?
                .as_str()
                .context("Failed to parse ask.quantity as str")?
                .parse()
                .context("Failed to parse ask.quantity as f32")?;
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
