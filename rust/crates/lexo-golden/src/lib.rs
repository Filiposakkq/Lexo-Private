//! The golden runner (docs/GOLDEN.md): reads the inputs and the recorded answers of `core/golden`, asks `lexo-core` for its own answers,
//! normalises them like `core/tests/golden_tool.py` does and compares. M0 was the skeleton (the inventory and the report); each milestone from M1 on
//! replaces `NotImplemented` for the cases of the modules it ports.
pub mod data;
pub mod diff;
pub mod normalize;
pub mod run;

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The folder `core/golden`: `LEXO_GOLDEN` if set, otherwise the one of this repository.
pub fn golden_dir() -> PathBuf {
    match std::env::var_os("LEXO_GOLDEN") {
        Some(p) => PathBuf::from(p),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../core/golden"),
    }
}

/// One thing that is checked: the answer Core gave to one input.
#[derive(Debug, Clone)]
pub struct Case {
    pub group: &'static str,
    pub id: String,
    pub expected: Value,
}

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Pass,
    Fail(String),
    NotImplemented,
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn keys(v: &Value) -> Vec<String> {
    v.as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default()
}

/// Every case the recorded answers hold, in a stable order.
pub fn inventory(golden: &Path) -> Result<Vec<Case>, String> {
    let exp = golden.join("expected");
    let mut out = Vec::new();
    let rpy = read_json(&exp.join("rpy.json"))?;
    for k in keys(&rpy) {
        out.push(Case { group: "rpy", id: k.clone(), expected: rpy[&k].clone() });
    }
    let rpa = read_json(&exp.join("rpa.json"))?;
    for k in keys(&rpa) {
        out.push(Case { group: "rpa", id: k.clone(), expected: rpa[&k].clone() });
    }
    let layout = read_json(&exp.join("layout.json"))?;
    for k in keys(&layout["propose"]) {
        out.push(Case { group: "layout", id: format!("propose/{k}"), expected: layout["propose"][&k].clone() });
    }
    for k in ["folder_number", "series_number"] {
        out.push(Case { group: "layout", id: k.to_string(), expected: layout[k].clone() });
    }
    let menus = read_json(&exp.join("menus.json"))?;
    for (i, m) in menus.as_array().ok_or("menus.json is not a list")?.iter().enumerate() {
        out.push(Case { group: "menus", id: format!("{i} {}:{}", m["script"].as_str().unwrap_or("?"), m["line"]), expected: m.clone() });
    }
    let validator = read_json(&exp.join("validator.json"))?;
    for v in validator.as_array().ok_or("validator.json is not a list")? {
        out.push(Case { group: "validator", id: v["name"].as_str().unwrap_or("?").to_string(), expected: v.clone() });
    }
    let spelling = read_json(&exp.join("spelling.json"))?;
    for k in ["vocab", "suggest"] {
        out.push(Case {
            group: "spelling",
            id: k.to_string(),
            expected: if k == "vocab" {
                json!({"sig": spelling["sig"], "vocab_size": spelling["vocab_size"], "vocab_total": spelling["vocab_total"], "by_fold": spelling["by_fold"]})
            } else {
                spelling["suggest"].clone()
            },
        });
    }
    for c in spelling["check"].as_array().ok_or("spelling.json has no check list")? {
        out.push(Case { group: "spelling", id: format!("check/{}", c["name"].as_str().unwrap_or("?")), expected: c.clone() });
    }
    let textedit = read_json(&exp.join("textedit.json"))?;
    for c in textedit["cases"].as_array().ok_or("textedit.json has no cases")? {
        out.push(Case { group: "textedit", id: format!("case/{}", c["name"].as_str().unwrap_or("?")), expected: c.clone() });
    }
    for k in ["case_like", "mask"] {
        out.push(Case { group: "textedit", id: k.to_string(), expected: textedit[k].clone() });
    }
    let fonts = read_json(&exp.join("fontmetrics.json"))?;
    for k in keys(&fonts) {
        out.push(Case { group: "fontmetrics", id: k.clone(), expected: fonts[&k].clone() });
    }
    let mut names: Vec<String> = std::fs::read_dir(exp.join("scenarios"))
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok()?.path().file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    names.sort();
    for name in names {
        let sc = read_json(&exp.join("scenarios").join(format!("{name}.json")))?;
        for (i, st) in sc["steps"].as_array().ok_or("a scenario has no steps")?.iter().enumerate() {
            out.push(Case { group: "scenarios", id: format!("{name}/{i} {}", st["op"].as_str().unwrap_or("?")), expected: st.clone() });
        }
        out.push(Case { group: "scenarios", id: format!("{name}/files_after"), expected: sc["files_after"].clone() });
    }
    let mut data_names: Vec<String> = std::fs::read_dir(exp.join("data"))
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok()?.path().file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    data_names.sort();
    for name in data_names {
        let sc = read_json(&exp.join("data").join(format!("{name}.json")))?;
        for (i, st) in sc["steps"].as_array().ok_or("a data scenario has no steps")?.iter().enumerate() {
            out.push(Case { group: "data", id: format!("{name}/{i} {}", st["op"].as_str().unwrap_or("?")), expected: st.clone() });
        }
        out.push(Case { group: "data", id: format!("{name}/files_after"), expected: sc["files_after"].clone() });
    }
    Ok(out)
}

