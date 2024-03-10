use futures::SinkExt;
use futures::StreamExt;
use reqwest;
use serde_json::json;
use serde_json::Value;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use uuid::Uuid;

use super::depth_snapshot_data::DepthSnapshot;
use super::tasks::Task;
use super::DepthData;
use super::LocalBook;

pub struct Process {
    tasks: HashMap<Uuid, Task>,
    task_transmitter: UnboundedSender<Task>,
    task_receiver: UnboundedReceiver<Task>,
    spot_books: HashMap<String, Arc<Mutex<LocalBook>>>,
}

impl Process {
    pub fn new() -> (Process, UnboundedSender<Task>) {
        let tasks = HashMap::new();

        let (task_transmitter, task_receiver) = unbounded_channel::<Task>();
        let manager_process = Process {
            tasks,
            task_transmitter: task_transmitter.clone(),
            task_receiver,
            spot_books: HashMap::new(),
        };

        (manager_process, task_transmitter)
    }

    pub async fn run(&mut self) {
        let mut ws = self.open_websocket().await;

        loop {
            // Processing the incoming tasks
            while let Ok(task) = self.task_receiver.try_recv() {
                let task_id = Uuid::new_v4();
                match task {
                    Task::Subscribe(symbol, sender) => {
                        ws.send(Message::Text(
                            json!({
                            "method": "SUBSCRIBE",
                            "params":
                            [
                                format!("{}@depth@100ms", symbol.to_string())
                            ],
                            "id": task_id.to_string()
                            })
                            .to_string(),
                        ))
                        .await
                        .expect("Unable to suscribe");
                        self.spot_books
                            .insert(symbol.to_string(), Arc::new(Mutex::new(LocalBook::new())));
                        self.tasks.insert(task_id, Task::Subscribe(symbol, sender));
                    }
                    Task::Unsubscribe(symbol, notify) => {
                        ws.send(Message::Text(
                            json!({
                            "method": "UNSUBSCRIBE",
                            "params":
                            [
                                format!("{}@depth@100ms", symbol.to_string())
                            ],
                            "id": task_id.to_string()
                            })
                            .to_string(),
                        ))
                        .await
                        .expect("Unable to suscribe");
                        self.spot_books.remove(&symbol.to_string()).unwrap();
                        self.tasks.insert(task_id,  Task::Unsubscribe(symbol, notify));
                    }
                    Task::SnapShot(spot, sender) => {
                        let snapshot = Process::depth_snapshot(&spot.to_string()).await;
                        let arc_book = self.spot_books.get_mut(&spot.to_string()).unwrap();
                        let mut book = arc_book.lock().await;
                        let last_update_id = snapshot.last_update_id;
                        for (price, quantity) in snapshot.bids {
                            book.update_with_bid(price, quantity, last_update_id, last_update_id)
                        }
                        for (price, quantity) in snapshot.asks {
                            book.update_with_ask(price, quantity, last_update_id, last_update_id)
                        }
                        drop(book);
                        sender.send(arc_book.clone()).unwrap();
                    },
                }
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
                                Some(id) => {
                                    match self
                                        .tasks
                                        .remove(&Uuid::from_str(id.as_str().unwrap()).unwrap())
                                    {
                                        None => println!("Received task not found {json_data}"),
                                        Some(task) => match task {
                                            Task::Subscribe(symbol,sender) => {
                                                    self.task_transmitter.send(Task::SnapShot(symbol, sender)).unwrap();
                                            }
                                            Task::Unsubscribe(_, notifier) => {
                                                notifier.notify_one();
                                                notifier.notify_one();
                                            }
                                            _ => ()
                                        },
                                    }
                                }
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
                                    if let Some(book) = self.spot_books.get_mut(symbol) {
                                        let mut book = book.lock().await;
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
    }

    async fn open_websocket(&self) -> WebSocketStream<MaybeTlsStream<TcpStream>> {
        let (ws, _) = connect_async("wss://stream.binance.com:443/stream")
            .await
            .map_err(|err| format!("Websocket connection error: {}", &err.to_string()))
            .unwrap();
        ws
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

pub struct ProcessHandle {
    tokio_handle: JoinHandle<()>,
    task_transmitter: UnboundedSender<Task>,
}

impl ProcessHandle {
    pub fn new(tokio_handle: JoinHandle<()>, task_transmitter: UnboundedSender<Task>) -> Self {
        ProcessHandle {
            tokio_handle,
            task_transmitter,
        }
    }

    pub fn spawn(&self, task: Task) {
        self.task_transmitter.send(task).unwrap();
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        self.tokio_handle.abort();
    }
}
