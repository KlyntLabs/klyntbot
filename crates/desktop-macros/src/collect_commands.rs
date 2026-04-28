use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse::Parse, parse2, punctuated::Punctuated, Path, Token};

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
            syn::LitStr::new(&ident, proc_macro2::Span::call_site())
        })
        .collect();

    quote! {
        pub const KLYNT_SPECTA_COMMAND_NAMES: &[&str] = &[#(#names),*];

        pub(crate) fn __klynt_specta_commands() -> ::tauri_specta::Commands<::tauri::Wry> {
            ::tauri_specta::collect_commands![#(#paths_vec),*]
        }
    }
}
