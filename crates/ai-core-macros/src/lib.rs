use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod ai_entity;
mod ai_event;
mod ai_feature;
pub(crate) mod attrs;

#[proc_macro_derive(AiEvent, attributes(ai))]
pub fn derive_ai_event(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    ai_event::expand(input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[proc_macro_derive(AiEntity, attributes(ai))]
pub fn derive_ai_entity(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    ai_entity::expand(input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[proc_macro_derive(AiFeature, attributes(ai))]
pub fn derive_ai_feature(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    ai_feature::expand(input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
