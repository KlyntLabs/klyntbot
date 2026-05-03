use tokio::sync::broadcast;

/// A typed pub/sub broker. Compile-time guarantees on event payload type up to the
/// Tauri serialization boundary. Adapter task fans `subscribe()` output → app.emit.
#[derive(Debug, Clone)]
pub struct TypedBroker<E: Clone + Send + 'static> {
    sender: broadcast::Sender<E>,
}

impl<E: Clone + Send + 'static> TypedBroker<E> {
    pub fn new(capacity: usize) -> Self {
        let (sender, _rx) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn publish(&self, event: E) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<E> {
        self.sender.subscribe()
    }

    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn broker_publishes_to_subscribers() {
        let b: TypedBroker<u64> = TypedBroker::new(16);
        let mut rx1 = b.subscribe();
        let mut rx2 = b.subscribe();
        b.publish(42);
        assert_eq!(rx1.recv().await.unwrap(), 42);
        assert_eq!(rx2.recv().await.unwrap(), 42);
    }

    #[tokio::test]
    async fn broker_drops_silently_with_no_subscribers() {
        let b: TypedBroker<u64> = TypedBroker::new(16);
        b.publish(7);
        assert_eq!(b.receiver_count(), 0);
    }
}
