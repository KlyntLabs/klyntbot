//! Micro-Reforge service (KCA Track 4).

use async_trait::async_trait;
use crate::services::micro_reforge_types::{MicroReforgeInput, MicroReforgeOutput};

#[async_trait]
pub trait MicroReforgeHandler: Send + Sync {
    async fn synthesize(&self, input: MicroReforgeInput) -> common::Result<MicroReforgeOutput>;
}

pub struct NoopMicroReforgeHandler;

#[async_trait]
impl MicroReforgeHandler for NoopMicroReforgeHandler {
    async fn synthesize(&self, _input: MicroReforgeInput) -> common::Result<MicroReforgeOutput> {
        Ok(MicroReforgeOutput::default())
    }
}
