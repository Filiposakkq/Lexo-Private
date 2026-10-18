//! The pack as a program: the application side (`SidecarProvider` of Core) starts `lexo-pro-sidecar` and the gated features of the application work through it.
use lexo_core::api;
use lexo_core::app::App;
use lexo_core::config::Config;
use lexo_core::extensions::{Provider, SidecarProvider};
use lexo_core::lang::Packs;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const SIDECAR: &str = env!("CARGO_BIN_EXE_lexo-pro-sidecar");

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_dir() {
            copy_tree(&e.path(), &to.join(e.file_name()));
        } else {
            std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}

#[test]
fn the_sidecar_speaks_the_protocol_and_the_application_uses_it() {
    // 1. the protocol, directly
    let p = SidecarProvider::start(Path::new(SIDECAR)).unwrap();
    assert_eq!((p.api(), p.version()), (1, env!("CARGO_PKG_VERSION").to_string()));
    assert_eq!(p.offers().len(), 5);
    assert!(p.ui_strings("cs").len() > 5 && p.ui_strings("xx").is_empty());
    let ok = p.call("replace.regex", "substitute", &json!({"pattern": "a", "replace": "b", "texts": ["banana"]})).unwrap();
    assert_eq!(ok["results"][0], json!({"new": "bbnbnb", "n": 3}));
    let e = p.call("replace.regex", "substitute", &json!({"pattern": "(", "replace": "b", "texts": []})).unwrap_err();
    assert!(e.0.text().contains("Invalid regular expression"), "{}", e.0.text());
    assert!(p.call("nothing", "here", &json!({})).unwrap_err().0.text().contains("no feature"));
    drop(p);

    // 2. an application that finds the sidecar through the environment
    let tmp: PathBuf = std::env::temp_dir().join(format!("lexo-pro-sidecar-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    copy_tree(&repo.join("core/golden/projects/basic"), &tmp.join("p"));
    let mut config = Config::new(Some(&tmp.join("cfg/config.json")));
    config.load();
    std::env::remove_var("LEXO_NO_PRO");
    std::env::set_var("LEXO_PRO_SIDECAR", SIDECAR);
    let mut app = App::new(config, Packs { bundled: repo.join("core/lexo/packs"), user: None });
    std::env::remove_var("LEXO_PRO_SIDECAR");
    app.open_project(&tmp.join("p/game/tl/czech").to_string_lossy(), false).unwrap();
    let features = api::call(&mut app, "features", &json!({})).unwrap();
    let features: Value = features.json().unwrap().clone();
    assert_eq!(features["tier"], "pro");
    assert!(features["features"].as_object().unwrap().values().all(|f| f["available"] == true), "{features}");
    let regex = api::call(&mut app, "replace/preview", &json!({"find": r"(\w+)", "replace": r"<\1>", "mode": "regex"})).unwrap();
    assert!(regex.json().unwrap()["total"].as_i64().unwrap() > 0);
    let fixes = api::call(&mut app, "fix/preview", &json!({"kinds": ["speaker", "spaces", "typo"]})).unwrap();
    assert!(fixes.json().unwrap()["items"].is_array());
    let targets = api::call(&mut app, "mt/targets", &json!({})).unwrap();
    assert!(targets.json().unwrap()["total"].as_i64().unwrap() > 0);
    let suggest = api::call(&mut app, "assign/suggest", &json!({})).unwrap();
    assert!(!suggest.json().unwrap()["items"].as_array().unwrap().is_empty());
    app.close();
    drop(app);
    let _ = std::fs::remove_dir_all(&tmp);
}
