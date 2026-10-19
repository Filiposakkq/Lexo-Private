//! The Rust pack gives the answers the Python pack gave (`testdata/pro-golden/pack.json`, made by `reference/python/pro/tests/golden_pro.py`).
use lexo_core::extensions::Provider;
use lexo_core::messages::render;
use lexo_core::mt::MtError;
use lexo_pro::LexoProProvider;
use serde_json::{json, Value};
use std::path::Path;

/// `urllib.parse.unquote_plus`.
fn unquote_plus(s: &str) -> String {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() => {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The network of the machine translation cases ("CZ " + the text; the key "bad" is refused): the same function as `fake_http` of `golden_pro.py`.
fn fake_http(url: &str, headers: &[(String, String)], data: &[u8]) -> Result<(u16, String), MtError> {
    let body = String::from_utf8_lossy(data).into_owned();
    let key = headers.iter().find(|(k, _)| k == "Authorization").map(|(_, v)| v.clone()).unwrap_or_default();
    if key.contains("bad") || body.contains("\"api_key\": \"bad\"") {
        return Ok((403, "forbidden".into()));
    }
    let answer = if url.contains("deepl") {
        let text = body.split('&').find_map(|p| p.strip_prefix("text=")).unwrap_or("");
        let text = unquote_plus(text);
        json!({"translations": [{"text": format!("CZ {text}")}]})
    } else if url.ends_with("/chat/completions") {
        let v: Value = serde_json::from_str(&body).unwrap();
        let msgs = v["messages"].as_array().unwrap();
        let user = msgs[msgs.len() - 1]["content"].as_str().unwrap();
        json!({"choices": [{"message": {"content": format!("CZ {}", user.split_once("Translate:
").unwrap().1)}}]})
    } else {
        let v: Value = serde_json::from_str(&body).unwrap();
        json!({"translatedText": format!("CZ {}", v["q"].as_str().unwrap())})
    };
    Ok((200, answer.to_string()))
}

fn golden() -> Value {
    // the licence file of the machine must not matter: without one the development default offers everything
    std::env::set_var("LEXO_LICENCE_FILE", std::env::temp_dir().join("aside-golden-no-licence.json"));
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../testdata/pro-golden/pack.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The first place where two values differ (numbers compare as numbers: 1 and 1.0 are the same here).
fn first_difference(got: &Value, want: &Value, path: &str) -> Option<String> {
    match (got, want) {
        (Value::Object(g), Value::Object(w)) => {
            for k in g.keys().chain(w.keys()) {
                match (g.get(k), w.get(k)) {
                    (Some(a), Some(b)) => {
                        if let Some(d) = first_difference(a, b, &format!("{path}.{k}")) {
                            return Some(d);
                        }
                    }
                    _ => return Some(format!("{path}.{k}: missing on one side")),
                }
            }
            None
        }
        (Value::Array(g), Value::Array(w)) => {
            if g.len() != w.len() {
                return Some(format!("{path}: {} items, expected {}", g.len(), w.len()));
            }
            g.iter().zip(w).enumerate().find_map(|(i, (a, b))| first_difference(a, b, &format!("{path}[{i}]")))
        }
        (Value::Number(a), Value::Number(b)) => (a.as_f64() != b.as_f64()).then(|| format!("{path}: {a}, expected {b}")),
        (a, b) => (a != b).then(|| format!("{path}: {a}, expected {b}")),
    }
}

// the demo mode is a variable of the process: everything runs in one test, one case after the other
#[test]
fn every_case_gives_the_answer_of_the_python_pack() {
    let g = golden();
    let provider = LexoProProvider::with_http(fake_http);
    let mut failures: Vec<String> = Vec::new();
    for case in g["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let env = case["env"].as_object().cloned().unwrap_or_default();
        for (k, v) in &env {
            std::env::set_var(k, v.as_str().unwrap());
        }
        let result = provider.call(case["feature"].as_str().unwrap(), case["method"].as_str().unwrap(), &case["payload"]);
        for k in env.keys() {
            std::env::remove_var(k);
        }
        let expect = &case["expect"];
        let verdict = match (&result, expect.get("answer"), expect.get("error")) {
            (Ok(got), Some(want), _) => first_difference(got, want, "$"),
            (Err(_), None, Some(_)) if id.starts_with("bad-") => None,
            (Err(e), None, Some(want)) => first_difference(&e.0.wire(), want, "$"),
            (Ok(_), None, Some(_)) => Some("an answer where Python gave an error".to_string()),
            (Err(e), Some(_), _) => Some(format!("an error ({}) where Python gave an answer", e.0.text())),
            _ => Some("the case has neither an answer nor an error".to_string()),
        };
        if let Some(d) = verdict {
            failures.push(format!("{id}: {d}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n{}",
        failures.len(),
        g["cases"].as_array().unwrap().len(),
        failures.iter().take(15).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn offers_and_texts_are_the_same() {
    let g = golden();
    let provider = LexoProProvider::new();
    std::env::remove_var("LEXO_PRO_DEMO");
    // the features that exist only in the Rust pack (the Python pack, which the recorded answers come from, does not have them)
    let mut offers = provider.offers();
    offers.remove("carry.over");
    offers.remove("memory.fuzzy");
    offers.remove("mt.profiles");
    offers.remove("style.check");
    offers.remove("release.build");
    offers.remove("spell.hunspell");
    offers.remove("team.manage");
    assert_eq!(Value::Object(offers), g["offers"]["licensed"]);
    assert_eq!(Value::Object(provider.ui_strings("cs")), g["ui_cs"]);
    assert_eq!(Value::Object(provider.ui_strings("xx")), g["ui_xx"]);
    let _ = render(&Value::Null);
}
