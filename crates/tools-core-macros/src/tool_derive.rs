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

    // Generate metadata() impl if any metadata attributes are provided
    let has_metadata = category.is_some() || tags.is_some() || cost.is_some();
    let metadata_impl = if has_metadata {
        let category_expr = match category.as_deref() {
            Some("General") | None => quote! { ::tools_core::ToolCategory::General },
            Some("FileSystem") => quote! { ::tools_core::ToolCategory::FileSystem },
            Some("Search") => quote! { ::tools_core::ToolCategory::Search },
            Some("Web") => quote! { ::tools_core::ToolCategory::Web },
            Some("Communication") => quote! { ::tools_core::ToolCategory::Communication },
            Some("TaskManagement") => quote! { ::tools_core::ToolCategory::TaskManagement },
            Some("Memory") => quote! { ::tools_core::ToolCategory::Memory },
            Some("Finance") => quote! { ::tools_core::ToolCategory::Finance },
            Some("Productivity") => quote! { ::tools_core::ToolCategory::Productivity },
            Some("System") => quote! { ::tools_core::ToolCategory::System },
            Some("Mcp") => quote! { ::tools_core::ToolCategory::Mcp },
            Some("Plugin") => quote! { ::tools_core::ToolCategory::Plugin },
            Some(other) => panic!(
                "#[tool(category = \"{}\")] is invalid. Use General, FileSystem, Search, Web, Communication, TaskManagement, Memory, Finance, Productivity, System, Mcp, or Plugin",
                other
            ),
        };

        let tags_expr = if let Some(ref tags_str) = tags {
            let tag_list: Vec<&str> = tags_str.split(',').map(|s| s.trim()).collect();
            quote! { vec![#(#tag_list.to_string()),*] }
        } else {
            quote! { vec![] }
        };

        let cost_expr = match cost.as_deref() {
            Some("Free") | None => quote! { ::tools_core::CostHint::Free },
            Some("Low") => quote! { ::tools_core::CostHint::Low },
            Some("Medium") => quote! { ::tools_core::CostHint::Medium },
            Some("High") => quote! { ::tools_core::CostHint::High },
            Some("Variable") => quote! { ::tools_core::CostHint::Variable },
            Some(other) => panic!(
                "#[tool(cost = \"{}\")] is invalid. Use Free, Low, Medium, High, or Variable",
                other
            ),
        };

        quote! {
            fn metadata(&self) -> ::tools_core::ToolMetadata {
                ::tools_core::ToolMetadata {
                    category: #category_expr,
                    tags: #tags_expr,
                    cost_hint: #cost_expr,
                    ..::std::default::Default::default()
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
                <Self as ::tools_core::ToolExecute>::execute(self, params, ctx).await
            }

            #permission_impl
            #metadata_impl
        }
    };

    TokenStream::from(expanded)
}
