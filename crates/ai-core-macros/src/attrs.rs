use proc_macro2::Span;
use syn::{Attribute, Expr, ExprLit, Lit};

pub struct AiEventAttr {
    pub importance: Option<f64>,
    pub importance_fn: Option<syn::Path>,
    pub salience: SalienceSpec,
    pub observation_template: Option<String>,
    pub entity_bridge: Option<EntityBridge>,
}

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

pub fn parse_ai_event_attr(attrs: &[Attribute]) -> syn::Result<AiEventAttr> {
    let ai_attr = attrs.iter().find(|a| a.path().is_ident("ai"))
        .ok_or_else(|| syn::Error::new(Span::call_site(),
            "every variant must have #[ai(...)] attribute"))?;

    let mut importance = None;
    let mut importance_fn = None;
    let mut salience = None;
    let mut observation_template = None;
    let mut entity_bridge = None;

    ai_attr.parse_nested_meta(|meta| {
        let name = meta.path.get_ident()
            .ok_or_else(|| meta.error("expected identifier"))?.to_string();
        match name.as_str() {
            "importance" => {
                let value: Expr = meta.value()?.parse()?;
                if let Expr::Lit(ExprLit { lit: Lit::Float(f), .. }) = value {
                    importance = Some(f.base10_parse::<f64>()?);
                } else if let Expr::Lit(ExprLit { lit: Lit::Int(i), .. }) = value {
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
    })
}

fn parse_salience(s: &str) -> syn::Result<SalienceSpec> {
    let s = s.trim();
    match s {
        "accumulate" => Ok(SalienceSpec::Accumulate),
        "extract" => Ok(SalienceSpec::Extract),
        "discard" => Ok(SalienceSpec::Discard),
        _ if s.starts_with("extract_if(") && s.ends_with(')') => {
            let inner = &s["extract_if(".len()..s.len()-1];
            let expr: syn::Expr = syn::parse_str(inner)
                .map_err(|e| syn::Error::new(Span::call_site(),
                    format!("invalid extract_if expression: {}", e)))?;
            Ok(SalienceSpec::ExtractIf(expr))
        }
        _ => Err(syn::Error::new(Span::call_site(),
            format!("unknown salience verdict: {}", s))),
    }
}

fn parse_entity_bridge(meta: &syn::meta::ParseNestedMeta) -> syn::Result<EntityBridge> {
    let mut ty = None;
    let mut name_from = None;
    let mut id_from = None;
    meta.parse_nested_meta(|inner| {
        let key = inner.path.get_ident()
            .ok_or_else(|| inner.error("expected identifier"))?.to_string();
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
        entity_type: ty.ok_or_else(|| syn::Error::new(Span::call_site(), "entity_bridge needs type"))?,
        name_from: name_from.ok_or_else(|| syn::Error::new(Span::call_site(), "entity_bridge needs name_from"))?,
        id_from: id_from.ok_or_else(|| syn::Error::new(Span::call_site(), "entity_bridge needs id_from"))?,
    })
}
