//! Proc macros for klyntbot tool framework.
//!
//! - `#[derive(DomainEnum)]` — generates from_str_loose, as_str, Display
//! - `#[derive(ActionParams)]` — generates JSON Schema + from_value
//! - `#[tool_actions]` — generates Tool::execute dispatch

use proc_macro::TokenStream;

mod action_params;
mod domain_enum;
mod tool_actions;

#[proc_macro_derive(DomainEnum, attributes(aliases, canonical))]
pub fn derive_domain_enum(input: TokenStream) -> TokenStream {
    domain_enum::derive(input)
}

#[proc_macro_derive(ActionParams, attributes(param))]
pub fn derive_action_params(input: TokenStream) -> TokenStream {
    action_params::derive(input)
}

#[proc_macro_attribute]
pub fn tool_actions(attr: TokenStream, item: TokenStream) -> TokenStream {
    tool_actions::expand(attr, item)
}
