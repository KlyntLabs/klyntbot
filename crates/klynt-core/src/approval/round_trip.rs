use super::decision::ApprovalDecision;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

pub type RequestId = String;

pub struct PendingApprovalsMap {
    inner: DashMap<RequestId, oneshot::Sender<ApprovalDecision>>,
}

impl PendingApprovalsMap {
    pub fn new() -> Self {
        Self {
            inner: DashMap::new(),
        }
    }
    pub fn register(&self, id: RequestId, tx: oneshot::Sender<ApprovalDecision>) {
        self.inner.insert(id, tx);
    }
    pub fn resolve(&self, id: &str, decision: ApprovalDecision) {
        if let Some((_, tx)) = self.inner.remove(id) {
            let _ = tx.send(decision);
        }
    }
    pub fn contains(&self, id: &str) -> bool {
        self.inner.contains_key(id)
    }
    pub fn len(&self) -> usize {
        self.inner.len()
    }
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    pub fn cancel_all(&self) {
        let keys: Vec<RequestId> = self.inner.iter().map(|e| e.key().clone()).collect();
        for key in keys {
            self.resolve(&key, ApprovalDecision::Cancelled);
        }
    }
}

impl Default for PendingApprovalsMap {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn await_decision(
    pending: &Arc<PendingApprovalsMap>,
    request_id: &str,
    cancel: CancellationToken,
    timeout: Duration,
) -> ApprovalDecision {
    let (tx, rx) = oneshot::channel();
    pending.register(request_id.to_string(), tx);
    tokio::select! {
        biased;
        v = rx => v.unwrap_or(ApprovalDecision::Cancelled),
        _ = cancel.cancelled() => {
            pending.inner.remove(request_id);
            ApprovalDecision::Cancelled
        }
        _ = tokio::time::sleep(timeout) => {
            pending.inner.remove(request_id);
            ApprovalDecision::TimedOut
        }
    }
}
