//! Profiles of characters: kept in the note of the glossary entry (hidden on the glossary page), sent with the batch machine translation as sentences for the language model.
use lexo_core::app::App;
use lexo_core::config::Config;
use lexo_core::extensions::Provider;
use lexo_core::lang::Packs;
use lexo_core::mt::MtError;
use lexo_pro::LexoProProvider;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::{Arc, Mutex};

fn no_licence_file() {
    // the licence file of the machine must not matter: without one the development default offers everything
    std::env::set_var("LEXO_LICENCE_FILE", std::env::temp_dir().join("aside-profiles-no-licence.json"));
}

fn open(tag: &str) -> (App, std::path::PathBuf) {
    no_licence_file();
    let tmp = std::env::temp_dir().join(format!("profiles-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let tl = tmp.join("game/tl/czech");
    std::fs::create_dir_all(&tl).unwrap();
    std::fs::write(tl.join("script.rpy"), "translate czech a_1:\n\n    # li \"Hello there, Alex.\"\n    li \"Hello there, Alex.\"\n\n").unwrap();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut config = Config::new(Some(&tmp.join("cfg/config.json")));
    config.load();
    let mut app = App::new(config, Packs { bundled: repo.join("rust/resources/packs"), user: None });
    app.ext.register(Box::new(LexoProProvider::new())).unwrap();
    app.open_project(&tl.to_string_lossy(), false).expect("the project opens");
    (app, tmp)
}

#[test]
fn a_profile_is_kept_in_the_note_hidden_on_the_glossary_page_and_survives_saving_the_note() {
    let (mut app, tmp) = open("note");
    app.glossary_set(&[json!({"code": "li", "name": "Lisa", "type": "speaker", "note": "The older sister."})]).unwrap();
    let out = app.profile_set("li", "f", "sarcastic", &json!({"alex": "informal", "bad": "rude"})).unwrap();
    assert_eq!(out["profile"], json!({"gender": "f", "style": "sarcastic", "to": {"alex": "informal"}}));
    // the glossary page sees only the free text...
    let g = app.glossary_payload().unwrap();
    let item = g["items"].as_array().unwrap().iter().find(|i| i["code"] == "li").unwrap();
    assert_eq!(item["note"], "The older sister.");
    // ...and saving that text from the page does not lose the profile
    app.glossary_set(&[json!({"code": "li", "name": "Lisa", "type": "speaker", "note": "The OLDER sister."})]).unwrap();
    let listed = app.profiles().unwrap();
    let li = listed["items"].as_array().unwrap().iter().find(|i| i["code"] == "li").unwrap();
    assert_eq!(li["profile"]["gender"], "f");
    assert_eq!(li["name"], "Lisa");
    // the file has the note and the line below it
    let csv = std::fs::read_to_string(tmp.join("game/tl/czech/.aside/glossary.csv")).unwrap();
    assert!(csv.contains("The OLDER sister.") && csv.contains("@profile gender=f; style=sarcastic; to:alex=informal"), "{csv}");
    // an empty profile removes only the line
    app.profile_set("li", "", "", &json!({})).unwrap();
    assert!(!std::fs::read_to_string(tmp.join("game/tl/czech/.aside/glossary.csv")).unwrap().contains("@profile"));
    assert_eq!(app.glossary_payload().unwrap()["items"].as_array().unwrap().iter().find(|i| i["code"] == "li").unwrap()["note"], "The OLDER sister.");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn the_batch_translation_sends_the_profile_of_the_speaker_to_the_language_model() {
    no_licence_file();
    // a fake model: it records the request and answers
    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let fake = move |_url: &str, _h: &[(String, String)], body: &[u8]| -> Result<(u16, String), MtError> {
        log.lock().unwrap().push(String::from_utf8_lossy(body).into_owned());
        Ok((200, json!({"choices": [{"message": {"content": "Ahoj, Alexi."}}]}).to_string()))
    };
    let p = LexoProProvider::with_http(fake);
    let cfg = json!({"provider": "openai", "consent": true, "api_key": "k", "target_lang": "cs", "base_url": "https://llm.example/v1"});
    let profile = json!({"speaker": {"code": "li", "name": "Lisa"}, "gender": "f", "style": "sarcastic", "to": {"Alex": "informal"}});
    let with = p
        .call(
            "mt.batch",
            "translate",
            &json!({"cfg": cfg, "requests": [{"id": 0, "source": "Hello there, Alex.", "context": [], "terms": {}, "profile": profile}]}),
        )
        .unwrap();
    assert_eq!(with["results"][0]["suggestion"], "Ahoj, Alexi.");
    let body: Value = serde_json::from_str(&seen.lock().unwrap()[0]).unwrap();
    let system = body["messages"][0]["content"].as_str().unwrap();
    assert!(system.contains("Lisa, is female") && system.contains("sarcastic") && system.contains("addresses Alex in the informal form"), "{system}");
    // a request without a profile has none of it
    p.call("mt.batch", "translate", &json!({"cfg": cfg, "requests": [{"id": 0, "source": "Hello there, Alex.", "context": [], "terms": {}}]})).unwrap();
    let body: Value = serde_json::from_str(&seen.lock().unwrap()[1]).unwrap();
    assert!(!body["messages"][0]["content"].as_str().unwrap().contains("female"));
    let _ = p.offers();
}
