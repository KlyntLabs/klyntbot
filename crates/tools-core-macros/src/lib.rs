//! Proc macros for klyntbot tool framework.
//!
//! - `#[derive(DomainEnum)]` — generates from_str_loose, as_str, Display
//! - `#[derive(ActionParams)]` — generates JSON Schema + from_value (inherent methods)
//! - `#[derive(ToolParams)]` — generates `ToolParams` trait impl (json_schema + from_args)
//! - `#[tool_actions]` — generates Tool::execute dispatch

use proc_macro::TokenStream;

mod action_params;
mod domain_enum;
mod helpers;
mod tool_actions;
mod tool_params;

#[proc_macro_derive(DomainEnum, attributes(aliases, canonical))]
pub fn derive_domain_enum(input: TokenStream) -> TokenStream {
    domain_enum::derive(input)
}

#[proc_macro_derive(ActionParams, attributes(param))]
pub fn derive_action_params(input: TokenStream) -> TokenStream {
    action_params::derive(input)
}

#[proc_macro_derive(ToolParams, attributes(param))]
pub fn derive_tool_params(input: TokenStream) -> TokenStream {
    tool_params::derive(input)
}

#[proc_macro_attribute]
pub fn tool_actions(attr: TokenStream, item: TokenStream) -> TokenStream {
    tool_actions::expand(attr, item)
}
