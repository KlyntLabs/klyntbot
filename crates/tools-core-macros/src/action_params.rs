use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Lit, Meta, Type};

pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            Fields::Unit => {
                return TokenStream::from(gen_empty_impl(name));
            }
            _ => panic!("ActionParams only supports named fields or unit structs"),
        },
        _ => panic!("ActionParams can only be derived for structs"),
    };

    let mut schema_properties = Vec::new();
    let mut required_fields = Vec::new();
    let mut from_value_fields = Vec::new();

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_name_str = field_name.to_string();
        let field_ty = &field.ty;

        // Parse #[param(...)] attributes
        let mut is_required = false;
        let mut min_val: Option<i64> = None;
        let mut max_val: Option<i64> = None;
        let mut min_length: Option<u64> = None;
        let mut max_length: Option<u64> = None;

        for attr in &field.attrs {
            if attr.path().is_ident("param") {
                if let Meta::List(list) = &attr.meta {
                    let tokens = list.tokens.clone();
                    let parser =
                        syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated;
                    let parsed = syn::parse::Parser::parse2(parser, tokens)
                        .expect("Failed to parse #[param(...)] attributes");
                    for meta in parsed {
                        match &meta {
                            Meta::Path(path) if path.is_ident("required") => {
                                is_required = true;
                            }
                            Meta::NameValue(nv) if nv.path.is_ident("min") => {
                                if let syn::Expr::Lit(lit) = &nv.value {
                                    if let Lit::Int(i) = &lit.lit {
                                        min_val = Some(i.base10_parse().unwrap());
                                    }
                                }
                            }
                            Meta::NameValue(nv) if nv.path.is_ident("max") => {
                                if let syn::Expr::Lit(lit) = &nv.value {
                                    if let Lit::Int(i) = &lit.lit {
                                        max_val = Some(i.base10_parse().unwrap());
                                    }
                                }
                            }
                            Meta::NameValue(nv) if nv.path.is_ident("min_length") => {
                                if let syn::Expr::Lit(lit) = &nv.value {
                                    if let Lit::Int(i) = &lit.lit {
                                        min_length = Some(i.base10_parse().unwrap());
                                    }
                                }
                            }
                            Meta::NameValue(nv) if nv.path.is_ident("max_length") => {
                                if let syn::Expr::Lit(lit) = &nv.value {
                                    if let Lit::Int(i) = &lit.lit {
                                        max_length = Some(i.base10_parse().unwrap());
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Get doc comments as description
        let description = get_doc_comment(&field.attrs);

        // Determine the JSON type info from the Rust type
        let type_info = classify_type(field_ty);
        let json_type_str = type_info.json_type;

        // Build schema property dynamically
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

        let min_insert = if let Some(min) = min_val {
            quote! { prop.insert("minimum".to_string(), ::serde_json::json!(#min)); }
        } else {
            quote! {}
        };

        let max_insert = if let Some(max) = max_val {
            quote! { prop.insert("maximum".to_string(), ::serde_json::json!(#max)); }
        } else {
            quote! {}
        };

        let min_length_insert = if let Some(ml) = min_length {
            quote! { prop.insert("minLength".to_string(), ::serde_json::json!(#ml)); }
        } else {
            quote! {}
        };

        let max_length_insert = if let Some(ml) = max_length {
            quote! { prop.insert("maxLength".to_string(), ::serde_json::json!(#ml)); }
        } else {
            quote! {}
        };

        schema_properties.push(quote! {
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
        });

        if is_required {
            required_fields.push(quote! { #field_name_str });
        }

        // Generate from_value extraction
        let extraction =
            gen_from_value_extraction(field_name, &field_name_str, &type_info, is_required);
        from_value_fields.push(extraction);
    }

    let expanded = quote! {
        impl #name {
            pub fn json_schema() -> ::serde_json::Value {
                let mut properties = ::serde_json::Map::new();
                #(#schema_properties)*

                let required: Vec<&str> = vec![#(#required_fields),*];

                let mut schema = ::serde_json::json!({
                    "type": "object",
                    "properties": ::serde_json::Value::Object(properties)
                });

                if !required.is_empty() {
                    schema["required"] = ::serde_json::json!(required);
                }

                schema
            }

            pub fn from_value(args: &::serde_json::Value) -> ::core::result::Result<Self, ::std::string::String> {
                Ok(Self {
                    #(#from_value_fields)*
                })
            }
        }
    };

    TokenStream::from(expanded)
}

struct TypeInfo {
    json_type: &'static str,
    is_optional: bool,
    is_array: bool,
    is_bool: bool,
    is_integer: bool,
    is_float: bool,
}

fn classify_type(ty: &Type) -> TypeInfo {
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

fn gen_from_value_extraction(
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

    // Required String
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

fn gen_empty_impl(name: &syn::Ident) -> proc_macro2::TokenStream {
    quote! {
        impl #name {
            pub fn json_schema() -> ::serde_json::Value {
                ::serde_json::json!({
                    "type": "object",
                    "properties": {}
                })
            }

            pub fn from_value(_args: &::serde_json::Value) -> ::core::result::Result<Self, ::std::string::String> {
                Ok(Self {})
            }
        }
    }
}

fn get_doc_comment(attrs: &[syn::Attribute]) -> Option<String> {
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
