use async_trait::async_trait;
use crate::services::extraction_critic_types::*;

#[async_trait]
pub trait ExtractionCriticHandler: Send + Sync {
    async fn judge(&self, input: ExtractionCriticInput) -> common::Result<ExtractionCriticOutput>;
}

pub struct NoopExtractionCriticHandler;

#[async_trait]
impl ExtractionCriticHandler for NoopExtractionCriticHandler {
    async fn judge(&self, _: ExtractionCriticInput) -> common::Result<ExtractionCriticOutput> {
        Ok(ExtractionCriticOutput::default())
    }
}
