//! Shared helpers for param derive macros (ActionParams, ToolParams).

use quote::quote;
use syn::{Lit, Meta, Type};

/// Type classification for JSON schema generation.
pub struct TypeInfo {
    pub json_type: &'static str,
    pub is_optional: bool,
    pub is_array: bool,
    pub is_bool: bool,
    pub is_integer: bool,
    pub is_float: bool,
}

/// Classify a Rust type into JSON schema type info.
pub fn classify_type(ty: &Type) -> TypeInfo {
    let type_str = quote!(#ty).to_string().replace(' ', "");

    if type_str == "String" {
        return TypeInfo {
            json_type: "string",
            is_optional: false,
            is_array: false,
            is_bool: false,
            is_integer: false,
            is_float: false,
        };
    }

    if type_str == "bool" {
        return TypeInfo {
            json_type: "boolean",
            is_optional: false,
            is_array: false,
            is_bool: true,
            is_integer: false,
            is_float: false,
        };
    }

    if matches!(
        type_str.as_str(),
        "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64"
    ) {
        return TypeInfo {
            json_type: "integer",
            is_optional: false,
            is_array: false,
            is_bool: false,
            is_integer: true,
            is_float: false,
        };
    }

    if matches!(type_str.as_str(), "f32" | "f64") {
        return TypeInfo {
            json_type: "number",
            is_optional: false,
            is_array: false,
            is_bool: false,
            is_integer: false,
            is_float: true,
        };
    }

    if type_str.starts_with("Vec<") {
        return TypeInfo {
            json_type: "array",
            is_optional: false,
            is_array: true,
            is_bool: false,
            is_integer: false,
            is_float: false,
        };
    }

    if type_str.starts_with("Option<") {
        let inner = &type_str[7..type_str.len() - 1];
        if inner == "String" {
            return TypeInfo {
                json_type: "string",
                is_optional: true,
                is_array: false,
                is_bool: false,
                is_integer: false,
                is_float: false,
            };
        }
        if inner == "bool" {
            return TypeInfo {
                json_type: "boolean",
                is_optional: true,
                is_array: false,
                is_bool: true,
                is_integer: false,
                is_float: false,
            };
        }
        if matches!(
            inner,
            "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32" | "i64"
        ) {
            return TypeInfo {
                json_type: "integer",
                is_optional: true,
                is_array: false,
                is_bool: false,
                is_integer: true,
                is_float: false,
            };
        }
        if matches!(inner, "f32" | "f64") {
            return TypeInfo {
                json_type: "number",
                is_optional: true,
                is_array: false,
                is_bool: false,
                is_integer: false,
                is_float: true,
            };
        }
    }

    // Fallback to string
    TypeInfo {
        json_type: "string",
        is_optional: false,
        is_array: false,
        is_bool: false,
        is_integer: false,
        is_float: false,
    }
}

/// Extract doc comments from attributes as a single joined string.
pub fn get_doc_comment(attrs: &[syn::Attribute]) -> Option<String> {
    let mut docs = Vec::new();
    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let syn::Expr::Lit(lit) = &nv.value {
                    if let Lit::Str(s) = &lit.lit {
                        docs.push(s.value().trim().to_string());
                    }
                }
            }
        }
    }
    if docs.is_empty() {
        None
    } else {
        Some(docs.join(" "))
    }
}

/// Parse `#[param(...)]` attributes on a field.
#[derive(Default)]
pub struct ParamAttrs {
    pub is_required: bool,
    pub min_val: Option<i64>,
    pub max_val: Option<i64>,
    pub min_length: Option<u64>,
    pub max_length: Option<u64>,
}

