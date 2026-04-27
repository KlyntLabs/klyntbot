use desktop_shared::commands::{
    AnnotationEnrichmentResponse, ConfusableResponse, DetectConfusablesParams,
    EnrichAnnotationParams, EvaluateTranslationParams, QuickTranslateParams,
    QuickTranslateResponse, TranslateBreakdownParams, TranslateBreakdownResponse,
    TranslationEvalResponse, VocabularySaveParams,
};
use desktop_shared::CommandResult;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
#[specta::specta]
pub async fn language_translate_breakdown(
    state: State<'_, Arc<AppCore>>,
    params: TranslateBreakdownParams,
) -> CommandResult<TranslateBreakdownResponse> {
    state.language_translate_breakdown(params).await
}

#[tauri::command]
#[specta::specta]
pub async fn language_evaluate_translation(
    state: State<'_, Arc<AppCore>>,
    params: EvaluateTranslationParams,
) -> CommandResult<TranslationEvalResponse> {
    state.language_evaluate_translation(params).await
}

#[tauri::command]
#[specta::specta]
pub async fn language_save_vocabulary(
    state: State<'_, Arc<AppCore>>,
    params: VocabularySaveParams,
) -> CommandResult<Vec<desktop_shared::commands::FlashcardResponse>> {
    state.language_save_vocabulary(params).await
}

#[tauri::command]
#[specta::specta]
pub async fn language_detect_confusables(
    state: State<'_, Arc<AppCore>>,
    params: DetectConfusablesParams,
) -> CommandResult<ConfusableResponse> {
    state.language_detect_confusables(params).await
}

#[tauri::command]
#[specta::specta]
pub async fn language_enrich_annotation(
    state: State<'_, Arc<AppCore>>,
    params: EnrichAnnotationParams,
) -> CommandResult<AnnotationEnrichmentResponse> {
    state.language_enrich_annotation(params).await
}

#[tauri::command]
#[specta::specta]
pub async fn language_quick_translate(
    state: State<'_, Arc<AppCore>>,
    params: QuickTranslateParams,
) -> CommandResult<QuickTranslateResponse> {
    state.language_quick_translate(params).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "language_translate_breakdown",
    "language_evaluate_translation",
    "language_save_vocabulary",
    "language_detect_confusables",
    "language_enrich_annotation",
    "language_quick_translate",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
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
        "language_quick_translate" => dev::val(
            core.language_quick_translate(try_field!(dev::parse_params(body)))
                .await,
        ),
        _ => return None,
    })
}
