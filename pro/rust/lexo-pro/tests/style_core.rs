//! Core + the pack together: the style check asks a (fake) language model about translated lines, with the profile of the speaker, and the ticked fix is written with `mt/apply`.
use lexo_core::api;
use lexo_core::app::App;
use lexo_core::config::Config;
use lexo_core::lang::Packs;
use lexo_core::mt::MtError;
use lexo_pro::LexoProProvider;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[test]
fn a_translated_line_is_checked_against_the_profile_and_the_fix_is_written() {
    std::env::set_var("LEXO_LICENCE_FILE", std::env::temp_dir().join("aside-style-no-licence.json")); // the licence file of the machine must not matter
    let tmp = std::env::temp_dir().join(format!("style-core-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let tl = tmp.join("game/tl/czech");
    std::fs::create_dir_all(&tl).unwrap();
    std::fs::write(
        tl.join("script.rpy"),
        "translate czech a_1:\n\n    # li \"Hello, Alex.\"\n    li \"Dobrý den, Alexi.\"\n\ntranslate czech a_2:\n\n    # li \"Bye, Alex.\"\n    li \"Bye, Alex.\"\n\n",
    )
    .unwrap();
    // a fake language model: it records what it was asked and always finds the same problem
    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let fake = move |_u: &str, _h: &[(String, String)], body: &[u8]| -> Result<(u16, String), MtError> {
        log.lock().unwrap().push(String::from_utf8_lossy(body).into_owned());
        let verdict = json!({"ok": false, "issues": [{"kind": "address", "explanation": "Lisa tyká Alexovi."}], "fix": "Ahoj, Alexi."});
        Ok((200, json!({"choices": [{"message": {"content": verdict.to_string()}}]}).to_string()))
    };
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut config = Config::new(Some(&tmp.join("cfg/config.json")));
    config.load();
    for (k, v) in [
        ("provider", json!("openai")),
        ("consent", json!(true)),
        ("api_key", json!("k")),
        ("base_url", json!("https://llm.example/v1")),
        ("target_lang", json!("cs")),
    ] {
        config.set(&format!("mt.{k}"), v);
    }
    let mut app = App::new(config, Packs { bundled: repo.join("core/lexo/packs"), user: None });
    app.ext.register(Box::new(LexoProProvider::with_http(fake))).unwrap();
    app.open_project(&tl.to_string_lossy(), false).expect("the project opens");
    app.profile_set("li", "f", "", &json!({"alex": "informal"})).unwrap();
    app.glossary_set(&[json!({"code": "alex", "name": "Alex", "type": "speaker", "note": ""})]).unwrap();

    // only the translated line is a target (the second one is the same as its original: not translated)
    let t = app.style_targets("", &Value::Null).unwrap();
    assert_eq!((t["total"].clone(), t["items"].as_array().unwrap().len()), (json!(1), 1), "{t}");
    let r = app.style_batch(&t["items"]).unwrap();
    let one = &r["results"][0];
    assert_eq!((one["ok"].clone(), one["fix"].clone()), (json!(false), json!("Ahoj, Alexi.")), "{r}");
    assert_eq!(one["remarks"][0]["kind"], "address");
    // the model was told who speaks (the profile of Lisa) and what the original and the translation are
    let body: Value = serde_json::from_str(&seen.lock().unwrap()[0]).unwrap();
    let (system, user) = (body["messages"][0]["content"].as_str().unwrap(), body["messages"][1]["content"].as_str().unwrap());
    assert!(system.contains("is female") && system.contains("informal form"), "{system}");
    assert!(user.contains("Original: Hello, Alex.") && user.contains("Translation: Dobrý den, Alexi."), "{user}");
    // the ticked fix is written like a machine translation
    let w = api::call(&mut app, "mt/apply", &json!({"selections": [{"file": "script.rpy", "line": one["line"], "text": one["fix"]}]})).unwrap();
    assert_eq!(w.json().unwrap()["changed"], 1);
    assert!(std::fs::read_to_string(tl.join("script.rpy")).unwrap().contains("Ahoj, Alexi."));
    // a service that is not a language model is refused with a message
    app.config.set("mt.provider", json!("deepl"));
    assert!(app.style_batch(&json!([{"file": "script.rpy", "line": one["line"]}])).is_err());
    let _ = std::fs::remove_dir_all(&tmp);
}
