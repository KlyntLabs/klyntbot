use crate::attrs::{
    parse_ai_enum_attr, parse_ai_event_attr, AiEventAttr, EntityBridge, SalienceSpec,
};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Variant};

pub fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let enum_ident = &input.ident;
    let data_enum = match &input.data {
        Data::Enum(e) => e,
        _ => {
            return Err(syn::Error::new_spanned(
                &input,
                "AiEvent can only be derived on enums",
            ))
        }
    };

    let domain_ident = parse_ai_enum_attr(&input.attrs)?;
    let domain_tokens = match &domain_ident {
        Some(ident) => quote! { ::ai_core::RecallDomain::#ident },
        None => quote! { ::ai_core::RecallDomain::General },
    };

    let mut arms = Vec::new();
    let mut kind_arms = Vec::new();
    let mut metric_specs_ts: Vec<proc_macro2::TokenStream> = Vec::new();

    for variant in &data_enum.variants {
        arms.push(render_variant(enum_ident, variant, &domain_tokens)?);

        let id = &variant.ident;
        let kind = id.to_string();
        let pattern = match &variant.fields {
            Fields::Named(_) => quote! { #enum_ident::#id { .. } },
            Fields::Unit => quote! { #enum_ident::#id },
            Fields::Unnamed(_) => quote! { #enum_ident::#id(..) },
        };
        kind_arms.push(quote! { #pattern => #kind });

        // Collect metric specs for FEATURE_METRICS constant
        let attr = parse_ai_event_attr(&variant.attrs)?;
        if let Some(metric) = &attr.metric {
            let m_name = &metric.name;
            let m_window = metric.window_secs;
            let m_min = metric.min_samples;
            let m_agg_ts = metric.aggregation.emit_tokens();
            let const_ident = syn::Ident::new(
                &format!("METRIC_SPEC_{}", id.to_string().to_uppercase()),
                id.span(),
            );
            metric_specs_ts.push(quote! {
                {
                    static #const_ident: ::ai_core::MetricSpec = ::ai_core::MetricSpec {
                        name: #m_name,
                        window_secs: #m_window,
                        min_samples: #m_min,
                        aggregation: #m_agg_ts,
                    };
                    &#const_ident
                }
            });
        }
    }

    // Handle empty enums by adding a wildcard arm
    let to_signal_match = if arms.is_empty() {
        quote! {
            match self {
                _ => unreachable!("empty enum"),
            }
        }
    } else {
        quote! {
            match self {
                #(#arms)*
            }
        }
    };

    let event_kind_match = if kind_arms.is_empty() {
        quote! {
            match self {
                _ => unreachable!("empty enum"),
            }
        }
    } else {
        quote! {
            match self {
                #(#kind_arms,)*
            }
        }
    };

    let feature_metrics_impl = quote! {
        impl #enum_ident {
            /// All `MetricSpec`s declared by variants of this enum via `#[ai(metric(...))]`.
            /// Registered by app-core at startup via `MetricRegistry::register_all`.
            pub const FEATURE_METRICS: &'static [&'static ::ai_core::MetricSpec] = &[
                #(#metric_specs_ts),*
            ];
        }
    };

    Ok(quote! {
        #[allow(unused_variables, unused_assignments)]
        impl ::ai_core::AiEventMeta for #enum_ident {
            fn to_signal(&self) -> ::ai_core::AiSignal {
                #to_signal_match
            }

            fn event_kind(&self) -> &'static str {
                #event_kind_match
            }
        }

        #feature_metrics_impl
    })
}

