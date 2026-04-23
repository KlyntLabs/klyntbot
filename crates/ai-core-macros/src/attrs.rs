use proc_macro2::Span;
use syn::{Attribute, Expr, ExprLit, Lit, LitInt, LitStr};

pub struct AiEventAttr {
    pub importance: Option<f64>,
    pub importance_fn: Option<syn::Path>,
    pub salience: SalienceSpec,
    pub observation_template: Option<String>,
    pub entity_bridge: Option<EntityBridge>,
    pub coaching: Option<CoachingSignalSpec>,
    pub metric: Option<MetricAttr>,
}

pub struct CoachingSignalSpec {
    pub app_from: Option<syn::Ident>,
    pub amount_from: Option<syn::Ident>,
    pub category_from: Option<syn::Ident>,
    pub rule: Option<String>,
}

#[allow(clippy::large_enum_variant)]
pub enum SalienceSpec {
    Accumulate,
    Extract,
    Discard,
    ExtractIf(syn::Expr),
}

pub struct EntityBridge {
    pub entity_type: String,
    pub name_from: syn::Ident,
    pub id_from: syn::Ident,
}

pub struct AiFeatureAttr {
    pub recall_domain: syn::Ident,
    pub skill: String,
    pub tool_name: Option<String>,
    pub entity_kind: Option<String>,
    pub event: syn::Path,
    pub recall_boost_when: Option<syn::Expr>,
    pub recall_priority_field: Option<syn::Ident>,
    pub recall_recency_field: Option<syn::Ident>,
    pub recall_status_filter: Option<syn::Expr>,
    pub mirror_snapshots: Vec<MirrorSnapshotAttr>,
    pub promotion_threshold: Option<u32>,
}

pub struct MirrorSnapshotAttr {
    pub name: String,
    pub flush_interval_secs: Option<u64>,
    pub subscribed_kinds: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MetricAttr {
    pub name: String,
    pub value_from: Expr,
    pub window_secs: u64,
    pub min_samples: u32,
    pub aggregation: Aggregation,
}

/// Mirror of `ai_core::metric::Aggregation` used only within the proc-macro crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aggregation {
    Avg,
    Sum,
    Count,
}

impl Aggregation {
    pub fn emit_tokens(&self) -> proc_macro2::TokenStream {
        match self {
            Aggregation::Avg => quote::quote!(::ai_core::Aggregation::Avg),
            Aggregation::Sum => quote::quote!(::ai_core::Aggregation::Sum),
            Aggregation::Count => quote::quote!(::ai_core::Aggregation::Count),
        }
    }

    pub fn parse_str(s: &str) -> Result<Self, String> {
        match s {
            "avg" => Ok(Aggregation::Avg),
            "sum" => Ok(Aggregation::Sum),
            "count" => Ok(Aggregation::Count),
            _ => Err(format!("aggregation must be avg|sum|count: {}", s)),
        }
    }
}

/// Compile-time parse of `"7d"` / `"1h"` / `"30m"` / `"15s"` into u64 seconds.
pub fn parse_window_secs(s: &str) -> Result<u64, String> {
    if s.is_empty() {
        return Err("window must be non-empty (e.g. \"7d\")".into());
    }
    let (n, unit) = s.split_at(s.len() - 1);
    let n: u64 = n
        .parse()
        .map_err(|_| format!("window prefix must be numeric: {}", s))?;
    let mult = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3_600,
        "d" => 86_400,
        _ => return Err(format!("window unit must be s|m|h|d: {}", s)),
    };
    Ok(n.saturating_mul(mult))
}

/// Parses an enum-level `#[ai(domain = "Tasks")]` attribute. The value must be
/// a `RecallDomain` variant ident. Returns `None` if the attribute is absent.
pub fn parse_ai_enum_attr(attrs: &[Attribute]) -> syn::Result<Option<syn::Ident>> {
    let Some(ai_attr) = attrs.iter().find(|a| a.path().is_ident("ai")) else {
        return Ok(None);
    };
    let mut domain: Option<syn::Ident> = None;
    ai_attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("domain") {
            let value: Expr = meta.value()?.parse()?;
            if let Expr::Lit(ExprLit {
                lit: Lit::Str(s), ..
            }) = value
            {
                domain = Some(syn::Ident::new(&s.value(), s.span()));
                return Ok(());
            }
            return Err(meta.error("expected string literal naming a RecallDomain variant"));
        }
        Err(meta.error("unknown enum-level ai attribute"))
    })?;
    Ok(domain)
}

