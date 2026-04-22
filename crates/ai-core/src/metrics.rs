/// Well-known coaching-side metrics extracted from an event payload.
///
/// Populated by the `#[ai(coaching_signal(...))]` attribute via
/// derive-generated code. Absent fields are `None`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AiMetrics {
    pub app: Option<String>,
    pub amount: Option<f64>,
    pub category: Option<String>,
}
