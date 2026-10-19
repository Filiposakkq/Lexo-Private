//! Core + the Pro pack together: spelling with a Hunspell dictionary (add the dictionary, check, plan a correction, write it, ignore a word).
use lexo_core::app::App;
use lexo_core::config::Config;
use lexo_core::lang::Packs;
use lexo_pro::LexoProProvider;
use serde_json::json;
use std::path::Path;

fn block(id: &str, src: &str, target: &str) -> String {
    format!("translate english {id}:\n\n    # e \"{src}\"\n    e \"{target}\"\n\n")
}

#[test]
fn unknown_words_are_listed_corrected_and_ignored() {
    let tmp = std::env::temp_dir().join(format!("spell-core-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    // the settings folder of this test (the dictionary is copied there) and no licence file: the development build counts as licensed
    std::env::set_var("LEXO_CONFIG_DIR", tmp.join("settings"));
    std::env::set_var("LEXO_LICENCE_FILE", std::env::temp_dir().join("aside-spell-no-licence.json"));
    let tl = tmp.join("p/game/tl/english");
    std::fs::create_dir_all(&tl).unwrap();
    std::fs::write(
        tl.join("script.rpy"),
        block("a_1", "Ahoj [hero], jak se máš?", "Helo [hero], how are you?")
            + &block("a_2", "Dobře.", "I am fine, thank you. Helo again!")
            + &block("a_3", "Cože?", "What is a flurb?")
            + &block("a_4", "Pozor.", "Be careful, Zoltan!"),
    )
    .unwrap();
    // a small dictionary with a suffix rule
    let dict = tmp.join("dict");
    std::fs::create_dir_all(&dict).unwrap();
    std::fs::write(dict.join("en_US.aff"), "SET UTF-8\nTRY esianrtolcdugmphbyfvkwz\nSFX S Y 1\nSFX S 0 s .\n").unwrap();
    std::fs::write(dict.join("en_US.dic"), "14\nHello\nhow\nare\nyou\nI\nam\nfine\nthank\nagain\nWhat\nis\na\nBe\ncareful/S\n").unwrap();
    std::fs::write(dict.join("hyph_en_US.dic"), "ignored").unwrap();

    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut config = Config::new(Some(&tmp.join("cfg/config.json")));
    config.load();
    let mut app = App::new(config, Packs { bundled: repo.join("rust/resources/packs"), user: None });
    app.ext.register(Box::new(LexoProProvider::new())).unwrap();
    app.open_project(&tl.to_string_lossy(), false).expect("the project opens");

    // without a dictionary there is nothing to check against
    let state = app.spell_state().unwrap();
    assert_eq!((state["available"].clone(), state["installed"].clone()), (json!(true), json!(false)), "{state}");
    assert!(app.spell_check("", &json!(null)).is_err());
    assert!(app.spell_dict_add(&tmp.join("nowhere").to_string_lossy()).is_err());

    // the folder holds one dictionary (the hyphenation file is not one)
    app.spell_dict_add(&dict.to_string_lossy()).unwrap();
    assert_eq!(app.spell_state().unwrap()["name"], "en_US");

    let r = app.spell_check("", &json!(null)).unwrap();
    let words: Vec<String> = r["items"].as_array().unwrap().iter().map(|i| i["word"].as_str().unwrap().to_string()).collect();
    // "Helo" twice (first: the most frequent), then "flurb" and "Zoltan" in alphabetical order; "hero" is only in a variable
    assert_eq!(words, vec!["Helo", "flurb", "Zoltan"], "{r}");
    assert_eq!(r["items"][0]["count"], 2);
    assert_eq!(r["items"][0]["suggestions"][0], "Hello");

    // a correction is planned first: the word whole, never inside [hero]
    let plan = app.spell_replace_plan("Helo", "Hello").unwrap();
    assert_eq!(plan["total"], 2);
    let edits: Vec<serde_json::Value> =
        plan["items"].as_array().unwrap().iter().map(|i| json!({"file": i["file"], "line": i["line"], "text": i["after"]})).collect();
    assert_eq!(edits[0]["text"], "Hello [hero], how are you?");
    let done = app.mt_apply(&json!(edits)).unwrap();
    assert_eq!(done["changed"], 2, "{done}");
    assert!(std::fs::read_to_string(tl.join("script.rpy")).unwrap().contains("Hello [hero], how are you?"));

    // a name the user ignores is not reported again (and the list is kept in the project)
    app.spell_ignore(&json!(["Zoltan"])).unwrap();
    let r = app.spell_check("", &json!(null)).unwrap();
    let words: Vec<&str> = r["items"].as_array().unwrap().iter().map(|i| i["word"].as_str().unwrap()).collect();
    assert_eq!(words, vec!["flurb"]);
    assert!(tl.join(".aside/pro/spelling.json").is_file());

    app.spell_dict_remove().unwrap();
    assert_eq!(app.spell_state().unwrap()["installed"], false);
}
