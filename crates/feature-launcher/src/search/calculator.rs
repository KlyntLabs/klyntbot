use regex::Regex;
use std::sync::LazyLock;

static WHAT_PCT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^what\s+%?\s*(?:percent\s+)?is\s+([\d.]+)\s+of\s+([\d.]+)$").unwrap()
});
static PCT_OF_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([\d.]+)\s*%\s*of\s+([\d.]+)$").unwrap()
});
static ADD_SUB_PCT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([\d.]+)\s*([+-])\s*([\d.]+)\s*%$").unwrap()
});
static UNIT_CONV_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([\d.]+)\s*°?\s*([a-z]+)\s+(?:to|in)\s+°?\s*([a-z]+)$").unwrap()
});

pub struct CalculatorResult {
    pub expression: String,
    pub result: f64,
}

pub struct Calculator;

impl Calculator {
    /// Try to evaluate a query as a math expression, hex/bin/oct conversion,
    /// percentage calculation, or unit conversion.
    /// Returns None if the query is not a valid expression.
    pub fn try_eval(query: &str) -> Option<CalculatorResult> {
        let expr = query.strip_prefix('=').unwrap_or(query).trim();
        if expr.is_empty() {
            return None;
        }

        // Try specialized parsers first (they won't match plain math)
        if let Some(r) = Self::try_base_conversion(expr) {
            return Some(r);
        }
        if let Some(r) = Self::try_percentage(expr) {
            return Some(r);
        }
        if let Some(r) = Self::try_unit_conversion(expr) {
            return Some(r);
        }

        // Fall back to meval math evaluation
        Self::try_math(query)
    }

