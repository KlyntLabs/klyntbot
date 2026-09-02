//! `#[derive(Tool)]` — generates `Tool` trait impl from metadata attributes.
//!
//! Requires:
//! - `#[tool(name = "...", description = "...")]` on the struct
//! - `params = "ParamsType"` to specify which ToolParams type to use
//! - Optional: `permission = "elevated"` for non-standard permission level
//! - Optional: `category = "FileSystem"` — maps to ToolCategory enum
//! - Optional: `tags = "file,read,content"` — comma-separated tags
//! - Optional: `cost = "Free"` — maps to CostHint enum
//! - Optional: `allowed_channels = "all"|"desktop_only"` — maps into ExposurePolicy
//! - Optional: `subagent = "true"|"false"` — maps into ExposurePolicy
//! - Optional: `mcp_exposure = "default"|"opt_in"|"forbidden"` — maps into ExposurePolicy
//! - The struct must also implement `ToolExecute<Params = ParamsType>`
//!
//! Generates the full `tools_core::Tool` implementation bridging the
//! untyped JSON interface to the typed `ToolExecute::execute()`.

use proc_macro::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{parse_macro_input, DeriveInput, Lit, Meta};

pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Parse #[tool(name = "...", description = "...", params = "...", permission = "...",
    //              category = "...", tags = "...", cost = "...", mcp_exposure = "...", ...)]
    let mut tool_name: Option<String> = None;
    let mut tool_description: Option<String> = None;
    let mut params_type: Option<syn::Ident> = None;
    let mut category: Option<String> = None;
    let mut tags: Option<String> = None;
    let mut cost: Option<String> = None;
    let mut concurrency_safe: Option<bool> = None;
    let mut allowed_channels: Option<String> = None;
    let mut subagent: Option<bool> = None;
    let mut mcp_exposure: Option<String> = None;
    let mut custom_timeout_secs: Option<u64> = None;
    let mut approval_class: Option<String> = None;
    let mut approval_scope: Option<String> = None;

    for attr in &input.attrs {
        if attr.path().is_ident("tool") {
            if let Meta::List(list) = &attr.meta {
                let tokens = list.tokens.clone();
                let parser = syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated;
                let parsed = syn::parse::Parser::parse2(parser, tokens)
                    .expect("Failed to parse #[tool(...)] attributes");
                /// Extract a string literal from a `Meta::NameValue`.
                fn expect_str_lit(nv: &syn::MetaNameValue) -> Option<String> {
                    if let syn::Expr::Lit(lit) = &nv.value {
                        if let Lit::Str(s) = &lit.lit {
                            return Some(s.value());
                        }
                    }
                    None
                }

                for meta in parsed {
                    if let Meta::NameValue(nv) = &meta {
                        if nv.path.is_ident("name") {
                            tool_name = expect_str_lit(nv);
                        } else if nv.path.is_ident("description") {
                            tool_description = expect_str_lit(nv);
                        } else if nv.path.is_ident("params") {
                            if let Some(s) = expect_str_lit(nv) {
                                params_type = Some(syn::Ident::new(&s, nv.value.span()));
                            }
                        } else if nv.path.is_ident("category") {
                            category = expect_str_lit(nv);
                        } else if nv.path.is_ident("tags") {
                            tags = expect_str_lit(nv);
                        } else if nv.path.is_ident("cost") {
                            cost = expect_str_lit(nv);
                        } else if nv.path.is_ident("concurrency_safe") {
                            if let Some(s) = expect_str_lit(nv) {
                                concurrency_safe = Some(matches!(s.as_str(), "true" | "1"));
                            }
                        } else if nv.path.is_ident("allowed_channels") {
                            allowed_channels = expect_str_lit(nv);
                        } else if nv.path.is_ident("subagent") {
                            if let Some(s) = expect_str_lit(nv) {
                                subagent = Some(match s.as_str() {
                                    "true" | "1" => true,
                                    "false" | "0" => false,
                                    other => panic!(
                                        "#[tool(subagent = \"{}\")] is invalid. Use \"true\" or \"false\"",
                                        other
                                    ),
                                });
                            }
                        } else if nv.path.is_ident("mcp_exposure") {
                            mcp_exposure = expect_str_lit(nv);
                        } else if nv.path.is_ident("custom_timeout_secs") {
                            if let Some(s) = expect_str_lit(nv) {
                                custom_timeout_secs = Some(s.parse().unwrap_or_else(|_| {
                                    panic!("#[tool(custom_timeout_secs = \"...\")] must be a valid integer")
                                }));
                            }
                        } else if nv.path.is_ident("approval_class") {
                            approval_class = expect_str_lit(nv);
                        } else if nv.path.is_ident("approval_scope") {
                            approval_scope = expect_str_lit(nv);
                        }
                    }
                }
            }
        }
    }

    let tool_name =
        tool_name.unwrap_or_else(|| panic!("#[derive(Tool)] requires #[tool(name = \"...\")]"));
    let tool_description = tool_description
        .unwrap_or_else(|| panic!("#[derive(Tool)] requires #[tool(description = \"...\")]"));
    let params_type =
        params_type.unwrap_or_else(|| panic!("#[derive(Tool)] requires #[tool(params = \"...\")]"));

    let metadata_impl = crate::helpers::gen_metadata_impl(&category, &tags, &cost);

    let concurrency_impl = match concurrency_safe {
        Some(true) => quote! {
            fn is_concurrency_safe(&self, _args: &::serde_json::Value) -> bool { true }
        },
        _ => quote! {},
    };

    let exposure_policy_impl = crate::helpers::gen_exposure_policy_impl(
        allowed_channels.as_deref(),
        subagent,
        mcp_exposure.as_deref(),
        "tool",
    );

    let custom_timeout_impl = if let Some(secs) = custom_timeout_secs {
        quote! {
            fn custom_timeout(&self) -> Option<::std::time::Duration> {
                Some(::std::time::Duration::from_secs(#secs))
            }
        }
    } else {
        quote! {}
    };

    let approval_class_impl = if let Some(ref class) = approval_class {
        let variant = match class.as_str() {
            "safe" => quote! { ::tools_core::approval_class::ApprovalClass::Safe },
            "sensitive" => quote! { ::tools_core::approval_class::ApprovalClass::Sensitive },
            "destructive" => quote! { ::tools_core::approval_class::ApprovalClass::Destructive },
            "admin" => quote! { ::tools_core::approval_class::ApprovalClass::Admin },
            other => panic!(
                "#[tool(approval_class = \"{}\")] is invalid. Use \"safe\", \"sensitive\", \"destructive\", or \"admin\"",
                other
            ),
        };
        quote! {
            fn approval_class(&self, _args: &::serde_json::Value) -> ::tools_core::approval_class::ApprovalClass {
                #variant
            }
        }
    } else {
        quote! {}
    };

    let approval_scope_impl = if let Some(ref resource_key) = approval_scope {
        let key = resource_key.clone();
        quote! {
            fn approval_scope(&self, args: &::serde_json::Value) -> ::tools_core::approval_class::ApprovalScope {
                if let Some(val) = args.get(#key).and_then(|v| v.as_str()) {
                    ::tools_core::approval_class::ApprovalScope::ToolActionResource(val.to_string())
                } else {
                    ::tools_core::approval_class::ApprovalScope::ToolAction
                }
            }
        }
    } else {
        quote! {}
    };

    let expanded = quote! {
        #[::async_trait::async_trait]
        impl ::tools_core::Tool for #name {
            fn name(&self) -> &str {
                #tool_name
            }

            fn description(&self) -> &str {
                #tool_description
            }

            fn parameters(&self) -> ::serde_json::Value {
                <#params_type as ::tools_core::ToolParams>::json_schema()
            }

            async fn execute(
                &self,
                args: ::serde_json::Value,
                ctx: &::tools_core::RoutingContext,
            ) -> ::common::Result<String> {
                let params = <#params_type as ::tools_core::ToolParams>::from_args(args)?;
                // Project the full RoutingContext into the narrow view this tool
                // declared via `ToolExecute::Ctx` (ADR-0002).
                let view = <<Self as ::tools_core::ToolExecute>::Ctx<'_>
                    as ::tools_core::FromRoutingContext>::project(ctx);
                <Self as ::tools_core::ToolExecute>::execute(self, params, view).await
            }

            #metadata_impl
            #concurrency_impl
            #exposure_policy_impl
            #custom_timeout_impl
            #approval_class_impl
            #approval_scope_impl
        }
    };

    TokenStream::from(expanded)
}
