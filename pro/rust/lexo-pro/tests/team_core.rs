//! Core + the Pro pack together: a team of two people who share one translation folder (no server): the lead creates it and assigns a file, the other one joins and sees it.
use lexo_core::app::App;
use lexo_core::config::Config;
use lexo_core::lang::Packs;
use lexo_pro::LexoProProvider;
use serde_json::json;
use std::path::Path;

fn app_of(tmp: &Path, who: &str, tl: &Path, pro: bool) -> App {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut config = Config::new(Some(&tmp.join(format!("cfg-{who}/config.json"))));
    config.load();
    config.set("author", json!(format!("{who}-0001")));
    let mut app = App::new(config, Packs { bundled: repo.join("rust/resources/packs"), user: None });
    if pro {
        app.ext.register(Box::new(LexoProProvider::new())).unwrap();
    }
    app.open_project(&tl.to_string_lossy(), false).expect("the project opens");
    app
}

#[test]
fn a_team_is_made_by_the_lead_and_seen_by_a_member_without_the_pack() {
    let tmp = std::env::temp_dir().join(format!("team-core-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::env::set_var("LEXO_LICENCE_FILE", std::env::temp_dir().join("aside-team-no-licence.json")); // no file: the development build counts as licensed
    let tl = tmp.join("p/game/tl/czech");
    std::fs::create_dir_all(&tl).unwrap();
    std::fs::write(tl.join("a.rpy"), "translate czech a_1:\n\n    # e \"Hello.\"\n    e \"Ahoj.\"\n\n").unwrap();
    std::fs::write(tl.join("b.rpy"), "translate czech b_1:\n\n    # e \"Bye.\"\n    e \"Nashle.\"\n\n").unwrap();

    let mut lead = app_of(&tmp, "lead", &tl, true);
    let mut ann = app_of(&tmp, "ann", &tl, false); // Free: no pack at all

    // no team yet; the lead must say who they are, then makes the team
    assert_eq!(lead.team_state().unwrap()["exists"], false);
    assert!(lead.team_create().is_err(), "the name comes first");
    lead.team_join("  Petr  ").unwrap();
    let s = lead.team_create().unwrap();
    assert_eq!((s["exists"].clone(), s["me"]["lead"].clone(), s["can_manage"].clone()), (json!(true), json!(true), json!(true)), "{s}");
    assert!(lead.team_create().is_err(), "a team is made once");

    // Ann (Free) joins and sees the team, but cannot make or change anything
    let s = ann.team_join("Ann").unwrap();
    assert_eq!(
        (s["exists"].clone(), s["me"]["joined"].clone(), s["me"]["lead"].clone(), s["can_manage"].clone()),
        (json!(true), json!(true), json!(false), json!(false)),
        "{s}"
    );
    assert!(ann.team_create().is_err() && ann.team_set(&json!({}), &json!({})).is_err());

    // the lead gives a.rpy to Ann (and a role); what is not allowed is dropped
    let s = lead
        .team_set(&json!({"ann-0001": "proofreader", "ghost-1": "translator"}), &json!({"a.rpy": ["ann-0001", "ghost-1"], "nowhere.rpy": ["ann-0001"]}))
        .unwrap();
    assert_eq!(s["dropped"], 3, "{s}");
    let ann_row = s["members"].as_array().unwrap().iter().find(|m| m["author"] == "ann-0001").unwrap();
    assert_eq!((ann_row["role"].clone(), ann_row["files"].clone()), (json!("proofreader"), json!(1)), "{ann_row}");

    // Ann sees her files without the pack
    let s = ann.team_state().unwrap();
    assert_eq!(s["my_files"], json!(["a.rpy"]), "{s}");
    assert_eq!(s["members"].as_array().unwrap().len(), 2);
    // and the lead's own row says lead
    assert!(s["members"].as_array().unwrap().iter().any(|m| m["name"] == "Petr" && m["role"] == "lead"));
}
