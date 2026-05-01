//! `#[derive(Tool)]` — generates `Tool` trait impl from metadata attributes.
//!
//! Requires:
//! - `#[tool(name = "...", description = "...")]` on the struct
//! - `params = "ParamsType"` to specify which ToolParams type to use
//! - Optional: `permission = "elevated"` for non-standard permission level
//! - Optional: `category = "FileSystem"` — maps to ToolCategory enum
//! - Optional: `tags = "file,read,content"` — comma-separated tags
//! - Optional: `cost = "Free"` — maps to CostHint enum
//! - The struct must also implement `ToolExecute<Params = ParamsType>`
//!
//! Generates the full `tools_core::Tool` implementation bridging the
//! untyped JSON interface to the typed `ToolExecute::execute()`.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Lit, Meta};

pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Parse #[tool(name = "...", description = "...", params = "...", permission = "...",
    //              category = "...", tags = "...", cost = "...")]
    let mut tool_name: Option<String> = None;
    let mut tool_description: Option<String> = None;
    let mut params_type: Option<syn::Ident> = None;
    let mut permission: Option<String> = None;
    let mut category: Option<String> = None;
    let mut tags: Option<String> = None;
    let mut cost: Option<String> = None;
    let mut concurrency_safe: Option<bool> = None;
    let mut allowed_channels: Option<String> = None;

    for attr in &input.attrs {
        if attr.path().is_ident("tool") {
            if let Meta::List(list) = &attr.meta {
                let tokens = list.tokens.clone();
                let parser = syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated;
                let parsed = syn::parse::Parser::parse2(parser, tokens)
                    .expect("Failed to parse #[tool(...)] attributes");
                for meta in parsed {
                    if let Meta::NameValue(nv) = &meta {
                        if nv.path.is_ident("name") {
                            if let syn::Expr::Lit(lit) = &nv.value {
                                if let Lit::Str(s) = &lit.lit {
                                    tool_name = Some(s.value());
                                }
                            }
                        } else if nv.path.is_ident("description") {
                            if let syn::Expr::Lit(lit) = &nv.value {
                                if let Lit::Str(s) = &lit.lit {
                                    tool_description = Some(s.value());
                                }
                            }
                        } else if nv.path.is_ident("params") {
                            if let syn::Expr::Lit(lit) = &nv.value {
                                if let Lit::Str(s) = &lit.lit {
                                    params_type = Some(syn::Ident::new(&s.value(), s.span()));
                                }
                            }
                        } else if nv.path.is_ident("permission") {
                            if let syn::Expr::Lit(lit) = &nv.value {
                                if let Lit::Str(s) = &lit.lit {
                                    permission = Some(s.value());
                                }
                            }
                        } else if nv.path.is_ident("category") {
                            if let syn::Expr::Lit(lit) = &nv.value {
                                if let Lit::Str(s) = &lit.lit {
                                    category = Some(s.value());
                                }
                            }
                        } else if nv.path.is_ident("tags") {
                            if let syn::Expr::Lit(lit) = &nv.value {
                                if let Lit::Str(s) = &lit.lit {
                                    tags = Some(s.value());
                                }
                            }
                        } else if nv.path.is_ident("cost") {
                            if let syn::Expr::Lit(lit) = &nv.value {
                                if let Lit::Str(s) = &lit.lit {
                                    cost = Some(s.value());
                                }
                            }
                        } else if nv.path.is_ident("concurrency_safe") {
                            if let syn::Expr::Lit(lit) = &nv.value {
                                if let Lit::Str(s) = &lit.lit {
                                    concurrency_safe =
                                        Some(matches!(s.value().as_str(), "true" | "1"));
                                }
                            }
                        } else if nv.path.is_ident("allowed_channels") {
                            if let syn::Expr::Lit(lit) = &nv.value {
                                if let Lit::Str(s) = &lit.lit {
                                    allowed_channels = Some(s.value());
                                }
                            }
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

    let permission_impl = if let Some(perm) = permission {
        let perm_variant = match perm.as_str() {
            "read_only" => quote! { ::tools_core::PermissionLevel::ReadOnly },
            "standard" => quote! { ::tools_core::PermissionLevel::Standard },
            "elevated" => quote! { ::tools_core::PermissionLevel::Elevated },
            "admin" => quote! { ::tools_core::PermissionLevel::Admin },
            other => panic!(
                "#[tool(permission = \"{}\")] is invalid. Use \"read_only\", \"standard\", \"elevated\", or \"admin\"",
                other
            ),
        };
        quote! {
            fn permission_level(&self) -> ::tools_core::PermissionLevel {
                #perm_variant
            }
        }
    } else {
        quote! {}
    };

    let metadata_impl = crate::helpers::gen_metadata_impl(&category, &tags, &cost);

    let concurrency_impl = match concurrency_safe {
        Some(true) => quote! {
            fn is_concurrency_safe(&self, _args: &::serde_json::Value) -> bool { true }
        },
        _ => quote! {},
    };

    let allowed_channels_impl = if let Some(channels) = allowed_channels {
        let mask_expr = match channels.as_str() {
            "coding_only"   => quote! { ::common::ChannelMask::CODING_ONLY },
            "desktop_only"  => quote! { ::common::ChannelMask::DESKTOP_ONLY },
            "non_coding"    => quote! { ::common::ChannelMask::NON_CODING },
            "all"           => quote! { ::common::ChannelMask::ALL },
            other => panic!(
                "#[tool(allowed_channels = \"{}\")] is invalid. Use \"all\", \"coding_only\", \"desktop_only\", or \"non_coding\"",
                other
            ),
        };
        quote! {
            fn allowed_channels(&self) -> ::common::ChannelMask {
                #mask_expr
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
                <Self as ::tools_core::ToolExecute>::execute(self, params, ctx).await
            }

            #permission_impl
            #metadata_impl
            #concurrency_impl
            #allowed_channels_impl
        }
    };

    TokenStream::from(expanded)
}
