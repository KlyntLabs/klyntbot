use heck::ToKebabCase;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Path, Token, parse::Parse, parse2, punctuated::Punctuated};

struct Input {
    paths: Punctuated<Path, Token![,]>,
}

impl Parse for Input {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self {
            paths: input.parse_terminated(Path::parse, Token![,])?,
        })
    }
}

pub fn expand(input: TokenStream) -> TokenStream {
    let Input { paths } = match parse2(input) {
        Ok(i) => i,
        Err(e) => return e.to_compile_error(),
    };

    let paths_vec: Vec<_> = paths.into_iter().collect();

    let names: Vec<_> = paths_vec
        .iter()
        .map(|path| {
            let ident = path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .unwrap_or_default();
            syn::LitStr::new(&ident.to_kebab_case(), proc_macro2::Span::call_site())
        })
        .collect();

    quote! {
        pub const KLYNT_SPECTA_EVENT_NAMES: &[&str] = &[#(#names),*];

        pub(crate) fn __klynt_specta_events() -> ::tauri_specta::Events {
            ::tauri_specta::collect_events![#(#paths_vec),*]
        }
    }
}
