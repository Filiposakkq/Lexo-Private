//! The scenarios on whole projects (`testdata/golden/scenarios`, docs/technical/GOLDEN.md): a copy of a fixture project is opened by the application and a list of operations is called through the
//! table of `api.rs`; the answer of every call and the `.rpy` files at the end are compared with the recorded ones. The same runner as `run_scenario` of `reference/python/core/tests/golden_tool.py`.
use crate::data::{base64_encode, roots, temp_dir};
use crate::normalize::normalize;
use lexo_core::api;
use lexo_core::app::App;
use lexo_core::app_base::ApiError;
use lexo_core::config::Config;
use lexo_core::lang::Packs;
use lexo_core::messages::render;
use lexo_core::util::sha1_hex;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

/// The parameters that name a translation file: written with `/` in a scenario, with the separator of the system for the call.
const PATH_PARAMS: [&str; 4] = ["file", "name", "files", "names"];

fn native(params: &Value) -> Value {
    fn fix(v: &Value) -> Value {
        match v {
            Value::String(s) => Value::String(s.replace('/', std::path::MAIN_SEPARATOR_STR)),
            Value::Array(a) => Value::Array(a.iter().map(fix).collect()),
            other => other.clone(),
        }
    }
    match params.as_object() {
        Some(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), if PATH_PARAMS.contains(&k.as_str()) { fix(v) } else { v.clone() })).collect()),
        None => json!({}),
    }
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let target = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_tree(&e.path(), &target)?;
        } else {
            std::fs::copy(e.path(), target)?;
        }
    }
    Ok(())
}

fn rpy_files(dir: &Path, root: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.filter_map(|e| e.ok()) {
        let p = e.path();
        if p.is_dir() {
            rpy_files(&p, root, out);
        } else if p.extension().is_some_and(|x| x == "rpy") {
            out.push((p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"), p));
        }
    }
}

/// Run one scenario and give its normalised record: `{"project", "steps", "files_after"}`.
pub fn run_scenario(golden: &Path, name: &str) -> Result<Value, String> {
    std::env::set_var("LEXO_NO_PRO", "1"); // the golden answers are those of Core without the Pro pack, whatever program lies next to this one
    let path = golden.join("scenarios").join(format!("{name}.json"));
    let spec: Value = serde_json::from_str(&std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?).map_err(|e| e.to_string())?;
    let tmp = temp_dir("lexo-golden-");
    let result = (|| {
        let project = spec["project"].as_str().ok_or("no project")?;
        let base = tmp.join("p");
        copy_tree(&golden.join("projects").join(project), &base).map_err(|e| e.to_string())?;
        let tl = base.join(spec["tl"].as_str().unwrap_or("game/tl/czech"));
        let cfg_dir = tmp.join("cfg");
        let mut config = Config::new(Some(&cfg_dir.join("config.json")));
        config.load();
        let packs = Packs { bundled: golden.join("../../rust/resources/packs"), user: Some(cfg_dir.join("packs")) };
        let mut app = App::new(config, packs);
        let mut steps = Vec::new();
        let opened = app.open_project(&tl.to_string_lossy(), false);
        let mut files = Map::new();
        if let Err(e) = opened {
            return Err(format!("the project could not be opened: {e}"));
        }
        for st in spec["steps"].as_array().ok_or("no steps")? {
            let op = st["op"].as_str().unwrap_or("");
            let params = st.get("params").cloned().unwrap_or_else(|| json!({}));
            let mut entry = Map::new();
            entry.insert("op".into(), json!(op));
            entry.insert("params".into(), params.clone());
            match api::call(&mut app, op, &native(&params)) {
                Ok(answer) => {
                    entry.insert("answer".into(), render(answer.json().unwrap_or(&Value::Null)));
                }
                Err(ApiError::Missing(k)) => {
                    entry.insert("error".into(), json!(format!("missing parameter '{k}'")));
                }
                Err(e) if e.is_app_error() => {
                    entry.insert("error".into(), json!(e.text().text()));
                }
                Err(e) => return Err(format!("{op}: {e}")),
            }
            steps.push(Value::Object(entry));
        }
        let mut found = Vec::new();
        rpy_files(&tl, &tl, &mut found);
        found.sort();
        for (rel, p) in found {
            let b = std::fs::read(&p).map_err(|e| e.to_string())?;
            let mut rec = Map::new();
            rec.insert("sha1".into(), json!(sha1_hex(&b)));
            match String::from_utf8(b.clone()) {
                Ok(t) => rec.insert("text".into(), json!(t)),
                Err(_) => rec.insert("base64".into(), json!(base64_encode(&b))),
            };
            files.insert(rel, Value::Object(rec));
        }
        app.close();
        Ok(normalize(&json!({"project": project, "steps": steps, "files_after": files}), &roots(&tmp)))
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result
}
