use std::collections::HashMap;
use std::mem::ManuallyDrop;
use std::sync::Arc;

use tokio::runtime;
use tokio::runtime::Builder;
use tokio::sync::Mutex;
use tracing;
use uuid::Uuid;

use super::process::Process;
use super::process::ProcessHandle;
use super::tasks::Task;
use super::LocalBook;
use crate::binance::Configuration;
use crate::binance::Schedule;
use crate::Spot;

pub struct Manager {
    configuration: Configuration,
    rt: ManuallyDrop<runtime::Runtime>,
    process_handles: HashMap<Uuid, ProcessHandle>,
    spot_mapping: HashMap<Spot, Uuid>,
    spot_books: HashMap<Spot, Arc<Mutex<LocalBook>>>,
}

impl Manager {
    pub fn new(configuration: Configuration) -> Self {
        let rt = Builder::new_multi_thread()
            .enable_io()
            .enable_time()
            .worker_threads(configuration.num_threads)
            .build()
            .unwrap();

        tracing::debug!("Creation of Binance Manager: {configuration:?}");

        Manager {
            configuration,
            rt: ManuallyDrop::new(rt),
            process_handles: HashMap::new(),
            spot_mapping: HashMap::new(),
            spot_books: HashMap::new(),
        }
    }

    async fn spawn(&mut self, symbol: Spot, task: Task) {
        tracing::debug!("Spawning {}", task);
        match task {
            Task::Subscribe(_, _) => {
                // If the manager is already tracking 'symbol' we don't have to suscribe
                if self.spot_mapping.contains_key(&symbol) {
                    todo!();
                }

                // Otherwise depending on the strategy, we might have to create a new process
                match self.configuration.scheduling_strategy {
                    Schedule::Balanced(n) => {
                        if self.process_handles.len() < n {
                            tracing::debug!("creating new process");
                            let process_id = self.new_process();
                            self.spot_mapping.insert(symbol, process_id);
                            self.process_handles
                                .get_mut(&process_id)
                                .unwrap()
                                .spawn(task);
                        } else {
                            let mut uuid_count: HashMap<Uuid, usize> = HashMap::new();
                            for (_, v) in &self.spot_mapping {
                                match uuid_count.get_mut(v) {
                                    Some(n) => *n = *n + 1,
                                    None => {
                                        uuid_count.insert(v.clone(), 1);
                                    }
                                }
                            }
                            let process_id =
                                uuid_count.iter().min_by(|x, y| x.1.cmp(y.1)).unwrap().0;

                            self.spot_mapping.insert(symbol, process_id.clone());
                            self.process_handles
                                .get_mut(&process_id)
                                .unwrap()
                                .spawn(task);
                        }
                    }
                    Schedule::MaxPerTask(n) => {
                        if self.process_handles.len() == 0 {
                            let process_id = self.new_process();
                            tracing::debug!("creating new process");
                            self.spot_mapping.insert(symbol, process_id);
                            self.process_handles
                                .get_mut(&process_id)
                                .unwrap()
                                .spawn(task);
                        } else {
                            let mut uuid_count: HashMap<Uuid, usize> = HashMap::new();
                            for (_, v) in &self.spot_mapping {
                                match uuid_count.get_mut(v) {
                                    Some(n) => *n = *n + 1,
                                    None => {
                                        uuid_count.insert(v.clone(), 1);
                                    }
                                }
                            }
                            let min_process =
                                uuid_count.iter().min_by(|x, y| x.1.cmp(y.1)).unwrap();
                            if *min_process.1 < n {
                                self.spot_mapping.insert(symbol, min_process.0.clone());
                                self.process_handles
                                    .get_mut(&min_process.0)
                                    .unwrap()
                                    .spawn(task);
                            } else {
                                let process_id = self.new_process();
                                self.spot_mapping.insert(symbol, process_id);
                                self.process_handles
                                    .get_mut(&process_id)
                                    .unwrap()
                                    .spawn(task);
                            }
                        }
                    }
                }
            }
            Task::Unsubscribe(spot, notifier) => {
                if let Some(uuid) = self.spot_mapping.get(&symbol) {
                    self.process_handles
                        .get_mut(uuid)
                        .unwrap()
                        .spawn(Task::Unsubscribe(spot, notifier.clone()));
                    notifier.notified().await;
                    if self.spot_mapping.iter().filter(|(_, v)| *v == uuid).count() == 1 {
                        tracing::debug!("deleting process");
                        self.process_handles.remove(uuid).unwrap();
                    }

                    self.spot_mapping.remove(&symbol).unwrap();
                    self.spot_books.remove(&symbol).unwrap();
                }
            }
            Task::SnapShot(spot, sender) => {
                if let Some(uuid) = self.spot_mapping.get(&symbol) {
                    self.process_handles
                        .get_mut(uuid)
                        .unwrap()
                        .spawn(Task::SnapShot(spot, sender));
                }
            }
        }
    }

    fn new_process(&mut self) -> Uuid {
        let process_id = Uuid::new_v4();

        let (mut process, task_transmitter) = Process::new();

        let tokio_handle = self.rt.spawn(async move {
            process.run().await;
        });

        let handle = ProcessHandle::new(tokio_handle, task_transmitter);
        self.process_handles.insert(process_id, handle);
        process_id
    }

    pub async fn subscribe(&mut self, symbol: &Spot) {
        let (receiver, task) = Task::subscribe(symbol.clone());
        self.spawn(symbol.clone(), task).await;
        self.spot_books
            .insert(symbol.clone(), receiver.await.unwrap());
    }

    pub async fn unsubscribe(&mut self, symbol: &Spot) {
        let (notifier, task) = Task::unsubscribe(symbol.clone());
        self.spawn(symbol.clone(), task).await;
        notifier.notified().await;
    }

    pub async fn force_snapshot(&mut self, symbol: &Spot) {
        let (receiver, task) = Task::snapshot(symbol.clone());
        self.spawn(symbol.clone(), task).await;
        let _ = receiver.await.unwrap();
    }

    pub async fn get_price(&mut self, symbol: &Spot) -> Option<f32> {
        let book = self.spot_books.get(symbol)?.lock().await;
        let (bid_price, bid_quantity) = book.best_bid();
        let (ask_price, ask_quantity) = book.best_ask();
        Some((bid_price * bid_quantity + ask_price * ask_quantity) / (bid_quantity + ask_quantity))
    }

    pub async fn get_spread(&mut self, symbol: &Spot) -> Option<f32> {
        let book = self.spot_books.get(symbol)?.lock().await;
        let (bid_price, _) = book.best_bid();
        let (ask_price, _) = book.best_ask();
        Some(ask_price - bid_price)
    }

    pub async fn get_imbalance(&mut self, symbol: &Spot) -> Option<f32> {
        let book = self.spot_books.get(symbol)?.lock().await;
        let (_, bid_quantity) = book.best_bid();
        let (_, ask_quantity) = book.best_ask();
        Some(bid_quantity / (bid_quantity + ask_quantity))
    }

    pub async fn best_bid(&mut self, symbol: &Spot) -> Option<(f32, f32)> {
        let book = self.spot_books.get(symbol)?.lock().await;
        Some(book.best_bid())
    }

    pub async fn best_ask(&mut self, symbol: &Spot) -> Option<(f32, f32)> {
        let book = self.spot_books.get(symbol)?.lock().await;
        Some(book.best_ask())
    }
}

impl Drop for Manager {
    fn drop(&mut self) {
        unsafe {
            let rt = ManuallyDrop::take(&mut self.rt);
            rt.shutdown_background();
        }
    }
}
