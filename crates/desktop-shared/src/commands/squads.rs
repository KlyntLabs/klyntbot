use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SquadResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub orchestrator_skill: String,
    pub default_interaction_mode: String,
    pub source: String,
    pub domains: Vec<String>,
    pub is_active: bool,
    pub members: Vec<SquadMemberResponse>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SquadMemberResponse {
    pub persona_id: String,
    pub persona_name: String,
    pub persona_icon: String,
    pub persona_role: String,
    pub role_in_squad: String,
    pub sort_order: i64,
}

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateSquadParams {
    pub name: String,
    pub description: String,
    pub icon: String,
    pub orchestrator_skill: String,
    pub domains: Vec<String>,
    pub member_persona_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSquadParams {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub domains: Option<Vec<String>>,
    pub orchestrator_skill: Option<String>,
    pub default_interaction_mode: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SquadMemberParams {
    pub squad_id: String,
    pub persona_id: String,
    pub role_in_squad: Option<String>,
    pub sort_order: Option<i64>,
}
