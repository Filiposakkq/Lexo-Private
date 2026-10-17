//! Port of `core/lexo/lang.py`: the language packs (data files, no code). So far: the pack itself, reading one pack folder, the plural rules.
//! Discovery in the user's folder and the interface texts (`ui.json`) come with the data files (M3) and the operations (M6).
//! The pack is passed to the functions that need it (the Python module keeps a global "current pack").
use crate::util::fold;
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub const FORMAT: i64 = 1;

#[derive(Debug, PartialEq)]
pub struct PackError(pub String);

impl std::fmt::Display for PackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// How a word is reduced so that glossary terms are found in any inflected form.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stem {
    /// The exact word (folded).
    None,
    /// Drop the last `drop` letters, keep at least `min`.
    Cut { drop: usize, min: usize },
}

impl Stem {
    pub fn apply(&self, word: &str) -> String {
        let folded = fold(word);
        match *self {
            Stem::None => folded,
            Stem::Cut { drop, min } => {
                let n = folded.chars().count();
                folded.chars().take(min.max(n.saturating_sub(drop))).collect()
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Pack {
    pub code: String,
    pub name: String,
    pub version: i64,
    pub repeat_ok: HashSet<String>,
    pub check_repeats: bool,
    pub number_words_ok: bool,
    pub stem: Stem,
    /// The letters that belong to the language and are not used in the source language; empty = any non-ASCII letter counts.
    pub letters: HashSet<char>,
    /// Name of a rule in `plural_rule` (how many forms a counted text has, which one is used).
    pub plural: String,
    pub folder: Option<PathBuf>,
}

impl Pack {
    /// The neutral pack `und`, built in and used when no pack matches.
    pub fn neutral() -> Pack {
        Pack {
            code: "und".into(),
            name: "Neutral".into(),
            version: 0,
            repeat_ok: HashSet::new(),
            check_repeats: false,
            number_words_ok: false,
            stem: Stem::None,
            letters: HashSet::new(),
            plural: "en".into(),
            folder: None,
        }
    }
}

/// Plural rules by name: (number of forms, number -> index of the form). The browser has the same table (ui/js/05-i18n.js); both must agree.
type PluralRule = (usize, fn(i64) -> usize);

fn plural_rule(name: &str) -> Option<PluralRule> {
    Some(match name {
        "en" => (2, |n| usize::from(n != 1)),
        "cs" | "sk" => (3, |n| {
            if n == 1 {
                0
            } else if (2..=4).contains(&n) {
                1
            } else {
                2
            }
        }),
        "pl" => (3, |n| {
            if n == 1 {
                0
            } else if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) {
                1
            } else {
                2
            }
        }),
        "ru" => (3, |n| {
            if n % 10 == 1 && n % 100 != 11 {
                0
            } else if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) {
                1
            } else {
                2
            }
        }),
        "fr" => (2, |n| usize::from(n != 0 && n != 1)),
        _ => return None,
    })
}

pub fn plural_forms(rule: &str) -> Option<usize> {
    plural_rule(rule).map(|(n, _)| n)
}

/// Index of the form of a counted text for the number `n` under the named rule (unknown rule = English).
pub fn plural_index(rule: &str, n: i64) -> usize {
    let (forms, f) = plural_rule(rule).or_else(|| plural_rule("en")).unwrap();
    f(n.abs()).min(forms - 1)
}

fn read(path: &Path) -> Result<Value, PackError> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let bytes = std::fs::read(path).map_err(|e| PackError(format!("{name}: {e}")))?;
    let text = String::from_utf8(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes).to_vec()).map_err(|e| PackError(format!("{name}: {e}")))?;
    serde_json::from_str(&text).map_err(|e| PackError(format!("{name}: {e}")))
}

fn whole_number(v: &Value) -> Option<i64> {
    v.as_i64()
}

fn stem_rule(raw: Option<&Value>) -> Result<Stem, PackError> {
    let Some(raw) = raw.filter(|v| !v.is_null()) else { return Ok(Stem::None) };
    let kind = raw.get("type").and_then(Value::as_str);
    if !raw.is_object() || !matches!(kind, Some("none" | "cut")) {
        return Err(PackError("rules.json: \"stem\" must be {\"type\": \"none\"} or {\"type\": \"cut\", \"drop\": N, \"min\": M}".into()));
    }
    if kind == Some("none") {
        return Ok(Stem::None);
    }
    let drop = raw.get("drop").and_then(whole_number);
    let min = raw.get("min").map_or(Some(3), whole_number);
    match (drop, min) {
        (Some(d), Some(m)) if d >= 0 && m >= 0 => Ok(Stem::Cut { drop: d as usize, min: m as usize }),
        _ => Err(PackError("rules.json: stem \"drop\" and \"min\" must be whole numbers, 0 or more".into())),
    }
}

