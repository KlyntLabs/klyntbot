//! Stale-memory source.
//!
//! Buffers `MemoryRetrieved` and `AssistantMsgCompleted` events and flushes
//! them into `memory_utilization` so that selective-delete can later find
//! uncited memories.

use ai_core::{mirror::mirror_flush_secs, AiSignal, MirrorSignalSource, MirrorSnapshotSpec};
use async_trait::async_trait;
use common::KlyntbotError;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Pending signal for stale-memory tracking.
#[derive(Debug, Clone)]
enum PendingSignal {
    MemoryRetrieved {
        memory_ids: Vec<String>,
        session_id: String,
        turn_id: Option<String>,
    },
    AssistantMsgCompleted {
        session_id: String,
        turn_id: Option<String>,
        cited_memory_ids: Vec<String>,
    },
}

/// Source that tracks which retrieved memories were actually cited.
pub struct StaleMemorySource {
    pool: storage::StoragePool,
    pending: Mutex<Vec<PendingSignal>>,
}

impl StaleMemorySource {
    /// Construct.
    pub fn new(pool: storage::StoragePool) -> Self {
        Self {
            pool,
            pending: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl MirrorSignalSource for StaleMemorySource {
    fn spec(&self) -> MirrorSnapshotSpec {
        MirrorSnapshotSpec {
            name: "stale_memory",
            subscribed_kinds: &["MemoryRetrieved", "AssistantMsgCompleted"],
            flush_interval_secs: Some(mirror_flush_secs(3600)),
        }
    }

    fn name(&self) -> &'static str {
        "stale-memory-source"
    }

    async fn accumulate(&self, signal: &AiSignal) -> common::Result<()> {
        match &signal.raw_event {
            Some(bus::DomainEvent::MemoryRetrieved {
                memory_ids,
                session_id,
                turn_id,
                ..
            }) => {
                let mut pending = self.pending.lock().await;
                pending.push(PendingSignal::MemoryRetrieved {
                    memory_ids: memory_ids.clone(),
                    session_id: session_id.clone(),
                    turn_id: turn_id.clone(),
                });
            }
            Some(bus::DomainEvent::AssistantMsgCompleted {
                session_id,
                turn_id,
                cited_memory_ids,
            }) => {
                let mut pending = self.pending.lock().await;
                pending.push(PendingSignal::AssistantMsgCompleted {
                    session_id: session_id.clone(),
                    turn_id: turn_id.clone(),
                    cited_memory_ids: cited_memory_ids.clone(),
                });
            }
            _ => {}
        }
        Ok(())
    }

    async fn flush(&self) -> common::Result<()> {
        let mut pending = self.pending.lock().await;
        if pending.is_empty() {
            return Ok(());
        }

        let to_process: Vec<PendingSignal> = std::mem::take(&mut *pending);
        drop(pending);

        for signal in &to_process {
            match signal {
                PendingSignal::MemoryRetrieved {
                    memory_ids,
                    session_id,
                    turn_id,
                } => {
                    for memory_id in memory_ids {
                        sqlx::query(
                            "INSERT INTO memory_utilization \
                             (id, memory_id, cited_in_response, session_id, turn_id) \
                             VALUES (?1, ?2, 0, ?3, ?4)",
                        )
                        .bind(format!("mu_{}", Uuid::new_v4().simple()))
                        .bind(memory_id)
                        .bind(session_id)
                        .bind(turn_id)
                        .execute(self.pool.inner())
                        .await
                        .map_err(|e| {
                            KlyntbotError::Storage(format!("memory_utilization insert: {e}"))
                        })?;
                    }
                }
                PendingSignal::AssistantMsgCompleted {
                    session_id,
                    turn_id,
                    cited_memory_ids,
                } => {
                    if cited_memory_ids.is_empty() {
                        continue;
                    }
                    for memory_id in cited_memory_ids {
                        if let Some(tid) = turn_id {
                            sqlx::query(
                                "UPDATE memory_utilization \
                                 SET cited_in_response = 1 \
                                 WHERE session_id = ?1 AND turn_id = ?2 \
                                 AND memory_id = ?3 AND cited_in_response = 0",
                            )
                            .bind(session_id)
                            .bind(tid)
                            .bind(memory_id)
                            .execute(self.pool.inner())
                            .await
                            .map_err(|e| {
                                KlyntbotError::Storage(format!("memory_utilization update: {e}"))
                            })?;
                        } else {
                            sqlx::query(
                                "UPDATE memory_utilization \
                                 SET cited_in_response = 1 \
                                 WHERE session_id = ?1 AND turn_id IS NULL \
                                 AND memory_id = ?2 AND cited_in_response = 0",
                            )
                            .bind(session_id)
                            .bind(memory_id)
                            .execute(self.pool.inner())
                            .await
                            .map_err(|e| {
                                KlyntbotError::Storage(format!("memory_utilization update: {e}"))
                            })?;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
