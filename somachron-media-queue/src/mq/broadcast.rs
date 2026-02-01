use std::{collections::HashMap, fmt::Debug};

use tokio::sync::broadcast;
use uuid::Uuid;

pub trait BroadcastEvent {
    fn init_event() -> Self;
}

pub struct Broadcaster<T> {
    clients: HashMap<Uuid, broadcast::Sender<T>>,
}
impl<T: BroadcastEvent + Debug + Clone + 'static> Broadcaster<T> {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    pub async fn subscribe(&self, item_id: &Uuid) -> Option<broadcast::Receiver<T>> {
        self.clients.get(item_id).map(|tx| tx.subscribe())
    }

    pub async fn add_client(&mut self, item_id: &Uuid) -> broadcast::Receiver<T> {
        let (tx, rx) = broadcast::channel::<T>(16);
        tx.send(T::init_event()).unwrap();

        self.clients.insert(item_id.clone(), tx);
        rx
    }

    pub async fn drop_sub(&mut self, item_id: &Uuid) {
        self.clients.remove(item_id);
    }

    pub async fn broadcast(&self, item_id: &Uuid, event: T) {
        if let Some(sender) = self.clients.get(item_id) {
            if let Err(err) = sender.send(event) {
                tracing::warn!("Failed to broadcast event: {}", err);
            }
        }
    }
}
