//! Port of `core/lexo/layout.py`: how a project's files are grouped (sections > groups), and the guessing of groups from file names.
//!
//! Ported here: the layout itself (`parse`, `to_json`), `propose`, `folder_number`, `series_number` and the helpers behind them.
//! Not yet: `load` and `save_groups` (they read and write `aside.json`, which come with the data files, M3), and the process-wide "current layout"
//! of the Python module: the functions that need the label of the group take it as a parameter.
use fancy_regex::Regex as FancyRegex;
use regex::Regex;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub const FILE_NAME: &str = "aside.json";
const DEFAULT_LABELS: [(&str, &str); 2] = [("section", "Season"), ("group", "Episode")];

#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub n: i64,
    /// `active` or `planned`.
    pub status: String,
    pub groups: Vec<i64>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Layout {
    pub language: String,
    /// One group per folder: group N is the N-th folder (a path prefix such as `scripts/A_home`).
    pub folders: Vec<String>,
    /// Files called `<series>`, `<series>2`, `<series>3` ... are groups 1, 2, 3 ...
    pub series: String,
    pub sections: Vec<Section>,
    pub labels: BTreeMap<String, String>,
}

impl Layout {
    pub fn new(sections: Vec<Section>, labels: BTreeMap<String, String>, language: &str, series: &str, folders: Vec<String>) -> Layout {
        let mut all: BTreeMap<String, String> = DEFAULT_LABELS.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        all.extend(labels);
        Layout { language: language.into(), folders, series: series.into(), sections, labels: all }
    }

    pub fn active_groups(&self) -> Vec<i64> {
        self.sections.iter().filter(|s| s.status == "active").flat_map(|s| s.groups.iter().copied()).collect()
    }

    pub fn section_of(&self, group: i64) -> Option<i64> {
        self.sections.iter().find(|s| s.groups.contains(&group)).map(|s| s.n)
    }

    pub fn groups_of(&self, section: i64) -> Vec<i64> {
        self.sections.iter().find(|s| s.n == section).map(|s| s.groups.clone()).unwrap_or_default()
    }

    pub fn to_json(&self) -> Value {
        let mut o = Map::new();
        o.insert("version".into(), json!(1));
        if !self.language.is_empty() {
            o.insert("language".into(), json!(self.language));
        }
        if !self.series.is_empty() {
            o.insert("series".into(), json!(self.series));
        }
        if !self.folders.is_empty() {
            o.insert("folders".into(), json!(self.folders));
        }
        o.insert("labels".into(), json!(self.labels));
        let sections: Vec<Value> = self
            .sections
            .iter()
            .map(|s| if s.status == "active" { json!({"n": s.n, "status": s.status, "groups": s.groups}) } else { json!({"n": s.n, "status": s.status}) })
            .collect();
        o.insert("sections".into(), Value::Array(sections));
        Value::Object(o)
    }
}

#[derive(Debug, PartialEq)]
pub struct LayoutError(pub String);

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Layout {
    /// The layout of a project without a `aside.json` (Python: `Layout()`).
    pub fn empty() -> Layout {
        Layout::new(Vec::new(), BTreeMap::new(), "", "", Vec::new())
    }
}

/// Read `aside.json` of a project: the layout and a note for the user (`None`, or a plain text when the file is broken or names a language without a pack).
pub fn load(data_dir: &std::path::Path, packs: &crate::lang::Packs) -> (Layout, Option<String>) {
    let path = data_dir.join(FILE_NAME);
    if !path.is_file() {
        return (Layout::empty(), None);
    }
    let read = || -> Result<Layout, String> {
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let text = String::from_utf8(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes).to_vec()).map_err(|e| e.to_string())?;
        let raw: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        parse(&raw).map_err(|e| e.0)
    };
    match read() {
        Ok(lay) => {
            let note = (!lay.language.is_empty() && packs.get(&lay.language).is_none())
                .then(|| format!("{FILE_NAME}: no language pack for \"{}\", neutral rules are used.", lay.language));
            (lay, note)
        }
        Err(e) => (Layout::empty(), Some(format!("{FILE_NAME} was ignored: {e}"))),
    }
}