/// The answer of Rust Core to one case, compared with the recorded one. A group is "not implemented" until its milestone has ported the modules it needs.
pub fn run_case(golden: &Path, case: &Case) -> Outcome {
    let got = match case.group {
        "rpy" => {
            let cases = match read_json(&golden.join("rpy/cases.json")) {
                Ok(c) => c,
                Err(e) => return Outcome::Fail(e),
            };
            match run::rpy_case(golden, &case.id, &cases[&case.id]) {
                Ok(v) => v,
                Err(e) => return Outcome::Fail(e),
            }
        }
        "rpa" => run::rpa_case(golden, &case.id),
        "layout" => match run::layout_case(golden, &case.id) {
            Ok(v) => v,
            Err(e) => return Outcome::Fail(e),
        },
        "spelling" => match run::spelling_case(golden, &case.id) {
            Ok(v) => v,
            Err(e) => return Outcome::Fail(e),
        },
        "textedit" => match run::textedit_case(golden, &case.id) {
            Ok(v) => v,
            Err(e) => return Outcome::Fail(e),
        },
        "fontmetrics" => run::fontmetrics_case(golden, &case.id),
        "data" => match run::data_case(golden, &case.id) {
            Ok(v) => v,
            Err(e) => return Outcome::Fail(e),
        },
        "validator" => match run::validator_case(golden, &case.id) {
            Ok(v) => v,
            Err(e) => return Outcome::Fail(e),
        },
        "menus" => match run::menus_case(golden, &case.id) {
            Ok(v) => v,
            Err(e) => return Outcome::Fail(e),
        },
        _ => return Outcome::NotImplemented,
    };
    match diff::first_difference(&got, &case.expected) {
        None => Outcome::Pass,
        Some(d) => Outcome::Fail(d),
    }
}

/// Per group: (passed, failed, not implemented).
pub fn summarize(results: &[(Case, Outcome)]) -> Vec<(&'static str, usize, usize, usize)> {
    let mut groups: Vec<&'static str> = Vec::new();
    for (c, _) in results {
        if !groups.contains(&c.group) {
            groups.push(c.group);
        }
    }
    groups
        .into_iter()
        .map(|g| {
            let of = |f: &dyn Fn(&Outcome) -> bool| results.iter().filter(|(c, o)| c.group == g && f(o)).count();
            (g, of(&|o| *o == Outcome::Pass), of(&|o| matches!(o, Outcome::Fail(_))), of(&|o| *o == Outcome::NotImplemented))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_inventory_finds_every_group_and_unique_ids() {
        let cases = inventory(&golden_dir()).expect("core/golden/expected is readable");
        for g in ["rpy", "rpa", "layout", "menus", "validator", "spelling", "textedit", "fontmetrics", "data", "scenarios"] {
            assert!(cases.iter().any(|c| c.group == g), "no case in {g}");
        }
        let mut ids: Vec<_> = cases.iter().map(|c| (c.group, c.id.clone())).collect();
        ids.sort();
        let n = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), n, "a case id is not unique");
    }

    #[test]
    fn every_ported_group_matches_the_recorded_answers() {
        let golden = golden_dir();
        for c in inventory(&golden)
            .unwrap()
            .into_iter()
            .filter(|c| ["rpy", "rpa", "layout", "menus", "validator", "spelling", "textedit", "fontmetrics", "data"].contains(&c.group))
        {
            assert_eq!(run_case(&golden, &c), Outcome::Pass, "{}/{}", c.group, c.id);
        }
    }

    #[test]
    fn the_groups_not_ported_yet_are_reported_as_todo_not_as_failures() {
        let golden = golden_dir();
        let results: Vec<_> = inventory(&golden)
            .unwrap()
            .into_iter()
            .map(|c| {
                let o = run_case(&golden, &c);
                (c, o)
            })
            .collect();
        let total: usize = summarize(&results).iter().map(|(_, p, f, n)| p + f + n).sum();
        assert_eq!(total, results.len());
        assert!(summarize(&results).iter().all(|(g, _, f, _)| *f == 0 || *g == "never"));
    }
}