/// Read one pack folder (`pack.json` and `rules.json`); the error says what is wrong.
pub fn load_dir(folder: &Path) -> Result<Pack, PackError> {
    let meta = read(&folder.join("pack.json"))?;
    let rules = read(&folder.join("rules.json"))?;
    let (Some(meta), Some(rules)) = (meta.as_object(), rules.as_object()) else {
        return Err(PackError("pack.json and rules.json must contain JSON objects".into()));
    };
    if meta.get("format").and_then(Value::as_i64) != Some(FORMAT) {
        let shown = meta.get("format").map_or("None".to_string(), |v| v.to_string());
        return Err(PackError(format!("pack.json: unsupported format {shown} (this app reads {FORMAT})")));
    }
    let code = meta.get("code").and_then(Value::as_str).filter(|c| !c.trim().is_empty());
    let name = meta.get("name").and_then(Value::as_str);
    let (Some(code), Some(name)) = (code, name) else { return Err(PackError("pack.json needs a text \"code\" and \"name\"".into())) };
    let version = match meta.get("version") {
        None => 0,
        Some(v) => v.as_i64().ok_or_else(|| PackError("pack.json: \"version\" must be a whole number".into()))?,
    };
    let repeat_ok_err = || PackError("rules.json: \"repeat_ok\" must be a list of words".into());
    let repeat_ok: HashSet<String> = match rules.get("repeat_ok") {
        None => HashSet::new(),
        Some(v) => {
            v.as_array().ok_or_else(repeat_ok_err)?.iter().map(|w| w.as_str().map(str::to_lowercase).ok_or_else(repeat_ok_err)).collect::<Result<_, _>>()?
        }
    };
    let flag = |key: &str| match rules.get(key) {
        None => Ok(false),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(PackError(format!("rules.json: \"{key}\" must be true or false"))),
    };
    let (check_repeats, number_words_ok) = (flag("check_repeats")?, flag("number_words_ok")?);
    let letters: HashSet<char> = match rules.get("letters") {
        None => HashSet::new(),
        Some(Value::String(s)) => s.chars().collect(),
        Some(_) => return Err(PackError("rules.json: \"letters\" must be a text holding the letters of the language".into())),
    };
    let plural = match rules.get("plural") {
        None => "en".to_string(),
        Some(v) => match v.as_str().filter(|p| plural_rule(p).is_some()) {
            Some(p) => p.to_string(),
            None => {
                let known = ["cs", "en", "fr", "pl", "ru", "sk"].join(", ");
                let shown = v.as_str().map_or(v.to_string(), |s| format!("'{s}'"));
                return Err(PackError(format!("rules.json: unknown \"plural\" rule {shown} (known: {known})")));
            }
        },
    };
    Ok(Pack {
        code: code.trim().to_lowercase(),
        name: name.to_string(),
        version,
        repeat_ok,
        check_repeats,
        number_words_ok,
        stem: stem_rule(rules.get("stem"))?,
        letters,
        plural,
        folder: Some(folder.to_path_buf()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plural_rules_match_the_table_of_the_python_module() {
        let cs = [(0, 2), (1, 0), (2, 1), (4, 1), (5, 2), (21, 2)];
        for (n, i) in cs {
            assert_eq!(plural_index("cs", n), i, "cs {n}");
        }
        assert_eq!([plural_index("pl", 12), plural_index("pl", 22), plural_index("pl", 5)], [2, 1, 2]);
        assert_eq!([plural_index("ru", 21), plural_index("ru", 11), plural_index("ru", 3)], [0, 2, 1]);
        assert_eq!([plural_index("fr", 0), plural_index("fr", 2)], [0, 1]);
        assert_eq!(plural_index("zz", 3), 1);
        assert_eq!(plural_index("cs", -1), 0);
    }

    #[test]
    fn stems_cut_the_end_but_keep_a_minimum() {
        let cut = Stem::Cut { drop: 2, min: 3 };
        assert_eq!(cut.apply("Příteli"), "prite");
        assert_eq!(cut.apply("Já"), "ja");
        assert_eq!(Stem::None.apply("Příteli"), "priteli");
    }
}