fn render_variant(
    enum_ident: &syn::Ident,
    variant: &Variant,
    domain_tokens: &TokenStream,
) -> syn::Result<TokenStream> {
    let var_ident = &variant.ident;
    let attr = parse_ai_event_attr(&variant.attrs)?;

    // Collect field names for destructuring.
    let field_idents: Vec<_> = match &variant.fields {
        Fields::Named(named) => named
            .named
            .iter()
            .map(|f| f.ident.clone().unwrap())
            .collect(),
        Fields::Unit => Vec::new(),
        Fields::Unnamed(_) => {
            return Err(syn::Error::new_spanned(
                variant,
                "AiEvent requires named fields or unit variants",
            ))
        }
    };

    let pattern = if field_idents.is_empty() {
        quote! { #enum_ident::#var_ident }
    } else {
        quote! { #enum_ident::#var_ident { #(#field_idents),* } }
    };

    let importance_expr = match (&attr.importance, &attr.importance_fn) {
        (Some(lit), None) => quote! { #lit },
        (None, Some(path)) => quote! { #path(self) },
        (Some(_), Some(_)) => {
            return Err(syn::Error::new_spanned(
                variant,
                "specify either importance or importance_fn, not both",
            ))
        }
        (None, None) => {
            return Err(syn::Error::new_spanned(
                variant,
                "each variant needs #[ai(importance = ...)] or importance_fn",
            ))
        }
    };

    let salience_expr = render_salience(&attr.salience, &field_idents);
    let content_expr = render_content(&attr, &field_idents);
    let entity_expr = render_entity(&attr.entity_bridge);
    let kind_lit = var_ident.to_string();

    let (coaching_flag, rule_expr, app_expr, amount_expr, category_expr) = match &attr.coaching {
        None => (
            quote! { false },
            quote! { None },
            quote! { None },
            quote! { None },
            quote! { None },
        ),
        Some(c) => {
            let rule = match &c.rule {
                Some(s) => quote! { Some(#s.to_string()) },
                None => quote! { None },
            };
            let app = match &c.app_from {
                Some(id) => quote! { Some(#id.to_string()) },
                None => quote! { None },
            };
            let amount = match &c.amount_from {
                Some(id) => quote! { Some(*#id as f64) },
                None => quote! { None },
            };
            let category = match &c.category_from {
                Some(id) => quote! { Some(#id.to_string()) },
                None => quote! { None },
            };
            (quote! { true }, rule, app, amount, category)
        }
    };

    let metric_samples_ts = if let Some(metric) = &attr.metric {
        let m_name = &metric.name;
        let m_value = &metric.value_from;
        quote! {
            vec![::ai_core::MetricSample {
                name: #m_name,
                value: (#m_value),
            }]
        }
    } else {
        quote! { Vec::new() }
    };

    Ok(quote! {
        #pattern => ::ai_core::AiSignal {
            domain: #domain_tokens,
            event_kind: #kind_lit,
            importance: #importance_expr,
            salience: #salience_expr,
            content: #content_expr,
            entity: #entity_expr,
            timestamp: ::jiff::Timestamp::now(),
            raw_event: None,
            metrics: ::ai_core::AiMetrics {
                app: #app_expr,
                amount: #amount_expr,
                category: #category_expr,
            },
            coaching_signal: #coaching_flag,
            coaching_rule: #rule_expr,
            metric_samples: #metric_samples_ts,
        },
    })
}

fn render_salience(spec: &SalienceSpec, fields: &[syn::Ident]) -> TokenStream {
    match spec {
        SalienceSpec::Accumulate => quote! { ::ai_core::SalienceVerdict::Accumulate },
        SalienceSpec::Extract => quote! { ::ai_core::SalienceVerdict::Extract },
        SalienceSpec::Discard => quote! { ::ai_core::SalienceVerdict::Discard },
        SalienceSpec::ExtractIf(expr) => {
            // Build a mapping of field names to their idents for substitution
            let field_map: std::collections::HashMap<String, &syn::Ident> =
                fields.iter().map(|f| (f.to_string(), f)).collect();

            // Replace bare identifiers in the expression with field references
            let modified_expr = replace_identifiers_in_expr(expr, &field_map);

            quote! {
                if #modified_expr {
                    ::ai_core::SalienceVerdict::Extract
                } else {
                    ::ai_core::SalienceVerdict::Accumulate
                }
            }
        }
    }
}

fn replace_identifiers_in_expr(
    expr: &syn::Expr,
    field_map: &std::collections::HashMap<String, &syn::Ident>,
) -> syn::Expr {
    use syn::{Expr, ExprPath, Path, PathSegment};

    match expr {
        Expr::Path(ExprPath { path, .. }) => {
            if let Some(ident) = path.get_ident() {
                let name = ident.to_string();
                if let Some(field_ident) = field_map.get(&name) {
                    // Replace with a reference to the field
                    return Expr::Path(ExprPath {
                        attrs: vec![],
                        qself: None,
                        path: Path {
                            leading_colon: None,
                            segments: vec![PathSegment {
                                ident: (*field_ident).clone(),
                                arguments: syn::PathArguments::None,
                            }]
                            .into_iter()
                            .collect(),
                        },
                    });
                }
            }
            expr.clone()
        }
        Expr::Binary(binary) => {
            let left = Box::new(replace_identifiers_in_expr(&binary.left, field_map));
            let right = Box::new(replace_identifiers_in_expr(&binary.right, field_map));
            Expr::Binary(syn::ExprBinary {
                attrs: binary.attrs.clone(),
                left,
                op: binary.op,
                right,
            })
        }
        Expr::Unary(unary) => {
            let expr = Box::new(replace_identifiers_in_expr(&unary.expr, field_map));
            Expr::Unary(syn::ExprUnary {
                attrs: unary.attrs.clone(),
                op: unary.op,
                expr,
            })
        }
        Expr::Paren(paren) => {
            let expr = Box::new(replace_identifiers_in_expr(&paren.expr, field_map));
            Expr::Paren(syn::ExprParen {
                attrs: paren.attrs.clone(),
                expr,
                paren_token: paren.paren_token,
            })
        }
        Expr::Call(call) => {
            let func = Box::new(replace_identifiers_in_expr(&call.func, field_map));
            let args = call
                .args
                .iter()
                .map(|arg| replace_identifiers_in_expr(arg, field_map))
                .collect();
            Expr::Call(syn::ExprCall {
                attrs: call.attrs.clone(),
                func,
                paren_token: call.paren_token,
                args,
            })
        }
        Expr::MethodCall(method) => {
            let receiver = Box::new(replace_identifiers_in_expr(&method.receiver, field_map));
            let args = method
                .args
                .iter()
                .map(|arg| replace_identifiers_in_expr(arg, field_map))
                .collect();
            Expr::MethodCall(syn::ExprMethodCall {
                attrs: method.attrs.clone(),
                receiver,
                dot_token: method.dot_token,
                method: method.method.clone(),
                turbofish: method.turbofish.clone(),
                paren_token: method.paren_token,
                args,
            })
        }
        Expr::Field(field) => {
            let base = Box::new(replace_identifiers_in_expr(&field.base, field_map));
            Expr::Field(syn::ExprField {
                attrs: field.attrs.clone(),
                base,
                dot_token: field.dot_token,
                member: field.member.clone(),
            })
        }
        Expr::Index(index) => {
            let expr = Box::new(replace_identifiers_in_expr(&index.expr, field_map));
            Expr::Index(syn::ExprIndex {
                attrs: index.attrs.clone(),
                expr,
                bracket_token: index.bracket_token,
                index: index.index.clone(),
            })
        }
        Expr::Tuple(tuple) => {
            let elems = tuple
                .elems
                .iter()
                .map(|elem| replace_identifiers_in_expr(elem, field_map))
                .collect();
            Expr::Tuple(syn::ExprTuple {
                attrs: tuple.attrs.clone(),
                paren_token: tuple.paren_token,
                elems,
            })
        }
        Expr::Array(array) => {
            let elems = array
                .elems
                .iter()
                .map(|elem| replace_identifiers_in_expr(elem, field_map))
                .collect();
            Expr::Array(syn::ExprArray {
                attrs: array.attrs.clone(),
                bracket_token: array.bracket_token,
                elems,
            })
        }
        Expr::Struct(struct_expr) => {
            let fields = struct_expr
                .fields
                .iter()
                .map(|field| syn::FieldValue {
                    attrs: field.attrs.clone(),
                    member: field.member.clone(),
                    colon_token: field.colon_token,
                    expr: replace_identifiers_in_expr(&field.expr, field_map),
                })
                .collect();
            Expr::Struct(syn::ExprStruct {
                attrs: struct_expr.attrs.clone(),
                qself: struct_expr.qself.clone(),
                path: struct_expr.path.clone(),
                brace_token: struct_expr.brace_token,
                fields,
                dot2_token: struct_expr.dot2_token,
                rest: struct_expr
                    .rest
                    .as_ref()
                    .map(|rest| Box::new(replace_identifiers_in_expr(rest, field_map))),
            })
        }
        Expr::If(if_expr) => {
            let cond = Box::new(replace_identifiers_in_expr(&if_expr.cond, field_map));
            let then_stmts = if_expr
                .then_branch
                .stmts
                .iter()
                .map(|stmt| replace_identifiers_in_stmt(stmt, field_map))
                .collect();
            let else_branch = if_expr.else_branch.as_ref().map(|(token, expr)| {
                (
                    *token,
                    Box::new(replace_identifiers_in_expr(expr, field_map)),
                )
            });
            Expr::If(syn::ExprIf {
                attrs: if_expr.attrs.clone(),
                if_token: if_expr.if_token,
                cond,
                then_branch: syn::Block {
                    brace_token: if_expr.then_branch.brace_token,
                    stmts: then_stmts,
                },
                else_branch,
            })
        }
        Expr::While(while_expr) => {
            let cond = Box::new(replace_identifiers_in_expr(&while_expr.cond, field_map));
            let stmts = while_expr
                .body
                .stmts
                .iter()
                .map(|stmt| replace_identifiers_in_stmt(stmt, field_map))
                .collect();
            Expr::While(syn::ExprWhile {
                attrs: while_expr.attrs.clone(),
                label: while_expr.label.clone(),
                while_token: while_expr.while_token,
                cond,
                body: syn::Block {
                    brace_token: while_expr.body.brace_token,
                    stmts,
                },
            })
        }
        Expr::ForLoop(for_loop) => {
            let expr = Box::new(replace_identifiers_in_expr(&for_loop.expr, field_map));
            let stmts = for_loop
                .body
                .stmts
                .iter()
                .map(|stmt| replace_identifiers_in_stmt(stmt, field_map))
                .collect();
            Expr::ForLoop(syn::ExprForLoop {
                attrs: for_loop.attrs.clone(),
                label: for_loop.label.clone(),
                for_token: for_loop.for_token,
                pat: for_loop.pat.clone(),
                in_token: for_loop.in_token,
                expr,
                body: syn::Block {
                    brace_token: for_loop.body.brace_token,
                    stmts,
                },
            })
        }
        Expr::Loop(loop_expr) => {
            let stmts = loop_expr
                .body
                .stmts
                .iter()
                .map(|stmt| replace_identifiers_in_stmt(stmt, field_map))
                .collect();
            Expr::Loop(syn::ExprLoop {
                attrs: loop_expr.attrs.clone(),
                label: loop_expr.label.clone(),
                loop_token: loop_expr.loop_token,
                body: syn::Block {
                    brace_token: loop_expr.body.brace_token,
                    stmts,
                },
            })
        }
        Expr::Match(match_expr) => {
            let expr = Box::new(replace_identifiers_in_expr(&match_expr.expr, field_map));
            let arms = match_expr
                .arms
                .iter()
                .map(|arm| syn::Arm {
                    attrs: arm.attrs.clone(),
                    pat: arm.pat.clone(),
                    guard: arm.guard.as_ref().map(|(token, expr)| {
                        (
                            *token,
                            Box::new(replace_identifiers_in_expr(expr, field_map)),
                        )
                    }),
                    fat_arrow_token: arm.fat_arrow_token,
                    body: Box::new(replace_identifiers_in_expr(&arm.body, field_map)),
                    comma: arm.comma,
                })
                .collect();
            Expr::Match(syn::ExprMatch {
                attrs: match_expr.attrs.clone(),
                match_token: match_expr.match_token,
                expr,
                brace_token: match_expr.brace_token,
                arms,
            })
        }
        Expr::Block(block) => {
            let stmts = block
                .block
                .stmts
                .iter()
                .map(|stmt| replace_identifiers_in_stmt(stmt, field_map))
                .collect();
            Expr::Block(syn::ExprBlock {
                attrs: block.attrs.clone(),
                label: block.label.clone(),
                block: syn::Block {
                    brace_token: block.block.brace_token,
                    stmts,
                },
            })
        }
        Expr::Let(let_expr) => {
            let expr = Box::new(replace_identifiers_in_expr(&let_expr.expr, field_map));
            Expr::Let(syn::ExprLet {
                attrs: let_expr.attrs.clone(),
                let_token: let_expr.let_token,
                pat: let_expr.pat.clone(),
                eq_token: let_expr.eq_token,
                expr,
            })
        }
        Expr::Assign(assign) => {
            let left = Box::new(replace_identifiers_in_expr(&assign.left, field_map));
            let right = Box::new(replace_identifiers_in_expr(&assign.right, field_map));
            Expr::Assign(syn::ExprAssign {
                attrs: assign.attrs.clone(),
                left,
                eq_token: assign.eq_token,
                right,
            })
        }
        Expr::Return(return_expr) => {
            let expr = return_expr
                .expr
                .as_ref()
                .map(|expr| Box::new(replace_identifiers_in_expr(expr, field_map)));
            Expr::Return(syn::ExprReturn {
                attrs: return_expr.attrs.clone(),
                return_token: return_expr.return_token,
                expr,
            })
        }
        Expr::Break(break_expr) => {
            let expr = break_expr
                .expr
                .as_ref()
                .map(|expr| Box::new(replace_identifiers_in_expr(expr, field_map)));
            Expr::Break(syn::ExprBreak {
                attrs: break_expr.attrs.clone(),
                label: break_expr.label.clone(),
                break_token: break_expr.break_token,
                expr,
            })
        }
        Expr::Continue(continue_expr) => Expr::Continue(continue_expr.clone()),
        Expr::Range(range) => {
            let start = range
                .start
                .as_ref()
                .map(|expr| Box::new(replace_identifiers_in_expr(expr, field_map)));
            let end = range
                .end
                .as_ref()
                .map(|expr| Box::new(replace_identifiers_in_expr(expr, field_map)));
            Expr::Range(syn::ExprRange {
                attrs: range.attrs.clone(),
                start,
                limits: range.limits,
                end,
            })
        }
        Expr::Cast(cast) => {
            let expr = Box::new(replace_identifiers_in_expr(&cast.expr, field_map));
            Expr::Cast(syn::ExprCast {
                attrs: cast.attrs.clone(),
                expr,
                as_token: cast.as_token,
                ty: cast.ty.clone(),
            })
        }
        Expr::Reference(reference) => {
            let expr = Box::new(replace_identifiers_in_expr(&reference.expr, field_map));
            Expr::Reference(syn::ExprReference {
                attrs: reference.attrs.clone(),
                and_token: reference.and_token,
                mutability: reference.mutability,
                expr,
            })
        }
        Expr::Group(group) => {
            let expr = Box::new(replace_identifiers_in_expr(&group.expr, field_map));
            Expr::Group(syn::ExprGroup {
                attrs: group.attrs.clone(),
                group_token: group.group_token,
                expr,
            })
        }
        Expr::Lit(lit) => Expr::Lit(lit.clone()),
        Expr::Macro(mac) => Expr::Macro(mac.clone()),
        Expr::Async(async_expr) => {
            let stmts = async_expr
                .block
                .stmts
                .iter()
                .map(|stmt| replace_identifiers_in_stmt(stmt, field_map))
                .collect();
            Expr::Async(syn::ExprAsync {
                attrs: async_expr.attrs.clone(),
                async_token: async_expr.async_token,
                capture: async_expr.capture,
                block: syn::Block {
                    brace_token: async_expr.block.brace_token,
                    stmts,
                },
            })
        }
        Expr::Await(await_expr) => {
            let base = Box::new(replace_identifiers_in_expr(&await_expr.base, field_map));
            Expr::Await(syn::ExprAwait {
                attrs: await_expr.attrs.clone(),
                base,
                dot_token: await_expr.dot_token,
                await_token: await_expr.await_token,
            })
        }
        Expr::Closure(closure) => {
            let body = Box::new(replace_identifiers_in_expr(&closure.body, field_map));
            Expr::Closure(syn::ExprClosure {
                attrs: closure.attrs.clone(),
                lifetimes: closure.lifetimes.clone(),
                constness: closure.constness,
                movability: closure.movability,
                asyncness: closure.asyncness,
                capture: closure.capture,
                or1_token: closure.or1_token,
                inputs: closure.inputs.clone(),
                or2_token: closure.or2_token,
                output: closure.output.clone(),
                body,
            })
        }
        Expr::Const(const_expr) => {
            let stmts = const_expr
                .block
                .stmts
                .iter()
                .map(|stmt| replace_identifiers_in_stmt(stmt, field_map))
                .collect();
            Expr::Const(syn::ExprConst {
                attrs: const_expr.attrs.clone(),
                const_token: const_expr.const_token,
                block: syn::Block {
                    brace_token: const_expr.block.brace_token,
                    stmts,
                },
            })
        }
        Expr::Repeat(repeat) => {
            let expr = Box::new(replace_identifiers_in_expr(&repeat.expr, field_map));
            let len = Box::new(replace_identifiers_in_expr(&repeat.len, field_map));
            Expr::Repeat(syn::ExprRepeat {
                attrs: repeat.attrs.clone(),
                bracket_token: repeat.bracket_token,
                expr,
                semi_token: repeat.semi_token,
                len,
            })
        }
        Expr::Try(try_expr) => {
            let expr = Box::new(replace_identifiers_in_expr(&try_expr.expr, field_map));
            Expr::Try(syn::ExprTry {
                attrs: try_expr.attrs.clone(),
                expr,
                question_token: try_expr.question_token,
            })
        }
        Expr::TryBlock(try_block) => {
            let stmts = try_block
                .block
                .stmts
                .iter()
                .map(|stmt| replace_identifiers_in_stmt(stmt, field_map))
                .collect();
            Expr::TryBlock(syn::ExprTryBlock {
                attrs: try_block.attrs.clone(),
                try_token: try_block.try_token,
                block: syn::Block {
                    brace_token: try_block.block.brace_token,
                    stmts,
                },
            })
        }
        Expr::Unsafe(unsafe_expr) => {
            let stmts = unsafe_expr
                .block
                .stmts
                .iter()
                .map(|stmt| replace_identifiers_in_stmt(stmt, field_map))
                .collect();
            Expr::Unsafe(syn::ExprUnsafe {
                attrs: unsafe_expr.attrs.clone(),
                unsafe_token: unsafe_expr.unsafe_token,
                block: syn::Block {
                    brace_token: unsafe_expr.block.brace_token,
                    stmts,
                },
            })
        }
        Expr::Yield(yield_expr) => {
            let expr = yield_expr
                .expr
                .as_ref()
                .map(|expr| Box::new(replace_identifiers_in_expr(expr, field_map)));
            Expr::Yield(syn::ExprYield {
                attrs: yield_expr.attrs.clone(),
                yield_token: yield_expr.yield_token,
                expr,
            })
        }
        Expr::Verbatim(tokens) => Expr::Verbatim(tokens.clone()),
        _ => expr.clone(),
    }
}

fn replace_identifiers_in_stmt(
    stmt: &syn::Stmt,
    field_map: &std::collections::HashMap<String, &syn::Ident>,
) -> syn::Stmt {
    match stmt {
        syn::Stmt::Local(local) => {
            let init = local.init.as_ref().map(|init| syn::LocalInit {
                eq_token: init.eq_token,
                expr: Box::new(replace_identifiers_in_expr(&init.expr, field_map)),
                diverge: init.diverge.as_ref().map(|(token, expr)| {
                    (
                        *token,
                        Box::new(replace_identifiers_in_expr(expr, field_map)),
                    )
                }),
            });
            syn::Stmt::Local(syn::Local {
                attrs: local.attrs.clone(),
                let_token: local.let_token,
                pat: local.pat.clone(),
                init,
                semi_token: local.semi_token,
            })
        }
        syn::Stmt::Item(item) => syn::Stmt::Item(item.clone()),
        syn::Stmt::Expr(expr, semi) => {
            syn::Stmt::Expr(replace_identifiers_in_expr(expr, field_map), *semi)
        }
        syn::Stmt::Macro(mac) => syn::Stmt::Macro(mac.clone()),
    }
}

fn render_content(attr: &AiEventAttr, fields: &[syn::Ident]) -> TokenStream {
    match &attr.observation_template {
        Some(template) => {
            let fmt_lit = syn::LitStr::new(template, proc_macro2::Span::call_site());
            // Only include fields that are actually referenced in the template
            let field_refs: Vec<_> = fields
                .iter()
                .filter_map(|f| {
                    let name = f.to_string();
                    if template.contains(&format!("{{{}}}", name))
                        || template.contains(&format!("{{{}:", name))
                    {
                        let name_ident = syn::Ident::new(&name, f.span());
                        Some(quote! { #name_ident = #f })
                    } else {
                        None
                    }
                })
                .collect();

            if field_refs.is_empty() {
                quote! { format!(#fmt_lit) }
            } else {
                quote! { format!(#fmt_lit, #(#field_refs),*) }
            }
        }
        None => quote! { String::new() },
    }
}

fn render_entity(bridge: &Option<EntityBridge>) -> TokenStream {
    match bridge {
        None => quote! { None },
        Some(b) => {
            let ty = &b.entity_type;
            let name_from = &b.name_from;
            let id_from = &b.id_from;
            quote! {
                Some(::ai_core::EntityRef {
                    entity_type: #ty,
                    id: #id_from.to_string(),
                    name: #name_from.to_string(),
                })
            }
        }
    }
}