/// Write the groups into `aside.json` (other keys, such as `language`, stay). `sections` = `(n, groups)`. Returns the new layout; fails for numbers that cannot be used,
/// before anything is written.
pub fn save_groups(
    data_dir: &std::path::Path,
    label: &str,
    sections: &[(i64, Vec<i64>)],
    series: &str,
    folders: &[String],
    clock: &dyn crate::clock::Clock,
) -> Result<Layout, LayoutError> {
    let path = data_dir.join(FILE_NAME);
    let mut raw: Map<String, Value> = std::fs::read(&path)
        .ok()
        .and_then(|b| String::from_utf8(b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&b).to_vec()).ok())
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| if let Value::Object(o) = v { Some(o) } else { None })
        .unwrap_or_default();
    let mut labels = raw.get("labels").and_then(Value::as_object).cloned().unwrap_or_default();
    labels.insert("group".into(), json!(label));
    raw.insert("version".into(), json!(1));
    raw.insert("labels".into(), Value::Object(labels));
    raw.insert("sections".into(), Value::Array(sections.iter().map(|(n, g)| json!({"n": n, "status": "active", "groups": g})).collect()));
    if series.is_empty() {
        raw.shift_remove("series");
    } else {
        raw.insert("series".into(), json!(series));
    }
    if folders.is_empty() {
        raw.shift_remove("folders");
    } else {
        raw.insert("folders".into(), json!(folders));
    }
    let raw = Value::Object(raw);
    let lay = parse(&raw)?; // validates: nothing is written for a bad one
    let text = crate::pyfmt::json_dumps_indent1(&raw) + "\n";
    crate::fsutil::atomic_write_text(&path, &text, None, None, clock).map_err(|e| LayoutError(e.to_string()))?;
    Ok(lay)
}

fn bad<T>(s: impl Into<String>) -> Result<T, LayoutError> {
    Err(LayoutError(s.into()))
}

fn series_ok(s: &str) -> bool {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\A[A-Za-z_][\w-]*\z").unwrap()).is_match(s)
}

/// Build a Layout from the decoded JSON; the error says what is wrong.
pub fn parse(raw: &Value) -> Result<Layout, LayoutError> {
    let Some(raw) = raw.as_object() else { return bad("the file must contain a JSON object") };
    let mut secs: Vec<Section> = Vec::new();
    let (mut seen_n, mut seen_g): (Vec<i64>, Vec<i64>) = (Vec::new(), Vec::new());
    let listed: Vec<&Value> = match raw.get("sections") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(a)) => a.iter().collect(),
        Some(Value::Object(o)) if o.is_empty() => Vec::new(),
        Some(_) => return bad("every section needs an integer \"n\""),
    };
    for s in listed {
        let Some(n) = s.as_object().and_then(|o| o.get("n")).and_then(Value::as_i64) else { return bad("every section needs an integer \"n\"") };
        let s = s.as_object().unwrap();
        if seen_n.contains(&n) {
            return bad(format!("section {n} is listed twice"));
        }
        seen_n.push(n);
        let status = match s.get("status") {
            None => "active",
            Some(Value::String(t)) if t == "active" || t == "planned" => t.as_str(),
            Some(_) => return bad(format!("section {n}: status must be \"active\" or \"planned\"")),
        };
        let groups: Vec<i64> = if status == "active" {
            match s.get("groups") {
                None => Vec::new(),
                Some(Value::Array(a)) => match a.iter().map(|g| g.as_i64().filter(|g| *g > 0)).collect::<Option<Vec<_>>>() {
                    Some(v) => v,
                    None => return bad(format!("section {n}: \"groups\" must be a list of positive integers")),
                },
                Some(_) => return bad(format!("section {n}: \"groups\" must be a list of positive integers")),
            }
        } else {
            Vec::new()
        };
        for g in &groups {
            if seen_g.contains(g) {
                return bad(format!("group {g} belongs to more than one section"));
            }
            seen_g.push(*g);
        }
        secs.push(Section { n, status: status.to_string(), groups });
    }
    let labels_raw: Map<String, Value> = match raw.get("labels") {
        None => Map::new(),
        Some(Value::Object(o)) if o.values().all(Value::is_string) => o.clone(),
        Some(_) => return bad("\"labels\" must map names to text"),
    };
    let language = match raw.get("language") {
        None => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(_) => return bad("\"language\" must be a language code such as \"cs\""),
    };
    let series = match raw.get("series") {
        None => String::new(),
        Some(Value::String(s)) if s.is_empty() || series_ok(s) => s.clone(),
        Some(_) => return bad("\"series\" must be a file name stem such as \"script\""),
    };
    let folders_err = || LayoutError("\"folders\" must be a list of folder paths such as \"scripts/A_home\"".into());
    let folders: Vec<String> = match raw.get("folders") {
        None => Vec::new(),
        Some(Value::Array(a)) if a.len() <= 200 => a
            .iter()
            .map(|f| f.as_str().filter(|f| !f.trim_matches('/').is_empty() && !f.split('/').any(|p| p == "..")).map(|f| f.trim_matches('/').to_string()))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(folders_err)?,
        Some(_) => return Err(folders_err()),
    };
    let labels = labels_raw
        .into_iter()
        .filter(|(k, _)| DEFAULT_LABELS.iter().any(|(d, _)| d == k))
        .map(|(k, v)| (k, v.as_str().unwrap_or_default().to_string()))
        .collect();
    Ok(Layout::new(secs, labels, &crate::util::py_strip(&language).to_lowercase(), &series, folders))
}

