use std::collections::HashMap;

#[derive(Debug, thiserror::Error)]
pub enum TemplateErr {
    #[error("missing argument: {0}")]
    MissingArg(String),
}

/// Substitute `{{name}}` placeholders in `s` with values from `args`.
/// Unknown placeholders return MissingArg. `{{{{` escapes to literal `{{`.
pub fn substitute(s: &str, args: &HashMap<String, String>) -> Result<String, TemplateErr> {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '{' && chars.peek().map(|(_, c)| *c) == Some('{') {
            chars.next();
            if chars.peek().map(|(_, c)| *c) == Some('{') {
                // This is `{{{{...}}}}` — scan for the closing `}}}}` and emit `{{...}}`
                chars.next(); // consume third `{`
                if chars.peek().map(|(_, c)| *c) == Some('{') {
                    chars.next(); // consume fourth `{`
                    let mut inner = String::new();
                    let mut closed = false;
                    while let Some(&(_, c)) = chars.peek() {
                        chars.next();
                        if c == '}' && chars.peek().map(|(_, c)| *c) == Some('}') {
                            chars.next(); // second `}`
                            if chars.peek().map(|(_, c)| *c) == Some('}') {
                                chars.next(); // third `}`
                                if chars.peek().map(|(_, c)| *c) == Some('}') {
                                    chars.next(); // fourth `}`
                                    closed = true;
                                    break;
                                }
                                inner.push(c);
                                inner.push('}');
                                inner.push('}');
                            } else {
                                inner.push(c);
                                inner.push('}');
                            }
                        } else {
                            inner.push(c);
                        }
                    }
                    out.push_str("{{");
                    out.push_str(&inner);
                    if closed {
                        out.push_str("}}");
                    }
                    continue;
                }
                // Only three `{` — treat as unclosed, fall through with name scanning
                // but we already consumed the third `{`, so just emit `{{{` literally
                out.push_str("{{{");
                continue;
            }
            let mut name = String::new();
            let mut closed = false;
            while let Some(&(_, c)) = chars.peek() {
                chars.next();
                if c == '}' && chars.peek().map(|(_, c)| *c) == Some('}') {
                    chars.next();
                    closed = true;
                    break;
                }
                name.push(c);
            }
            if !closed {
                out.push_str("{{");
                out.push_str(&name);
                continue;
            }
            let value = args.get(&name).ok_or(TemplateErr::MissingArg(name.clone()))?;
            out.push_str(value);
        } else {
            out.push(c);
            let _ = i;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn simple_substitution() {
        let r = substitute("hello {{name}}", &args(&[("name", "world")])).unwrap();
        assert_eq!(r, "hello world");
    }

    #[test]
    fn missing_arg_errors() {
        let r = substitute("hi {{x}}", &args(&[]));
        assert!(matches!(r, Err(TemplateErr::MissingArg(s)) if s == "x"));
    }

    #[test]
    fn escape_double_brace() {
        let r = substitute("{{{{not}}}}", &args(&[])).unwrap();
        assert_eq!(r, "{{not}}");
    }

    #[test]
    fn no_args_passthrough() {
        let r = substitute("plain text", &args(&[])).unwrap();
        assert_eq!(r, "plain text");
    }
}
