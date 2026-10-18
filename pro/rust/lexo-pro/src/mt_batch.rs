//! Port of `pro/lexo_pro/mt_batch.py`: batch machine translation (the Pro feature `mt.batch`): which lines a batch takes, and the translating of many lines.
//!
//! Core chooses the candidate lines, gathers each line's context and glossary terms and later checks and writes the suggestions; this module cuts the list to the limit of the licence and
//! does the translating (it uses Core's single-line `mt::translate`, which stays free). Plain data in, plain data out.
use crate::licence;
use lexo_core::extensions::ExtensionError;
use lexo_core::messages::{py_str, Text};
use lexo_core::mt::{self, Http};
use serde_json::{json, Map, Value};

/// `{items: [{file, line}], total, chars, limited?}`: the candidates `{file, line, chars}` cut to the limit.
pub fn targets(p: &Value) -> Value {
    let all: Vec<Value> = p["items"].as_array().cloned().unwrap_or_default();
    let total = all.len();
    let (items, cut) = licence::cap("mt.batch", all);
    let chars: i64 = items.iter().map(|i| i["chars"].as_i64().or_else(|| py_str(&i["chars"]).parse().ok()).unwrap_or(0)).sum();
    let mut out = match cut {
        Some(Value::Object(o)) => o,
        _ => Map::new(),
    };
    out.insert("items".into(), Value::Array(items.iter().map(|i| json!({"file": i["file"], "line": i["line"]})).collect()));
    out.insert("total".into(), json!(total));
    out.insert("chars".into(), json!(chars));
    Value::Object(out)
}

/// Suggestions for the requests `[{id, source, context, terms}]` with the settings `cfg`. A failed line carries its own error; a rejected key, missing consent or a broken connection
/// stops the batch (the same message for every line would be useless).
pub fn translate(p: &Value, http: Http) -> Result<Value, ExtensionError> {
    let cfg = p["cfg"].as_object().cloned().ok_or_else(|| ExtensionError(Text::Plain("The request has no settings.".into())))?;
    let mut out: Vec<Value> = Vec::new();
    for r in p["requests"].as_array().into_iter().flatten() {
        let terms: Vec<(String, String)> = r["terms"].as_object().into_iter().flatten().map(|(k, v)| (k.clone(), py_str(v))).collect();
        let context: Vec<String> = r["context"].as_array().into_iter().flatten().map(py_str).collect();
        match mt::translate(&cfg, &py_str(&r["source"]), &terms, &context, http) {
            Err(e) => {
                let err = e.0.wire();
                out.push(json!({"id": r["id"], "error": err}));
                if e.0 != mt::no_text() {
                    return Ok(json!({"results": out, "stopped": err}));
                }
            }
            Ok(res) => out.push(
                json!({"id": r["id"], "suggestion": res.text, "warnings": res.warnings.iter().map(|w| w.wire()).collect::<Vec<_>>(), "provider": res.provider}),
            ),
        }
    }
    Ok(json!({"results": out, "stopped": ""}))
}
