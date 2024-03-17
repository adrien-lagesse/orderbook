use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use futures::SinkExt;
use futures::StreamExt;
use reqwest;
use serde_json::json;
use serde_json::Value;
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

use super::data::DepthSnapshot;
use super::data::DepthUpdate;
use super::LocalBook;
use super::Task;

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
    
    async fn process_task_queue(&mut self, ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>) -> anyhow::Result<()>{
        while let Ok(task) = self.task_receiver.try_recv() {
            match task {
                Task::Subscribe(symbol, sender) => {
                    let task_id = Uuid::new_v4();
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
                    .context("Unable to suscribe")?;
                    self.spot_books
                        .insert(symbol.to_string(), Arc::new(Mutex::new(LocalBook::new())));
                    self.tasks.insert(task_id, Task::Subscribe(symbol, sender));
                }
                Task::Unsubscribe(symbol, notify) => {
                    let task_id = Uuid::new_v4();
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
                    self.tasks
                        .insert(task_id, Task::Unsubscribe(symbol, notify));
                }
                Task::SnapShot(spot, sender) => {
                    let snapshot = Process::depth_snapshot(&spot.to_string()).await.unwrap();
                    let arc_book = self.spot_books.get_mut(&spot.to_string()).unwrap();
                    let mut book = arc_book.lock().await;
                    snapshot.update_book(&mut book);
                    drop(book);
                    sender.send(arc_book.clone()).unwrap();
                }
            }
        }
        Ok(())
    }
    async fn open_websocket() -> WebSocketStream<MaybeTlsStream<TcpStream>> {
        let (ws, _) = connect_async("wss://stream.binance.com:443/stream")
            .await
            .map_err(|err| format!("Websocket connection error: {}", &err.to_string()))
            .unwrap();
        ws
    }
    async fn depth_snapshot(symbol: &str) -> anyhow::Result<DepthSnapshot> {
        let symbol_uppercase = String::from(symbol)
            .chars()
            .map(|c| c.to_uppercase().to_string())
            .collect::<String>();
        let response = reqwest::get(format!(
            "https://api.binance.com/api/v3/depth?symbol={symbol_uppercase}&limit=5000"
        ))
        .await
        .context("Http Error while requesting order book")?;
        let text_response = response
            .text()
            .await
            .context(format!("Unable to parse to text"))?;
        let json_data: Value = serde_json::from_str(&text_response)
            .context("Unable to parse to json {text_response}")?;
        DepthSnapshot::from_json_message(&json_data)
            .context(format!("Error decoding message: {json_data}"))
    }

    async fn process_message(&mut self, message : Message) {
        match message {
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
                                Task::Subscribe(symbol, sender) => {
                                    self.task_transmitter
                                        .send(Task::SnapShot(symbol, sender))
                                        .unwrap();
                                }
                                Task::Unsubscribe(_, notifier) => {
                                    notifier.notify_one();
                                    notifier.notify_one();
                                }
                                _ => (),
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
                        let depth_update =
                            DepthUpdate::from_json_message(stream_data).unwrap();
                        if let Some(book) = self.spot_books.get_mut(symbol) {
                            let mut book = book.lock().await;
                            depth_update.update_book(&mut book)
                        }
                    }
                }
            }
            _ => ()
        }
    }
    
    pub async fn run(&mut self) {
        let mut ws = Process::open_websocket().await;

        loop {
            // Processing the incoming tasks
            self.process_task_queue(&mut ws).await.unwrap();

            // Process incoming messages
            match timeout(Duration::from_micros(10), ws.next()).await {
                Ok(res) => {
                    let message = res
                        .expect("Empty Stream")
                        .map_err(|err| format!("Websocket Message Error: {}", &err.to_string()))
                        .unwrap();

                    self.process_message(message).await;
                }
                Err(_) => (),
            }
        }
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
