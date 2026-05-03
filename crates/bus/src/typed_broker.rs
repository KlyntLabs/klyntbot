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

    /// K12: TypedBroker delivers events to subscribers in publish order.
    #[tokio::test]
    async fn k12_broker_delivers_in_publish_order() {
        let b: TypedBroker<u64> = TypedBroker::new(256);
        let mut rx = b.subscribe();

        let count = 100u64;
        for i in 0..count {
            b.publish(i);
        }

        for expected in 0..count {
            let got = rx.recv().await.unwrap();
            assert_eq!(got, expected, "event {expected} out of order");
        }
    }

    /// K12: Multiple subscribers see the same sequence.
    #[tokio::test]
    async fn k12_multiple_subscribers_see_same_order() {
        let b: TypedBroker<u64> = TypedBroker::new(256);
        let mut rx1 = b.subscribe();
        let mut rx2 = b.subscribe();

        for i in 0..50 {
            b.publish(i);
        }

        for expected in 0..50 {
            let a = rx1.recv().await.unwrap();
            let b = rx2.recv().await.unwrap();
            assert_eq!(a, expected);
            assert_eq!(b, expected);
        }
    }
}
