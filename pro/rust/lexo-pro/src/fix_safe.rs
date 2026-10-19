//! Port of `reference/python/pro/lexo_pro/fix_safe.py`: safe bulk fixes with a preview (the Pro feature `fixes.safe`): planning the fixes and chaining them on one line.
//!
//! Three kinds, all of them touch only what they must: `speaker` (the speaker, attributes and trailing code of the line are copied from the original comment, the quoted text is never
//! touched), `spaces` (collapse a double space between words unless the original has one) and `typo` (apply the spelling hints: missing diacritics, swapped or doubled letters). Tags,
//! variables and escapes are protected. Plain data in, plain data out: Core finds the candidate lines and writes the confirmed result with its single writer; this module decides what each
//! fix is. Untranslated lines are never candidates.
use crate::licence;
use fancy_regex::Regex;
use lexo_core::messages::{msg, py_str, Message};
use lexo_core::rpy;
use lexo_core::textedit::{case_like, sub_pattern, LETTER};
use lexo_core::util::{norm_ws, py_strip};
use serde_json::{json, Map, Value};

pub const KINDS: [&str; 3] = ["speaker", "spaces", "typo"];

/// The code line with speaker and trailing code taken from the original, or `None` when not applicable.
pub fn fixed_line(src_who: &str, src_suffix: &str, raw_line: &str) -> Option<String> {
    let m = rpy::string_re().find(raw_line)?;
    let indent: String = raw_line.chars().take_while(|c| lexo_core::util::is_space(*c)).collect();
    let (who, suffix) = (py_strip(src_who), py_strip(src_suffix));
    let mut out = indent;
    if !who.is_empty() {
        out.push_str(who);
        out.push(' ');
    }
    out.push_str(&raw_line[m.start()..m.end()]);
    if !suffix.is_empty() {
        out.push(' ');
        out.push_str(suffix);
    }
    Some(out)
}

fn probe() -> &'static Regex {
    static R: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    R.get_or_init(|| Regex::new(r"\S {2,}\S").unwrap())
}

fn is_match(re: &Regex, text: &str) -> bool {
    re.is_match(text).unwrap_or(false)
}

/// Collapse runs of spaces between words, unless the original has them too. Returns the new text or `None`.
pub fn fix_spaces(source: &str, target: &str) -> Option<String> {
    if !is_match(probe(), target) || is_match(probe(), source) {
        return None;
    }
    static R: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = R.get_or_init(|| Regex::new(r"(?<=\S) {2,}(?=\S)").unwrap());
    let (new, n) = sub_pattern(re, target, true, 0, &mut |_| Ok(" ".to_string())).ok()?;
    (n > 0 && new != target).then_some(new)
}

/// Apply the spelling hints `[{word, better, typo_kind}]` to the text. Returns (new text, [(word, better, kind)]) or `None`.
/// What a typo fix changed: (word, better word, kind).
pub type Changes = Vec<(String, String, String)>;

pub fn fix_typos(target: &str, issues: &[Value]) -> Option<(String, Changes)> {
    let (mut new, mut done) = (target.to_string(), Vec::new());
    for iss in issues {
        let (w, better) = (iss.get("word").and_then(Value::as_str).unwrap_or(""), iss.get("better").and_then(Value::as_str).unwrap_or(""));
        if w.is_empty() || better.is_empty() {
            continue;
        }
        let Ok(pat) = Regex::new(&format!("(?i)(?<!{LETTER}){}(?!{LETTER})", fancy_regex::escape(w))) else { continue };
        let Ok((changed, n)) = sub_pattern(&pat, &new, true, 1, &mut |m| Ok(case_like(m.get(0).map_or("", |g| g.as_str()), better))) else { continue };
        if n > 0 {
            new = changed;
            done.push((w.to_string(), better.to_string(), iss.get("typo_kind").map_or(String::new(), py_str)));
        }
    }
    (!done.is_empty()).then_some((new, done))
}

fn skip_value(m: Option<Message>) -> Value {
    m.map_or(json!(""), |m| m.wire())
}

