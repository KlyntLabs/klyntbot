//! `#[derive(ToolParams)]` — generates `ToolParams` trait implementation.
//!
//! Like `ActionParams` but generates a trait impl (enabling generic code)
//! and uses `common::Result` instead of `Result<Self, String>`.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

use crate::helpers::collect_field_tokens;

pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            Fields::Unit => {
                return TokenStream::from(gen_empty_impl(name));
            }
            _ => panic!("ToolParams only supports named fields or unit structs"),
        },
        _ => panic!("ToolParams can only be derived for structs"),
    };

    let (schema_properties, required_fields, from_value_fields) = collect_field_tokens(fields);

    let expanded = quote! {
        impl ::tools_core::ToolParams for #name {
            fn json_schema() -> ::serde_json::Value {
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

            fn from_args(args: ::serde_json::Value) -> ::common::Result<Self> {
                let args = &args;
                (|| -> ::core::result::Result<Self, ::std::string::String> {
                    Ok(Self {
                        #(#from_value_fields)*
                    })
                })().map_err(|e| ::common::KlyntbotError::Tool(::common::ToolError::InvalidParams(e)))
            }
        }
    };

    TokenStream::from(expanded)
}

fn gen_empty_impl(name: &syn::Ident) -> proc_macro2::TokenStream {
    quote! {
        impl ::tools_core::ToolParams for #name {
            fn json_schema() -> ::serde_json::Value {
                ::serde_json::json!({
                    "type": "object",
                    "properties": {}
                })
            }

            fn from_args(_args: ::serde_json::Value) -> ::common::Result<Self> {
                Ok(Self {})
            }
        }
    }
}
