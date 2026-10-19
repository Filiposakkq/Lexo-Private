//! Spelling with a dictionary (the Pro feature `spell.hunspell`): the translated lines are checked against a Hunspell dictionary (`.dic` + `.aff`) of the language of the project, which the user
//! adds (it is copied to the settings folder of the application). The checking itself (reading the dictionary, taking the affixes off, the suggestions) is the Pro pack's; Core chooses the words
//! (the translated text without tags and variables, never the words of the original or of the glossary, nor those the user said to ignore), shows what the pack did not recognise and, for a
//! correction, plans the lines to change; the change itself is written with `mt/apply` (one undoable step, approved lines are never overwritten). The free check from the project's own
//! vocabulary (`spelling.rs`) is separate and stays as it is. These calls are not in the contract of Core.
use crate::app::App;
use crate::app_base::{app_err, Res};
use crate::config::config_dir;
use crate::fsutil;
use crate::messages::{msg, py_str};
use crate::project::DATA_DIR_NAME;
use crate::util::py_strip;
use crate::validator::plain_text;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

pub const FEATURE: &str = "spell.hunspell";

/// What is listed at most (the rest stays for the next run, after the first ones are corrected or ignored).
const MAX_ITEMS: usize = 500;
const MAX_WORDS: usize = 20000;
const MAX_PLACES: usize = 6;
const MAX_REPLACE: usize = 500;

/// The words of a text with their `-` and `'` parts: `don't`, `well-known`.
pub fn tokens(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let (mut out, mut i) = (Vec::new(), 0);
    let letter = |c: char| c.is_alphabetic();
    while i < chars.len() {
        if !letter(chars[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && letter(chars[i]) {
            i += 1;
        }
        // a part joined by one - or ' that is followed by a letter
        while i + 1 < chars.len() && matches!(chars[i], '-' | '\'' | '’') && letter(chars[i + 1]) {
            i += 1;
            while i < chars.len() && letter(chars[i]) {
                i += 1;
            }
        }
        out.push(chars[start..i].iter().collect());
    }
    out
}

/// Replace the whole word `word` by `to`, but not inside `[variables]`, `{tags}` or `%(names)s`. Returns the new text and how many were replaced.
pub fn replace_word(text: &str, word: &str, to: &str) -> (String, usize) {
    if word.is_empty() {
        return (text.to_string(), 0);
    }
    let chars: Vec<char> = text.chars().collect();
    let w: Vec<char> = word.chars().collect();
    let (mut out, mut i, mut depth, mut n) = (String::new(), 0usize, 0i32, 0usize);
    let word_char = |c: char| c.is_alphanumeric() || c == '_';
    while i < chars.len() {
        let c = chars[i];
        match c {
            '[' | '{' => depth += 1,
            ']' | '}' => depth = (depth - 1).max(0),
            _ => {}
        }
        let fits = depth == 0
            && chars[i..].starts_with(&w)
            && (i == 0 || !word_char(chars[i - 1]))
            && chars.get(i + w.len()).is_none_or(|x| !word_char(*x))
            && !(i >= 2 && chars[i - 1] == '(' && chars[i - 2] == '%');
        if fits {
            out.push_str(to);
            i += w.len();
            n += 1;
        } else {
            out.push(c);
            i += 1;
        }
    }
    (out, n)
}

fn file_line(x: &Value) -> (String, i64) {
    (py_str(&x["file"]), x["line"].as_i64().or_else(|| py_str(&x["line"]).trim().parse().ok()).unwrap_or(0))
}

/// The `.dic` files of a folder that are dictionaries of words (not hyphenation `hyph_*` or thesaurus `th_*` files).
fn dic_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
            n.ends_with(".dic") && !n.starts_with("hyph") && !n.starts_with("th_")
        })
        .collect();
    v.sort();
    v
}

impl App {
    fn spell_gate(&self) -> Res<()> {
        self.gate(FEATURE).map(|_| ())
    }

    /// The folder of the dictionary of this project's language, in the settings of the application (not in the project: a dictionary is not shared).
    fn spell_dir(&self) -> Res<PathBuf> {
        let language = self.o()?.store.layout.language.clone();
        let name: String = language.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
        Ok(config_dir().join("dictionaries").join(if name.is_empty() { "default".to_string() } else { name }))
    }

    fn spell_files(&self) -> Res<Option<(PathBuf, PathBuf, String)>> {
        let dir = self.spell_dir()?;
        let (dic, aff) = (dir.join("dict.dic"), dir.join("dict.aff"));
        if !dic.is_file() || !aff.is_file() {
            return Ok(None);
        }
        let name = std::fs::read_to_string(dir.join("name.txt")).unwrap_or_default().trim().to_string();
        Ok(Some((dic, aff, name)))
    }

    fn spell_ignore_path(&self) -> Res<PathBuf> {
        Ok(self.o()?.project.root.join(DATA_DIR_NAME).join("pro").join("spelling.json"))
    }

