//! The library of games on Home: a project that is opened is on it, a folder can be added by hand, pinned and removed, and nothing of the game is touched.
use lexo_core::app::App;
use lexo_core::config::Config;
use lexo_core::lang::Packs;
use serde_json::Value;
use std::path::Path;

fn app_in(tmp: &Path) -> App {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut config = Config::new(Some(&tmp.join("cfg/config.json")));
    config.load();
    App::new(config, Packs { bundled: repo.join("rust/resources/packs"), user: None })
}

fn names(v: &Value) -> Vec<String> {
    v["games"].as_array().unwrap().iter().map(|g| g["name"].as_str().unwrap().to_string()).collect()
}

#[test]
fn games_are_on_home_when_opened_or_added_and_can_be_pinned_and_removed() {
    let tmp = std::env::temp_dir().join(format!("library-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    // a game with a translation, and a game without one (only its scripts)
    let ready = tmp.join("Ready Game");
    let tl = ready.join("game/tl/czech");
    std::fs::create_dir_all(&tl).unwrap();
    std::fs::write(ready.join("game/options.rpy"), "define config.name = _(\"Ready Game\")\ndefine config.version = \"0.14\"\n").unwrap();
    std::fs::write(tl.join("script.rpy"), "translate czech a_1:\n\n    # e \"Hello.\"\n    e \"Ahoj.\"\n\n    # e \"Bye.\"\n    e \"Bye.\"\n\n").unwrap();
    let bare = tmp.join("Bare Game");
    std::fs::create_dir_all(bare.join("game")).unwrap();
    std::fs::write(bare.join("game/script.rpy"), "label start:\n    e \"Hi\"\n").unwrap();

    let mut app = app_in(&tmp);
    assert!(names(&app.library_list().unwrap()).is_empty(), "nothing on Home at first");

    // a game folder that is added by hand: only a card; a folder that is not there is refused
    assert!(app.library_add(&tmp.join("nowhere").to_string_lossy()).is_err());
    let l = app.library_add(&bare.to_string_lossy()).unwrap();
    assert_eq!((l["games"][0]["name"].clone(), l["games"][0]["state"].clone()), ("Bare Game".into(), "needs_prepare".into()), "{l}");
    assert_eq!(app.library_add(&bare.to_string_lossy()).unwrap()["games"].as_array().unwrap().len(), 1, "added once");

    // opening a project puts it on Home with its numbers; the card of a game folder added by hand and the translation folder opened later are one game
    let l = app.library_add(&ready.to_string_lossy()).unwrap();
    assert_eq!(l["games"].as_array().unwrap().len(), 2);
    app.open_project(&tl.to_string_lossy(), false).unwrap();
    let l = app.library_list().unwrap();
    let g = l["games"].as_array().unwrap().iter().find(|g| g["name"] == "Ready Game").expect("the opened game is on Home");
    assert_eq!(
        (g["state"].clone(), g["version"].clone(), g["language"].clone(), g["total"].clone(), g["done"].clone()),
        ("ready".into(), "0.14".into(), "czech".into(), 2.into(), 1.into()),
        "{g}"
    );
    assert_eq!(l["games"].as_array().unwrap().len(), 2, "one card per game: {l}");
    assert_eq!(names(&l)[0], "Ready Game", "the one that was worked on is first");

    // pinned ones come first; removing takes only the card
    let l = app.library_pin(&bare.to_string_lossy(), true).unwrap();
    assert_eq!(names(&l)[0], "Bare Game", "{l}");
    // a card is found by the path as the user may write it (with a `..`, a short Windows name), not only by the exact text that was stored
    let dotted = tmp.join("Ready Game").join("..").join("Bare Game");
    let l = app.library_pin(&dotted.to_string_lossy(), false).unwrap();
    assert_eq!(names(&l)[0], "Ready Game", "unpinned through another spelling of the path: {l}");
    let l = app.library_pin(&dotted.to_string_lossy(), true).unwrap();
    assert_eq!(names(&l)[0], "Bare Game", "{l}");
    let l = app.library_remove(&bare.to_string_lossy()).unwrap();
    assert_eq!(names(&l), vec!["Ready Game"]);
    assert!(bare.join("game/script.rpy").is_file(), "no file of the game is touched");

    // a game whose folder is gone is still listed, as missing
    std::fs::remove_dir_all(&ready).unwrap();
    assert_eq!(app.library_list().unwrap()["games"][0]["state"], "missing");
}
