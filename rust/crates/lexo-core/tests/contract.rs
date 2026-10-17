//! The interface table of Rust Core is the table of the Python Core (docs/API.md): `core/contract/api.json` is generated from the Python one and must equal `api::describe()`.
use lexo_core::api;
use serde_json::Value;

#[test]
fn the_table_equals_the_contract_of_the_python_core() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../core/contract/api.json");
    let want: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let got = api::describe();
    let (want, got) = (want.as_array().unwrap(), got.as_array().unwrap());
    let names = |v: &[Value]| v.iter().map(|o| o["name"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(names(got), names(want), "the operations differ");
    for (g, w) in got.iter().zip(want) {
        assert_eq!(g, w, "operation {}", g["name"]);
    }
}

// ---------------------------------------------------------------- the shapes of the answers (core/contract/shapes.json)
use lexo_core::app::App;
use lexo_core::config::Config;
use lexo_core::lang::Packs;
use lexo_core::messages::render;
use serde_json::{json, Map};
use std::path::{Path, PathBuf};

const DYNAMIC_AFTER: usize = 25; // an object with more keys than this is a map of unknown keys (texts, names), not a record
const MAX_DEPTH: usize = 5;

const GAME_SCRIPT: &str = "label start:\n    scene room\n    show anna smile at left\n    mc \"Hello there, friend.\"\n    menu:\n        \"Invite her\":\n            mc \"Coffee?\"\n        \"Leave\":\n            pass\n    mc \"Bye.\"\n";
const TRANSLATION: &str = "# game/script.rpy:4\ntranslate czech a1:\n\n    # mc \"Hello there, friend.\"\n    mc \"Ahoj tam, příteli.\"\n\n# game/script.rpy:7\ntranslate czech a2:\n\n    # mc \"Coffee?\"\n    mc \"Coffee?\"\n\n# game/script.rpy:10\ntranslate czech a3:\n\n    # mc \"Bye.\"\n    de \"Nashle.\"\n\ntranslate czech strings:\n\n    # game/script.rpy:5\n    old \"Invite her\"\n    new \"Pozvat ji\"\n\n    # game/script.rpy:5\n    old \"Leave\"\n    new \"Leave\"\n";

fn shape(value: &Value, depth: usize) -> Value {
    match value {
        Value::Null => json!({"type": "null"}),
        Value::Bool(_) => json!({"type": "boolean"}),
        Value::Number(_) => json!({"type": "number"}),
        Value::String(_) => json!({"type": "string"}),
        Value::Object(o) => {
            if depth >= MAX_DEPTH {
                json!({"type": "object"})
            } else if o.len() > DYNAMIC_AFTER {
                json!({"type": "object", "dynamic": true})
            } else {
                let keys: Map<String, Value> = o.iter().map(|(k, v)| (k.clone(), shape(v, depth + 1))).collect();
                json!({"type": "object", "keys": keys})
            }
        }
        Value::Array(a) => {
            if depth >= MAX_DEPTH {
                json!({"type": "array"})
            } else {
                json!({"type": "array", "items": a.first().map_or(Value::Null, |v| shape(v, depth + 1))})
            }
        }
    }
}

/// Problems where `live` no longer fits `recorded`: a key that vanished, a type that changed. New keys are fine.
fn compatible(recorded: &Value, live: &Value, path: &str) -> Vec<String> {
    if recorded.is_null() || live.is_null() {
        return Vec::new();
    }
    let (rt, lt) = (recorded["type"].as_str().unwrap_or(""), live["type"].as_str().unwrap_or(""));
    if rt == "null" || lt == "null" {
        return Vec::new(); // an optional value that was empty (or is empty now)
    }
    if rt != lt {
        return vec![format!("{path}: was {rt}, is {lt}")];
    }
    let mut out = Vec::new();
    if rt == "object" {
        if let (Some(r), Some(l)) = (recorded.get("keys").and_then(Value::as_object), live.get("keys").and_then(Value::as_object)) {
            for (k, sub) in r {
                match l.get(k) {
                    None => out.push(format!("{path}.{k}: missing")),
                    Some(ls) => out.extend(compatible(sub, ls, &format!("{path}.{k}"))),
                }
            }
        }
    }
    if rt == "array" {
        out.extend(compatible(&recorded["items"], &live["items"], &format!("{path}[]")));
    }
    out
}

fn fixture(tmp: &Path) -> PathBuf {
    let tl = tmp.join("g/game/tl/czech");
    std::fs::create_dir_all(&tl).unwrap();
    std::fs::write(tmp.join("g/game/script.rpy"), GAME_SCRIPT).unwrap();
    std::fs::write(tl.join("script.rpy"), TRANSLATION).unwrap();
    tl
}

#[test]
fn every_operation_answers_in_the_recorded_shape() {
    let tmp = std::env::temp_dir().join(format!("lexo-contract-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut config = Config::new(Some(&tmp.join("cfg/config.json")));
    config.load();
    let mut app = App::new(config, Packs { bundled: repo.join("core/lexo/packs"), user: Some(tmp.join("cfg/packs")) });
    app.open_project(&fixture(&tmp).to_string_lossy(), false).unwrap();
    api::call(&mut app, "glossary/set", &json!({"items": [{"code": "friend", "name": "pritel", "type": "term"}]})).unwrap(); // the example of glossary/occurrences
    let recorded: Value = serde_json::from_str(&std::fs::read_to_string(repo.join("core/contract/shapes.json")).unwrap()).unwrap();
    let mut ops: Vec<Value> = api::describe().as_array().unwrap().clone();
    ops.sort_by_key(|o| (o["effect"] != "read", o["name"].as_str().unwrap().to_string()));
    let mut problems: Vec<String> = Vec::new();
    let mut seen = 0;
    for op in ops {
        if op["example"].is_null() || op["effect"] == "external" || op["kind"] == "binary" {
            continue;
        }
        let name = op["name"].as_str().unwrap();
        let answer = match api::call(&mut app, name, &op["example"]) {
            Ok(a) => render(a.json().unwrap()),
            Err(e) => {
                problems.push(format!("{name}: {e}"));
                continue;
            }
        };
        seen += 1;
        match recorded.get(name) {
            Some(r) => problems.extend(compatible(r, &shape(&answer, 0), &format!("{name} $"))),
            None => problems.push(format!("{name}: no recorded shape")),
        }
    }
    app.close();
    let _ = std::fs::remove_dir_all(&tmp);
    assert_eq!(seen, recorded.as_object().unwrap().len(), "the number of operations that were run");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
