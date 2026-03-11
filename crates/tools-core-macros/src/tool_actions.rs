use proc_macro::TokenStream;
use quote::quote;
use syn::parse::Parser;
use syn::{parse_macro_input, Expr, FnArg, ImplItem, ItemImpl, Lit, Meta, Type};

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_args = syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated
        .parse(attr)
        .expect("Failed to parse tool_actions attributes");

    let mut tool_name = String::new();
    let mut tool_description = String::new();
    let mut category: Option<String> = None;
    let mut tags: Option<String> = None;
    let mut cost: Option<String> = None;

    for meta in &attr_args {
        if let Meta::NameValue(nv) = meta {
            if nv.path.is_ident("name") {
                if let Expr::Lit(lit) = &nv.value {
                    if let Lit::Str(s) = &lit.lit {
                        tool_name = s.value();
                    }
                }
            } else if nv.path.is_ident("description") {
                if let Expr::Lit(lit) = &nv.value {
                    if let Lit::Str(s) = &lit.lit {
                        tool_description = s.value();
                    }
                }
            } else if nv.path.is_ident("category") {
                if let Expr::Lit(lit) = &nv.value {
                    if let Lit::Str(s) = &lit.lit {
                        category = Some(s.value());
                    }
                }
            } else if nv.path.is_ident("tags") {
                if let Expr::Lit(lit) = &nv.value {
                    if let Lit::Str(s) = &lit.lit {
                        tags = Some(s.value());
                    }
                }
            } else if nv.path.is_ident("cost") {
                if let Expr::Lit(lit) = &nv.value {
                    if let Lit::Str(s) = &lit.lit {
                        cost = Some(s.value());
                    }
                }
            }
        }
    }

    let impl_block = parse_macro_input!(item as ItemImpl);
    let self_ty = &impl_block.self_ty;

    // Collect action methods
    struct ActionInfo {
        action_name: String,
        method_name: syn::Ident,
        params_type: Type,
    }

    let mut actions = Vec::new();
    let mut other_items = Vec::new();

    for impl_item in &impl_block.items {
        if let ImplItem::Fn(method) = impl_item {
            let mut action_name = None;

            for attr in &method.attrs {
                if attr.path().is_ident("action") {
                    if let Meta::List(list) = &attr.meta {
                        let tokens = list.tokens.clone();
                        let parser =
                            syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated;
                        if let Ok(parsed) = parser.parse2(tokens) {
                            for meta in parsed {
                                if let Meta::NameValue(nv) = &meta {
                                    if nv.path.is_ident("name") {
                                        if let Expr::Lit(lit) = &nv.value {
                                            if let Lit::Str(s) = &lit.lit {
                                                action_name = Some(s.value());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if let Some(name) = action_name {
                // Extract params type from method signature (second arg after &self)
                let params_type = extract_params_type(&method.sig);

                actions.push(ActionInfo {
                    action_name: name,
                    method_name: method.sig.ident.clone(),
                    params_type,
                });
            }

            // Emit the method with action attribute stripped
            let mut cleaned_method = method.clone();
            cleaned_method
                .attrs
                .retain(|a| !a.path().is_ident("action"));
            other_items.push(ImplItem::Fn(cleaned_method));
        } else {
            other_items.push(impl_item.clone());
        }
    }

    // Generate action enum entries for parameters schema
    let action_names: Vec<&str> = actions.iter().map(|a| a.action_name.as_str()).collect();

    // Generate schema merging: collect all action params schemas
    let schema_merges: Vec<_> = actions
        .iter()
        .map(|a| {
            let params_ty = &a.params_type;
            quote! {
                {
                    let action_schema = #params_ty::json_schema();
                    if let Some(props) = action_schema.get("properties").and_then(|p| p.as_object()) {
                        for (key, value) in props {
                            if !merged_properties.contains_key(key) {
                                merged_properties.insert(key.clone(), value.clone());
                            }
                        }
                    }
                }
            }
        })
        .collect();

    // Generate dispatch match arms
    let dispatch_arms: Vec<_> = actions
        .iter()
        .map(|a| {
            let action_name = &a.action_name;
            let method_name = &a.method_name;
            let params_ty = &a.params_type;
            quote! {
                #action_name => {
                    let params = #params_ty::from_value(&args)
                        .map_err(|e| common::ToolError::InvalidParams(e))?;
                    self.#method_name(params, ctx).await
                }
            }
        })
        .collect();

    let metadata_impl = crate::helpers::gen_metadata_impl(&category, &tags, &cost);

    let expanded = quote! {
        impl #self_ty {
            #(#other_items)*
        }

        #[::async_trait::async_trait]
        impl ::tools_core::Tool for #self_ty {
            fn name(&self) -> &str {
                #tool_name
            }

            fn description(&self) -> &str {
                #tool_description
            }

            #metadata_impl

            fn parameters(&self) -> ::serde_json::Value {
                let mut merged_properties = ::serde_json::Map::new();

                // Add action enum
                merged_properties.insert(
                    "action".to_string(),
                    ::serde_json::json!({
                        "type": "string",
                        "enum": [#(#action_names),*],
                        "description": "The action to perform"
                    }),
                );

                // Merge all action param schemas
                #(#schema_merges)*

                ::serde_json::json!({
                    "type": "object",
                    "properties": ::serde_json::Value::Object(merged_properties),
                    "required": ["action"]
                })
            }

            async fn execute(&self, args: ::serde_json::Value, ctx: &::tools_core::RoutingContext) -> ::common::Result<String> {
                let action = args.get("action")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| common::ToolError::InvalidParams(
                        "missing required 'action' parameter".to_string()
                    ))?;

                match action {
                    #(#dispatch_arms)*
                    unknown => Err(common::ToolError::InvalidParams(
                        format!("unknown action: {}", unknown)
                    ).into()),
                }
            }
        }
    };

    TokenStream::from(expanded)
}

fn extract_params_type(sig: &syn::Signature) -> Type {
    // Method sig: fn method_name(&self, params: ParamsType, ctx: &RoutingContext) -> Result<String>
    // We want the second argument (index 1, after &self)
    let inputs: Vec<_> = sig.inputs.iter().collect();
    if inputs.len() < 2 {
        panic!(
            "Action method '{}' must have at least 2 args: params and ctx",
            sig.ident
        );
    }

    match &inputs[1] {
        FnArg::Typed(pat_type) => *pat_type.ty.clone(),
        _ => panic!("Expected typed argument for params"),
    }
}