    /// The words the user said to ignore (lower case), shared with the project.
    fn spell_ignored(&self) -> Res<BTreeSet<String>> {
        let text = std::fs::read_to_string(self.spell_ignore_path()?).unwrap_or_default();
        let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap_or_else(|_| json!({}));
        Ok(v["ignore"].as_array().into_iter().flatten().map(|w| py_str(w).to_lowercase()).collect())
    }

    fn spell_ignored_save(&self, words: &BTreeSet<String>) -> Res<()> {
        let path = self.spell_ignore_path()?;
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let text = serde_json::to_string_pretty(&json!({"ignore": words.iter().collect::<Vec<_>>()})).unwrap_or_default();
        fsutil::atomic_write_text(&path, &text, Some("\n"), None, &*self.clock)
            .map_err(|e| app_err(msg("The list of ignored words could not be saved: {reason}", &[("reason", json!(e.to_string()))])))
    }

    /// `{available, language, installed, name, ignored}`: what the spelling panel shows first.
    pub fn spell_state(&self) -> Res<Value> {
        self.need_project()?;
        let o = self.o()?;
        let files = self.spell_files()?;
        Ok(json!({
            "available": self.ext.decide(FEATURE).available,
            "language": o.store.layout.language,
            "installed": files.is_some(),
            "name": files.map(|f| f.2).unwrap_or_default(),
            "ignored": self.spell_ignored()?.len(),
        }))
    }

    /// Add the dictionary of this project's language from a `.dic` file or a folder that holds one (its `.aff` file lies next to it): both are copied to the settings of the application.
    pub fn spell_dict_add(&mut self, path: &str) -> Res<Value> {
        self.need_project()?;
        self.spell_gate()?;
        let p = PathBuf::from(py_strip(path).trim_matches('"'));
        let dic = if p.is_dir() {
            let found = dic_files(&p);
            match found.len() {
                0 => return Err(app_err(msg("There is no dictionary (.dic file) in this folder.", &[]))),
                1 => found[0].clone(),
                _ => {
                    let names: Vec<String> = found.iter().filter_map(|f| f.file_name().map(|n| n.to_string_lossy().into_owned())).collect();
                    return Err(app_err(msg(
                        "There are several dictionaries in this folder ({names}); choose the .dic file itself.",
                        &[("names", json!(names.join(", ")))],
                    )));
                }
            }
        } else if p.is_file() {
            p
        } else {
            return Err(app_err(msg("The dictionary was not found: {path}", &[("path", json!(path))])));
        };
        let aff = dic.with_extension("aff");
        if !aff.is_file() {
            return Err(app_err(msg(
                "The dictionary needs its .aff file next to it ({name}).",
                &[("name", json!(aff.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()))],
            )));
        }
        let dir = self.spell_dir()?;
        let io = |e: std::io::Error| app_err(msg("The dictionary could not be copied: {reason}", &[("reason", json!(e.to_string()))]));
        std::fs::create_dir_all(&dir).map_err(io)?;
        std::fs::copy(&dic, dir.join("dict.dic")).map_err(io)?;
        std::fs::copy(&aff, dir.join("dict.aff")).map_err(io)?;
        let name = dic.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        std::fs::write(dir.join("name.txt"), &name).map_err(io)?;
        // read it once now: a broken dictionary is reported at once, not at the first check
        let probe = self.ext_call(
            FEATURE,
            "check",
            &json!({"dic": dir.join("dict.dic").to_string_lossy(), "aff": dir.join("dict.aff").to_string_lossy(), "words": []}),
        );
        if let Err(e) = probe {
            let _ = std::fs::remove_file(dir.join("dict.dic"));
            let _ = std::fs::remove_file(dir.join("dict.aff"));
            return Err(e);
        }
        Ok(json!({"ok": true, "name": name}))
    }

    pub fn spell_dict_remove(&mut self) -> Res<Value> {
        self.need_project()?;
        let dir = self.spell_dir()?;
        for f in ["dict.dic", "dict.aff", "name.txt"] {
            let _ = std::fs::remove_file(dir.join(f));
        }
        Ok(json!({"ok": true}))
    }

    /// Words to ignore from now on (shared with the project).
    pub fn spell_ignore(&mut self, words: &Value) -> Res<Value> {
        self.need_project()?;
        let mut set = self.spell_ignored()?;
        for w in words.as_array().into_iter().flatten() {
            let w = py_strip(&py_str(w)).to_lowercase();
            if !w.is_empty() && w.chars().count() < 80 {
                set.insert(w);
            }
        }
        self.spell_ignored_save(&set)?;
        Ok(json!({"ok": true, "ignored": set.len()}))
    }