// ---------------------------------------------------------------- groups found from file names

/// The words a project may call its numbered units, with the pattern that recognises them in a file or folder name.
pub const GROUP_WORDS: [(&str, &str); 7] =
    [("Folder", "a^"), ("Episode", "episode|ep"), ("Chapter", "chapter|chap|ch"), ("Part", "part"), ("Day", "day"), ("Update", "update|upd"), ("Act", "act")];

fn season_re() -> &'static FancyRegex {
    static R: OnceLock<FancyRegex> = OnceLock::new();
    R.get_or_init(|| FancyRegex::new(r"(?<![a-z0-9])season[ _-]?0*(\d{1,2})(?!\d)").unwrap())
}

/// Pattern that finds the number of a group in a lower-case path; an unknown word counts as "Episode".
pub fn group_regex(word: &str) -> &'static FancyRegex {
    static R: OnceLock<Vec<(&'static str, FancyRegex)>> = OnceLock::new();
    let all = R.get_or_init(|| {
        GROUP_WORDS.iter().map(|(w, pat)| (*w, FancyRegex::new(&format!(r"(?<![a-z0-9])(?:{pat})[ _-]?0*(\d{{1,3}})(?!\d)")).unwrap())).collect()
    });
    all.iter().find(|(w, _)| *w == word).or_else(|| all.iter().find(|(w, _)| *w == "Episode")).map(|(_, r)| r).unwrap()
}

/// All numbers `rx` finds in `text` (group 1 of every match).
fn numbers(rx: &FancyRegex, text: &str) -> Vec<i64> {
    rx.captures_iter(text).filter_map(|c| c.ok()).filter_map(|c| c.get(1)).filter_map(|m| m.as_str().parse().ok()).collect()
}

fn slashed(path: &str) -> String {
    path.replace('\\', "/")
}

/// The group number of a file that lies in one of the project's folders (the longest matching folder wins); `None` otherwise.
pub fn folder_number(path: &str, folders: &[String]) -> Option<usize> {
    let p = slashed(path).to_lowercase();
    let mut best: Option<usize> = None;
    for (i, f) in folders.iter().enumerate() {
        let low = f.to_lowercase().trim_matches('/').to_string();
        if p.starts_with(&format!("{low}/")) && best.is_none_or(|b| low.chars().count() > folders[b - 1].trim_matches('/').chars().count()) {
            best = Some(i + 1);
        }
    }
    best
}

/// Folders that split the files into a handful of groups: the shallowest level (1 to 3 folders deep) with 3 to 60 folders, none of them holding more than
/// 60 % of the files. Returns the folder paths (original spelling), sorted; empty when no level fits.
fn best_folders(names: &[String]) -> Vec<String> {
    let parts: Vec<Vec<String>> = names.iter().map(|n| slashed(n).split('/').map(String::from).collect()).collect();
    for depth in 1..=3usize {
        let mut order: Vec<String> = Vec::new();
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for p in &parts {
            if p.len() > depth {
                let key = p[..depth].join("/");
                if !counts.contains_key(&key) {
                    order.push(key.clone());
                }
                *counts.entry(key).or_insert(0) += 1;
            }
        }
        let total: usize = counts.values().sum();
        if (3..=60).contains(&counts.len()) && total > 0 && *counts.values().max().unwrap() as f64 / total as f64 <= 0.6 {
            order.sort_by_key(|k| k.to_lowercase());
            return order;
        }
    }
    Vec::new()
}

