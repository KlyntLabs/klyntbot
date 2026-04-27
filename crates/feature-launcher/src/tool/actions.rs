use tools_core::ActionParams;

#[derive(Debug, ActionParams)]
pub struct SearchParams {
    /// Search query. Empty returns recent/frecent items.
    #[param(required)]
    pub query: String,
    /// Maximum number of results (default 10).
    pub limit: Option<i64>,
}

#[derive(Debug, ActionParams)]
pub struct ExecuteParams {
    /// Item ID returned from `search` (e.g. "app:/Applications/Slack.app").
    #[param(required)]
    pub item_id: String,
    /// Item kind discriminator (e.g. "application", "script", "systemCommand").
    #[param(required)]
    pub kind: String,
}

#[derive(Debug, ActionParams)]
pub struct ApplyWindowParams {
    /// Window action: "leftHalf" | "rightHalf" | "topHalf" | "bottomHalf" |
    /// "leftThird" | "centerThird" | "rightThird" | "maximize" | "center" | "restore"
    /// or "preset:<name>" for named presets.
    #[param(required)]
    pub action: String,
}

#[derive(Debug, ActionParams)]
pub struct PinParams {
    /// Item ID to pin/unpin.
    #[param(required)]
    pub item_id: String,
    /// Item kind discriminator.
    #[param(required)]
    pub kind: String,
}
