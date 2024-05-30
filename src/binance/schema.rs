pub mod outgoing {
    use serde_json::json;
    use uuid::Uuid;

    use crate::Spot;

    pub fn subscribe(symbol: &Spot, task_id: &Uuid) -> String {
        json!({
        "method": "SUBSCRIBE",
        "params":
        [
            format!("{}@depth@100ms", symbol.to_string())
        ],
        "id": task_id.to_string()
        })
        .to_string()
    }

    pub fn subscribe_many(symbols: &[Spot], task_id: &Uuid) -> String {
        let params: Vec<String> = symbols
            .iter()
            .map(|symbol| format!("{}@depth@100ms", symbol.to_string()))
            .collect();
        json!({
        "method": "SUBSCRIBE",
        "params": params,
        "id": task_id.to_string()
        })
        .to_string()
    }

    pub fn unsubscribe(symbol: Spot, task_id: &Uuid) -> String {
        json!({
        "method": "UNSUBSCRIBE",
        "params":
        [
            format!("{}@depth@100ms", symbol.to_string())
        ],
        "id": task_id.to_string()
        })
        .to_string()
    }

    pub fn unsubscribe_many(symbols: &[Spot], task_id: &Uuid) -> String {
        let params: Vec<String> = symbols
            .iter()
            .map(|symbol| format!("{}@depth@100ms", symbol.to_string()))
            .collect();
        json!({
        "method": "UNSUBSCRIBE",
        "params": params,
        "id": task_id.to_string()
        })
        .to_string()
    }
}

pub mod incoming {
    use serde;
    use serde::Deserialize;
    use serde_json::Value;

    #[derive(Deserialize)]
    struct RequestResponse {
        result: Value,
        id: String,
    }

    #[derive(Deserialize)]
    struct Level(f64, f64);

    impl Level {
        fn price(&self) -> f64 {
            self.0
        }

        fn quantity(&self) -> f64 {
            self.1
        }
    }

    #[derive(Deserialize)]
    struct DepthUpdate {
        #[serde(rename = "e")]
        event_type: String,

        #[serde(rename = "E")]
        event_time: u64,

        #[serde(rename = "s")]
        symbol: String,

        #[serde(rename = "U")]
        first_update_id: u64,

        #[serde(rename = "u")]
        last_update_id: u64,

        #[serde(rename = "b")]
        bids: Vec<Level>,

        #[serde(rename = "a")]
        asks: Vec<Level>,
    }

    #[derive(Deserialize)]
    struct SnapShot {
        #[serde(rename = "lastUpdateId")]
        last_update_id: u64,

        bids: Vec<Level>,

        asks: Vec<Level>,
    }
}