pub fn parse_ai_event_attr(attrs: &[Attribute]) -> syn::Result<AiEventAttr> {
    let ai_attr = attrs
        .iter()
        .find(|a| a.path().is_ident("ai"))
        .ok_or_else(|| {
            syn::Error::new(
                Span::call_site(),
                "every variant must have #[ai(...)] attribute",
            )
        })?;

    let mut importance = None;
    let mut importance_fn = None;
    let mut salience = None;
    let mut observation_template = None;
    let mut entity_bridge = None;
    let mut coaching = None;
    let mut metric = None;

    ai_attr.parse_nested_meta(|meta| {
        let name = meta
            .path
            .get_ident()
            .ok_or_else(|| meta.error("expected identifier"))?
            .to_string();
        match name.as_str() {
            "importance" => {
                let value: Expr = meta.value()?.parse()?;
                if let Expr::Lit(ExprLit {
                    lit: Lit::Float(f), ..
                }) = value
                {
                    importance = Some(f.base10_parse::<f64>()?);
                } else if let Expr::Lit(ExprLit {
                    lit: Lit::Int(i), ..
                }) = value
                {
                    importance = Some(i.base10_parse::<f64>()?);
                } else {
                    return Err(meta.error("importance must be a numeric literal"));
                }
            }
            "importance_fn" => {
                let s: syn::LitStr = meta.value()?.parse()?;
                importance_fn = Some(syn::parse_str(&s.value())?);
            }
            "salience" => {
                let s: syn::LitStr = meta.value()?.parse()?;
                salience = Some(parse_salience(&s.value())?);
            }
            "observation_template" => {
                let s: syn::LitStr = meta.value()?.parse()?;
                observation_template = Some(s.value());
            }
            "entity_bridge" => {
                entity_bridge = Some(parse_entity_bridge(&meta)?);
            }
            "coaching_signal" => {
                coaching = Some(parse_coaching_signal(&meta)?);
            }
            "metric" => {
                metric = Some(parse_metric_attr(&meta)?);
            }
            other => return Err(meta.error(format!("unknown ai() key: {}", other))),
        }
        Ok(())
    })?;

    Ok(AiEventAttr {
        importance,
        importance_fn,
        salience: salience.unwrap_or(SalienceSpec::Accumulate),
        observation_template,
        entity_bridge,
        coaching,
        metric,
    })
}

fn parse_salience(s: &str) -> syn::Result<SalienceSpec> {
    let s = s.trim();
    match s {
        "accumulate" => Ok(SalienceSpec::Accumulate),
        "extract" => Ok(SalienceSpec::Extract),
        "discard" => Ok(SalienceSpec::Discard),
        _ if s.starts_with("extract_if(") && s.ends_with(')') => {
            let inner = &s["extract_if(".len()..s.len() - 1];
            let expr: syn::Expr = syn::parse_str(inner).map_err(|e| {
                syn::Error::new(
                    Span::call_site(),
                    format!("invalid extract_if expression: {}", e),
                )
            })?;
            Ok(SalienceSpec::ExtractIf(expr))
        }
        _ => Err(syn::Error::new(
            Span::call_site(),
            format!("unknown salience verdict: {}", s),
        )),
    }
}

fn parse_coaching_signal(meta: &syn::meta::ParseNestedMeta) -> syn::Result<CoachingSignalSpec> {
    let mut app_from = None;
    let mut amount_from = None;
    let mut category_from = None;
    let mut rule = None;
    // If the attribute is written as `coaching_signal` without parentheses,
    // `meta` has no nested body and parse_nested_meta returns an error.
    // Treat that as a bare flag (all fields None).
    if meta.input.peek(syn::Token![=]) {
        // `coaching_signal = ...` — nothing to parse
        return Ok(CoachingSignalSpec {
            app_from,
            amount_from,
            category_from,
            rule,
        });
    }
    // If the next token is not an opening parenthesis, this is a bare flag.
    if !meta.input.peek(syn::token::Paren) {
        return Ok(CoachingSignalSpec {
            app_from,
            amount_from,
            category_from,
            rule,
        });
    }
    meta.parse_nested_meta(|inner| {
        let key = inner
            .path
            .get_ident()
            .ok_or_else(|| inner.error("expected identifier"))?
            .to_string();
        match key.as_str() {
            "app_from" => {
                let s: syn::LitStr = inner.value()?.parse()?;
                app_from = Some(syn::Ident::new(&s.value(), s.span()));
            }
            "amount_from" => {
                let s: syn::LitStr = inner.value()?.parse()?;
                amount_from = Some(syn::Ident::new(&s.value(), s.span()));
            }
            "category_from" => {
                let s: syn::LitStr = inner.value()?.parse()?;
                category_from = Some(syn::Ident::new(&s.value(), s.span()));
            }
            "rule" => {
                let s: syn::LitStr = inner.value()?.parse()?;
                rule = Some(s.value());
            }
            other => return Err(inner.error(format!("unknown coaching_signal key: {other}"))),
        }
        Ok(())
    })?;
    Ok(CoachingSignalSpec {
        app_from,
        amount_from,
        category_from,
        rule,
    })
}

