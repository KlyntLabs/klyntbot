use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Ident, LitStr};

pub fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let struct_ident = &input.ident;

    let ai_attr =
        input
            .attrs
            .iter()
            .find(|a| a.path().is_ident("ai"))
            .ok_or_else(|| {
                syn::Error::new_spanned(&input,
            "AiFeature requires #[ai(recall_domain = \"...\", skill = \"...\", event = ...)]")
            })?;

    let mut recall_domain: Option<String> = None;
    let mut skill: Option<String> = None;
    let mut event_ty: Option<syn::Path> = None;

    ai_attr.parse_nested_meta(|meta| {
        let name = meta
            .path
            .get_ident()
            .ok_or_else(|| meta.error("expected identifier"))?
            .to_string();
        match name.as_str() {
            "recall_domain" => {
                let s: LitStr = meta.value()?.parse()?;
                recall_domain = Some(s.value());
            }
            "skill" => {
                let s: LitStr = meta.value()?.parse()?;
                skill = Some(s.value());
            }
            "event" => {
                let s: LitStr = meta.value()?.parse()?;
                event_ty = Some(syn::parse_str(&s.value())?);
            }
            other => return Err(meta.error(format!("unknown ai() key: {}", other))),
        }
        Ok(())
    })?;

    let domain_variant = Ident::new(
        &recall_domain
            .ok_or_else(|| syn::Error::new_spanned(&input, "AiFeature needs recall_domain"))?,
        proc_macro2::Span::call_site(),
    );
    let skill = skill.ok_or_else(|| syn::Error::new_spanned(&input, "AiFeature needs skill"))?;
    let event_path = event_ty.ok_or_else(|| {
        syn::Error::new_spanned(&input, "AiFeature needs event = \"path::to::EventEnum\"")
    })?;

    Ok(quote! {
        impl ::ai_core::AiFeature for #struct_ident {
            const DOMAIN: ::ai_core::RecallDomain = ::ai_core::RecallDomain::#domain_variant;
            const SKILL: &'static str = #skill;
            type Event = #event_path;
        }
    })
}
