//! What Rust Core answers for the cases of the groups that are ported, in the shape `golden_tool.py` records them.
use lexo_core::lang::{self, Pack};
use lexo_core::messages::render;
use lexo_core::rpa::{self, read_entry, read_index};
use lexo_core::rpy::{self, Entry, ParsedFile};
use lexo_core::util::sha1_hex;
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
