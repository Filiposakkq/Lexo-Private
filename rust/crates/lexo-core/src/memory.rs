//! Port of `core/lexo/memory.py`: the translation memory of the project itself: what was already translated, looked up by the English original.
//!
//! Matching is done on the ORIGINAL text, so declension of the target language does not get in the way of finding a match. Everything here is only offered as a
//! suggestion that the translator inserts and adapts.
//!   exact    the same original (ignoring case, punctuation, tags, variables) already translated -> its translations
//!   similar  an original that differs by a few words -> its translation, with the ratio of similarity
//!   groups   one original translated in several different ways (consistency check)
//! Python walks a `set` of words when it counts the candidates of `similar`; its order is random, so it only shows when more than 80 candidates tie. Here the words are
//! taken in sorted order.
use crate::pyfmt::{casefold, round_digits};
use crate::util::{norm_ws, py_split_ws};
use crate::validator::plain_text;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
use std::sync::OnceLock;

fn word_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"[^\W_]+(?:'[^\W_]+)?").unwrap())
}

fn var_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\[[^\]\[]*\]|\{[^}]*\}").unwrap())
}

pub fn tokens(text: &str) -> Vec<String> {
    word_re().find_iter(&plain_text(text).to_lowercase()).map(|m| m.as_str().to_string()).collect()
}

/// Variables and tags of a text, for a warning that a reused translation carries different ones: (text, how many times) in the order they first appear.
pub fn tags_of(text: &str) -> Vec<(String, usize)> {
    let mut c = crate::util::Counter::default();
    for m in var_re().find_iter(text) {
        c.add(m.as_str().to_string());
    }
    c.order.iter().map(|k| (k.clone(), c.get(k))).collect()
}

/// `difflib.SequenceMatcher(None, a, b, autojunk=False).ratio()`
pub fn ratio(a: &[String], b: &[String]) -> f64 {
    let mut b2j: HashMap<&str, Vec<usize>> = HashMap::new();
    for (j, t) in b.iter().enumerate() {
        b2j.entry(t.as_str()).or_default().push(j);
    }
    let longest = |alo: usize, ahi: usize, blo: usize, bhi: usize| -> (usize, usize, usize) {
        let (mut besti, mut bestj, mut bestsize) = (alo, blo, 0usize);
        let mut j2len: HashMap<usize, usize> = HashMap::new();
        for (i, ai) in a.iter().enumerate().take(ahi).skip(alo) {
            let mut newj2len: HashMap<usize, usize> = HashMap::new();
            if let Some(js) = b2j.get(ai.as_str()) {
                for &j in js {
                    if j < blo {
                        continue;
                    }
                    if j >= bhi {
                        break;
                    }
                    let k = if j == 0 { 0 } else { j2len.get(&(j - 1)).copied().unwrap_or(0) } + 1;
                    newj2len.insert(j, k);
                    if k > bestsize {
                        besti = i + 1 - k;
                        bestj = j + 1 - k;
                        bestsize = k;
                    }
                }
            }
            j2len = newj2len;
        }
        (besti, bestj, bestsize)
    };
    let mut matches = 0usize;
    let mut queue = vec![(0usize, a.len(), 0usize, b.len())];
    while let Some((alo, ahi, blo, bhi)) = queue.pop() {
        let (i, j, k) = longest(alo, ahi, blo, bhi);
        if k > 0 {
            matches += k;
            if alo < i && blo < j {
                queue.push((alo, i, blo, j));
            }
            if i + k < ahi && j + k < bhi {
                queue.push((i + k, ahi, j + k, bhi));
            }
        }
    }
    let total = a.len() + b.len();
    if total == 0 {
        1.0
    } else {
        2.0 * matches as f64 / total as f64
    }
}

#[derive(Debug, Clone)]
pub struct Row {
    pub file: String,
    pub line: i64,
    pub who: String,
    pub source: String,
    pub target: String,
    pub tokens: Vec<String>,
}

pub struct Memory {
    pub rows: Vec<Row>,
    exact: HashMap<Vec<String>, Vec<usize>>,
    inv: HashMap<String, Vec<usize>>,
    pub common: BTreeSet<String>,
}

const COMMON_FRACTION: f64 = 0.04;

fn where_json(r: &Row) -> Value {
    json!({"file": r.file, "line": r.line, "who": r.who})
}

impl Memory {
    /// rows: (file, line, who, source, target) of translated lines.
    pub fn new(rows: &[(String, i64, String, String, String)]) -> Memory {
        let mut m = Memory { rows: Vec::new(), exact: HashMap::new(), inv: HashMap::new(), common: BTreeSet::new() };
        for (file, line, who, source, target) in rows {
            let toks = tokens(source);
            if toks.is_empty() || target.is_empty() {
                continue;
            }
            let i = m.rows.len();
            m.rows.push(Row { file: file.clone(), line: *line, who: who.clone(), source: source.clone(), target: target.clone(), tokens: toks.clone() });
            m.exact.entry(toks.clone()).or_default().push(i);
            for t in toks.iter().collect::<BTreeSet<_>>() {
                m.inv.entry(t.clone()).or_default().push(i);
            }
        }
        let limit = 50f64.max(COMMON_FRACTION * m.rows.len() as f64);
        m.common = m.inv.iter().filter(|(_, ids)| ids.len() as f64 > limit).map(|(t, _)| t.clone()).collect();
        m
    }

