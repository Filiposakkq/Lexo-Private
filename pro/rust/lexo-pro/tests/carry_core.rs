//! Core + the Pro pack together: a translation is carried over from an old version folder into a new one, as one undoable change.
use lexo_core::app::App;
use lexo_core::config::Config;
use lexo_core::lang::Packs;
use lexo_pro::LexoProProvider;
use serde_json::json;
use std::path::Path;

fn write(dir: &Path, text: &str) {
    let tl = dir.join("game/tl/czech");
    std::fs::create_dir_all(&tl).unwrap();
    std::fs::write(tl.join("script.rpy"), text).unwrap();
}

fn block(id: &str, src: &str, target: &str) -> String {
    format!("translate czech {id}:\n\n    # e \"{src}\"\n    e \"{target}\"\n\n")
}

#[test]
fn a_translation_is_carried_over_to_the_new_version_and_undone() {
    let tmp = std::env::temp_dir().join(format!("carry-core-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let (old, new) = (tmp.join("old"), tmp.join("new"));
    write(
        &old,
        &(block("a_1", "Good morning, everyone, how are you today?", "Dobré ráno všem, jak se dnes máte?")
            + &block("a_2", "We should go to the old library after school.", "Měli bychom po škole jít do staré knihovny.")
            + &block("a_3", "Nothing like this exists in the new version.", "Nic takového v nové verzi není.")),
    );
    write(
        &new,
        &(block("b_9", "Good morning, everyone, how are you today?", "")
            + &block("b_8", "We should go to the old library after the school.", "")
            + &block("b_7", "A completely different sentence appears here.", "")),
    );
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut config = Config::new(Some(&tmp.join("cfg/config.json")));
    config.load();
    let mut app = App::new(config, Packs { bundled: repo.join("core/lexo/packs"), user: None });
    app.ext.register(Box::new(LexoProProvider::new())).unwrap();
    app.open_project(&new.join("game/tl/czech").to_string_lossy(), false).expect("the new version opens");

    let old_path = old.join("game/tl/czech").to_string_lossy().into_owned();
    let preview = app.carry_preview(&old_path, 0.8).unwrap();
    assert_eq!(preview["counts"], json!({"exact": 1, "near": 1, "none": 1, "total": 3}), "{preview}");
    let items = preview["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|i| i["file"] == "script.rpy" && i["line"].as_i64().unwrap() > 0 && !i["old_source"].as_str().unwrap().is_empty()));

    let chosen: Vec<_> = items.iter().map(|i| json!({"file": i["file"], "line": i["line"]})).collect();
    let done = app.carry_apply(&old_path, 0.8, &chosen).unwrap();
    assert_eq!(done["changed"], 2, "{done}");
    let text = std::fs::read_to_string(new.join("game/tl/czech/script.rpy")).unwrap();
    assert!(text.contains("Dobré ráno všem, jak se dnes máte?") && text.contains("Měli bychom po škole jít do staré knihovny."));
    assert!(!text.contains("Nic takového"));

    // nothing is left to carry over for those lines, and the open project is not its own old version
    let again = app.carry_preview(&old_path, 0.8).unwrap();
    assert_eq!(again["items"].as_array().unwrap().len(), 0);
    let same = app.carry_preview(&new.join("game/tl/czech").to_string_lossy(), 0.8);
    assert!(same.is_err());
    let _ = std::fs::remove_dir_all(&tmp);
}
