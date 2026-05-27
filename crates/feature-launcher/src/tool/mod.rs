use crate::tool::actions::*;
use crate::types::WindowAction;
use crate::{FrequencyRepo, PinsRepo, SourceRegistry};
use common::Result;
use std::sync::Arc;
use tools_core::tool_actions;

pub mod actions;

pub struct LauncherTool {
    registry: Arc<SourceRegistry>,
    frequency: Arc<FrequencyRepo>,
    pins: Arc<PinsRepo>,
}

impl LauncherTool {
    pub fn new(
        registry: Arc<SourceRegistry>,
        frequency: Arc<FrequencyRepo>,
        pins: Arc<PinsRepo>,
    ) -> Self {
        Self {
            registry,
            frequency,
            pins,
        }
    }
}

#[tool_actions(
    ctx = "()",
    name = "launcher",
    description = "Search and execute launcher items: apps, scripts, files, system commands, window layouts, browser bookmarks, contacts, and more.",
    category = "System",
    tags = "launcher,search,apps,files,commands",
    cost = "Free"
)]
impl LauncherTool {
    #[action(name = "search")]
    async fn search(&self, params: SearchParams, _ctx: ()) -> Result<String> {
        let limit = params.limit.unwrap_or(10) as usize;
        let results = self.registry.search(&params.query, limit).await;
        Ok(serde_json::to_string(&results)?)
    }

    #[action(name = "execute")]
    async fn execute(&self, params: ExecuteParams, _ctx: ()) -> Result<String> {
        self.frequency
            .record_usage(&params.item_id, &params.kind)
            .await?;
        Ok("{\"status\":\"recorded\"}".to_string())
    }

    #[action(name = "apply_window")]
    async fn apply_window(&self, params: ApplyWindowParams, _ctx: ()) -> Result<String> {
        let action = parse_window_action(&params.action)?;
        crate::window_manager().execute(&action)?;
        Ok("{\"status\":\"ok\"}".to_string())
    }

    #[action(name = "pin")]
    async fn pin(&self, params: PinParams, _ctx: ()) -> Result<String> {
        self.pins.pin(&params.item_id, &params.kind).await?;
        Ok("{\"status\":\"pinned\"}".to_string())
    }

    #[action(name = "unpin")]
    async fn unpin(&self, params: PinParams, _ctx: ()) -> Result<String> {
        self.pins.unpin(&params.item_id, &params.kind).await?;
        Ok("{\"status\":\"unpinned\"}".to_string())
    }
}

fn parse_window_action(s: &str) -> Result<WindowAction> {
    if let Some(rest) = s.strip_prefix("preset:") {
        return Ok(WindowAction::Preset(rest.to_string()));
    }
    Ok(match s {
        "leftHalf" => WindowAction::LeftHalf,
        "rightHalf" => WindowAction::RightHalf,
        "topHalf" => WindowAction::TopHalf,
        "bottomHalf" => WindowAction::BottomHalf,
        "leftThird" => WindowAction::LeftThird,
        "centerThird" => WindowAction::CenterThird,
        "rightThird" => WindowAction::RightThird,
        "maximize" => WindowAction::Maximize,
        "center" => WindowAction::Center,
        "restore" => WindowAction::Restore,
        other => {
            return Err(
                common::ToolError::InvalidParams(format!("unknown window action: {other}")).into(),
            )
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_actions() {
        assert!(matches!(
            parse_window_action("leftHalf").unwrap(),
            WindowAction::LeftHalf
        ));
        assert!(matches!(
            parse_window_action("preset:left-third").unwrap(),
            WindowAction::Preset(_)
        ));
        assert!(parse_window_action("garbage").is_err());
    }
}