    /// The words the dictionary does not know, with how often and where they occur and what the pack suggests: `{items: [{word, count, places, suggestions}], words, truncated}`.
    pub fn spell_check(&self, file: &str, lines: &Value) -> Res<Value> {
        self.need_project()?;
        self.spell_gate()?;
        let Some((dic, aff, _)) = self.spell_files()? else {
            return Err(app_err(msg("No dictionary is added for this language yet.", &[])));
        };
        let o = self.o()?;
        let want: Option<HashSet<(String, i64)>> = lines.as_array().filter(|a| !a.is_empty()).map(|a| a.iter().map(file_line).collect());
        let files: Vec<String> = match &want {
            Some(w) => w.iter().map(|x| x.0.clone()).collect::<BTreeSet<_>>().into_iter().collect(),
            None if file.is_empty() => o.project.files.keys().cloned().collect(),
            None => vec![file.to_string()],
        };
        let mut known: HashSet<String> = HashSet::new();
        for (c, g) in &o.store.glossary {
            known.extend(tokens(c).into_iter().chain(tokens(&g.n)).map(|w| w.to_lowercase()));
        }
        let ignored = self.spell_ignored()?;
        let mut seen: BTreeMap<String, (usize, Vec<(String, i64)>)> = BTreeMap::new();
        for rel in files {
            if !o.project.files.contains_key(&rel) {
                return Err(crate::app::unknown_file());
            }
            for e in o.project.entries(&rel) {
                if (e.unit != "dialogue" && e.unit != "string") || e.target.trim().is_empty() || !o.project.is_translated(&o.store, &rel, e) {
                    continue;
                }
                if want.as_ref().is_some_and(|w| !w.contains(&(rel.clone(), e.line as i64))) {
                    continue;
                }
                let src: HashSet<String> = tokens(&plain_text(e.source.as_deref().unwrap_or(""))).into_iter().map(|w| w.to_lowercase()).collect();
                for w in tokens(&plain_text(&e.target)) {
                    let lower = w.to_lowercase();
                    let letters = w.chars().count();
                    let shouting = letters <= 5 && w.chars().all(|c| !c.is_lowercase());
                    if letters < 2 || shouting || src.contains(&lower) || known.contains(&lower) || ignored.contains(&lower) {
                        continue;
                    }
                    let slot = seen.entry(w).or_insert((0, Vec::new()));
                    slot.0 += 1;
                    if slot.1.len() < MAX_PLACES {
                        slot.1.push((rel.clone(), e.line as i64));
                    }
                }
            }
        }
        let words: Vec<&String> = seen.keys().take(MAX_WORDS).collect();
        let res = self.ext_call(FEATURE, "check", &json!({"dic": dic.to_string_lossy(), "aff": aff.to_string_lossy(), "words": words}))?;
        let mut items: Vec<Value> = res["unknown"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|u| {
                let w = py_str(&u["word"]);
                let (count, places) = seen.get(&w)?;
                Some(json!({"word": w, "count": count, "suggestions": u["suggestions"], "places": places.iter().map(|(f, l)| json!({"file": f, "line": l})).collect::<Vec<_>>()}))
            })
            .collect();
        items.sort_by(|a, b| {
            b["count"].as_u64().cmp(&a["count"].as_u64()).then_with(|| py_str(&a["word"]).to_lowercase().cmp(&py_str(&b["word"]).to_lowercase()))
        });
        let truncated = items.len() > MAX_ITEMS || seen.len() > MAX_WORDS;
        items.truncate(MAX_ITEMS);
        Ok(json!({"items": items, "words": seen.len(), "dictionary_words": res["roots"], "truncated": truncated}))
    }

    /// The lines that would change if `word` became `to`, as `{file, line, before, after}` (not inside tags or variables; approved lines are listed but marked, and `mt/apply` leaves them).
    pub fn spell_replace_plan(&self, word: &str, to: &str) -> Res<Value> {
        self.need_project()?;
        self.spell_gate()?;
        if py_strip(word).is_empty() || py_strip(to).is_empty() {
            return Err(app_err(msg("Enter the word and its replacement.", &[])));
        }
        let o = self.o()?;
        let mut items: Vec<Value> = Vec::new();
        let mut total = 0usize;
        for (rel, _) in o.project.files.iter() {
            for e in o.project.entries(rel) {
                if (e.unit != "dialogue" && e.unit != "string") || e.target.trim().is_empty() {
                    continue;
                }
                let (after, n) = replace_word(&e.target, word, to);
                if n == 0 {
                    continue;
                }
                total += 1;
                if items.len() < MAX_REPLACE {
                    items.push(json!({"file": rel, "line": e.line, "before": e.target, "after": after, "approved": self.entry_row(rel, e) == "approved"}));
                }
            }
        }
        Ok(json!({"items": items, "total": total, "truncated": total > items.len()}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_keep_their_hyphen_and_apostrophe_parts() {
        assert_eq!(tokens("It's a well-known tale, isn’t it? 2024 - end"), vec!["It's", "a", "well-known", "tale", "isn’t", "it", "end"]);
    }

    #[test]
    fn a_word_is_replaced_whole_and_never_inside_a_variable_or_a_tag() {
        assert_eq!(
            replace_word("Ahoj [hero], hero! Heroes {b}hero{/b} %(hero)s", "hero", "hrdina"),
            ("Ahoj [hero], hrdina! Heroes {b}hrdina{/b} %(hero)s".to_string(), 2)
        );
        assert_eq!(replace_word("preložit, preložit.", "preložit", "přeložit").1, 2);
        assert_eq!(replace_word("nothing here", "x", "y").1, 0);
    }
}