/// Parse `#[param(...)]` attributes from a field's attribute list.
pub fn parse_param_attrs(attrs: &[syn::Attribute]) -> ParamAttrs {
    let mut result = ParamAttrs::default();

    for attr in attrs {
        if attr.path().is_ident("param") {
            if let Meta::List(list) = &attr.meta {
                let tokens = list.tokens.clone();
                let parser = syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated;
                let parsed = syn::parse::Parser::parse2(parser, tokens)
                    .expect("Failed to parse #[param(...)] attributes");
                for meta in parsed {
                    match &meta {
                        Meta::Path(path) if path.is_ident("required") => {
                            result.is_required = true;
                        }
                        Meta::NameValue(nv) if nv.path.is_ident("min") => {
                            if let syn::Expr::Lit(lit) = &nv.value {
                                if let Lit::Int(i) = &lit.lit {
                                    result.min_val = Some(i.base10_parse().unwrap());
                                }
                            }
                        }
                        Meta::NameValue(nv) if nv.path.is_ident("max") => {
                            if let syn::Expr::Lit(lit) = &nv.value {
                                if let Lit::Int(i) = &lit.lit {
                                    result.max_val = Some(i.base10_parse().unwrap());
                                }
                            }
                        }
                        Meta::NameValue(nv) if nv.path.is_ident("min_length") => {
                            if let syn::Expr::Lit(lit) = &nv.value {
                                if let Lit::Int(i) = &lit.lit {
                                    result.min_length = Some(i.base10_parse().unwrap());
                                }
                            }
                        }
                        Meta::NameValue(nv) if nv.path.is_ident("max_length") => {
                            if let syn::Expr::Lit(lit) = &nv.value {
                                if let Lit::Int(i) = &lit.lit {
                                    result.max_length = Some(i.base10_parse().unwrap());
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    result
}

/// Generate the from_value field extraction for a single field.
pub fn gen_from_value_extraction(
    field_name: &syn::Ident,
    field_name_str: &str,
    type_info: &TypeInfo,
    is_required: bool,
) -> proc_macro2::TokenStream {
    if type_info.is_array {
        return quote! {
            #field_name: args.get(#field_name_str)
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_str().map(::std::string::String::from)).collect())
                .unwrap_or_default(),
        };
    }

    if type_info.is_optional {
        if type_info.is_bool {
            return quote! {
                #field_name: args.get(#field_name_str).and_then(|v| v.as_bool()),
            };
        }
        if type_info.is_integer {
            return quote! {
                #field_name: args.get(#field_name_str)
                    .and_then(|v| v.as_u64())
                    .map(|n| n as _),
            };
        }
        if type_info.is_float {
            return quote! {
                #field_name: args.get(#field_name_str)
                    .and_then(|v| v.as_f64())
                    .map(|n| n as _),
            };
        }
        // Optional String
        return quote! {
            #field_name: args.get(#field_name_str)
                .and_then(|v| v.as_str())
                .map(::std::string::String::from),
        };
    }

    if type_info.is_bool {
        if is_required {
            return quote! {
                #field_name: args.get(#field_name_str)
                    .and_then(|v| v.as_bool())
                    .ok_or_else(|| format!("missing required '{}' parameter", #field_name_str))?,
            };
        }
        return quote! {
            #field_name: args.get(#field_name_str)
                .and_then(|v| v.as_bool())
                .unwrap_or_default(),
        };
    }

    if type_info.is_integer {
        if is_required {
            return quote! {
                #field_name: args.get(#field_name_str)
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| format!("missing required '{}' parameter", #field_name_str))? as _,
            };
        }
        return quote! {
            #field_name: args.get(#field_name_str)
                .and_then(|v| v.as_i64())
                .unwrap_or_default() as _,
        };
    }

    if type_info.is_float {
        if is_required {
            return quote! {
                #field_name: args.get(#field_name_str)
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| format!("missing required '{}' parameter", #field_name_str))? as _,
            };
        }
        return quote! {
            #field_name: args.get(#field_name_str)
                .and_then(|v| v.as_f64())
                .unwrap_or_default() as _,
        };
    }

    // String
    if is_required {
        return quote! {
            #field_name: args.get(#field_name_str)
                .and_then(|v| if v.is_null() { None } else { v.as_str() })
                .ok_or_else(|| format!("missing required '{}' parameter", #field_name_str))?
                .to_string(),
        };
    }

    // Non-optional, non-required String (fallback)
    quote! {
        #field_name: args.get(#field_name_str)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
    }
}

/// Process struct fields into schema properties, required fields, and from_value extractions.
/// Shared by both `ActionParams` and `ToolParams` derive macros.
pub fn collect_field_tokens(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::Token![,]>,
) -> (
    Vec<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
) {
    let mut schema_properties = Vec::new();
    let mut required_fields = Vec::new();
    let mut from_value_fields = Vec::new();

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_name_str = field_name.to_string();
        let field_ty = &field.ty;

        let attrs = parse_param_attrs(&field.attrs);
        let description = get_doc_comment(&field.attrs);
        let type_info = classify_type(field_ty);

        schema_properties.push(gen_schema_property(
            &field_name_str,
            &type_info,
            &description,
            &attrs,
        ));

        if attrs.is_required {
            required_fields.push(quote! { #field_name_str });
        }

        from_value_fields.push(gen_from_value_extraction(
            field_name,
            &field_name_str,
            &type_info,
            attrs.is_required,
        ));
    }

    (schema_properties, required_fields, from_value_fields)
}

/// Generate the `metadata()` method TokenStream from optional category, tags, cost strings.
/// Returns empty TokenStream if no metadata attributes are provided.
pub fn gen_metadata_impl(
    category: &Option<String>,
    tags: &Option<String>,
    cost: &Option<String>,
) -> proc_macro2::TokenStream {
    let has_metadata = category.is_some() || tags.is_some() || cost.is_some();
    if !has_metadata {
        return quote! {};
    }

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
            "category = \"{}\" is invalid. Use General, FileSystem, Search, Web, Communication, TaskManagement, Memory, Finance, Productivity, System, Mcp, or Plugin",
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
            "cost = \"{}\" is invalid. Use Free, Low, Medium, High, or Variable",
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
}

/// Generate schema property insertion code for a field.
pub fn gen_schema_property(
    field_name_str: &str,
    type_info: &TypeInfo,
    description: &Option<String>,
    attrs: &ParamAttrs,
) -> proc_macro2::TokenStream {
    let json_type_str = type_info.json_type;

    let desc_insert = if let Some(ref desc) = description {
        quote! { prop.insert("description".to_string(), ::serde_json::Value::String(#desc.to_string())); }
    } else {
        quote! {}
    };

    let array_items_insert = if type_info.is_array {
        quote! { prop.insert("items".to_string(), ::serde_json::json!({"type": "string"})); }
    } else {
        quote! {}
    };

    let min_insert = if let Some(min) = attrs.min_val {
        quote! { prop.insert("minimum".to_string(), ::serde_json::json!(#min)); }
    } else {
        quote! {}
    };

    let max_insert = if let Some(max) = attrs.max_val {
        quote! { prop.insert("maximum".to_string(), ::serde_json::json!(#max)); }
    } else {
        quote! {}
    };

    let min_length_insert = if let Some(ml) = attrs.min_length {
        quote! { prop.insert("minLength".to_string(), ::serde_json::json!(#ml)); }
    } else {
        quote! {}
    };

    let max_length_insert = if let Some(ml) = attrs.max_length {
        quote! { prop.insert("maxLength".to_string(), ::serde_json::json!(#ml)); }
    } else {
        quote! {}
    };

    quote! {
        {
            let mut prop = ::serde_json::Map::new();
            prop.insert("type".to_string(), ::serde_json::Value::String(#json_type_str.to_string()));
            #desc_insert
            #array_items_insert
            #min_insert
            #max_insert
            #min_length_insert
            #max_length_insert
            properties.insert(#field_name_str.to_string(), ::serde_json::Value::Object(prop));
        }
    }
}
