//! Style check with a language model (the Pro feature `style.check`): the translation of a line is read together with its original, the profile of the speaker (gender, how they speak, who
//! they address formally) and the lines before it, and the model says whether the form of address, the gender forms, the tone or the meaning are wrong, and gives a corrected text.
//!
//! Core chooses the lines, gathers the context and the profile and later checks and writes what the user ticks; this module cuts the list to the limit of the licence, asks the model and
//! reads its answer. It uses the service the user set up for the machine translation (an OpenAI-compatible one; DeepL and LibreTranslate are not language models). Plain data in, plain data out.
use crate::licence;
use crate::profiles;
use lexo_core::extensions::ExtensionError;
use lexo_core::messages::{msg, py_str, Text};
use lexo_core::mt::{self, Http};
use serde_json::{json, Map, Value};

/// `{items: [{file, line}], total, chars, limited?}`: the candidates `{file, line, chars}` cut to the limit.
pub fn targets(p: &Value) -> Value {
    let all: Vec<Value> = p["items"].as_array().cloned().unwrap_or_default();
    let total = all.len();
    let (items, cut) = licence::cap("style.check", all);
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

const SYSTEM: &str = "You check the style of a translation of dialogue from a visual novel (Ren'Py), from {src} to {dst}. \
Look only for these problems: (address) the form of address (formal or informal) is wrong or inconsistent with the profile; (gender) masculine or feminine forms are wrong for the speaker \
or for the person spoken to; (tone) the tone or the voice does not fit the character; (meaning) the meaning of the original is lost or changed. Do not comment on taste or on \
small alternatives. Keep every tag and variable of the translation, such as [name], {i} or %(x)s, exactly as it is. \
Answer ONLY with a JSON object, nothing else: {\"ok\": true} when there is nothing to correct, otherwise \
{\"ok\": false, \"issues\": [{\"kind\": \"address|gender|tone|meaning\", \"explanation\": \"one short sentence in {dst}\"}], \"fix\": \"the corrected translation\"}.";

/// The JSON object of an answer of the model (it may wrap it in text or in a code block).
fn json_of(answer: &str) -> Option<Value> {
    let (a, b) = (answer.find('{')?, answer.rfind('}')?);
    serde_json::from_str(answer.get(a..=b)?).ok()
}

/// One line: the model's verdict `{ok, issues: [{kind, explanation}], fix}`.
fn check_one(cfg: &Map<String, Value>, r: &Value, http: Http, profiles_allowed: bool) -> Result<Value, mt::MtError> {
    let get = |k: &str| cfg.get(k).map(py_str).unwrap_or_default();
    let base = {
        let b = get("base_url");
        if b.trim().is_empty() {
            "https://api.openai.com/v1".to_string()
        } else {
            mt::check_url(b.trim())?
        }
    };
    let src = Some(get("source_lang")).filter(|s| !s.is_empty()).unwrap_or_else(|| "EN".into());
    let dst = get("target_lang");
    let mut system = SYSTEM.replace("{src}", &src).replace("{dst}", &dst);
    // the profile of the speaker (a premium feature of its own: without it the profile is not used)
    let profile = if profiles_allowed && r["profile"].is_object() { profiles::describe(&r["profile"]) } else { String::new() };
    if !profile.trim().is_empty() {
        system += &format!(" About the speaker: {}", profile.trim());
    }
    let context: Vec<String> = r["context"].as_array().into_iter().flatten().map(py_str).collect();
    let user = if context.is_empty() { String::new() } else { format!("Lines before (original, for context):\n{}\n\n", context.join("\n")) }
        + &format!("Speaker: {}\nOriginal: {}\nTranslation: {}", py_str(&r["who"]), py_str(&r["source"]), py_str(&r["target"]));
    let model = Some(get("model")).filter(|m| !m.is_empty()).unwrap_or_else(|| "gpt-4o-mini".into());
    let body = json!({"model": model, "temperature": 0, "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}]});
    let mut headers = vec![("Content-Type".to_string(), "application/json".to_string())];
    let key = get("api_key");
    if !key.trim().is_empty() {
        headers.push(("Authorization".to_string(), format!("Bearer {}", key.trim())));
    }
    let res = mt::call(&format!("{base}/chat/completions"), &headers, body.to_string().as_bytes(), http)?;
    let content = res["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string();
    let verdict = json_of(&content).ok_or_else(|| mt::MtError(msg("The answer of the service does not have the expected shape.", &[])))?;
    let ok = verdict["ok"].as_bool().unwrap_or(false);
    let issues: Vec<Value> = verdict["issues"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|i| {
            let kind = py_str(&i["kind"]);
            let kind = if ["address", "gender", "tone", "meaning"].contains(&kind.as_str()) { kind } else { "other".to_string() };
            let explanation = py_str(&i["explanation"]);
            (!explanation.trim().is_empty()).then(|| json!({"kind": kind, "explanation": explanation.trim()}))
        })
        .collect();
    let fix = py_str(&verdict["fix"]);
    // a "fix" that is the translation itself, or that comes without an issue, is not a fix
    let fix = if ok || issues.is_empty() || fix.trim() == py_str(&r["target"]).trim() { String::new() } else { fix.trim().to_string() };
    Ok(json!({"ok": ok || issues.is_empty(), "issues": issues, "fix": fix}))
}

/// Verdicts for the requests `[{id, who, source, target, context, profile}]` with the settings `cfg`. A failed line carries its own error; a rejected key, missing consent, a service that is
/// not a language model or a broken connection stops the batch (the same message for every line would be useless).
pub fn check(p: &Value, http: Http) -> Result<Value, ExtensionError> {
    check_with(p, http, licence::offer("mt.profiles").is_some())
}

fn check_with(p: &Value, http: Http, profiles_allowed: bool) -> Result<Value, ExtensionError> {
    let plain = |t: &str| ExtensionError(Text::Plain(t.to_string()));
    let cfg = p["cfg"].as_object().cloned().ok_or_else(|| plain("The request has no settings."))?;
    let provider = py_str(&cfg.get("provider").cloned().unwrap_or(Value::Null));
    if provider != "openai" {
        return Err(ExtensionError(Text::new_msg(msg(
            "The style check needs a language model: choose the OpenAI-compatible service in Settings → Machine translation.",
            &[],
        ))));
    }
    if !cfg.get("consent").is_some_and(lexo_core::util::truthy) {
        return Err(ExtensionError(Text::new_msg(msg("Missing consent to send text to an external service (Settings → Machine translation).", &[]))));
    }
    let mut out: Vec<Value> = Vec::new();
    for r in p["requests"].as_array().into_iter().flatten() {
        match check_one(&cfg, r, http, profiles_allowed) {
            Err(e) => {
                let err = e.0.wire();
                out.push(json!({"id": r["id"], "error": err}));
                return Ok(json!({"results": out, "stopped": err}));
            }
            Ok(v) => out.push(json!({"id": r["id"], "ok": v["ok"], "issues": v["issues"], "fix": v["fix"]})),
        }
    }
    Ok(json!({"results": out, "stopped": ""}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[allow(clippy::type_complexity)]
    fn model(answers: Vec<&str>) -> (impl Fn(&str, &[(String, String)], &[u8]) -> Result<(u16, String), mt::MtError>, std::rc::Rc<RefCell<Vec<String>>>) {
        let seen = std::rc::Rc::new(RefCell::new(Vec::new()));
        let (log, queue) = (seen.clone(), RefCell::new(answers.into_iter().map(String::from).collect::<Vec<_>>()));
        (
            move |_u: &str, _h: &[(String, String)], body: &[u8]| {
                log.borrow_mut().push(String::from_utf8_lossy(body).into_owned());
                let a = queue.borrow_mut().remove(0);
                Ok((200, json!({"choices": [{"message": {"content": a}}]}).to_string()))
            },
            seen,
        )
    }

    fn cfg() -> Value {
        json!({"provider": "openai", "consent": true, "api_key": "k", "target_lang": "cs", "base_url": "https://llm.example/v1"})
    }

    fn req(target: &str) -> Value {
        json!({"id": 0, "who": "li", "source": "Hello, Alex.", "target": target, "context": ["Hi."], "profile": {"speaker": {"code": "li", "name": "Lisa"}, "gender": "", "style": "", "to": {"Alex": "informal"}}})
    }

    #[test]
    fn a_verdict_with_issues_and_a_fix_is_read_even_when_the_model_wraps_it() {
        let (http, seen) = model(vec!["Sure!\n```json\n{\"ok\": false, \"issues\": [{\"kind\": \"address\", \"explanation\": \"Má tykat.\"}, {\"kind\": \"weird\", \"explanation\": \"x\"}], \"fix\": \"Ahoj, Alexi.\"}\n```"]);
        let out = check_with(&json!({"cfg": cfg(), "requests": [req("Dobrý den, Alexi.")]}), &http, true).unwrap();
        let r = &out["results"][0];
        assert_eq!((r["ok"].clone(), r["fix"].clone()), (json!(false), json!("Ahoj, Alexi.")));
        assert_eq!(r["issues"][0], json!({"kind": "address", "explanation": "Má tykat."}));
        assert_eq!(r["issues"][1]["kind"], "other"); // an unknown kind is "other"
        let body: Value = serde_json::from_str(&seen.borrow()[0]).unwrap();
        let (system, user) = (body["messages"][0]["content"].as_str().unwrap(), body["messages"][1]["content"].as_str().unwrap());
        assert!(system.contains("from EN to cs") && system.contains("Lisa addresses Alex in the informal form"), "{system}");
        assert!(user.contains("Original: Hello, Alex.") && user.contains("Translation: Dobrý den, Alexi.") && user.contains("Hi."), "{user}");
    }

    #[test]
    fn nothing_to_correct_and_a_fix_that_is_the_translation_itself_give_no_fix() {
        let (http, _) = model(vec![
            "{\"ok\": true}",
            "{\"ok\": false, \"issues\": [{\"kind\": \"tone\", \"explanation\": \"x\"}], \"fix\": \"Ahoj, Alexi.\"}",
            "{\"ok\": false, \"issues\": [], \"fix\": \"Zbytečné.\"}",
        ]);
        let out = check_with(&json!({"cfg": cfg(), "requests": [req("a"), req("Ahoj, Alexi."), req("b")]}), &http, true).unwrap();
        let r = out["results"].as_array().unwrap();
        assert_eq!((r[0]["ok"].clone(), r[0]["fix"].clone()), (json!(true), json!("")));
        assert_eq!(r[1]["fix"], ""); // the same as the translation
        assert_eq!((r[2]["ok"].clone(), r[2]["fix"].clone()), (json!(true), json!("")));
        // a "fix" without an issue
    }

    #[test]
    fn it_needs_a_language_model_and_the_consent_and_stops_on_an_answer_that_is_not_a_verdict() {
        let (http, _) = model(vec!["I think it is fine."]);
        let mut c = cfg();
        c["provider"] = json!("deepl");
        assert!(check_with(&json!({"cfg": c, "requests": [req("a")]}), &http, true).is_err());
        let mut c = cfg();
        c["consent"] = json!(false);
        assert!(check_with(&json!({"cfg": c, "requests": [req("a")]}), &http, true).is_err());
        let out = check_with(&json!({"cfg": cfg(), "requests": [req("a"), req("b")]}), &http, true).unwrap();
        assert_eq!(out["results"].as_array().unwrap().len(), 1, "the batch stops at the first answer that is not a verdict");
        assert!(out["stopped"].is_object() || out["stopped"].is_string());
    }
}
