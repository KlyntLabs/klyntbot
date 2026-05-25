use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse2, FnArg, ItemFn, Pat, Type, Visibility};

use crate::{errors::*, parse::ParsedCommand};

/// snake_case → camelCase, matching Tauri's automatic arg-name conversion.
/// This is the whole point: the neutral handler reads `body["areaId"]` using the
/// exact same rule Tauri uses, so the two transports cannot disagree.
fn to_camel(s: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for ch in s.chars() {
        if ch == '_' {
            upper = true;
        } else if upper {
            out.extend(ch.to_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn type_is_option(ty: &Type) -> bool {
    quote!(#ty)
        .to_string()
        .replace(' ', "")
        .starts_with("Option<")
}

/// A param type Tauri injects rather than deserializes (so it can't cross the
/// dev HTTP seam). Commands taking one of these stay Tauri-only (`json: None`).
fn type_is_special(ty: &Type) -> bool {
    let s = quote!(#ty).to_string();
    s.contains("AppHandle") || s.contains("Window") || s.contains("Webview")
}

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

    // Collect the user's declared data params, and decide which branch we're in.
    let mut has_special = false;
    let mut data: Vec<(syn::Ident, Type, String, bool)> = Vec::new();
    for arg in fn_inputs.iter() {
        if let FnArg::Typed(pt) = arg {
            if type_is_special(&pt.ty) {
                has_special = true;
                continue;
            }
            if let Pat::Ident(pi) = &*pt.pat {
                let camel = to_camel(&pi.ident.to_string());
                let opt = type_is_option(&pt.ty);
                data.push((pi.ident.clone(), (*pt.ty).clone(), camel, opt));
            }
        }
    }

    // The dispatcher is identical in both branches (Tauri IPC adapter).
    let dispatcher = quote! {
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
    };

    // ── Branch A: command takes a Tauri-injected param → stays Tauri-only ──────
    if has_special {
        return quote! {
            #[::tauri::command]
            #[::specta::specta]
            #(#fn_attrs)*
            #fn_vis #fn_async fn #fn_ident(
                state: ::tauri::State<'_, ::std::sync::Arc<crate::app_core::AppCore>>,
                #fn_inputs
            ) -> ::desktop_shared::CommandResult<#return_ty> {
                #fn_block
            }

            #dispatcher

            #[::linkme::distributed_slice(crate::specta_builder::KLYNT_COMMANDS)]
            #[allow(non_upper_case_globals)]
            static #registration_ident: crate::specta_builder::CommandRegistration =
                crate::specta_builder::CommandRegistration {
                    name: #fn_name_str,
                    invoke: #dispatcher_ident,
                    json: ::std::option::Option::None,
                    source: crate::specta_builder::SourceKind::Klynt,
                };
        };
    }

    // ── Branch B: all params deserializable → transport-neutral handler ────────
    let inner_ident = format_ident!("__klynt_inner_{}", fn_ident);
    let json_ident = format_ident!("__klynt_json_{}", fn_ident);
    let arg_idents: Vec<_> = data.iter().map(|(id, ..)| id.clone()).collect();

    // Per-param JSON extraction, mirroring Tauri's camelCase arg mapping.
    let extractions = data.iter().map(|(id, ty, camel, is_opt)| {
        if *is_opt {
            quote! {
                let #id: #ty = match __body.get(#camel) {
                    ::std::option::Option::Some(v) => ::serde_json::from_value(v.clone())
                        .map_err(|e| ::desktop_shared::errors::ApiError::new("VALIDATION", e.to_string()))?,
                    ::std::option::Option::None => ::std::option::Option::None,
                };
            }
        } else {
            quote! {
                let #id: #ty = match __body.get(#camel) {
                    ::std::option::Option::Some(v) => ::serde_json::from_value(v.clone())
                        .map_err(|e| ::desktop_shared::errors::ApiError::new("VALIDATION", e.to_string()))?,
                    ::std::option::Option::None => return ::std::result::Result::Err(
                        ::desktop_shared::errors::ApiError::new(
                            "VALIDATION",
                            format!("missing required field: {}", #camel),
                        ),
                    ),
                };
            }
        }
    });

    quote! {
        // Tauri IPC adapter: build a Tauri-backed emitter, delegate to the shared inner.
        #[::tauri::command]
        #[::specta::specta]
        #(#fn_attrs)*
        #fn_vis #fn_async fn #fn_ident(
            state: ::tauri::State<'_, ::std::sync::Arc<crate::app_core::AppCore>>,
            app: ::tauri::AppHandle,
            #fn_inputs
        ) -> ::desktop_shared::CommandResult<#return_ty> {
            let __emitter = crate::commands::TauriEmitter::new(app);
            #inner_ident(state.inner(), &__emitter, #(#arg_idents),*).await
        }

        // Shared implementation. `state: &Arc<AppCore>` preserves the deref
        // semantics of `tauri::State`, and `emitter` is available for mutations.
        #[doc(hidden)]
        #[allow(unused_variables, clippy::too_many_arguments)]
        #fn_async fn #inner_ident(
            state: &::std::sync::Arc<crate::app_core::AppCore>,
            emitter: &dyn ::app_core::events::AppEventEmitter,
            #fn_inputs
        ) -> ::desktop_shared::CommandResult<#return_ty> {
            #fn_block
        }

        // Transport-neutral adapter: decode JSON args, run, encode result.
        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #json_ident(
            __body: ::serde_json::Value,
            __core: ::std::sync::Arc<crate::app_core::AppCore>,
            __emitter: ::std::sync::Arc<dyn ::app_core::events::AppEventEmitter>,
        ) -> ::std::pin::Pin<Box<dyn ::std::future::Future<
            Output = ::desktop_shared::CommandResult<::serde_json::Value>,
        > + Send>> {
            Box::pin(async move {
                #(#extractions)*
                let __ret = #inner_ident(&__core, __emitter.as_ref(), #(#arg_idents),*).await?;
                ::std::result::Result::Ok(
                    ::serde_json::to_value(__ret).unwrap_or(::serde_json::Value::Null),
                )
            })
        }

        #dispatcher

        #[::linkme::distributed_slice(crate::specta_builder::KLYNT_COMMANDS)]
        #[allow(non_upper_case_globals)]
        static #registration_ident: crate::specta_builder::CommandRegistration =
            crate::specta_builder::CommandRegistration {
                name: #fn_name_str,
                invoke: #dispatcher_ident,
                json: ::std::option::Option::Some(#json_ident),
                source: crate::specta_builder::SourceKind::Klynt,
            };
    }
}