fn file_stem_lower(path: &str) -> String {
    let name = slashed(path).rsplit('/').next().unwrap_or("").to_string();
    let stem = match name.rfind('.') {
        Some(i) => name[..i].to_string(),
        None => name,
    };
    stem.to_lowercase()
}

/// The group number of a file named `<series>`, `<series>2`, `<series>3` ... (the bare name is 1); `None` for any other file.
pub fn series_number(path: &str, series: &str) -> Option<i64> {
    if series.is_empty() {
        return None;
    }
    let stem = file_stem_lower(path);
    let s = series.to_lowercase();
    if stem == s {
        return Some(1);
    }
    let rx = Regex::new(&format!(r"\A{}[ _-]?0*(\d{{1,3}})\z", regex::escape(&s))).ok()?;
    rx.captures(&stem).and_then(|m| m[1].parse().ok())
}

fn stem_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\A([a-z][a-z_]*?)[ _-]?0*(\d{1,3})?\z").unwrap())
}

/// The file name stem whose files form a numbered series (script, script2, script3 ...): (stem, numbers, files). A series needs the numbers 1, 2, 3 ... without a
/// gap (at least three), and in a big project (more than 30 files) at least 15 % of all files: scene families such as `bedroom1`, `bedroom4`, `bedroom5` are not episodes.
fn best_series(paths: &[String]) -> (String, Vec<i64>, usize) {
    let mut order: Vec<String> = Vec::new();
    let mut fam: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    for p in paths {
        let name = p.rsplit('/').next().unwrap_or("");
        let stem = match name.rfind('.') {
            Some(i) => &name[..i],
            None => name,
        };
        if let Some(m) = stem_re().captures(stem) {
            let num = m.get(2).and_then(|n| n.as_str().parse().ok()).unwrap_or(1);
            let key = m[1].to_string();
            if !fam.contains_key(&key) {
                order.push(key.clone());
            }
            let nums = fam.entry(key).or_default();
            if !nums.contains(&num) {
                nums.push(num);
            }
        }
    }
    let mut best: (String, Vec<i64>, usize) = (String::new(), Vec::new(), 0);
    for stem in order {
        let mut ordered = fam[&stem].clone();
        ordered.sort_unstable();
        if ordered.len() < 3 || ordered != (1..=ordered.len() as i64).collect::<Vec<_>>() {
            continue;
        }
        if paths.len() > 30 && (ordered.len() as f64) / (paths.len() as f64) < 0.15 {
            continue;
        }
        if (ordered.len(), stem.as_str()) > (best.1.len(), best.0.as_str()) {
            let n = ordered.len();
            best = (stem, ordered, n);
        }
    }
    best
}

#[derive(Debug, PartialEq)]
pub struct Proposal {
    pub label: String,
    pub groups: Vec<i64>,
    /// (section number, its groups)
    pub sections: Vec<(i64, Vec<i64>)>,
    pub matched: usize,
    pub total: usize,
    pub series: String,
    pub folders: Vec<String>,
}

impl Proposal {
    pub fn to_json(&self) -> Value {
        json!({
            "label": self.label, "groups": self.groups, "matched": self.matched, "total": self.total, "series": self.series, "folders": self.folders,
            "sections": self.sections.iter().map(|(n, g)| json!({"n": n, "groups": g})).collect::<Vec<_>>(),
        })
    }
}

