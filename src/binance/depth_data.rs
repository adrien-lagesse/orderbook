use serde_json::Value;

#[derive(Debug)]
pub struct DepthData {
    pub event_time: u64,
    pub ticker: String,
    pub first_update_id: u64,
    pub last_update_id: u64,
    pub bids: Vec<(f32, f32)>,
    pub asks: Vec<(f32, f32)>,
}

impl DepthData {
    pub fn from_message(json_message: &Value) -> Self {
        let event_time = json_message
            .get("E")
            .expect(&format!(
                "No event time in the JSON object: {}",
                json_message
            ))
            .as_u64()
            .expect(&format!("Event time is not a u64: {}", json_message));

        let ticker = json_message
            .get("s")
            .expect(&format!(
                "No ticker value in the JSON object: {}",
                json_message
            ))
            .as_str()
            .expect(&format!("Ticker is not a String: {}", json_message));

        let first_update_id = json_message
            .get("U")
            .expect(&format!(
                "No first update ID in JSON object: {}",
                json_message
            ))
            .as_u64()
            .expect(&format!("First Update ID is not a u64: {}", json_message));

        let last_update_id = json_message
            .get("u")
            .expect(&format!(
                "No last update ID in JSON object: {}",
                json_message
            ))
            .as_u64()
            .expect(&format!("Last Update ID is not u64: {}", json_message));

        let mut bids: Vec<(f32, f32)> = vec![];
        let arr = json_message
            .get("b")
            .expect(&format!("No bids in JSON object: {}", json_message))
            .as_array()
            .expect(&format!(
                "Bids are not in an array format: {}",
                json_message
            ));

        for bid in arr {
            let price: f32 = bid
                .as_array()
                .expect("Bid is not an array")
                .get(0)
                .expect("No first index in bid")
                .as_str()
                .expect("Failed to parse bid.price_level as str")
                .parse()
                .expect("Failed to parse bid.price_level as f32");

            let quantity: f32 = bid
                .as_array()
                .expect("Bid is not an array")
                .get(1)
                .expect("No second index in bid")
                .as_str()
                .expect("Failed to parse bid.quantity as str")
                .parse()
                .expect("Failed to parse bid.quantity as f32");

            bids.push((price, quantity));
        }

        let mut asks: Vec<(f32, f32)> = vec![];
        let arr = json_message
            .get("a")
            .expect("No asks in JSON object")
            .as_array()
            .expect("Asks are not in an array format");

        for ask in arr {
            let price: f32 = ask
                .as_array()
                .expect("Ask is not an array")
                .get(0)
                .expect("No first index in ask")
                .as_str()
                .expect("Failed to parse ask.price_level as str")
                .parse()
                .expect("Failed to parse ask.price_level as f32");

            let quantity: f32 = ask
                .as_array()
                .expect("Ask is not an array")
                .get(1)
                .expect("No second index in ask")
                .as_str()
                .expect("Failed to parse ask.quantity as str")
                .parse()
                .expect("Failed to parse ask.quantity as f32");
            asks.push((price, quantity));
        }

        DepthData {
            event_time: event_time,
            ticker: String::from(ticker),
            first_update_id: first_update_id,
            last_update_id: last_update_id,
            bids: bids,
            asks: asks,
        }
    }
}
