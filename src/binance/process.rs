use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use futures::SinkExt;
use futures::StreamExt;
use reqwest;
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
use tracing;
use tracing::field;
use uuid::Uuid;

use super::data::DepthSnapshot;
use super::LocalBook;
use super::Task;
use crate::binance::schema::incoming;
use crate::binance::schema::outgoing;
use crate::Result;

pub struct Process {
    name: String,
    tasks: HashMap<Uuid, Task>,
    task_receiver: UnboundedReceiver<Task>,
    spot_books: HashMap<String, Arc<Mutex<LocalBook>>>,
}

impl Process {
    pub fn new(name: &str) -> (Process, UnboundedSender<Task>) {
        let tasks = HashMap::new();

        let (task_transmitter, task_receiver) = unbounded_channel::<Task>();
        let manager_process = Process {
            name: name.to_string(),
            tasks,
            task_receiver,
            spot_books: HashMap::new(),
        };

        (manager_process, task_transmitter)
    }

    async fn process_task_queue(
        &mut self,
        ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
    ) -> Result<()> {
        while let Ok(task) = self.task_receiver.try_recv() {
            tracing::info!(target: "orderbook-processes", process_name=self.name, "processing {task}");
            match task {
                Task::Subscribe(symbol, sender) => {
                    // Add an ID to the Subscription message so we know can notify the user when the subscription is complete
                    let task_id = Uuid::new_v4();
                    // Binance Stream subscription
                    ws.send(Message::Text(outgoing::subscribe(&symbol, &task_id)))
                        .await?;

                    // Adding a new LocalBook to the process and adding the task
                    self.spot_books
                        .insert(symbol.to_string(), Arc::new(Mutex::new(LocalBook::new())));
                    self.tasks.insert(task_id, Task::Subscribe(symbol, sender));
                }
                Task::Unsubscribe(symbol, sender) => {
                    // Add an ID to the Subscription message so we know can notify the user when the subscription is complete
                    let task_id = Uuid::new_v4();
                    // Binance Stream unsubscription
                    ws.send(Message::Text(outgoing::unsubscribe(&symbol, &task_id)))
                        .await?;

                    // Adding removing the LocalBook
                    self.spot_books.remove(&symbol.to_string()).unwrap();
                    self.tasks
                        .insert(task_id, Task::Unsubscribe(symbol, sender));
                }
                Task::SnapShot(spot, sender) => {
                    let snapshot = Process::depth_snapshot(&spot.to_string()).await.unwrap();
                    let arc_book = self.spot_books.get_mut(&spot.to_string()).unwrap();
                    let mut book = arc_book.lock().await;
                    snapshot.update_book(&mut book);
                    drop(book);
                    sender.send(Ok(())).unwrap();
                }
            }
        }
        Ok(())
    }

    async fn open_websocket(&self) -> WebSocketStream<MaybeTlsStream<TcpStream>> {
        let (ws, _) = connect_async("wss://stream.binance.com:443/stream")
            .await
            .map_err(|err| format!("Websocket connection error: {}", &err.to_string()))
            .unwrap();
        tracing::debug!(target: "orderbook-processes", process_name=self.name, "websocket opened");
        ws
    }

    async fn depth_snapshot(symbol: &str) -> crate::Result<DepthSnapshot> {
        let symbol_uppercase = String::from(symbol)
            .chars()
            .map(|c| c.to_uppercase().to_string())
            .collect::<String>();
        let response = reqwest::get(format!(
            "https://api.binance.com/api/v3/depth?symbol={symbol_uppercase}&limit=5000"
        ))
        .await
        .map_err(|e| crate::Error::SnapshotHTTPError {
            symbol: symbol.to_string(),
            source: e,
        })?;
        let text_response =
            response
                .text()
                .await
                .map_err(|_| crate::Error::SnapshotParsingError {
                    symbol: symbol.to_string(),
                    message: "Unable to parse body to text".to_string(),
                    body: "".to_string(),
                })?;

        let json_data: Value = serde_json::from_str(&text_response).map_err(|e| {
            crate::Error::SnapshotParsingError {
                symbol: symbol.to_string(),
                message: "Unable to parse body into JSON".to_string(),
                body: text_response,
            }
        })?;

        DepthSnapshot::from_json_message(&json_data)
    }

    async fn process_message(&mut self, message: Message) {
        if let Message::Text(text_data) = message {
            match serde_json::from_str::<incoming::IncomingMessage>(&text_data).unwrap() {
                incoming::IncomingMessage::Response(request_response) => {
                    match self
                        .tasks
                        .remove(&Uuid::from_str(&request_response.id).unwrap())
                    {
                        Some(task) => {
                            tracing::info!(target: "orderbook-processes", process_name=self.name, "validating {task}");
                            match task {
                                Task::Subscribe(symbol, sender) => sender
                                    .send(Ok(self
                                        .spot_books
                                        .get(&symbol.to_string())
                                        .unwrap()
                                        .clone()))
                                    .unwrap(),
                                Task::SnapShot(_, _) => (),
                                Task::Unsubscribe(_, sender) => {
                                    sender.send(Ok(())).unwrap();
                                }
                            }
                        }
                        None => (),
                    }
                }
                incoming::IncomingMessage::DepthUpdate(depth_update) => {
                    if let Some(book) = self.spot_books.get(depth_update.get_symbol_str()) {
                        let mut book = book.lock().await;
                        depth_update.get_bids().iter().for_each(|level| {
                            book.update_with_bid(
                                level.price(),
                                level.quantity(),
                                depth_update.get_first_update_id(),
                                depth_update.get_last_update_id(),
                            );
                        });
                        depth_update.get_asks().iter().for_each(|level| {
                            book.update_with_ask(
                                level.price(),
                                level.quantity(),
                                depth_update.get_first_update_id(),
                                depth_update.get_last_update_id(),
                            );
                        });
                    }
                }
                incoming::IncomingMessage::Other(v) => {
                    tracing::warn!(target: "orderbook-processes", process_name=self.name, "unknown incoming message");
                    tracing::debug!(target: "orderbook-unknown-message", process_name=self.name, message=v.to_string());
                }
            }
        }
    }

    pub async fn run(&mut self) {
        tracing::info!(target:"orderbook-processes", process_name=self.name, "starting process");
        let mut ws = self.open_websocket().await;

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
