use crate::attrs::{parse_ai_feature_attr, MirrorSnapshotAttr};
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

    let priority = feat
        .recall_priority_field
        .as_ref()
        .map(|i| quote! { Some(stringify!(#i)) })
        .unwrap_or_else(|| quote! { None });
    let recency = feat
        .recall_recency_field
        .as_ref()
        .map(|i| quote! { Some(stringify!(#i)) })
        .unwrap_or_else(|| quote! { None });
    let status = match &feat.recall_status_filter {
        Some(expr) => {
            let s = quote!(#expr).to_string();
            quote! { Some(#s) }
        }
        None => quote! { None },
    };

    let mirror_specs_tokens = render_mirror_specs(&feat.mirror_snapshots);

    let promote_threshold_ts = match feat.promotion_threshold {
        Some(n) => quote! { Some(#n as usize) },
        None => quote! { None },
    };

    let tool_name_const = match &feat.tool_name {
        Some(s) => quote! { Some(#s) },
        None => quote! { None },
    };
    let entity_kind_const = match &feat.entity_kind {
        Some(s) => quote! { Some(#s) },
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

            pub const MIRROR_SNAPSHOTS: &'static [::ai_core::MirrorSnapshotSpec] =
                #mirror_specs_tokens;

            /// Per-domain promotion threshold override. When `Some`, beats the global
            /// `accumulate_promote_threshold` config in `BackgroundConsolidationService`.
            pub const PROMOTE_THRESHOLD_OVERRIDE: Option<usize> = #promote_threshold_ts;

            pub const TOOL_NAME: Option<&'static str> = #tool_name_const;
            pub const ENTITY_KIND: Option<&'static str> = #entity_kind_const;

            pub fn register(reg: &mut ::ai_core::AiFeatureRegistry) {
                reg.register(::ai_core::FeatureRecord {
                    domain: <Self as ::ai_core::AiFeature>::DOMAIN,
                    skill: <Self as ::ai_core::AiFeature>::SKILL,
                    tool_name: Self::TOOL_NAME,
                    entity_kind: Self::ENTITY_KIND,
                });
            }
        }
    })
}

fn render_mirror_specs(snapshots: &[MirrorSnapshotAttr]) -> TokenStream {
    if snapshots.is_empty() {
        return quote! { &[] };
    }
    let entries = snapshots.iter().map(|s| {
        let name = &s.name;
        let interval = match s.flush_interval_secs {
            Some(v) => quote! { Some(#v) },
            None => quote! { None },
        };
        let kinds = s.subscribed_kinds.iter().map(|k| quote! { #k });
        quote! {
            ::ai_core::MirrorSnapshotSpec {
                name: #name,
                subscribed_kinds: &[ #(#kinds),* ],
                flush_interval_secs: #interval,
            }
        }
    });
    quote! { &[ #(#entries),* ] }
}
