//! `#[derive(ToolParams)]` — generates `ToolParams` trait implementation.
//!
//! Like `ActionParams` but generates a trait impl (enabling generic code)
//! and uses `common::Result` instead of `Result<Self, String>`.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

use crate::helpers::{
    classify_type, gen_from_value_extraction, gen_schema_property, get_doc_comment,
    parse_param_attrs,
};

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
