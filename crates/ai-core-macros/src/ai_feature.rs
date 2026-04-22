use crate::attrs::parse_ai_feature_attr;
use proc_macro2::TokenStream;
use quote::quote;

pub fn expand(input: syn::DeriveInput) -> syn::Result<TokenStream> {
    let struct_ident = &input.ident;

    let feat = parse_ai_feature_attr(&input.attrs)?;

    let recall_domain_ident = &feat.recall_domain;
    let skill_lit = &feat.skill;
    let event_path = &feat.event;

    Ok(quote! {
        impl ::ai_core::AiFeature for #struct_ident {
            const DOMAIN: ::ai_core::RecallDomain = ::ai_core::RecallDomain::#recall_domain_ident;
            const SKILL: &'static str = #skill_lit;
            type Event = #event_path;
        }
    })
}
