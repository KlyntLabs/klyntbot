use crate::engine::HookEngine;
use crate::engine::discovery::ConfigLayerStack;
use crate::schema::HookConfig;
use std::path::PathBuf;

#[derive(Clone, Debug, Default)]
pub struct HookRegistry {
    pub engine: HookEngine,
    pub layers: ConfigLayerStack,
}

impl HookRegistry {
    pub fn empty() -> Self {
        Self {
            engine: HookEngine::empty(),
            layers: ConfigLayerStack::default(),
        }
    }

    pub fn from_config(cfg: HookConfig) -> Self {
        Self {
            engine: HookEngine::from_config(cfg),
            layers: ConfigLayerStack::default(),
        }
    }

    pub fn with_layers(mut self, user_dir: Option<PathBuf>, project_dir: Option<PathBuf>) -> Self {
        self.layers = ConfigLayerStack { user_dir, project_dir };
        self
    }
}
