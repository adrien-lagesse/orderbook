use std::{collections::HashMap, sync::Arc};

use tokio::sync::{Mutex, MutexGuard};

use super::LocalBook;

#[derive(Clone)]
pub struct ManagerSharedStorage {
    local_books: Arc<Mutex<HashMap<String, LocalBook>>>,
}

impl ManagerSharedStorage {
    pub fn new() -> Self {
        ManagerSharedStorage {
            local_books: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn add_symbol(&mut self, symbol: &str) {
        let _ = self
            .local_books
            .lock()
            .await
            .insert(symbol.to_string(), LocalBook::new());
    }

    pub async fn remove_symbol(&mut self, symbol: &str) {
        let _ = self.local_books.lock().await.remove(symbol);
    }

    pub async fn get_books(&mut self) -> MutexGuard<'_, HashMap<String, LocalBook>> {
        self.local_books.lock().await
    }
}