/// Guess the groups of a project from its file names. The word (episode, chapter, part, day, update, act) that names the most different numbers wins;
/// `season N` in a name makes sections. Nothing is written. Empty groups = nothing recognisable.
pub fn propose(names: &[String]) -> Proposal {
    let paths: Vec<String> = names.iter().map(|n| slashed(n).to_lowercase()).collect();
    let mut best: (&str, Vec<i64>, usize) = ("", Vec::new(), 0);
    for (word, _) in GROUP_WORDS {
        let rx = group_regex(word);
        let mut nums: Vec<i64> = Vec::new();
        let mut files = 0usize;
        for p in &paths {
            let found = numbers(rx, p);
            if !found.is_empty() {
                nums.extend(found);
                files += 1;
            }
        }
        nums.sort_unstable();
        nums.dedup();
        if nums.len() >= 2 && (nums.len(), files) > (best.1.len(), best.2) {
            nums.truncate(200);
            // the length used for the comparison above is the one before the cut, like `sorted(nums)[:200]` after `len(nums)`
            best = (word, nums, files);
        }
    }
    let (word, groups, matched) = best;
    let (stem, snums, sfiles) = best_series(&paths);
    let folders = best_folders(names);
    if snums.len() > groups.len() {
        let cut: Vec<i64> = snums.iter().copied().take(200).collect();
        return Proposal { label: "Episode".into(), groups: cut.clone(), sections: vec![(1, cut)], matched: sfiles, total: paths.len(), series: stem, folders };
    }
    if groups.is_empty() {
        return Proposal { label: "Episode".into(), groups: Vec::new(), sections: Vec::new(), matched: 0, total: paths.len(), series: String::new(), folders };
    }
    let rx = group_regex(word);
    let mut season_of_group: BTreeMap<i64, i64> = BTreeMap::new();
    for p in &paths {
        let mut seasons = numbers(season_re(), p);
        seasons.sort_unstable();
        seasons.dedup();
        if seasons.len() == 1 {
            for g in numbers(rx, p) {
                season_of_group.entry(g).or_insert(seasons[0]);
            }
        }
    }
    let distinct: std::collections::BTreeSet<i64> = season_of_group.values().copied().collect();
    let sections = if distinct.len() >= 2 {
        let first = *distinct.iter().next().unwrap();
        let mut by_season: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
        for g in &groups {
            by_season.entry(*season_of_group.get(g).unwrap_or(&first)).or_default().push(*g);
        }
        by_season.into_iter().collect()
    } else {
        vec![(1, groups.clone())]
    };
    Proposal { label: word.into(), groups, sections, matched, total: paths.len(), series: String::new(), folders }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn folder_number_picks_the_longest_folder() {
        let f = names(&["scripts/home", "scripts/home/kitchen", "BG"]);
        assert_eq!(folder_number("scripts\\home\\a.rpy", &f), Some(1));
        assert_eq!(folder_number("scripts/home/kitchen/b.rpy", &f), Some(2));
        assert_eq!(folder_number("bg/x.rpy", &f), Some(3));
        assert_eq!(folder_number("bgx/d.rpy", &f), None);
    }

    #[test]
    fn series_numbers() {
        assert_eq!(series_number("game/Script.rpy", "script"), Some(1));
        assert_eq!(series_number("script_03.rpy", "script"), Some(3));
        assert_eq!(series_number("script_a.rpy", "script"), None);
        assert_eq!(series_number("x.rpy", ""), None);
    }

    #[test]
    fn a_layout_is_validated_like_python() {
        let ok = parse(
            &json!({"language": " CS ", "sections": [{"n": 1, "groups": [1, 2]}, {"n": 2, "status": "planned"}], "labels": {"group": "Chapter", "x": "y"}}),
        )
        .unwrap();
        assert_eq!(
            (ok.language.as_str(), ok.active_groups(), ok.labels["group"].as_str(), ok.labels["section"].as_str()),
            ("cs", vec![1, 2], "Chapter", "Season")
        );
        assert_eq!(parse(&json!([])).unwrap_err().0, "the file must contain a JSON object");
        assert_eq!(parse(&json!({"sections": [{"n": 1, "groups": [1]}, {"n": 2, "groups": [1]}]})).unwrap_err().0, "group 1 belongs to more than one section");
        assert_eq!(parse(&json!({"folders": ["a/../b"]})).unwrap_err().0, "\"folders\" must be a list of folder paths such as \"scripts/A_home\"");
        assert_eq!(parse(&json!({"series": "9x"})).unwrap_err().0, "\"series\" must be a file name stem such as \"script\"");
        assert_eq!(ok.to_json()["sections"][1], json!({"n": 2, "status": "planned"}));
    }
}
