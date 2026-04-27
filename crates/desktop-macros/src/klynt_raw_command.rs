use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse2, ItemFn};

use crate::errors::{err, ERR_NOT_FUNCTION};

pub fn expand(input: TokenStream) -> TokenStream {
    let fn_item: ItemFn = match parse2(input.clone()) {
        Ok(f) => f,
        Err(_) => return err(input, ERR_NOT_FUNCTION),
    };

    let fn_ident = &fn_item.sig.ident;
    let dispatcher_ident = format_ident!("__klynt_dispatch_{}", fn_ident);
    let registration_ident = format_ident!("__klynt_command_{}", fn_ident);
    let fn_name_str = fn_ident.to_string();

    quote! {
        #fn_item

        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #dispatcher_ident(invoke: ::tauri::ipc::Invoke<::tauri::Wry>) -> bool {
            fn call_handler<H>(handler: H, invoke: ::tauri::ipc::Invoke<::tauri::Wry>) -> bool
            where
                H: Fn(::tauri::ipc::Invoke<::tauri::Wry>) -> bool,
            {
                handler(invoke)
            }
            call_handler(::tauri::generate_handler![#fn_ident], invoke)
        }

        #[::linkme::distributed_slice(crate::specta_builder::KLYNT_COMMANDS)]
        #[allow(non_upper_case_globals)]
        static #registration_ident: crate::specta_builder::CommandRegistration =
            crate::specta_builder::CommandRegistration {
                name: #fn_name_str,
                invoke: #dispatcher_ident,
                source: crate::specta_builder::SourceKind::Raw,
            };
    }
}
