//! Port of `core/lexo/messages.py` (the subset the modules use: `msg`, `msgn`, `wire`, `render`).
//!
//! A message is written once, in English, with `{placeholders}`. On the way to the browser it travels as `{"$m": template, "p": values}`
//! (and `"$n"`: the plural template for counted messages) and the browser translates it. `text()` is the formatted English text.
use regex::Regex;
use serde_json::{json, Map, Value};
use std::sync::OnceLock;

fn placeholder() -> &'static Regex {
    static P: OnceLock<Regex> = OnceLock::new();
    P.get_or_init(|| Regex::new(r"\{(\w+)\}").unwrap())
}

/// `str(value)` of Python for the values a message carries (a float is written like Rust's `{:?}`, which differs from Python for very large and very small numbers).
pub fn py_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Number(n) if n.is_f64() => format!("{:?}", n.as_f64().unwrap()),
        other => other.to_string(),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub template: String,
    /// The template for numbers other than 1 (counted messages), or `None`.
    pub plural: Option<String>,
    pub params: Map<String, Value>,
}

impl Message {
    pub fn new(template: &str, plural: Option<&str>, params: Map<String, Value>) -> Message {
        Message { template: template.into(), plural: plural.map(String::from), params }
    }

    /// The formatted English text.
    pub fn text(&self) -> String {
        let one = self.plural.is_none() || self.params.get("n") == Some(&json!(1));
        let template = if one { &self.template } else { self.plural.as_ref().unwrap() };
        placeholder()
            .replace_all(template, |c: &regex::Captures| self.params.get(&c[1]).map(|v| py_str(&render(v))).unwrap_or_else(|| c[0].to_string()))
            .into_owned()
    }

    /// `{"$m": template, "p": values}` and `"$n"` for a counted message.
    pub fn wire(&self) -> Value {
        let mut out = Map::new();
        out.insert("$m".into(), json!(self.template));
        out.insert("p".into(), Value::Object(self.params.clone()));
        if let Some(p) = &self.plural {
            out.insert("$n".into(), json!(p));
        }
        Value::Object(out)
    }
}

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text())
    }
}

fn params_of(params: &[(&str, Value)]) -> Map<String, Value> {
    params.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

pub fn msg(template: &str, params: &[(&str, Value)]) -> Message {
    Message::new(template, None, params_of(params))
}

/// A counted message: `one` is used for n == 1, `many` otherwise; both contain {n}.
pub fn msgn(n: i64, one: &str, many: &str, params: &[(&str, Value)]) -> Message {
    let mut p = params_of(params);
    p.insert("n".into(), json!(n));
    Message::new(one, Some(many), p)
}

/// The English text of what `wire` produced: every `{"$m": ..., "p": ...}` becomes its formatted text (`$j`: joined messages).
pub fn render(value: &Value) -> Value {
    match value {
        Value::Object(o) if o.contains_key("$j") => {
            let sep = o.get("sep").and_then(Value::as_str).unwrap_or("");
            let end = o.get("end").and_then(Value::as_str).unwrap_or("");
            let parts: Vec<String> = o["$j"].as_array().map(|a| a.iter().map(|p| py_str(&render(p))).collect()).unwrap_or_default();
            Value::String(parts.join(sep) + end)
        }
        Value::Object(o) if o.contains_key("$m") => {
            let params: Map<String, Value> =
                o.get("p").and_then(Value::as_object).map(|p| p.iter().map(|(k, v)| (k.clone(), render(v))).collect()).unwrap_or_default();
            let template = o["$m"].as_str().unwrap_or("");
            Value::String(Message::new(template, o.get("$n").and_then(Value::as_str), params).text())
        }
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), render(v))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(render).collect()),
        other => other.clone(),
    }
}

/// What an error carries: a `Message` (which the browser translates) or a plain text (shown as it is, like a plain `str` in Python).
#[derive(Clone, Debug, PartialEq)]
pub enum Text {
    Msg(Message),
    Plain(String),
}

impl Text {
    pub fn text(&self) -> String {
        match self {
            Text::Msg(m) => m.text(),
            Text::Plain(s) => s.clone(),
        }
    }

    pub fn wire(&self) -> Value {
        match self {
            Text::Msg(m) => m.wire(),
            Text::Plain(s) => Value::String(s.clone()),
        }
    }
}

impl From<Message> for Text {
    fn from(m: Message) -> Text {
        Text::Msg(m)
    }
}

impl From<String> for Text {
    fn from(s: String) -> Text {
        Text::Plain(s)
    }
}

impl From<&str> for Text {
    fn from(s: &str) -> Text {
        Text::Plain(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_are_filled_and_unknown_ones_stay() {
        let m = msg("Cannot save {name}: {reason} {other}", &[("name", json!("a.rpy")), ("reason", json!(3))]);
        assert_eq!(m.text(), "Cannot save a.rpy: 3 {other}");
    }

    #[test]
    fn counted_messages_pick_the_form_by_n() {
        assert_eq!(msgn(1, "{n} file", "{n} files", &[]).text(), "1 file");
        assert_eq!(msgn(3, "{n} file", "{n} files", &[]).text(), "3 files");
        assert_eq!(msgn(0, "{n} file", "{n} files", &[]).text(), "0 files");
    }

    #[test]
    fn wire_and_render_round_trip_to_the_english_text() {
        let w = json!({"a": [msg("Hi {x}", &[("x", json!(true))]).wire()], "b": msgn(2, "{n} one", "{n} many", &[]).wire()});
        assert_eq!(render(&w), json!({"a": ["Hi True"], "b": "2 many"}));
        assert_eq!(render(&json!({"$j": [msg("a", &[]).wire(), "b"], "sep": ", ", "end": "."})), json!("a, b."));
    }
}
