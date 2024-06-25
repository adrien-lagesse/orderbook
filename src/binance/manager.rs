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

        Manager {
            configuration,
            rt: ManuallyDrop::new(rt),
            process_handles: HashMap::new(),
            spot_mapping: HashMap::new(),
            spot_books: HashMap::new(),
        }
    }

    fn register_symbol(&mut self, symbol: &Spot, process_id: Uuid, book: Arc<Mutex<LocalBook>>) {
        self.spot_mapping.insert(symbol.clone(), process_id);
        self.spot_books.insert(symbol.clone(), book);
    }

    fn unregister_symbol(&mut self, symbol: &Spot, process_id: Uuid) {
        self.spot_mapping.remove(symbol).unwrap();
        self.spot_books.remove(symbol).unwrap();
        if !self.spot_mapping.values().any(|id| process_id == *id) {
            self.process_handles.remove(&process_id).unwrap();
        }
    }

    fn least_used_process(&mut self) -> Option<(Uuid, usize)> {
        let mut uuid_count: HashMap<Uuid, usize> = HashMap::new();
        for (_, v) in &self.spot_mapping {
            match uuid_count.get_mut(v) {
                Some(n) => *n = *n + 1,
                None => {
                    uuid_count.insert(v.clone(), 1);
                }
            }
        }
        let (process_id, nb_symbols) = uuid_count.iter().min_by(|x, y| x.1.cmp(y.1))?;
        Some((*process_id, *nb_symbols))
    }

    fn spawn(&mut self, task: Task) -> crate::Result<Uuid> {
        match task {
            Task::Subscribe(symbol, sender) => {
                // If the manager is already tracking 'symbol' we don't have to suscribe
                if self.spot_mapping.contains_key(&symbol) {
                    return Err(crate::Error::AlreadyTracked { symbol });
                }

                // Otherwise depending on the strategy, we might have to create a new process
                match self.configuration.scheduling_strategy {
                    Schedule::Balanced(n) if self.process_handles.len() < n => {
                        let process_id = self.new_process();
                        self.process_handles
                            .get_mut(&process_id)
                            .unwrap()
                            .spawn(Task::Subscribe(symbol, sender));
                        return Ok(process_id);
                    }
                    Schedule::Balanced(n) => {
                        let (process_id, _) = self.least_used_process().unwrap();
                        self.process_handles
                            .get_mut(&process_id)
                            .unwrap()
                            .spawn(Task::Subscribe(symbol, sender));
                        return Ok(process_id);
                    }
                    Schedule::MaxPerTask(n) if self.process_handles.len() == 0 => {
                        let process_id = self.new_process();
                        self.process_handles
                            .get_mut(&process_id)
                            .unwrap()
                            .spawn(Task::Subscribe(symbol, sender));
                        return Ok(process_id);
                    }
                    Schedule::MaxPerTask(n) => {
                        let (process_id, nb_symbols) = self.least_used_process().unwrap();
                        if nb_symbols < n {
                            self.process_handles
                                .get_mut(&process_id)
                                .unwrap()
                                .spawn(Task::Subscribe(symbol, sender));
                            return Ok(process_id);
                        } else {
                            let process_id = self.new_process();
                            self.process_handles
                                .get_mut(&process_id)
                                .unwrap()
                                .spawn(Task::Subscribe(symbol, sender));
                            return Ok(process_id);
                        }
                    }
                }
            }
            Task::Unsubscribe(symbol, sender) => {
                let process_id = self
                    .spot_mapping
                    .get(&symbol)
                    .ok_or(crate::Error::NotTracked { symbol: symbol.clone() })?;
                self.process_handles
                    .get_mut(&process_id)
                    .unwrap()
                    .spawn(Task::Unsubscribe(symbol, sender));
                return Ok(*process_id);
            }
            Task::SnapShot(symbol, sender) => {
                let process_id = self
                    .spot_mapping
                    .get(&symbol)
                    .ok_or(crate::Error::NotTracked { symbol: symbol.clone() })?;
                self.process_handles
                    .get_mut(process_id)
                    .unwrap()
                    .spawn(Task::SnapShot(symbol, sender));
                return Ok(*process_id);
            }
        }
    }

    fn new_process(&mut self) -> Uuid {
        let process_id = Uuid::new_v4();

        let (mut process, task_transmitter) = Process::new(&process_id.to_string());

        let tokio_handle = self.rt.spawn(async move {
            process.run().await;
        });

        let handle = ProcessHandle::new(tokio_handle, task_transmitter);
        self.process_handles.insert(process_id, handle);
        process_id
    }

    pub async fn force_snapshot(&mut self, symbol: &Spot) -> crate::Result<()> {
        let (receiver, task) = Task::snapshot(symbol.clone());
        self.spawn(task)?;
        receiver.await.unwrap()?;
        Ok(())
    }

    pub async fn subscribe(&mut self, symbol: &Spot) -> crate::Result<()> {
        let (receiver, task) = Task::subscribe(symbol.clone());
        let process_id = self.spawn(task)?;
        self.register_symbol(&symbol, process_id, receiver.await.unwrap()?);
        self.force_snapshot(symbol).await?;
        Ok(())
    }

    pub async fn unsubscribe(&mut self, symbol: &Spot) -> crate::Result<()> {
        let (receiver, task) = Task::unsubscribe(symbol.clone());
        let process_id = self.spawn(task)?;
        self.unregister_symbol(symbol, process_id);
        receiver.await.unwrap()?;
        Ok(())
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
