//! What Rust Core answers for the cases of the groups that are ported, in the shape `golden_tool.py` records them.
use lexo_core::lang::{self, Pack};
use lexo_core::messages::render;
use lexo_core::rpa::{self, read_entry, read_index};
use lexo_core::rpy::{self, Entry, ParsedFile};
use lexo_core::spelling::Spelling;
use lexo_core::util::sha1_hex;
use lexo_core::validator::SpellCheck;
use lexo_core::{fontmetrics, layout, menus, textedit, validator};
use serde_json::{json, Map, Value};
use std::path::Path;

/// The pack named in `testdata/golden/rpy/cases.json`: `cs` is the Czech pack of the app, `neutral` any non-ASCII letter.
pub fn pack_named(name: &str, repo_root: &Path) -> Result<Pack, String> {
    match name {
        "cs" => lang::load_dir(&repo_root.join("rust/resources/packs/cs")).map_err(|e| e.to_string()),
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
    let (done, todo, appr) = rpy::count(&p.entries, &Default::default(), None);
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

/// One entry of `run_validator`: an Entry built from the case, checked with the Czech pack.
pub fn validator_case(golden: &Path, name: &str) -> Result<Value, String> {
    let pack = pack_named("cs", &golden.join("../.."))?;
    let cases = read_json(&golden.join("validator/cases.json"))?;
    let c = cases["cases"].as_array().and_then(|a| a.iter().find(|c| c["name"] == name)).ok_or("no such validator case")?;
    let text = |k: &str| c.get(k).and_then(Value::as_str).map(String::from);
    let who = text("who").unwrap_or_default();
    let suffix = text("suffix").unwrap_or_default();
    let (source, target) = (text("source").unwrap_or_default(), text("target").unwrap_or_default());
    let (src_who, src_suffix) = (text("src_who").unwrap_or_else(|| who.clone()), text("src_suffix").unwrap_or_else(|| suffix.clone()));
    let category = if source == target || target.is_empty() { rpy::classify(&target, &pack) } else { "" };
    let entry = Entry {
        line: 1,
        src_line: 1,
        block: "translate czech x".into(),
        unit: "dialogue",
        src_raw: format!("{src_who} \"{source}\"").trim().to_string(),
        cur_raw: format!("{who} \"{target}\"").trim().to_string(),
        who,
        suffix,
        source: Some(source),
        target,
        src_who,
        src_suffix,
        category,
        ..Entry::default()
    };
    let issues = validator::check_entry(&entry, &pack, &validator::Checks::default());
    Ok(json!({
        "name": name, "pending": entry.pending(), "category": entry.category,
        "issues": issues.iter().map(|i| json!({"code": i.code, "severity": i.severity, "category": i.category})).collect::<Vec<_>>(),
    }))
}

/// The issues of the checks (without terms, with the default settings, with the spelling hints when `spell` is given) for one parsed file, as `[[line, issue], ...]` with the messages in English.
pub fn checked_json(p: &ParsedFile, pack: &Pack, spell: Option<&dyn SpellCheck>) -> Value {
    let found = validator::check_parsed(p, pack, &validator::Checks { spell, ..validator::Checks::default() });
    render(&Value::Array(found.iter().map(|(line, issue, _)| json!([line, issue.to_json()])).collect()))
}

/// `run_spelling` of `golden_tool.py`, split in the cases of the inventory: `vocab`, `suggest` and `check/<name>`.
pub fn spelling_case(golden: &Path, id: &str) -> Result<Value, String> {
    let spec = read_json(&golden.join("spelling/cases.json"))?;
    let known = strings(&spec["known"]);
    let sp = Spelling::build(&strings(&spec["targets"]), &known);
    match id {
        "vocab" => {
            let by_fold: Map<String, Value> = sp.by_fold.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
            Ok(json!({"sig": sp.sig, "vocab_size": sp.vocab.map.len(), "vocab_total": sp.vocab.map.values().sum::<usize>(), "by_fold": by_fold}))
        }
        "suggest" => {
            let mut out = Map::new();
            for w in strings(&spec["suggest"]) {
                out.insert(w.clone(), json!(sp.suggest(&w).map(|(k, b)| json!([k, b]))));
            }
            Ok(Value::Object(out))
        }
        _ => {
            let name = id.strip_prefix("check/").ok_or("unknown spelling case")?;
            let c = spec["check"].as_array().and_then(|a| a.iter().find(|c| c["name"] == name)).ok_or("no such check case")?;
            let issues = sp.check(c["source"].as_str().unwrap_or(""), c["target"].as_str().unwrap_or(""));
            Ok(json!({"name": name, "issues": render(&Value::Array(issues.iter().map(|i| i.to_json()).collect()))}))
        }
    }
}

/// `run_textedit` of `golden_tool.py`, split in `case/<name>`, `case_like` and `mask`.
pub fn textedit_case(golden: &Path, id: &str) -> Result<Value, String> {
    let spec = read_json(&golden.join("textedit/cases.json"))?;
    match id {
        "case_like" => Ok(Value::Array(
            spec["case_like"]
                .as_array()
                .ok_or("no case_like list")?
                .iter()
                .map(|p| json!(textedit::case_like(p[0].as_str().unwrap_or(""), p[1].as_str().unwrap_or(""))))
                .collect(),
        )),
        "mask" => Ok(Value::Array(strings(&spec["mask"]).iter().map(|t| json!(textedit::mask(t))).collect())),
        _ => {
            let name = id.strip_prefix("case/").ok_or("unknown textedit case")?;
            let c = spec["cases"].as_array().and_then(|a| a.iter().find(|c| c["name"] == name)).ok_or("no such textedit case")?;
            let text = |k: &str| c[k].as_str().unwrap_or("");
            let mut out = Map::new();
            out.insert("name".into(), json!(name));
            match textedit::make_replacer(text("find"), text("replace"), text("mode"), c["case"].as_bool().unwrap_or(false)) {
                Ok(r) => {
                    let (new, n) = r.apply(text("text"), c["protect"].as_bool().unwrap_or(true));
                    out.insert("result".into(), json!([new, n]));
                }
                Err(m) => {
                    out.insert("error".into(), json!(m.text()));
                }
            }
            Ok(Value::Object(out))
        }
    }
}

/// `run_fontmetrics` of `golden_tool.py` for one font file.
pub fn fontmetrics_case(golden: &Path, id: &str) -> Value {
    let data = std::fs::read(golden.join("fonts").join(id)).unwrap_or_default();
    match fontmetrics::advances(&data) {
        None => Value::Null,
        Some(a) => json!({"upm": a.upm, "adv": a.adv.iter().map(|(k, v)| (k.to_string(), json!(v))).collect::<Map<String, Value>>()}),
    }
}

/// A step (or the files at the end) of a data scenario. A scenario is run once and its answers are kept for the cases that belong to it.
pub fn data_case(golden: &Path, id: &str) -> Result<Value, String> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<String, Result<Value, String>>>> = OnceLock::new();
    let (name, rest) = id.split_once('/').ok_or("bad data case id")?;
    let key = format!("{}|{name}", golden.display());
    let result = {
        let mut cache = CACHE.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap();
        cache.entry(key).or_insert_with(|| crate::data::run_data_scenario(&golden.join("data").join(format!("{name}.json")))).clone()
    }?;
    if rest == "files_after" {
        return Ok(result["files_after"].clone());
    }
    let index: usize = rest.split(' ').next().and_then(|n| n.parse().ok()).ok_or("bad data step id")?;
    result["steps"].get(index).cloned().ok_or_else(|| "no such step".to_string())
}

/// One step (or the files at the end) of a scenario on a whole project; the scenario is run once and its record kept.
pub fn scenario_case(golden: &Path, id: &str) -> Result<Value, String> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<String, Result<Value, String>>>> = OnceLock::new();
    let (name, rest) = id.split_once('/').ok_or("bad scenario case id")?;
    let key = format!("{}|{name}", golden.display());
    let result = {
        let mut cache = CACHE.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap();
        cache.entry(key).or_insert_with(|| crate::scenario::run_scenario(golden, name)).clone()
    }?;
    if rest == "files_after" {
        return Ok(result["files_after"].clone());
    }
    let index: usize = rest.split(' ').next().and_then(|n| n.parse().ok()).ok_or("bad scenario step id")?;
    result["steps"].get(index).cloned().ok_or_else(|| "no such step".to_string())
}
