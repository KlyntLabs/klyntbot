use crate::attrs::parse_ai_feature_attr;
use proc_macro2::TokenStream;
use quote::quote;

pub fn expand(input: syn::DeriveInput) -> syn::Result<TokenStream> {
    let struct_ident = &input.ident;

    let feat = parse_ai_feature_attr(&input.attrs)?;

    let recall_domain_ident = &feat.recall_domain;
    let skill_lit = &feat.skill;
    let event_path = &feat.event;

    let boost_expr = match &feat.recall_boost_when {
        Some(expr) => quote! {
            fn score_query(&self, query: &::ai_core::RecallQuery) -> f64 {
                if { #expr } { 1.0 } else { 0.3 }
            }
        },
        None => quote! {},
    };

    let priority = feat.recall_priority_field.as_ref()
        .map(|i| quote! { Some(stringify!(#i)) })
        .unwrap_or_else(|| quote! { None });
    let recency = feat.recall_recency_field.as_ref()
        .map(|i| quote! { Some(stringify!(#i)) })
        .unwrap_or_else(|| quote! { None });
    let status = match &feat.recall_status_filter {
        Some(expr) => {
            let s = quote!(#expr).to_string();
            quote! { Some(#s) }
        }
        None => quote! { None },
    };

    Ok(quote! {
        impl ::ai_core::AiFeature for #struct_ident {
            const DOMAIN: ::ai_core::RecallDomain = ::ai_core::RecallDomain::#recall_domain_ident;
            const SKILL: &'static str = #skill_lit;
            type Event = #event_path;
        }

        impl ::ai_core::RecallProvider for #struct_ident {
            fn domain(&self) -> ::ai_core::RecallDomain {
                ::ai_core::RecallDomain::#recall_domain_ident
            }
            #boost_expr
        }

        impl #struct_ident {
            pub const RECALL_SPEC: ::ai_core::RecallSpec = ::ai_core::RecallSpec {
                priority_field: #priority,
                recency_field: #recency,
                status_filter: #status,
            };
        }
    })
}
