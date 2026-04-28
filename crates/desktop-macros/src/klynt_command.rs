use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{ItemFn, Visibility, parse2};

use crate::{errors::*, parse::ParsedCommand};

pub fn expand(input: TokenStream) -> TokenStream {
    let fn_item: ItemFn = match parse2(input.clone()) {
        Ok(f) => f,
        Err(_) => return err(input, ERR_NOT_FUNCTION),
    };

    let parsed = ParsedCommand { fn_item };

    if !matches!(parsed.fn_item.vis, Visibility::Public(_)) {
        return err(parsed.fn_item.sig.fn_token, ERR_MISSING_PUB);
    }
    if parsed.fn_item.sig.asyncness.is_none() {
        return err(parsed.fn_item.sig.fn_token, ERR_MISSING_ASYNC);
    }
    if let Some(state_param) = parsed.declared_state_param() {
        return err(state_param, ERR_DECLARED_STATE);
    }
    if parsed.return_type().is_none() {
        return err(&parsed.fn_item.sig, ERR_MISSING_RETURN);
    }
    if parsed.return_type_is_result() {
        let ty = parsed.return_type().unwrap();
        return err(ty, ERR_RESULT_RETURN);
    }

    let return_ty = parsed.return_type().unwrap();
    let fn_ident = &parsed.fn_item.sig.ident;
    let fn_vis = &parsed.fn_item.vis;
    let fn_async = &parsed.fn_item.sig.asyncness;
    let fn_inputs = &parsed.fn_item.sig.inputs;
    let fn_block = &parsed.fn_item.block;
    let fn_attrs = &parsed.fn_item.attrs;
    let dispatcher_ident = format_ident!("__klynt_dispatch_{}", fn_ident);
    let registration_ident = format_ident!("__klynt_command_{}", fn_ident);
    let fn_name_str = fn_ident.to_string();

    quote! {
        #[::tauri::command]
        #[::specta::specta]
        #(#fn_attrs)*
        #fn_vis #fn_async fn #fn_ident(
            state: ::tauri::State<'_, ::std::sync::Arc<crate::app_core::AppCore>>,
            #fn_inputs
        ) -> ::desktop_shared::CommandResult<#return_ty> {
            #fn_block
        }

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
                source: crate::specta_builder::SourceKind::Klynt,
            };
    }
}
