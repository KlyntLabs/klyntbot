use crate::{AiSignal, SignalConsumer};
use bus::{DomainEvent, DomainEventBus};
use std::sync::Arc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Translator: DomainEvent -> Option<AiSignal>. Returns None when the event
/// has no pipeline registration (e.g. transient infra events).
pub type Translator = Arc<dyn Fn(&DomainEvent) -> Option<AiSignal> + Send + Sync>;

#[allow(dead_code)]
pub struct SignalRouter {
    handle: JoinHandle<()>,
    cancel: CancellationToken,
}

impl SignalRouter {
    pub fn start<F>(
        bus: Arc<DomainEventBus>,
        consumers: Vec<Arc<dyn SignalConsumer>>,
        translator: F,
    ) -> Self
    where
        F: Fn(&DomainEvent) -> Option<AiSignal> + Send + Sync + 'static,
    {
        let translator: Translator = Arc::new(translator);
        let cancel = CancellationToken::new();
        let cancel_child = cancel.clone();

        // Subscribe synchronously so we don't miss events published
        // immediately after `start` returns.
        let mut rx = bus.subscribe();

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_child.cancelled() => return,
                    event = rx.recv() => {
                        let Ok(event) = event else { continue };
                        let Some(mut signal) = translator(&event) else { continue };
                        signal.raw_event = Some(event);
                        let signal = Arc::new(signal);
                        let handles: Vec<_> = consumers
                            .iter()
                            .map(|c| {
                                let c = Arc::clone(c);
                                let signal = Arc::clone(&signal);
                                tokio::spawn(async move {
                                    if let Err(e) = c.consume(&signal).await {
                                        tracing::warn!(consumer = c.name(), error = %e,
                                            "SignalConsumer failed");
                                    }
                                })
                            })
                            .collect();
                        for h in handles {
                            if let Err(e) = h.await {
                                tracing::warn!("SignalConsumer task panicked: {e}");
                            }
                        }
                    }
                }
            }
        });

        Self { handle, cancel }
    }

    pub fn shutdown(self) {
        self.cancel.cancel();
    }
}
