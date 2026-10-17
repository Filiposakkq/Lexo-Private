//! What Rust Core answers for the cases of the groups that are ported, in the shape `golden_tool.py` records them.
use lexo_core::lang::{self, Pack};
use lexo_core::messages::render;
use lexo_core::rpa::{self, read_entry, read_index};
use lexo_core::rpy::{self, Entry, ParsedFile};
use lexo_core::util::sha1_hex;
use lexo_core::{layout, menus};
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::path::Path;

/// The pack named in `core/golden/rpy/cases.json`: `cs` is the Czech pack of the app, `neutral` any non-ASCII letter.
pub fn pack_named(name: &str, repo_root: &Path) -> Result<Pack, String> {
    match name {
        "cs" => lang::load_dir(&repo_root.join("core/lexo/packs/cs")).map_err(|e| e.to_string()),
        "neutral" => Ok(Pack::neutral()),
        other => Err(format!("unknown language {other:?} in cases.json")),
    }
}

fn entry_json(e: &Entry) -> Value {
    let (block, norm) = e.key();
    json!({
        "line": e.line, "src_line": e.src_line, "block": e.block, "unit": e.unit, "who": e.who, "suffix": e.suffix,
        "source": e.source, "target": e.target, "src_raw": e.src_raw, "cur_raw": e.cur_raw, "src_who": e.src_who,
        "src_suffix": e.src_suffix, "category": e.category, "ref": e.reference, "pending": e.pending(), "key": [block, norm],
    })
}

/// `parse_case` of `golden_tool.py`.
pub fn parsed_json(p: &ParsedFile) -> Value {
    let mut languages: Vec<&String> = p.languages.iter().collect();
    languages.sort();
    let (done, todo, appr) = rpy::count(&p.entries, &HashSet::new(), None);
    render(&json!({
        "entries": p.entries.iter().map(entry_json).collect::<Vec<_>>(),
        "block_ids": p.block_ids.iter().map(|(l, i, n)| json!([l, i, n])).collect::<Vec<_>>(),
        "structure": p.structure.iter().map(|(n, code, m)| json!([n, code, m.wire()])).collect::<Vec<_>>(),
        "digest": p.digest,
        "languages": languages,
        "counts": [done, todo, appr],
    }))
}

pub fn rpy_case(golden: &Path, id: &str, spec: &Value) -> Result<Value, String> {
    let root = golden.join("../..");
    let pack = pack_named(spec["language"].as_str().unwrap_or("cs"), &root)?;
    let file = spec.get("file").and_then(Value::as_str).map(String::from).unwrap_or_else(|| format!("{id}.rpy"));
    let data = std::fs::read(golden.join("rpy/cases").join(&file)).map_err(|e| format!("{file}: {e}"))?;
    Ok(parsed_json(&rpy::parse_bytes(&data, &pack)))
}

/// `run_rpa` of `golden_tool.py` for one archive; an error is reduced to its first words (the rest is the wording of Python's own pickle / zlib errors).
pub fn rpa_case(golden: &Path, id: &str) -> Value {
    let path = golden.join("rpa/archives").join(format!("{id}.rpa"));
    match read_index(&path) {
        Err(e) => {
            let text = e.to_string();
            json!({"error": text.split(':').next().unwrap_or("").split(" blocked ").next().unwrap_or("")})
        }
        Ok((version, index)) => {
            let mut files = Map::new();
            for (name, entry) in &index {
                let data = read_entry(&path, entry, rpa::DEFAULT_ENTRY_LIMIT).unwrap_or_default();
                let prefix: String = entry.prefix.iter().map(|b| format!("{b:02x}")).collect();
                files.insert(name.clone(), json!({"offset": entry.offset, "length": entry.length, "prefix": prefix, "sha1": sha1_hex(&data)}));
            }
            json!({"version": version, "files": files})
        }
    }
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect()).unwrap_or_default()
}

/// One entry of `run_layout`: `propose/<name>`, `folder_number` or `series_number`.
pub fn layout_case(golden: &Path, id: &str) -> Result<Value, String> {
    let cases = read_json(&golden.join("layout/cases.json"))?;
    if let Some(name) = id.strip_prefix("propose/") {
        let case = cases["propose"].as_array().and_then(|a| a.iter().find(|c| c["name"] == name)).ok_or("no such propose case")?;
        return Ok(layout::propose(&strings(&case["files"])).to_json());
    }
    let mut out = Map::new();
    match id {
        "folder_number" => {
            let folders = strings(&cases["folder_number"]["folders"]);
            for p in strings(&cases["folder_number"]["paths"]) {
                out.insert(p.clone(), json!(layout::folder_number(&p, &folders)));
            }
        }
        "series_number" => {
            let series = cases["series_number"]["series"].as_str().unwrap_or("");
            for p in strings(&cases["series_number"]["paths"]) {
                out.insert(p.clone(), json!(layout::series_number(&p, series)));
            }
        }
        _ => return Err(format!("unknown layout case {id}")),
    }
    Ok(Value::Object(out))
}

/// One entry of `run_menus`: the id starts with the index of the case in `menus/cases.json`.
pub fn menus_case(golden: &Path, id: &str) -> Result<Value, String> {
    let index: usize = id.split(' ').next().and_then(|n| n.parse().ok()).ok_or("bad menus case id")?;
    let cases = read_json(&golden.join("menus/cases.json"))?;
    let c = cases["cases"].get(index).ok_or("no such menus case")?;
    let script = c["script"].as_str().unwrap_or("");
    let text = std::fs::read_to_string(golden.join("menus/scripts").join(script)).map_err(|e| format!("{script}: {e}"))?;
    let lines = lexo_core::util::py_splitlines(&text);
    let line = c["line"].as_u64().unwrap_or(0) as usize;
    let choice = c["choice"].as_str();
    let found = if c["around"].as_bool().unwrap_or(false) { menus::menu_around(&lines, line, choice) } else { menus::find_menu(&lines, line, choice) };
    Ok(json!({
        "script": script, "line": c["line"], "choice": c["choice"], "around": c["around"].as_bool().unwrap_or(false), "result": found.map(|m| m.to_json()),
    }))
}
