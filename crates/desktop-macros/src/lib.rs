//! Klynt's Tauri command attribute macros. See
//! `docs/superpowers/specs/2026-04-27-typed-command-macros-design.md`.

use proc_macro::TokenStream;

mod collect_commands;
mod collect_events;
mod errors;
mod klynt_command;
mod klynt_raw_command;
mod parse;

#[proc_macro_attribute]
pub fn klynt_command(_attr: TokenStream, item: TokenStream) -> TokenStream {
    klynt_command::expand(item.into()).into()
}

#[proc_macro_attribute]
pub fn klynt_raw_command(_attr: TokenStream, item: TokenStream) -> TokenStream {
    klynt_raw_command::expand(item.into()).into()
}

/// Single source-of-truth macro for tauri-specta command registration.
///
/// Emits both `::tauri_specta::collect_commands![...]` (wrapped in a
/// `pub(crate)` helper) and a `pub const KLYNT_SPECTA_COMMAND_NAMES: &[&str]`
/// array extracted from the last path segment of each function.
///
/// Invoke once at module scope:
/// ```ignore
/// desktop_macros::klynt_collect_commands![
///     crate::commands::agents::agent_list_profiles,
///     crate::commands::chat::chat_threads,
/// ];
/// ```
#[proc_macro]
pub fn klynt_collect_commands(input: TokenStream) -> TokenStream {
    collect_commands::expand(input.into()).into()
}

/// Single source-of-truth macro for tauri-specta event registration.
///
/// Emits both `::tauri_specta::collect_events![...]` (wrapped in a
/// `pub(crate)` helper) and a `pub const KLYNT_SPECTA_EVENT_NAMES: &[&str]`
/// array using kebab-case of the last path segment of each event type.
///
/// Invoke once at module scope:
/// ```ignore
/// desktop_macros::klynt_collect_events![
///     desktop_shared::events::ActivitySwitchPayload,
/// ];
/// ```
#[proc_macro]
pub fn klynt_collect_events(input: TokenStream) -> TokenStream {
    collect_events::expand(input.into()).into()
}
