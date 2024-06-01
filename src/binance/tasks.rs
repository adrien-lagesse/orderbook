use std::fmt::Display;
use std::sync::Arc;

use tokio::sync::oneshot;
use tokio::sync::Mutex;

use crate::binance::LocalBook;
use crate::Spot;

pub enum Task {
    Subscribe(Spot, oneshot::Sender<Arc<Mutex<LocalBook>>>),
    SnapShot(Spot, oneshot::Sender<Arc<Mutex<LocalBook>>>),
    Unsubscribe(Spot, oneshot::Sender<()>),
}

impl Task {
    pub fn subscribe(spot: Spot) -> (oneshot::Receiver<Arc<Mutex<LocalBook>>>, Self) {
        let (sender, receiver) = oneshot::channel();
        (receiver, Task::Subscribe(spot, sender))
    }

    pub fn snapshot(spot: Spot) -> (oneshot::Receiver<Arc<Mutex<LocalBook>>>, Self) {
        let (sender, receiver) = oneshot::channel();
        (receiver, Task::SnapShot(spot, sender))
    }

    pub fn unsubscribe(spot: Spot) -> (oneshot::Receiver<()>, Self) {
        let (sender, receiver) = oneshot::channel();
        (receiver, Task::Unsubscribe(spot, sender))
    }
}

impl Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Task::Subscribe(symbol, _) => write!(f, "Task::Subscribe({})", symbol.to_string()),
            Task::SnapShot(symbol, _) => write!(f, "Task::Snapshot({})", symbol.to_string()),
            Task::Unsubscribe(symbol, _) => write!(f, "Task::Unsubscribe({})", symbol.to_string()),
        }
    }
}
