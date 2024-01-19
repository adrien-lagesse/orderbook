use std::mem::ManuallyDrop;

use tokio::{
    runtime::{self, Builder},
    sync::mpsc::UnboundedSender,
    task::JoinHandle,
};

use super::{
    manager_communication::Task, manager_process::ManagerProcess,
    manager_storage::ManagerSharedStorage,
};

pub struct Manager {
    rt: ManuallyDrop<runtime::Runtime>,
    task_transmitter: UnboundedSender<Task>,
    shared_storage: ManagerSharedStorage,
    main_process_handle: Option<JoinHandle<()>>,
}

impl Manager {
    pub fn new() -> Self {
        let rt = Builder::new_multi_thread()
            .enable_io()
            .enable_time()
            .worker_threads(1)
            .build()
            .unwrap();

        let mut process = ManagerProcess::new();
        let shared_storage = process.get_shared_storage();
        let task_transmitter = process.get_task_transmitter();

        let main_process_handle = rt.spawn(async move {
            process.run().await;
        });

        let manager = Manager {
            rt: ManuallyDrop::new(rt),
            task_transmitter,
            shared_storage,
            main_process_handle: Some(main_process_handle),
        };

        manager
    }

    pub async fn suscribe(&mut self, symbol: &str) {
        let tx = self.task_transmitter.clone();
        let task = Task::subscribe(symbol);
        tx.send(task.clone()).unwrap();
        task.is_done().await;
    }

    pub async fn unsubscribe(&mut self, symbol: &str) {
        let tx = self.task_transmitter.clone();
        let task = Task::unsubscribe(symbol);
        tx.send(task.clone()).unwrap();
        task.is_done().await;
    }

    pub async fn close(&mut self) {
        let tx = self.task_transmitter.clone();
        let task = Task::stop();
        tx.send(task.clone()).unwrap();
        task.is_done().await;
        let handle = std::mem::replace(&mut self.main_process_handle, None);
        handle
            .expect("Try closing when already closed")
            .await
            .unwrap();
    }

    pub async fn get_price(&mut self, symbol: &str) -> Option<f32> {
        let books = self.shared_storage.get_books().await;
        let book = books.get(symbol)?;
        let (bid_price, bid_quantity) = book.best_bid();
        let (ask_price, ask_quantity) = book.best_ask();
        Some((bid_price * bid_quantity + ask_price * ask_quantity) / (bid_quantity + ask_quantity))
    }

    pub async fn get_spread(&mut self, symbol: &str) -> Option<f32> {
        let books = self.shared_storage.get_books().await;
        let book = books.get(symbol)?;
        let (bid_price, _) = book.best_bid();
        let (ask_price, _) = book.best_ask();
        Some(ask_price - bid_price)
    }

    pub async fn get_imbalance(&mut self, symbol: &str) -> Option<f32> {
        let books = self.shared_storage.get_books().await;
        let book = books.get(symbol)?;
        let (_, bid_quantity) = book.best_bid();
        let (_, ask_quantity) = book.best_ask();
        Some(bid_quantity / (bid_quantity + ask_quantity))
    }

    pub async fn best_bid(&mut self, symbol: &str) -> Option<(f32, f32)> {
        let books = self.shared_storage.get_books().await;
        let book = books.get(symbol)?;
        Some(book.best_bid())
    }

    pub async fn best_ask(&mut self, symbol: &str) -> Option<(f32, f32)> {
        let books = self.shared_storage.get_books().await;
        let book = books.get(symbol)?;
        Some(book.best_ask())
    }
}

impl Drop for Manager {
    fn drop(&mut self) {
        if self.main_process_handle.is_some() {
            panic!("Manager dropped before being closed")
        }
        unsafe {
            let rt = ManuallyDrop::take(&mut self.rt);
            rt.shutdown_background();
        }
    }
}
