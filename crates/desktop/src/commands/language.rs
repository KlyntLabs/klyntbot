use desktop_shared::commands::{
    AnnotationEnrichmentResponse, ConfusableResponse, DetectConfusablesParams,
    EnrichAnnotationParams, EvaluateTranslationParams, TranslateBreakdownParams,
    TranslateBreakdownResponse, TranslationEvalResponse, VocabularySaveParams,
};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn language_translate_breakdown(
    state: State<'_, Arc<AppCore>>,
    params: TranslateBreakdownParams,
) -> Result<TranslateBreakdownResponse, ApiError> {
    state.language_translate_breakdown(params).await
}

#[tauri::command]
pub async fn language_evaluate_translation(
    state: State<'_, Arc<AppCore>>,
    params: EvaluateTranslationParams,
) -> Result<TranslationEvalResponse, ApiError> {
    state.language_evaluate_translation(params).await
}

#[tauri::command]
pub async fn language_save_vocabulary(
    state: State<'_, Arc<AppCore>>,
    params: VocabularySaveParams,
) -> Result<serde_json::Value, ApiError> {
    let cards = state.language_save_vocabulary(params).await?;
    Ok(serde_json::to_value(cards).unwrap_or_default())
}

#[tauri::command]
pub async fn language_detect_confusables(
    state: State<'_, Arc<AppCore>>,
    params: DetectConfusablesParams,
) -> Result<ConfusableResponse, ApiError> {
    state.language_detect_confusables(params).await
}

#[tauri::command]
pub async fn language_enrich_annotation(
    state: State<'_, Arc<AppCore>>,
    params: EnrichAnnotationParams,
) -> Result<AnnotationEnrichmentResponse, ApiError> {
    state.language_enrich_annotation(params).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "language_translate_breakdown",
    "language_evaluate_translation",
    "language_save_vocabulary",
    "language_detect_confusables",
    "language_enrich_annotation",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "language_translate_breakdown" => dev::val(
            core.language_translate_breakdown(try_field!(dev::parse_params(body)))
                .await,
        ),
        "language_evaluate_translation" => dev::val(
            core.language_evaluate_translation(try_field!(dev::parse_params(body)))
                .await,
        ),
        "language_save_vocabulary" => dev::val(
            core.language_save_vocabulary(try_field!(dev::parse_params(body)))
                .await,
        ),
        "language_detect_confusables" => dev::val(
            core.language_detect_confusables(try_field!(dev::parse_params(body)))
                .await,
        ),
        "language_enrich_annotation" => dev::val(
            core.language_enrich_annotation(try_field!(dev::parse_params(body)))
                .await,
        ),
        _ => return None,
    })
}
