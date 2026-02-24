//! Integration tests for /api/settings endpoints.
//!
//! Covers AC-14s.x: GET all sections (redacted), GET section, PATCH section,
//! and critically: secret field redaction.
//!
//! Settings security: Secret<String> fields must NEVER be exposed in API responses.
//! Known secret field names: apiKey, token, botToken, appToken, imapPassword,
//! smtpPassword, secret, appSecret, encryptKey, clientSecret.

mod common;

// ── GET /api/settings ─────────────────────────────────────────────────────────

#[tokio::test]
async fn get_settings_returns_full_config() {
    // AC-14s.1: GET /api/settings returns the entire config as JSON
    // TODO: implement
}

#[tokio::test]
async fn get_settings_redacts_api_key_fields() {
    // AC-14s.1 SECURITY: "apiKey" fields are replaced with "••••••"
    // This is the most critical security test — API keys must NEVER appear in responses
    // TODO: implement
    // let resp = GET /api/settings
    // walk the JSON tree, assert any "apiKey" field value == "••••••"
}

#[tokio::test]
async fn get_settings_redacts_bot_token_fields() {
    // AC-14s.1 SECURITY: "botToken" fields are replaced with "••••••"
    // TODO: implement
}

#[tokio::test]
async fn get_settings_redacts_all_known_secret_field_names() {
    // AC-14s.1 SECURITY: All known secret field names are redacted:
    // apiKey, token, botToken, appToken, imapPassword, smtpPassword,
    // secret, appSecret, encryptKey, clientSecret
    // Walk entire response tree and assert none of these have non-bullet values
    // TODO: implement
}

#[tokio::test]
async fn get_settings_non_secret_fields_are_readable() {
    // AC-14s.1: Non-secret fields (e.g., model, host, port) are not redacted
    // TODO: implement
}

// ── GET /api/settings/{section} ───────────────────────────────────────────────

#[tokio::test]
async fn get_settings_section_todo() {
    // AC-14s.2: GET /api/settings/todo returns todo config section
    // TODO: implement
}

#[tokio::test]
async fn get_settings_section_agents() {
    // AC-14s.2: GET /api/settings/agents returns agents config section
    // TODO: implement
}

#[tokio::test]
async fn get_settings_section_not_found_returns_404() {
    // AC-14s.2: GET /api/settings/nonexistent → 404
    // TODO: implement
}

#[tokio::test]
async fn get_settings_section_also_redacts_secrets() {
    // AC-14s.2 SECURITY: Section-level GET also redacts secrets
    // TODO: implement
}

// ── PATCH /api/settings/{section} ────────────────────────────────────────────

#[tokio::test]
async fn patch_settings_section_merges_changes() {
    // AC-14s.3: PATCH /api/settings/todo { "enrichment": { "enabled": false } }
    // Applies JSON merge-patch (RFC 7396) to the section
    // Verify: GET /api/settings/todo shows updated value
    // TODO: implement
}

#[tokio::test]
async fn patch_settings_section_does_not_overwrite_unreferenced_fields() {
    // AC-14s.3: Merge-patch semantics — omitted fields are preserved
    // TODO: implement
}

#[tokio::test]
async fn patch_settings_ignores_redacted_placeholder_values() {
    // AC-14s.3 SECURITY: If a client sends { "apiKey": "••••••" },
    // the server MUST NOT write "••••••" to disk.
    // The handler skips any value that equals the redaction placeholder.
    // TODO: implement
}

#[tokio::test]
async fn patch_settings_section_not_found_returns_404() {
    // AC-14s.3: PATCH /api/settings/nonexistent → 404
    // TODO: implement
}

// ── Data dir read-only ────────────────────────────────────────────────────────

#[tokio::test]
async fn patch_settings_data_dir_is_rejected() {
    // AC-14s.x, UX §1.3: dataDir is read-only — PATCH that includes it must be rejected
    // (either ignored or 422 — per BA spec dataDir changes require restart + migration)
    // TODO: implement
}
