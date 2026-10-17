//! Port of `core/lexo/spelling.py`: typo hints without a dictionary: the project's own translated text is the vocabulary.
//!
//! A word that occurs once or twice but looks like a mistyped form of a word used many times is reported (missing diacritics, swapped letters, a doubled
//! letter). These are hints only (info / warning, never errors). Words from the source and glossary entries are never reported.
use crate::messages::msg;
use crate::util::{fold, sha1_hex, Counter};
use crate::validator::{plain_text, Issue, SpellCheck};
use regex::Regex;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

/// A "correct" word must occur at least this often ...
const MIN_FREQUENT: usize = 5;
/// ... and this many times more often than the suspicious one.
const FACTOR: usize = 5;
/// A suspicious word occurs at most this often.
const RARE: usize = 3;
const MIN_LEN: usize = 4;

fn word_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"[^\W\d_]{3,}").unwrap())
}

pub fn words_of(text: &str) -> Vec<String> {
    word_re().find_iter(&plain_text(text)).map(|m| m.as_str().to_lowercase()).collect()
}

pub struct Spelling {
    pub vocab: Counter,
    known: HashSet<String>,
    pub by_fold: HashMap<String, String>,
    pub sig: String,
}

impl Spelling {
    pub fn new(vocab: Counter, known: &[String]) -> Spelling {
        let known: HashSet<String> = known.iter().map(|k| k.to_lowercase()).collect();
        let mut by_fold: HashMap<String, String> = HashMap::new();
        for w in &vocab.order {
            let n = vocab.get(w);
            let f = fold(w);
            if f != *w && n >= MIN_FREQUENT && n > by_fold.get(&f).map_or(0, |best| vocab.get(best)) {
                by_fold.insert(f, w.clone());
            }
        }
        let total: usize = vocab.map.values().sum();
        let sig = sha1_hex(format!("{}|{}|{}", vocab.map.len(), total, known.len()).as_bytes())[..12].to_string();
        Spelling { vocab, known, by_fold, sig }
    }

    /// `targets`: the translated texts.
    pub fn build<S: AsRef<str>>(targets: &[S], known: &[String]) -> Spelling {
        Spelling::new(Counter::from(targets.iter().flat_map(|t| words_of(t.as_ref()))), known)
    }

    fn frequent(&self, v: &str, c: usize) -> bool {
        let n = self.vocab.get(v);
        n >= MIN_FREQUENT && n >= FACTOR * c.max(1)
    }

    /// (kind, better word) for a suspicious word, or `None`.
    pub fn suggest(&self, w: &str) -> Option<(&'static str, String)> {
        let c = self.vocab.get(w);
        let chars: Vec<char> = w.chars().collect();
        if chars.len() < MIN_LEN || c > RARE || self.known.contains(w) {
            return None;
        }
        if fold(w) == w {
            if let Some(better) = self.by_fold.get(w).filter(|b| self.frequent(b, c)) {
                return Some(("diacritics", better.clone()));
            }
        }
        for i in 1..chars.len().saturating_sub(3) {
            // swapped neighbours, not in the ending
            if chars[i] != chars[i + 1] {
                let mut v = chars.clone();
                v.swap(i, i + 1);
                let v: String = v.into_iter().collect();
                if self.frequent(&v, c) {
                    return Some(("swap", v));
                }
            }
        }
        for i in 0..chars.len() - 1 {
            // a letter typed twice
            if chars[i] == chars[i + 1] {
                let mut v = chars.clone();
                v.remove(i);
                let v: String = v.into_iter().collect();
                if self.frequent(&v, c) {
                    return Some(("double", v));
                }
            }
        }
        None
    }
}

impl SpellCheck for Spelling {
    fn check(&self, source: &str, target: &str) -> Vec<Issue> {
        let src_words: HashSet<String> = words_of(source).into_iter().collect();
        let mut out = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for w in words_of(target) {
            if seen.contains(&w) || src_words.contains(&w) {
                continue;
            }
            seen.insert(w.clone());
            let Some((kind, better)) = self.suggest(&w) else { continue };
            let (text, sev) = if kind == "diacritics" {
                (
                    msg("Missing diacritics? “{w}” → “{better}”.", &[("w", json!(w)), ("better", json!(better))]),
                    if w.chars().count() >= 6 { "warning" } else { "info" },
                )
            } else {
                (msg("A typo? “{w}” → “{better}”.", &[("w", json!(w)), ("better", json!(better))]), "warning")
            };
            out.push(
                Issue::new(sev, "typo-suspect", text)
                    .cat("text")
                    .extra(json!({"fixable": true, "kind": "typo", "word": w, "better": better, "typo_kind": kind})),
            );
            if out.len() >= 3 {
                break;
            }
        }
        out
    }
}
