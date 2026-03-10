use std::sync::Arc;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::normalizers::normalize_domain_event;
use crate::service::ActivityIngestionService;

/// Subscribes to the DomainEventBus and ingests normalized events into the unified log.
pub struct ActivityLogSubscriber {
    cancel: CancellationToken,
    _handle: JoinHandle<()>,
}

impl ActivityLogSubscriber {
    pub fn start(
        domain_bus: &bus::DomainEventBus,
        service: Arc<ActivityIngestionService>,
        cancel: CancellationToken,
    ) -> Self {
        let mut rx = domain_bus.subscribe();
        let cancel_clone = cancel.clone();

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => {
                        debug!("ActivityLogSubscriber shutting down");
                        break;
                    }
                    result = rx.recv() => {
                        match result {
                            Ok(event) => {
                                let entry = normalize_domain_event(&event);
                                if let Err(e) = service.ingest(entry).await {
                                    warn!("Failed to ingest domain event: {e}");
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                warn!("ActivityLogSubscriber lagged by {n} events");
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                debug!("DomainEventBus closed");
                                break;
                            }
                        }
                    }
                }
            }
        });

        Self {
            cancel,
            _handle: handle,
        }
    }

    pub fn stop(&self) {
        self.cancel.cancel();
    }
}

impl Drop for ActivityLogSubscriber {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
