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

    pub fn unsubscribe(symbol: &Spot, task_id: &Uuid) -> String {
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
    pub struct RequestResponse {
        pub result: Value,
        pub id: String,
    }

    #[derive(Deserialize)]
    pub struct Level(String, String);

    impl Level {
        pub fn price(&self) -> f32 {
            self.0.parse().unwrap()
        }

        pub fn quantity(&self) -> f32 {
            self.1.parse().unwrap()
        }
    }

    #[derive(Deserialize)]
    struct DepthUpdateInner {
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
    pub struct DepthUpdate {
        stream: String,
        data: DepthUpdateInner,
    }

    impl DepthUpdate {
        pub fn get_symbol_str(&self) -> &str {
            // the stream variable is 'symbol@depth@timestamp'
            self.stream.split("@").collect::<Vec<&str>>()[0]

            // self.data.symbol // we might consider using this
        }

        pub fn get_event_time(&self) -> u64 {
            self.data.event_time
        }

        pub fn get_first_update_id(&self) -> u64 {
            self.data.first_update_id
        }

        pub fn get_last_update_id(&self) -> u64 {
            self.data.last_update_id
        }

        pub fn get_bids(&self) -> &Vec<Level> {
            &self.data.bids
        }

        pub fn get_asks(&self) -> &Vec<Level> {
            &self.data.asks
        }
    }

    #[derive(Deserialize)]
    struct SnapShot {
        #[serde(rename = "lastUpdateId")]
        last_update_id: u64,

        bids: Vec<Level>,

        asks: Vec<Level>,
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    pub enum IncomingMessage {
        Response(RequestResponse),
        DepthUpdate(DepthUpdate),
        Other(Value),
    }
}