pub fn parse_ai_feature_attr(attrs: &[syn::Attribute]) -> syn::Result<AiFeatureAttr> {
    let ai_attr = attrs
        .iter()
        .find(|a| a.path().is_ident("ai"))
        .ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "AiFeature derive requires #[ai(...)]",
            )
        })?;

    let mut recall_domain = None;
    let mut skill = None;
    let mut tool_name = None;
    let mut entity_kind = None;
    let mut event = None;
    let mut recall_boost_when = None;
    let mut recall_priority_field = None;
    let mut recall_recency_field = None;
    let mut recall_status_filter = None;
    let mut mirror_snapshots: Vec<MirrorSnapshotAttr> = Vec::new();
    let mut promotion_threshold: Option<u32> = None;

    ai_attr.parse_nested_meta(|meta| {
        let k = meta
            .path
            .get_ident()
            .ok_or_else(|| meta.error("expected identifier"))?
            .to_string();
        match k.as_str() {
            "recall_domain" => {
                let s: syn::LitStr = meta.value()?.parse()?;
                recall_domain = Some(syn::Ident::new(&s.value(), s.span()));
            }
            "skill" => {
                let s: syn::LitStr = meta.value()?.parse()?;
                skill = Some(s.value());
            }
            "tool_name" => {
                let s: syn::LitStr = meta.value()?.parse()?;
                tool_name = Some(s.value());
            }
            "entity_kind" => {
                let s: syn::LitStr = meta.value()?.parse()?;
                entity_kind = Some(s.value());
            }
            "event" => {
                let s: syn::LitStr = meta.value()?.parse()?;
                event = Some(syn::parse_str(&s.value())?);
            }
            "recall_boost_when" => {
                let s: syn::LitStr = meta.value()?.parse()?;
                recall_boost_when = Some(syn::parse_str(&s.value())?);
            }
            "recall_priority_field" => {
                let s: syn::LitStr = meta.value()?.parse()?;
                recall_priority_field = Some(syn::Ident::new(&s.value(), s.span()));
            }
            "recall_recency_field" => {
                let s: syn::LitStr = meta.value()?.parse()?;
                recall_recency_field = Some(syn::Ident::new(&s.value(), s.span()));
            }
            "recall_status_filter" => {
                let s: syn::LitStr = meta.value()?.parse()?;
                recall_status_filter = Some(syn::parse_str(&s.value())?);
            }
            "mirror_snapshot" => {
                mirror_snapshots.push(parse_mirror_snapshot(&meta)?);
            }
            "promotion_threshold" => {
                let n: syn::LitInt = meta.value()?.parse()?;
                promotion_threshold = Some(n.base10_parse()?);
            }
            other => return Err(meta.error(format!("unknown ai() key: {other}"))),
        }
        Ok(())
    })?;

    Ok(AiFeatureAttr {
        recall_domain: recall_domain.ok_or_else(|| {
            syn::Error::new(proc_macro2::Span::call_site(), "recall_domain is required")
        })?,
        skill: skill
            .ok_or_else(|| syn::Error::new(proc_macro2::Span::call_site(), "skill is required"))?,
        tool_name,
        entity_kind,
        event: event.ok_or_else(|| {
            syn::Error::new(proc_macro2::Span::call_site(), "event path is required")
        })?,
        recall_boost_when,
        recall_priority_field,
        recall_recency_field,
        recall_status_filter,
        mirror_snapshots,
        promotion_threshold,
    })
}

