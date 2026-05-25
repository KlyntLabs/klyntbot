use cognitive::mirror::{
    BrainVersion, FeedbackTarget, MirrorResponse, MirrorState, NarrativeSnippet, RoutingSnapshot,
    TrendNarrative, UserFeedback,
};
use desktop_macros::klynt_command;
use desktop_shared::types::EntityKind;
use uuid::Uuid;

#[klynt_command]
pub async fn get_mirror_state() -> MirrorState {
    Ok(state.mirror_facade()?.get_state().await?)
}

#[klynt_command]
pub async fn get_routing_history(days: Option<u32>) -> Vec<RoutingSnapshot> {
    Ok(state
        .mirror_facade()?
        .get_routing_history(days.unwrap_or(7))
        .await?)
}

#[klynt_command]
pub async fn get_mirror_narratives(limit: Option<u32>) -> Vec<TrendNarrative> {
    Ok(state
        .mirror_facade()?
        .get_narratives(limit.unwrap_or(10))
        .await?)
}

#[klynt_command]
pub async fn get_pending_snippets() -> Vec<NarrativeSnippet> {
    Ok(state.mirror_facade()?.get_pending_snippets().await?)
}

#[klynt_command]
pub async fn submit_mirror_feedback(
    item_id: Uuid,
    target: FeedbackTarget,
    feedback: UserFeedback,
) -> () {
    state
        .mirror_facade()?
        .submit_feedback(item_id, target, feedback)
        .await?;
    emitter.emit_entity_updated(EntityKind::MirrorSnippet, &item_id.to_string());
    Ok(())
}

#[klynt_command]
pub async fn generate_mirror_response(query: String) -> MirrorResponse {
    Ok(state
        .mirror_facade()?
        .generate_mirror_response(query, None)
        .await?)
}

#[klynt_command]
pub async fn approve_meta_rule(rule_id: Uuid) -> () {
    state.mirror_facade()?.approve_meta_rule(rule_id).await?;
    emitter.emit_entity_updated(EntityKind::MirrorSnippet, &rule_id.to_string());
    Ok(())
}

#[klynt_command]
pub async fn dismiss_meta_rule(rule_id: Uuid) -> () {
    state.mirror_facade()?.dismiss_meta_rule(rule_id).await?;
    emitter.emit_entity_updated(EntityKind::MirrorSnippet, &rule_id.to_string());
    Ok(())
}

#[klynt_command]
pub async fn get_brain_versions() -> Vec<BrainVersion> {
    Ok(state.mirror_facade()?.get_brain_versions().await?)
}

#[klynt_command]
pub async fn revert_brain_version(version: u32) -> BrainVersion {
    let result = state.mirror_facade()?.revert_to_version(version).await?;
    emitter.emit_entity_updated(EntityKind::BrainVersion, &version.to_string());
    Ok(result)
}

#[klynt_command]
pub async fn kill_trial(trial_id: String) -> () {
    state.mirror_facade()?.kill_trial(&trial_id).await?;
    emitter.emit_entity_updated(EntityKind::MirrorSnippet, &trial_id);
    Ok(())
}

#[klynt_command]
pub async fn continue_trial(trial_id: String) -> () {
    state.mirror_facade()?.continue_trial(&trial_id).await?;
    emitter.emit_entity_updated(EntityKind::MirrorSnippet, &trial_id);
    Ok(())
}
