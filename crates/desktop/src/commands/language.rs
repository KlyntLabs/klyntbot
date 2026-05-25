use desktop_macros::klynt_command;
use desktop_shared::commands::{
    AnnotationEnrichmentResponse, ConfusableResponse, DetectConfusablesParams,
    EnrichAnnotationParams, EvaluateTranslationParams, QuickTranslateParams,
    QuickTranslateResponse, TranslateBreakdownParams, TranslateBreakdownResponse,
    TranslationEvalResponse, VocabularySaveParams,
};

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
