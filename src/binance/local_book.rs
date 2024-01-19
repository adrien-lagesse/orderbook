use std::collections::BTreeMap;

#[derive(Debug)]
struct LocalBookRow {
    price: f32,
    quantity: f32,
    first_update_id: u64,
    last_update_id: u64,
}

#[derive(Debug)]
pub struct LocalBook {
    last_updated_id: Option<u64>,
    bids: BTreeMap<u64, LocalBookRow>,
    asks: BTreeMap<u64, LocalBookRow>,
}

impl LocalBook {
    pub fn new() -> Self {
        LocalBook {
            last_updated_id: Some(0),
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    pub fn update_with_bid(
        &mut self,
        price_level: f32,
        quantity: f32,
        first_update_id: u64,
        last_update_id: u64,
    ) {
        if quantity != 0 as f32 {
            self.bids.insert(
                (price_level * 1000000000.0) as u64,
                LocalBookRow {
                    price: price_level,
                    quantity,
                    first_update_id,
                    last_update_id,
                },
            );
        } else if quantity == 0 as f32 {
            self.bids.remove(&((price_level * 1000000000.0) as u64));
        }
    }

    pub fn update_with_ask(
        &mut self,
        price_level: f32,
        quantity: f32,
        first_update_id: u64,
        last_update_id: u64,
    ) {
        if quantity != 0 as f32 {
            self.asks.insert(
                (price_level * 1000000000.0) as u64,
                LocalBookRow {
                    price: price_level,
                    quantity,
                    first_update_id,
                    last_update_id,
                },
            );
        } else if quantity == 0 as f32 {
            self.asks.remove(&((price_level * 1000000000.0) as u64));
        }
    }

    pub fn best_bid(&self) -> (f32, f32) {
        let max_key = self.bids.keys().max().unwrap();
        let row = self.bids.get(max_key).unwrap();
        (row.price, row.quantity)
    }

    pub fn best_ask(&self) -> (f32, f32) {
        let min_key = self.asks.keys().min().unwrap();
        let row = self.asks.get(min_key).unwrap();
        (row.price, row.quantity)
    }
}