    fn try_math(query: &str) -> Option<CalculatorResult> {
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
        if first == '-'
            && expr
                .chars()
                .nth(1)
                .is_none_or(|c| !c.is_ascii_digit() && c != '(')
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

    /// Detect hex (0xFF), binary (0b1010), octal (0o777) literals → decimal,
    /// or "hex 255", "bin 42", "oct 255" → convert decimal to that base.
    fn try_base_conversion(expr: &str) -> Option<CalculatorResult> {
        let lower = expr.trim().to_lowercase();

        // 0x... → decimal
        if let Some(hex_str) = lower.strip_prefix("0x") {
            let val = u64::from_str_radix(hex_str, 16).ok()?;
            return Some(CalculatorResult {
                expression: format!("0x{} = {}", hex_str.to_uppercase(), val),
                result: val as f64,
            });
        }

        // 0b... → decimal
        if let Some(bin_str) = lower.strip_prefix("0b") {
            let val = u64::from_str_radix(bin_str, 2).ok()?;
            return Some(CalculatorResult {
                expression: format!("0b{} = {}", bin_str, val),
                result: val as f64,
            });
        }

        // 0o... → decimal
        if let Some(oct_str) = lower.strip_prefix("0o") {
            let val = u64::from_str_radix(oct_str, 8).ok()?;
            return Some(CalculatorResult {
                expression: format!("0o{} = {}", oct_str, val),
                result: val as f64,
            });
        }

        // "hex <decimal>" → hex
        if let Some(num_str) = lower.strip_prefix("hex ") {
            let val: u64 = num_str.trim().parse().ok()?;
            return Some(CalculatorResult {
                expression: format!("{} = 0x{:X}", val, val),
                result: val as f64,
            });
        }

        // "bin <decimal>" → binary
        if let Some(num_str) = lower.strip_prefix("bin ") {
            let val: u64 = num_str.trim().parse().ok()?;
            return Some(CalculatorResult {
                expression: format!("{} = 0b{:b}", val, val),
                result: val as f64,
            });
        }

        // "oct <decimal>" → octal
        if let Some(num_str) = lower.strip_prefix("oct ") {
            let val: u64 = num_str.trim().parse().ok()?;
            return Some(CalculatorResult {
                expression: format!("{} = 0o{:o}", val, val),
                result: val as f64,
            });
        }

        None
    }

    /// Detect percentage patterns:
    /// - "20% of 150" → 30
    /// - "150 + 20%" → 180
    /// - "150 - 20%" → 120
    /// - "what % is 30 of 150" → 20%
    fn try_percentage(expr: &str) -> Option<CalculatorResult> {
        let lower = expr.trim().to_lowercase();

        // "what % is X of Y" or "what percent is X of Y"
        if let Some(caps) = WHAT_PCT_RE.captures(&lower) {
            let part: f64 = caps[1].parse().ok()?;
            let whole: f64 = caps[2].parse().ok()?;
            if whole == 0.0 {
                return None;
            }
            let pct = (part / whole) * 100.0;
            return Some(CalculatorResult {
                expression: format!("{} is {}% of {}", part, format_number(pct), whole),
                result: pct,
            });
        }

        // "X% of Y"
        if let Some(caps) = PCT_OF_RE.captures(&lower) {
            let pct: f64 = caps[1].parse().ok()?;
            let whole: f64 = caps[2].parse().ok()?;
            let result = whole * pct / 100.0;
            return Some(CalculatorResult {
                expression: format!(
                    "{}% of {} = {}",
                    format_number(pct),
                    whole,
                    format_number(result)
                ),
                result,
            });
        }

        // "X + Y%" or "X - Y%"
        if let Some(caps) = ADD_SUB_PCT_RE.captures(&lower) {
            let base: f64 = caps[1].parse().ok()?;
            let op = &caps[2];
            let pct: f64 = caps[3].parse().ok()?;
            let delta = base * pct / 100.0;
            let result = if op == "+" {
                base + delta
            } else {
                base - delta
            };
            return Some(CalculatorResult {
                expression: format!(
                    "{} {} {}% = {}",
                    base,
                    op,
                    format_number(pct),
                    format_number(result)
                ),
                result,
            });
        }

        None
    }

    /// Detect unit conversion patterns like "5km to miles", "100°F to °C".
    fn try_unit_conversion(expr: &str) -> Option<CalculatorResult> {
        let lower = expr.trim().to_lowercase();

        // Pattern: <number> <unit> to <unit>
        // Also handle ° variants: "100°f to °c", "100 f to c"
        let caps = UNIT_CONV_RE.captures(&lower)?;
        let value: f64 = caps[1].parse().ok()?;
        let from = caps[2].to_string();
        let to = caps[3].to_string();

        convert_units(value, &from, &to)
    }
}

fn format_number(n: f64) -> String {
    if (n - n.round()).abs() < 1e-10 {
        format!("{}", n as i64)
    } else {
        format!("{:.4}", n)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

fn convert_units(value: f64, from: &str, to: &str) -> Option<CalculatorResult> {
    let (result, from_label, to_label) = match (from, to) {
        // Distance
        ("km", "miles" | "mi") => (value * 0.621371, "km", "miles"),
        ("miles" | "mi", "km") => (value * 1.60934, "miles", "km"),
        ("m", "ft" | "feet") => (value * 3.28084, "m", "ft"),
        ("ft" | "feet", "m") => (value * 0.3048, "ft", "m"),
        ("cm", "inches" | "in") => (value * 0.393701, "cm", "in"),
        ("inches" | "in", "cm") => (value * 2.54, "in", "cm"),

        // Weight
        ("kg", "lbs" | "lb" | "pounds") => (value * 2.20462, "kg", "lbs"),
        ("lbs" | "lb" | "pounds", "kg") => (value * 0.453592, "lbs", "kg"),

        // Temperature
        ("c", "f") => (value * 9.0 / 5.0 + 32.0, "°C", "°F"),
        ("f", "c") => ((value - 32.0) * 5.0 / 9.0, "°F", "°C"),

        // Volume
        ("l" | "liters" | "litres", "gal" | "gallons") => (value * 0.264172, "L", "gal"),
        ("gal" | "gallons", "l" | "liters" | "litres") => (value * 3.78541, "gal", "L"),

        _ => return None,
    };

    Some(CalculatorResult {
        expression: format!(
            "{} {} = {} {}",
            format_number(value),
            from_label,
            format_number(result),
            to_label
        ),
        result,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Math ────────────────────────────────────────────────────────────

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

    // ── Base conversion ─────────────────────────────────────────────────

    #[test]
    fn test_hex_to_decimal() {
        let r = Calculator::try_eval("0xFF").unwrap();
        assert!((r.result - 255.0).abs() < f64::EPSILON);
        assert!(r.expression.contains("255"));
    }

    #[test]
    fn test_binary_to_decimal() {
        let r = Calculator::try_eval("0b1010").unwrap();
        assert!((r.result - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_octal_to_decimal() {
        let r = Calculator::try_eval("0o777").unwrap();
        assert!((r.result - 511.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_decimal_to_hex() {
        let r = Calculator::try_eval("hex 255").unwrap();
        assert!((r.result - 255.0).abs() < f64::EPSILON);
        assert!(r.expression.contains("0xFF"));
    }

    #[test]
    fn test_decimal_to_bin() {
        let r = Calculator::try_eval("bin 42").unwrap();
        assert!((r.result - 42.0).abs() < f64::EPSILON);
        assert!(r.expression.contains("0b101010"));
    }

    #[test]
    fn test_decimal_to_oct() {
        let r = Calculator::try_eval("oct 255").unwrap();
        assert!((r.result - 255.0).abs() < f64::EPSILON);
        assert!(r.expression.contains("0o377"));
    }

    // ── Percentages ─────────────────────────────────────────────────────

    #[test]
    fn test_pct_of() {
        let r = Calculator::try_eval("20% of 150").unwrap();
        assert!((r.result - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_add_pct() {
        let r = Calculator::try_eval("150 + 20%").unwrap();
        assert!((r.result - 180.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_sub_pct() {
        let r = Calculator::try_eval("150 - 20%").unwrap();
        assert!((r.result - 120.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_what_pct() {
        let r = Calculator::try_eval("what % is 30 of 150").unwrap();
        assert!((r.result - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_what_percent_is() {
        let r = Calculator::try_eval("what percent is 30 of 150").unwrap();
        assert!((r.result - 20.0).abs() < f64::EPSILON);
    }

    // ── Unit conversions ────────────────────────────────────────────────

    #[test]
    fn test_km_to_miles() {
        let r = Calculator::try_eval("5km to miles").unwrap();
        assert!((r.result - 3.106855).abs() < 0.001);
    }

    #[test]
    fn test_miles_to_km() {
        let r = Calculator::try_eval("3 miles to km").unwrap();
        assert!((r.result - 4.82802).abs() < 0.001);
    }

    #[test]
    fn test_f_to_c() {
        let r = Calculator::try_eval("100°F to °C").unwrap();
        assert!((r.result - 37.7778).abs() < 0.01);
    }

    #[test]
    fn test_c_to_f() {
        let r = Calculator::try_eval("0 c to f").unwrap();
        assert!((r.result - 32.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_kg_to_lbs() {
        let r = Calculator::try_eval("5kg to lbs").unwrap();
        assert!((r.result - 11.0231).abs() < 0.001);
    }

    #[test]
    fn test_lbs_to_kg() {
        let r = Calculator::try_eval("10 lbs to kg").unwrap();
        assert!((r.result - 4.53592).abs() < 0.001);
    }

    #[test]
    fn test_m_to_ft() {
        let r = Calculator::try_eval("5m to ft").unwrap();
        assert!((r.result - 16.4042).abs() < 0.001);
    }

    #[test]
    fn test_cm_to_inches() {
        let r = Calculator::try_eval("10cm to inches").unwrap();
        assert!((r.result - 3.93701).abs() < 0.001);
    }

    #[test]
    fn test_l_to_gal() {
        let r = Calculator::try_eval("5 l to gal").unwrap();
        assert!((r.result - 1.32086).abs() < 0.001);
    }

    #[test]
    fn test_ft_to_cm_via_m() {
        // "5ft to cm" — ft isn't directly mapped to cm, should return None
        assert!(Calculator::try_eval("5ft to cm").is_none());
    }

    #[test]
    fn test_unit_conversion_with_in_keyword() {
        let r = Calculator::try_eval("10 km in miles").unwrap();
        assert!((r.result - 6.21371).abs() < 0.001);
    }
}