/// `{items, total, fixable, counts, limited?}`: every fix of the requested kinds for the candidates, cut to the limit of the licence.
pub fn plan(p: &Value) -> Value {
    let wanted: Vec<String> = p
        .get("kinds")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
        .map(|a| a.iter().map(py_str).collect())
        .unwrap_or_else(|| vec!["speaker".to_string()]);
    let kinds: Vec<&str> = wanted.iter().filter_map(|w| KINDS.iter().copied().find(|k| k == w)).collect();
    let mut out: Vec<Map<String, Value>> = Vec::new();
    let obj = |v: Value| v.as_object().cloned().unwrap_or_default();
    if kinds.contains(&"speaker") {
        for c in p.get("speaker").and_then(Value::as_array).into_iter().flatten() {
            let after = fixed_line(&py_str(&c["src_who"]), &py_str(&c["src_suffix"]), &py_str(&c["cur_raw"]));
            let skip = match &after {
                None => Some(msg("The line does not contain text in quotes.", &[])),
                Some(a) if norm_ws(a) == norm_ws(&py_str(&c["src_raw"])) => {
                    Some(msg("After the fix the line would be identical to the original (it would count as untranslated) – fix it by hand.", &[]))
                }
                Some(_) => None,
            };
            out.push(obj(json!({
                "file": c["file"], "line": c["line"], "kind": "speaker", "before": c["cur_raw"], "after": after.unwrap_or_default(), "default": skip.is_none(), "code": true,
                "note": msg("speaker / attributes from the original", &[]).wire(), "skip": skip_value(skip),
            })));
        }
    }
    for c in p.get("text").and_then(Value::as_array).into_iter().flatten() {
        let (source, target) = (py_str(&c["source"]), py_str(&c["target"]));
        if kinds.contains(&"spaces") {
            if let Some(new) = fix_spaces(&source, &target) {
                out.push(obj(json!({"file": c["file"], "line": c["line"], "kind": "spaces", "before": target, "after": new, "default": true, "note": msg("double space", &[]).wire(), "skip": "", "code": false})));
            }
        }
        if kinds.contains(&"typo") {
            let issues: Vec<Value> = c.get("issues").and_then(Value::as_array).cloned().unwrap_or_default();
            if let Some((new, done)) = fix_typos(&target, &issues) {
                let sure = done.iter().all(|(w, _, k)| k == "diacritics" && w.chars().count() >= 6);
                let note = done.iter().map(|(w, b, _)| format!("{w} → {b}")).collect::<Vec<_>>().join("; ");
                out.push(obj(json!({"file": c["file"], "line": c["line"], "kind": "typo", "before": target, "after": new, "default": sure, "note": note, "skip": "", "code": false})));
            }
        }
    }
    out.sort_by_key(|r| (py_str(&r["file"]).to_lowercase(), r["line"].as_i64().unwrap_or(0), py_str(&r["kind"])));
    let counts: Map<String, Value> = kinds.iter().map(|k| (k.to_string(), json!(out.iter().filter(|r| r["kind"] == *k).count()))).collect();
    let full = out.len();
    let (out, cut) = licence::cap("fixes.safe", out);
    let fixable = out.iter().filter(|r| r["skip"] == "").count();
    let mut answer = match cut {
        Some(Value::Object(o)) => o,
        _ => Map::new(),
    };
    answer.insert("items".into(), Value::Array(out.into_iter().map(Value::Object).collect()));
    answer.insert("total".into(), json!(full));
    answer.insert("fixable".into(), json!(fixable));
    answer.insert("counts".into(), Value::Object(counts));
    Value::Object(answer)
}

/// The text of each requested line after the chosen text fixes, one after another (spaces first, then typos): `[{file, line, text}]`.
pub fn chain(p: &Value) -> Value {
    let mut out: Vec<Value> = Vec::new();
    for c in p["items"].as_array().into_iter().flatten() {
        let mut text = py_str(&c["target"]);
        let kinds: Vec<String> = c["kinds"].as_array().into_iter().flatten().map(py_str).collect();
        if kinds.iter().any(|k| k == "spaces") {
            if let Some(new) = fix_spaces(&py_str(&c["source"]), &text) {
                text = new;
            }
        }
        if kinds.iter().any(|k| k == "typo") {
            let issues: Vec<Value> = c.get("issues").and_then(Value::as_array).cloned().unwrap_or_default();
            if let Some((new, _)) = fix_typos(&text, &issues) {
                text = new;
            }
        }
        out.push(json!({"file": c["file"], "line": c["line"], "text": text}));
    }
    json!({"items": out})
}
