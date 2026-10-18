//! `lexo-golden --dump-ops <translation folder> <scratch folder>`: the answers of the read operations of the application for a project (for the comparison with the Python Core on
//! real games, milestone M9). The application WRITES into the project folder it opens (the `.lexo` folder, the lock, the derived files), so the folder must be a copy.
use lexo_core::api;
use lexo_core::app::App;
use lexo_core::config::Config;
use lexo_core::lang::Packs;
use lexo_core::messages::render;
use serde_json::{json, Map, Value};
use std::path::Path;

/// The read operations and the parameters they are asked with.
pub fn read_ops() -> Vec<(&'static str, Value)> {
    vec![
        ("overview", json!({})),
        ("files", json!({})),
        ("entries", json!({"limit": 500})),
        ("validation", json!({"limit": 500, "group": "line"})),
        ("ignore/list", json!({})),
        ("glossary", json!({})),
        ("groups/suggest", json!({})),
        ("consistency", json!({})),
        ("missed/functions", json!({})),
        ("game/characters", json!({})),
        ("snapshots", json!({})),
        ("settings", json!({})),
    ]
}

pub fn dump_ops(tl: &Path, scratch: &Path) -> Value {
    let cfg = scratch.join("cfg");
    let mut config = Config::new(Some(&cfg.join("config.json")));
    config.load();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut app = App::new(config, Packs { bundled: repo.join("core/lexo/packs"), user: Some(cfg.join("packs")) });
    let mut out = Map::new();
    match app.open_project(&tl.to_string_lossy(), false) {
        Err(e) => {
            out.insert("open".into(), json!({"error": e.text().text()}));
        }
        Ok(_) => {
            for (name, params) in read_ops() {
                let answer = match api::call(&mut app, name, &params) {
                    Ok(a) => render(a.json().unwrap_or(&Value::Null)),
                    Err(e) => json!({"error": e.text().text()}),
                };
                out.insert(name.to_string(), answer);
            }
        }
    }
    app.close();
    Value::Object(out)
}

/// `lexo-golden --run-ops <translation folder (a COPY)> <scratch folder> <ops.json>`: run a list of `[operation, parameters]` on the project and give `[{answer} | {error}]` for each.
/// The list is made by the Python side of the comparison (`rust/tools/diff_ops_real_games.py`), so both Cores are asked exactly the same.
pub fn run_ops(tl: &Path, scratch: &Path, ops: &Value) -> Value {
    let cfg = scratch.join("cfg");
    let mut config = Config::new(Some(&cfg.join("config.json")));
    config.load();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut app = App::new(config, Packs { bundled: repo.join("core/lexo/packs"), user: Some(cfg.join("packs")) });
    let mut out: Vec<Value> = Vec::new();
    match app.open_project(&tl.to_string_lossy(), false) {
        Err(e) => out.push(json!({"open_error": e.text().text()})),
        Ok(_) => {
            for item in ops.as_array().into_iter().flatten() {
                let (name, params) = (item[0].as_str().unwrap_or(""), &item[1]);
                out.push(match api::call(&mut app, name, params) {
                    Ok(a) => json!({"answer": render(a.json().unwrap_or(&Value::Null))}),
                    Err(e) => json!({"error": e.text().text()}),
                });
            }
        }
    }
    app.close();
    Value::Array(out)
}