    /// Group rows by identical translation (case and spacing ignored), most used first.
    fn grouped(rows: &[&Row], skip: Option<(&str, i64)>) -> Vec<Value> {
        let mut groups: Vec<(String, usize, Vec<Value>)> = Vec::new();
        let mut at: HashMap<String, usize> = HashMap::new();
        for r in rows {
            if skip.is_some_and(|(f, l)| r.file == f && r.line == l) {
                continue;
            }
            let key = casefold(&norm_ws(&r.target));
            let i = *at.entry(key).or_insert_with(|| {
                groups.push((r.target.clone(), 0, Vec::new()));
                groups.len() - 1
            });
            groups[i].1 += 1;
            if groups[i].2.len() < 5 {
                groups[i].2.push(where_json(r));
            }
        }
        groups.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        groups.into_iter().map(|(target, count, lines)| json!({"target": target, "count": count, "lines": lines})).collect()
    }

    pub fn exact_matches(&self, source: &str, skip: Option<(&str, i64)>) -> Vec<Value> {
        let toks = tokens(source);
        if toks.is_empty() {
            return Vec::new();
        }
        let rows: Vec<&Row> = self.exact.get(&toks).map(|ids| ids.iter().map(|i| &self.rows[*i]).collect()).unwrap_or_default();
        Self::grouped(&rows, skip)
    }

    pub fn similar(&self, source: &str, skip: Option<(&str, i64)>, limit: usize, min_ratio: f64) -> Vec<Value> {
        let toks = tokens(source);
        if toks.len() < 2 {
            return Vec::new();
        }
        // how many words of the source each row shares (the order of first sight is kept: ties are resolved by it)
        let mut order: Vec<usize> = Vec::new();
        let mut score: HashMap<usize, usize> = HashMap::new();
        for t in toks.iter().collect::<BTreeSet<_>>().into_iter().filter(|t| !self.common.contains(*t)) {
            for &i in self.inv.get(t).map(|v| &v[..v.len().min(400)]).unwrap_or(&[]) {
                if !score.contains_key(&i) {
                    order.push(i);
                }
                *score.entry(i).or_insert(0) += 1;
            }
        }
        order.sort_by_key(|i| std::cmp::Reverse(score[i])); // stable, like `most_common`
        let mut found: Vec<(String, f64, Value)> = Vec::new();
        let mut at: HashMap<String, usize> = HashMap::new();
        for &i in order.iter().take(80) {
            let r = &self.rows[i];
            if skip.is_some_and(|(f, l)| r.file == f && r.line == l) || r.tokens == toks {
                continue;
            }
            let rt = ratio(&toks, &r.tokens);
            if rt < min_ratio {
                continue;
            }
            let key = casefold(&norm_ws(&r.target));
            let entry = json!({"source": r.source, "target": r.target, "ratio": round_digits(rt, 2), "file": r.file, "line": r.line, "who": r.who});
            let rounded = round_digits(rt, 2);
            match at.get(&key) {
                Some(&pos) if rt > found[pos].1 => found[pos] = (key, rounded, entry),
                Some(_) => {}
                None => {
                    at.insert(key.clone(), found.len());
                    found.push((key, rounded, entry));
                }
            }
        }
        found.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then_with(|| a.2["target"].as_str().cmp(&b.2["target"].as_str())));
        found.into_iter().take(limit).map(|f| f.2).collect()
    }

    /// Originals that were translated in more than one way. By default the original must be EXACTLY the same text (case, punctuation and tags included); `loose`
    /// compares only the words; `by_speaker` keeps lines of different speakers apart.
    pub fn groups(&self, limit: usize, loose: bool, by_speaker: bool) -> Vec<Value> {
        let mut order: Vec<String> = Vec::new();
        let mut buckets: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, r) in self.rows.iter().enumerate() {
            let base = if loose { r.tokens.join(" ") } else { format!("={}", norm_ws(&r.source)) };
            let who = if by_speaker && !r.who.is_empty() { py_split_ws(&r.who).first().copied().unwrap_or("").to_string() } else { String::new() };
            let key = if by_speaker { format!("{base}|{who}") } else { base };
            if !buckets.contains_key(&key) {
                order.push(key.clone());
            }
            buckets.entry(key).or_default().push(i);
        }
        let mut out: Vec<(usize, String, Value)> = Vec::new();
        for key in order {
            let ids = &buckets[&key];
            if ids.len() < 2 {
                continue;
            }
            let rows: Vec<&Row> = ids.iter().map(|i| &self.rows[*i]).collect();
            let mut variants: Vec<(String, usize, Vec<Value>)> = Vec::new();
            let mut at: HashMap<String, usize> = HashMap::new();
            for r in &rows {
                let k = casefold(&norm_ws(&r.target));
                let i = *at.entry(k).or_insert_with(|| {
                    variants.push((r.target.clone(), 0, Vec::new()));
                    variants.len() - 1
                });
                variants[i].1 += 1;
                variants[i].2.push(where_json(r));
            }
            if variants.len() < 2 {
                continue;
            }
            variants.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            let sources: BTreeSet<String> = rows.iter().map(|r| norm_ws(&r.source)).collect();
            let value = json!({
                "key": key, "source": rows[0].source, "total": rows.len(), "sources": sources,
                "variants": variants.into_iter().map(|(target, count, lines)| json!({"target": target, "count": count, "lines": lines})).collect::<Vec<_>>(),
            });
            out.push((rows.len(), key, value));
        }
        out.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        out.into_iter().take(limit).map(|g| g.2).collect()
    }
}
