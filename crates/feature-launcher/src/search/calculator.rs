pub struct CalculatorResult {
    pub expression: String,
    pub result: f64,
}

pub struct Calculator;

impl Calculator {
    /// Try to evaluate a query as a math expression.
    /// Strips leading `=` prefix if present.
    /// Returns None if the query is not a valid expression.
    pub fn try_eval(query: &str) -> Option<CalculatorResult> {
        let expr = query.strip_prefix('=').unwrap_or(query).trim();
        if expr.is_empty() {
            return None;
        }

        // Quick check: must start with digit, (, math function, or - followed by digit
        let first = expr.chars().next()?;
        let has_prefix = query.starts_with('=');
        if !first.is_ascii_digit() && first != '(' && first != '-' {
            // Allow function names (sqrt, sin, cos, etc.) only with = prefix
            if !has_prefix || !first.is_ascii_alphabetic() {
                return None;
            }
        }
        #[allow(clippy::unnecessary_map_or)] // is_none_or requires Rust 1.82, MSRV is 1.75
        if first == '-'
            && expr
                .chars()
                .nth(1)
                .map_or(true, |c| !c.is_ascii_digit() && c != '(')
        {
            return None;
        }

        match meval::eval_str(expr) {
            Ok(result) if result.is_finite() => Some(CalculatorResult {
                expression: expr.to_string(),
                result,
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_math() {
        let result = Calculator::try_eval("3 + 4").unwrap();
        assert!((result.result - 7.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_prefix_stripped() {
        let result = Calculator::try_eval("=sqrt(16)").unwrap();
        assert!((result.result - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_not_math_returns_none() {
        assert!(Calculator::try_eval("hello world").is_none());
        assert!(Calculator::try_eval("3d printer").is_none());
    }

    #[test]
    fn test_complex_expression() {
        let result = Calculator::try_eval("(10 + 5) * 2 / 3").unwrap();
        assert!((result.result - 10.0).abs() < f64::EPSILON);
    }
}
