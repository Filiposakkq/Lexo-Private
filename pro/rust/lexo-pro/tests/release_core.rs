//! Core + the pack together: a release is built as a zip with the translation, the files attached, the texts of the pack; it is recorded; the next release knows what changed.
use lexo_core::app::App;
use lexo_core::config::Config;
use lexo_core::lang::Packs;
use lexo_pro::LexoProProvider;
use serde_json::json;
use std::io::Read;
use std::path::Path;

fn block(id: &str, src: &str, target: &str) -> String {
    format!("translate czech {id}:\n\n    # e \"{src}\"\n    e \"{target}\"\n\n")
}

fn names(zip: &Path) -> Vec<String> {
    let mut z = zip::ZipArchive::new(std::fs::File::open(zip).unwrap()).unwrap();
    let mut v: Vec<String> = (0..z.len()).map(|i| z.by_index(i).unwrap().name().to_string()).collect();
    v.sort();
    v
}

fn read(zip: &Path, name: &str) -> String {
    let mut z = zip::ZipArchive::new(std::fs::File::open(zip).unwrap()).unwrap();
    let mut s = String::new();
    z.by_name(name).unwrap().read_to_string(&mut s).unwrap();
    s
}

#[test]
fn a_release_is_built_as_a_zip_recorded_and_the_next_one_shows_what_changed() {
    std::env::set_var("LEXO_LICENCE_FILE", std::env::temp_dir().join("aside-release-no-licence.json")); // the licence file of the machine must not matter
    let tmp = std::env::temp_dir().join(format!("release-core-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let game = tmp.join("game");
    let tl = game.join("tl/czech");
    std::fs::create_dir_all(&tl).unwrap();
    std::fs::create_dir_all(tmp.join("fonts")).unwrap();
    std::fs::write(game.join("options.rpy"), "define config.name = _(\"Midnight Paradise\")\ndefine config.version = \"1.1\"\n").unwrap();
    std::fs::write(
        tl.join("script.rpy"),
        block("a_1", "Good morning, everyone.", "Dobré ráno všem.")
            + &block("a_2", "We should go to the library.", "Měli bychom jít do knihovny.")
            + &block("a_3", "Nothing yet.", "Nothing yet."),
    )
    .unwrap();
    std::fs::create_dir_all(tl.join(".aside")).ok();
    std::fs::write(tmp.join("fonts/DejaVu.ttf"), b"not really a font").unwrap();

    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut config = Config::new(Some(&tmp.join("cfg/config.json")));
    config.load();
    let mut app = App::new(config, Packs { bundled: repo.join("rust/resources/packs"), user: None });
    app.ext.register(Box::new(LexoProProvider::new())).unwrap();
    app.open_project(&tl.to_string_lossy(), false).expect("the project opens");

    // the game and the language are read, there is no release yet
    let state = app.release_state().unwrap();
    assert_eq!(
        (state["game"]["name"].clone(), state["game"]["version"].clone(), state["language"].clone()),
        (json!("Midnight Paradise"), json!("1.1"), json!("czech")),
        "{state}"
    );
    // attach a font, write the credits
    let font = tmp.join("fonts/DejaVu.ttf").to_string_lossy().into_owned();
    app.release_set(
        &json!({"extras": [{"from": font, "to": "game/tl/czech/fonts/DejaVu.ttf"}], "credits": "Anna, Tom", "out_dir": tmp.join("out").to_string_lossy()}),
    )
    .unwrap();

    // a bad version is refused, a good one is previewed with the warning about the line that is not translated
    let bad = app.release_preview("v1", "").unwrap();
    assert_eq!(bad["can_build"], false);
    let p = app.release_preview("1.0", "The first one.").unwrap();
    assert_eq!((p["can_build"].clone(), p["files"].clone(), p["extras"].clone()), (json!(true), json!(1), json!(1)), "{p}");
    assert!(p["checks"].as_array().unwrap().iter().any(|c| c["code"] == "untranslated"), "{p}");
    app.release_build("1.0", "The first one.", false).unwrap();
    let r = app.release_state().unwrap();
    assert_eq!(r["releases"].as_array().unwrap().len(), 1);

    // the zip: the texts, the language script, the translation, the font
    let zip = tmp.join("out/Midnight-Paradise-czech-v1.0.zip");
    assert_eq!(
        names(&zip),
        vec!["CHANGELOG.txt", "README.txt", "game/tl/czech/fonts/DejaVu.ttf", "game/tl/czech/script.rpy", "game/zz_czech_default_language.rpy"],
        "the .aside folder and the backups never go into a release"
    );
    assert!(read(&zip, "README.txt").contains("Jak to nainstalovat") && read(&zip, "README.txt").contains("Anna, Tom"));
    assert!(read(&zip, "CHANGELOG.txt").contains("First release.") && read(&zip, "CHANGELOG.txt").contains("The first one."));
    assert!(read(&zip, "game/zz_czech_default_language.rpy").contains("config.default_language = \"czech\""));
    assert_eq!(read(&zip, "game/tl/czech/script.rpy"), std::fs::read_to_string(tl.join("script.rpy")).unwrap(), "the translation goes in as it is");
    assert!(tmp.join("out/Midnight-Paradise-czech-v1.0.zip.sha256").is_file() && tmp.join("out/Midnight-Paradise-czech-v1.0-release-notes.md").is_file());
    let sha = std::fs::read_to_string(tmp.join("out/Midnight-Paradise-czech-v1.0.zip.sha256")).unwrap();
    assert_eq!(sha.split(' ').next().unwrap().len(), 64);

    // the same version again is refused; after a change the next release says what changed
    assert!(app.release_build("1.0", "", false).is_err(), "a version is released once");
    app.release_set(&json!({"extras": [], "credits": "", "out_dir": tmp.join("out").to_string_lossy()})).unwrap();
    std::fs::write(
        tl.join("script.rpy"),
        block("a_1", "Good morning, everyone.", "Dobré ráno vám všem.")
            + &block("a_2", "We should go to the library.", "Měli bychom jít do knihovny.")
            + &block("a_3", "Nothing yet.", "Zatím nic."),
    )
    .unwrap();
    app.refresh(true, false, true).unwrap();
    let b = app.release_build("1.1", "", false).unwrap();
    let changelog = b["changelog"].as_str().unwrap();
    assert!(changelog.contains("Changes since 1.0"), "{changelog}");
    assert!(changelog.contains("1 lines translated that were not before") && changelog.contains("1 lines changed"), "{changelog}");
    assert!(changelog.contains("files changed (1)"), "{changelog}");
    let _ = std::fs::remove_dir_all(&tmp);
}
