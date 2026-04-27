//! Klynt's Tauri command attribute macros. See
//! `docs/superpowers/specs/2026-04-27-typed-command-macros-design.md`.

use proc_macro::TokenStream;

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
