use std::{collections::HashMap, sync::Arc};

use tokio::sync::Notify;

#[derive(Clone)]
pub enum TaskKind {
    Suscribe(String),
    Unsuscribe(String),
    DepthSnapshot(String),
    Stop,
}

#[derive(Clone)]
pub struct Task {
    pub kind: TaskKind,
    pub notifier: Arc<Notify>,
}

impl Task {
    pub fn subscribe(symbol: &str) -> Self {
        Task {
            kind: TaskKind::Suscribe(symbol.to_string()),
            notifier: Arc::new(Notify::new()),
        }
    }

    pub fn unsubscribe(symbol: &str) -> Self {
        Task {
            kind: TaskKind::Unsuscribe(symbol.to_string()),
            notifier: Arc::new(Notify::new()),
        }
    }

    pub fn depth_snapshot(symbol: &str) -> Self {
        Task {
            kind: TaskKind::DepthSnapshot(symbol.to_string()),
            notifier: Arc::new(Notify::new()),
        }
    }

    pub fn stop() -> Self {
        Task {
            kind: TaskKind::Stop,
            notifier: Arc::new(Notify::new()),
        }
    }

    pub fn done(&self) {
        self.notifier.notify_one();
    }

    pub async fn is_done(&self) {
        self.notifier.notified().await;
    }
}
