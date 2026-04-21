use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Ident, LitStr};

pub fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let struct_ident = &input.ident;

    // Parse container attribute: #[ai(entity_type = "...", embed_on = [...])]
    let ai_attr = input.attrs.iter().find(|a| a.path().is_ident("ai"))
        .ok_or_else(|| syn::Error::new_spanned(&input,
            "AiEntity requires #[ai(entity_type = \"...\", embed_on = [...])] on the struct"))?;

    let mut entity_type: Option<String> = None;
    let mut embed_fields: Vec<Ident> = Vec::new();

    ai_attr.parse_nested_meta(|meta| {
        let name = meta.path.get_ident()
            .ok_or_else(|| meta.error("expected identifier"))?.to_string();
        match name.as_str() {
            "entity_type" => {
                let s: LitStr = meta.value()?.parse()?;
                entity_type = Some(s.value());
            }
            "embed_on" => {
                meta.input.parse::<syn::Token![=]>()?;
                let content;
                syn::bracketed!(content in meta.input);
                let list: syn::punctuated::Punctuated<LitStr, syn::Token![,]> =
                    content.parse_terminated(|s| s.parse::<LitStr>(), syn::Token![,])?;
                for lit in list {
                    embed_fields.push(syn::Ident::new(&lit.value(), lit.span()));
                }
            }
            other => return Err(meta.error(format!("unknown ai() key: {}", other))),
        }
        Ok(())
    })?;

    let entity_type = entity_type.ok_or_else(|| syn::Error::new_spanned(&input,
        "AiEntity needs #[ai(entity_type = \"...\")]"))?;

    // Verify fields exist and collect accessor code.
    let data_struct = match &input.data {
        Data::Struct(s) => s,
        _ => return Err(syn::Error::new_spanned(&input,
            "AiEntity can only be derived on structs")),
    };
    let field_map: std::collections::HashMap<String, &syn::Field> = match &data_struct.fields {
        Fields::Named(n) => n.named.iter()
            .filter_map(|f| f.ident.as_ref().map(|i| (i.to_string(), f)))
            .collect(),
        _ => return Err(syn::Error::new_spanned(&input,
            "AiEntity requires named fields")),
    };

    let accessors = embed_fields.iter().map(|id| {
        let name = id.to_string();
        let field = field_map.get(&name)
            .ok_or_else(|| syn::Error::new(id.span(),
                format!("embed_on references unknown field: {}", name)))?;
        let is_option = is_option_type(&field.ty);
        Ok(if is_option {
            quote! { self.#id.as_deref().unwrap_or("") }
        } else {
            quote! { self.#id.as_str() }
        })
    }).collect::<syn::Result<Vec<_>>>()?;

    Ok(quote! {
        impl ::ai_core::AiEntity for #struct_ident {
            fn entity_type() -> &'static str { #entity_type }

            fn embed_text(&self) -> String {
                let parts: Vec<&str> = vec![ #(#accessors),* ]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect();
                parts.join("\n")
            }
        }
    })
}

fn is_option_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(p) = ty {
        p.path.segments.last().map(|s| s.ident == "Option").unwrap_or(false)
    } else {
        false
    }
}