fn parse_mirror_snapshot(meta: &syn::meta::ParseNestedMeta) -> syn::Result<MirrorSnapshotAttr> {
    let mut name: Option<String> = None;
    let mut flush_interval_secs: Option<u64> = None;
    let mut subscribed_kinds: Vec<String> = Vec::new();

    meta.parse_nested_meta(|inner| {
        let key = inner
            .path
            .get_ident()
            .ok_or_else(|| inner.error("expected identifier"))?
            .to_string();
        match key.as_str() {
            "name" => {
                let s: syn::LitStr = inner.value()?.parse()?;
                name = Some(s.value());
            }
            "flush_interval_secs" => {
                let value: syn::Expr = inner.value()?.parse()?;
                let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(i), .. }) = value else {
                    return Err(inner.error("flush_interval_secs must be an integer literal"));
                };
                flush_interval_secs = Some(i.base10_parse::<u64>()?);
            }
            "event_kinds" => {
                // `event_kinds = ["TaskFocusChanged", "TaskCompleted"]` (bracketed string list).
                let arr: syn::ExprArray = inner.value()?.parse()?;
                for elem in arr.elems {
                    let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) = elem else {
                        return Err(inner.error(
                            "event_kinds must be a list of string literals, e.g. [\"Foo\", \"Bar\"]",
                        ));
                    };
                    subscribed_kinds.push(s.value());
                }
            }
            other => {
                return Err(inner.error(format!(
                    "unknown mirror_snapshot key: {other} \
                     (expected name / flush_interval_secs / event_kinds)"
                )));
            }
        }
        Ok(())
    })?;

    Ok(MirrorSnapshotAttr {
        name: name.ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "mirror_snapshot requires name = \"...\"",
            )
        })?,
        flush_interval_secs,
        subscribed_kinds,
    })
}

pub(crate) fn parse_metric_attr(meta: &syn::meta::ParseNestedMeta) -> syn::Result<MetricAttr> {
    let mut name: Option<String> = None;
    let mut value_from: Option<Expr> = None;
    let mut window: Option<u64> = None;
    let mut min_samples: u32 = 3; // default
    let mut aggregation: Option<Aggregation> = None;

    meta.parse_nested_meta(|nested| {
        let key = nested
            .path
            .get_ident()
            .ok_or_else(|| nested.error("expected identifier"))?
            .to_string();
        match key.as_str() {
            "name" => {
                let s: LitStr = nested.value()?.parse()?;
                name = Some(s.value());
            }
            "value_from" => {
                let e: Expr = nested.value()?.parse()?;
                value_from = Some(e);
            }
            "window" => {
                let s: LitStr = nested.value()?.parse()?;
                window = Some(parse_window_secs(&s.value()).map_err(|e| nested.error(e))?);
            }
            "min_samples" => {
                let n: LitInt = nested.value()?.parse()?;
                min_samples = n.base10_parse()?;
            }
            "aggregation" => {
                let s: LitStr = nested.value()?.parse()?;
                aggregation =
                    Some(Aggregation::parse_str(&s.value()).map_err(|e| nested.error(e))?);
            }
            other => return Err(nested.error(format!("unknown metric() key: {}", other))),
        }
        Ok(())
    })?;

    Ok(MetricAttr {
        name: name.ok_or_else(|| meta.error("metric: name is required"))?,
        value_from: value_from.ok_or_else(|| meta.error("metric: value_from is required"))?,
        window_secs: window.ok_or_else(|| meta.error("metric: window is required"))?,
        min_samples,
        aggregation: aggregation.ok_or_else(|| meta.error("metric: aggregation is required"))?,
    })
}

fn parse_entity_bridge(meta: &syn::meta::ParseNestedMeta) -> syn::Result<EntityBridge> {
    let mut ty = None;
    let mut name_from = None;
    let mut id_from = None;
    meta.parse_nested_meta(|inner| {
        let key = inner
            .path
            .get_ident()
            .ok_or_else(|| inner.error("expected identifier"))?
            .to_string();
        match key.as_str() {
            "type" => {
                let s: syn::LitStr = inner.value()?.parse()?;
                ty = Some(s.value());
            }
            "name_from" => {
                let s: syn::LitStr = inner.value()?.parse()?;
                name_from = Some(syn::Ident::new(&s.value(), s.span()));
            }
            "id_from" => {
                let s: syn::LitStr = inner.value()?.parse()?;
                id_from = Some(syn::Ident::new(&s.value(), s.span()));
            }
            other => return Err(inner.error(format!("unknown entity_bridge key: {}", other))),
        }
        Ok(())
    })?;
    Ok(EntityBridge {
        entity_type: ty
            .ok_or_else(|| syn::Error::new(Span::call_site(), "entity_bridge needs type"))?,
        name_from: name_from
            .ok_or_else(|| syn::Error::new(Span::call_site(), "entity_bridge needs name_from"))?,
        id_from: id_from
            .ok_or_else(|| syn::Error::new(Span::call_site(), "entity_bridge needs id_from"))?,
    })
}
