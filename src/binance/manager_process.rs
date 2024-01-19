use std::collections::HashMap;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use reqwest;
use serde_json::{json, Value};
use tokio::{
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    time::timeout,
};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::core::IDGenerator;

use super::{
    depth_snapshot_data::DepthSnapshot,
    manager_communication::{Task, TaskKind},
    manager_storage::ManagerSharedStorage,
    DepthData,
};

pub struct ManagerProcess {
    tasks: HashMap<i32, Task>,
    task_transmitter: UnboundedSender<Task>,
    task_receiver: UnboundedReceiver<Task>,
    id_generator: IDGenerator,
    shared_storage: ManagerSharedStorage,
}

impl ManagerProcess {
    pub fn new() -> ManagerProcess {
        let tasks = HashMap::new();

        let (task_transmitter, task_receiver) = unbounded_channel::<Task>();

        let id_generator = IDGenerator::new();

        let shared_storage = ManagerSharedStorage::new();

        let manager_process = ManagerProcess {
            tasks,
            task_transmitter,
            task_receiver,
            id_generator,
            shared_storage,
        };

        manager_process
    }

    pub fn get_task_transmitter(&self) -> UnboundedSender<Task> {
        self.task_transmitter.clone()
    }

    pub fn get_shared_storage(&self) -> ManagerSharedStorage {
        self.shared_storage.clone()
    }

    pub async fn run(&mut self) {
        let (mut ws, _) = connect_async("wss://stream.binance.com:443/stream")
            .await
            .map_err(|err| format!("Websocket connection error: {}", &err.to_string()))
            .unwrap();

        let mut is_running = true;

        while is_running {
            // Processing the incoming tasks
            while let Ok(task) = self.task_receiver.try_recv() {
                let task_id = self.id_generator.next();
                match &task.kind {
                    TaskKind::Suscribe(symbol) => {
                        ws.send(Message::Text(
                            json!({
                            "method": "SUBSCRIBE",
                            "params":
                            [
                                format!("{symbol}@depth@100ms")
                            ],
                            "id": task_id
                            })
                            .to_string(),
                        ))
                        .await
                        .expect("Unable to suscribe");
                        self.shared_storage.add_symbol(symbol).await;
                    }
                    TaskKind::Unsuscribe(symbol) => {
                        ws.send(Message::Text(
                            json!({
                            "method": "UNSUBSCRIBE",
                            "params":
                            [
                                format!("{symbol}@depth@100ms")
                            ],
                            "id": task_id
                            })
                            .to_string(),
                        ))
                        .await
                        .expect("Unable to suscribe");
                        self.shared_storage.remove_symbol(symbol).await;
                    }
                    TaskKind::DepthSnapshot(symbol) => {
                        let snapshot = Self::depth_snapshot(symbol).await;
                        let mut books = self.shared_storage.get_books().await;
                        let book = books.get_mut(symbol).unwrap();
                        for bid in snapshot.bids {
                            book.update_with_bid(
                                bid.0,
                                bid.1,
                                snapshot.last_update_id,
                                snapshot.last_update_id,
                            );
                        }
                        for ask in snapshot.asks {
                            book.update_with_ask(
                                ask.0,
                                ask.1,
                                snapshot.last_update_id,
                                snapshot.last_update_id,
                            );
                        }
                        task.done();
                    }
                    TaskKind::Stop => {
                        is_running = false;
                        task.done();
                    }
                }
                self.tasks.insert(task_id, task);
            }

            match timeout(Duration::from_micros(10), ws.next()).await {
                Ok(res) => {
                    let message = res
                        .expect("Empty Stream")
                        .map_err(|err| format!("Websocket Message Error: {}", &err.to_string()))
                        .unwrap();

                    match message {
                        Message::Binary(_) => panic!("Binary message received"),
                        Message::Frame(_) => panic!("Frame message received"),
                        Message::Ping(data) => {
                            ws.send(Message::Pong(data))
                                .await
                                .expect("Unable to send pong");
                        }
                        Message::Pong(_) => {}
                        Message::Close(data) => println!("Closed frame: {:?}", data),
                        Message::Text(text_data) => {
                            let json_data = serde_json::from_str::<Value>(&text_data).unwrap();
                            match json_data.get("id") {
                                Some(id) => match self.tasks.get(&(id.as_i64().unwrap() as i32)) {
                                    None => println!("Received task not found {json_data}"),
                                    Some(task) => {
                                        if let TaskKind::Suscribe(symbol) = &task.kind {
                                            self.task_transmitter
                                                .send(Task {
                                                    kind: TaskKind::DepthSnapshot(String::from(
                                                        symbol,
                                                    )),
                                                    notifier: task.notifier.clone(),
                                                })
                                                .unwrap();
                                        } else {
                                            task.done();
                                        }
                                    }
                                },
                                None => {
                                    let symbol = json_data
                                        .get("stream")
                                        .unwrap()
                                        .as_str()
                                        .unwrap()
                                        .split("@")
                                        .collect::<Vec<&str>>()[0];
                                    let stream_data = json_data.get("data").unwrap();
                                    let diff_depth = DepthData::from_message(stream_data);
                                    let mut books = self.shared_storage.get_books().await;
                                    if let Some(book) = books.get_mut(symbol) {
                                        for bid in diff_depth.bids {
                                            book.update_with_bid(
                                                bid.0,
                                                bid.1,
                                                diff_depth.first_update_id,
                                                diff_depth.last_update_id,
                                            );
                                        }
                                        for ask in diff_depth.asks {
                                            book.update_with_ask(
                                                ask.0,
                                                ask.1,
                                                diff_depth.first_update_id,
                                                diff_depth.last_update_id,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(_) => (),
            }
        }
        ws.close(None).await.unwrap();
    }

    async fn depth_snapshot(symbol: &str) -> DepthSnapshot {
        let symbol_uppercase = String::from(symbol)
            .chars()
            .map(|c| c.to_uppercase().to_string())
            .collect::<String>();
        let x = reqwest::get(format!(
            "https://api.binance.com/api/v3/depth?symbol={symbol_uppercase}&limit=5000"
        ))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
        let json_data: Value = serde_json::from_str(&x).unwrap();
        DepthSnapshot::from_message(&json_data)
    }
}
