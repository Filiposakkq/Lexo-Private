//! The few places where Python's own formatting and number rules decide the bytes of a file or an answer: `json.dumps` with its default separators, `round`, `float()`,
//! `str.casefold`. The files written by the Python Core are read by the Rust Core and the other way round, and the golden data compares the bytes.
use serde_json::Value;

/// `json.dumps(value, ensure_ascii=ensure_ascii)` with the default separators (`", "` and `": "`).
pub fn json_dumps(value: &Value, ensure_ascii: bool) -> String {
    let mut out = String::new();
    write_value(value, ensure_ascii, &mut out);
    out
}

/// `json.dumps(value, ensure_ascii=False, indent=1)`: one space per level.
pub fn json_dumps_indent1(value: &Value) -> String {
    use serde::Serialize;
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, serde_json::ser::PrettyFormatter::with_indent(b" "));
    let _ = value.serialize(&mut ser);
    String::from_utf8_lossy(&buf).into_owned()
}

fn write_string(s: &str, ensure_ascii: bool, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if ensure_ascii && !(' '..='~').contains(&c) => {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{u:04x}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn write_value(v: &Value, ensure_ascii: bool, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&n.to_string()),
        Value::String(s) => write_string(s, ensure_ascii, out),
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_value(x, ensure_ascii, out);
            }
            out.push(']');
        }
        Value::Object(o) => {
            out.push('{');
            for (i, (k, x)) in o.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_string(k, ensure_ascii, out);
                out.push_str(": ");
                write_value(x, ensure_ascii, out);
            }
            out.push('}');
        }
    }
}

/// Python's `round(x)`: halves go to the even number.
pub fn round_half_even(x: f64) -> f64 {
    let r = x.round();
    if (x - x.trunc()).abs() == 0.5 {
        2.0 * (x / 2.0).round()
    } else {
        r
    }
}

/// Python's `round(x, digits)`: the decimal rounding of the exact binary value, the nearest double to that decimal.
pub fn round_digits(x: f64, digits: usize) -> f64 {
    format!("{x:.digits$}").parse().unwrap_or(x)
}

/// Python's `float(text)` for the usual spellings: surrounding white space is allowed; `None` where Python raises.
pub fn float(text: &str) -> Option<f64> {
    let t = crate::util::py_strip(text);
    if t.is_empty() || t.contains('_') {
        return None;
    }
    t.parse().ok()
}

/// `int(float(text))`: the whole part; `None` for text that is no number and for infinity and NaN.
pub fn int_of_float(text: &str) -> Option<i64> {
    float(text).filter(|f| f.is_finite()).map(|f| f.trunc() as i64)
}

/// `str.casefold()` for the letters that matter in practice.
pub fn casefold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'ß' | 'ẞ' => out.push_str("ss"),
            'ſ' => out.push('s'),
            'ς' => out.push('σ'),
            'µ' => out.push('μ'),
            other => out.extend(other.to_lowercase()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dumps_uses_the_separators_of_python() {
        let v = json!({"t": "x", "k": ["a", 1], "n": null, "e": {}, "f": 1.5});
        assert_eq!(json_dumps(&v, false), "{\"t\": \"x\", \"k\": [\"a\", 1], \"n\": null, \"e\": {}, \"f\": 1.5}");
        assert_eq!(json_dumps(&json!("č\u{2028}\u{7f}\n\u{1}😀"), false), "\"č\u{2028}\u{7f}\\n\\u0001😀\"");
        assert_eq!(json_dumps(&json!("č\u{7f}😀"), true), "\"\\u010d\\u007f\\ud83d\\ude00\"");
    }

    #[test]
    fn rounding_follows_python() {
        assert_eq!([round_half_even(0.5), round_half_even(1.5), round_half_even(2.5), round_half_even(-0.5), round_half_even(2.4)], [0.0, 2.0, 2.0, -0.0, 2.0]);
        assert_eq!(round_digits(0.285, 2), 0.28);
        assert_eq!(round_digits(2.675, 2), 2.67);
        assert_eq!(round_digits(1.0, 1), 1.0);
    }

    #[test]
    fn float_and_casefold() {
        assert_eq!(float(" 5,5 "), None);
        assert_eq!(float(" 5.5 "), Some(5.5));
        assert_eq!(int_of_float("7.9"), Some(7));
        assert_eq!(int_of_float("nan"), None);
        assert_eq!(casefold("Straße ΟΔΟΣ"), "strasse οδοσ");
    }
}
