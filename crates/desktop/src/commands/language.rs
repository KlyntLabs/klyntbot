use desktop_macros::klynt_command;
use desktop_shared::commands::{
    AnnotationEnrichmentResponse, ConfusableResponse, DetectConfusablesParams,
    EnrichAnnotationParams, EvaluateTranslationParams, QuickTranslateParams,
    QuickTranslateResponse, TranslateBreakdownParams, TranslateBreakdownResponse,
    TranslationEvalResponse, VocabularySaveParams,
};
use desktop_shared::CommandResult;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn language_translate_breakdown(
    params: TranslateBreakdownParams,
) -> TranslateBreakdownResponse {
    state.language_translate_breakdown(params).await
}

#[klynt_command]
pub async fn language_evaluate_translation(
    params: EvaluateTranslationParams,
) -> TranslationEvalResponse {
    state.language_evaluate_translation(params).await
}

#[klynt_command]
pub async fn language_save_vocabulary(
    params: VocabularySaveParams,
) -> Vec<desktop_shared::commands::FlashcardResponse> {
    state.language_save_vocabulary(params).await
}

#[klynt_command]
pub async fn language_detect_confusables(params: DetectConfusablesParams) -> ConfusableResponse {
    state.language_detect_confusables(params).await
}

#[klynt_command]
pub async fn language_enrich_annotation(
    params: EnrichAnnotationParams,
) -> AnnotationEnrichmentResponse {
    state.language_enrich_annotation(params).await
}

#[klynt_command]
pub async fn language_quick_translate(params: QuickTranslateParams) -> QuickTranslateResponse {
    state.language_quick_translate(params).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

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
