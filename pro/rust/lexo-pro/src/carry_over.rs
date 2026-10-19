//! Carry a translation over to a new version of a game (the Pro feature `carry.over`).
//!
//! A game is updated: lines are edited, scenes are renamed or moved (their translate ids change), lines are added and removed. The translation of the old version is
//! looked up for every line of the new version, by the ORIGINAL text (not by the id, which changes):
//!   exact  the same original (ignoring case, punctuation, tags, variables), the same speaker, the same variables -> the translation is carried over as it is
//!   near   the original differs by a few words (ratio of similarity >= `min_ratio`) -> the translation is carried over and the line is marked for a review
//!   none   nothing found -> the line stays untranslated
//! Plain data in, plain data out: Core reads the old project and the new files and writes what the user confirms.
use lexo_core::memory::{ratio, tags_of, tokens};
use serde_json::{json, Value};
use std::collections::HashMap;

/// A line of the old version (translated) or of the new one (`target` empty): `id` is whatever the caller wants back (file and key).
#[derive(Debug, Clone)]
pub struct Line {
    pub id: String,
    pub who: String,
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub new: String,
    pub old: String,
    pub kind: &'static str,
    pub ratio: f64,
    pub target: String,
    /// The variables or tags of the new original differ from the old one: the carried translation may need a fix.
    pub tags_differ: bool,
}

/// The words of an original, joined: two originals that differ only in case, punctuation, tags or variables have the same key.
fn key(source: &str) -> String {
    tokens(source).join(" ")
}

fn same_who(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// The most frequent translation in a list (the first one on a tie).
fn most_common(targets: &[usize], old: &[Line]) -> usize {
    let mut best = targets[0];
    let mut best_n = 0;
    for &t in targets {
        let n = targets.iter().filter(|&&u| old[u].target == old[t].target).count();
        if n > best_n {
            best = t;
            best_n = n;
        }
    }
    best
}

/// Looks up the translation of every line of `new` in `old`. Lines of `old` without a translation are ignored. The order of the answer is the order of `new`.
pub fn find(old: &[Line], new: &[Line], min_ratio: f64) -> Vec<Match> {
    let old: Vec<Line> = old.iter().filter(|l| !l.target.trim().is_empty() && !key(&l.source).is_empty()).cloned().collect();
    let old_tokens: Vec<Vec<String>> = old.iter().map(|l| tokens(&l.source)).collect();
    // the lines of the old version by their key and speaker, and an index of the words (for the lookup of similar lines)
    let mut by_key: HashMap<(String, String), Vec<usize>> = HashMap::new();
    let mut postings: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, l) in old.iter().enumerate() {
        by_key.entry((old_tokens[i].join(" "), l.who.trim().to_lowercase())).or_default().push(i);
        let mut seen = std::collections::HashSet::new();
        for t in &old_tokens[i] {
            if seen.insert(t.as_str()) {
                postings.entry(t.as_str()).or_default().push(i);
            }
        }
    }
    let mut out = Vec::new();
    for n in new {
        let nt = tokens(&n.source);
        if nt.is_empty() {
            continue;
        }
        let who = n.who.trim().to_lowercase();
        // exact: the same words and the same variables; when the variables differ the line is only "near"
        if let Some(c) = by_key.get(&(nt.join(" "), who.clone())) {
            let same_tags: Vec<usize> = c.iter().copied().filter(|&i| tags_of(&old[i].source) == tags_of(&n.source)).collect();
            if !same_tags.is_empty() {
                let i = most_common(&same_tags, &old);
                out.push(Match { new: n.id.clone(), old: old[i].id.clone(), kind: "exact", ratio: 1.0, target: old[i].target.clone(), tags_differ: false });
                continue;
            }
            // the same words but other variables or tags: the translation is carried over for a review, with the warning
            let i = most_common(c, &old);
            out.push(Match { new: n.id.clone(), old: old[i].id.clone(), kind: "near", ratio: 1.0, target: old[i].target.clone(), tags_differ: true });
            continue;
        }
        // near: candidates share words with the new line; the rare words are looked at first
        if nt.len() < 2 {
            continue;
        }
        let mut overlap: HashMap<usize, usize> = HashMap::new();
        for t in nt.iter().collect::<std::collections::BTreeSet<_>>() {
            if let Some(p) = postings.get(t.as_str()) {
                if p.len() <= 300 {
                    for &i in p {
                        *overlap.entry(i).or_insert(0) += 1;
                    }
                }
            }
        }
        let mut cands: Vec<(usize, usize)> = overlap.into_iter().filter(|&(i, _)| same_who(&old[i].who, &n.who)).collect();
        cands.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut best: Option<(usize, f64)> = None;
        for &(i, _) in cands.iter().take(25) {
            let r = ratio(&nt, &old_tokens[i]);
            if r >= min_ratio && best.is_none_or(|(_, b)| r > b) {
                best = Some((i, r));
            }
        }
        if let Some((i, r)) = best {
            let tags_differ = tags_of(&old[i].source) != tags_of(&n.source);
            out.push(Match {
                new: n.id.clone(),
                old: old[i].id.clone(),
                kind: "near",
                ratio: (r * 1000.0).round() / 1000.0,
                target: old[i].target.clone(),
                tags_differ,
            });
        }
    }
    out
}

fn line(v: &Value) -> Line {
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    Line { id: s("id"), who: s("who"), source: s("source"), target: s("target") }
}

/// `{old: [{id, who, source, target}], new: [{id, who, source}], min_ratio?}` -> `{items: [{new, old, kind, ratio, target, tags_differ}], counts: {exact, near, none, total}}`.
/// The counts cover all lines of `new` (they are the "value" shown before a licence); the items may be cut to the limit of the licence by the caller.
pub fn run(p: &Value) -> Value {
    let list = |k: &str| p[k].as_array().map(|a| a.iter().map(line).collect::<Vec<_>>()).unwrap_or_default();
    let (old, new) = (list("old"), list("new"));
    let min_ratio = p["min_ratio"].as_f64().filter(|r| (0.5..=1.0).contains(r)).unwrap_or(0.8);
    let items = find(&old, &new, min_ratio);
    let exact = items.iter().filter(|m| m.kind == "exact").count();
    let near = items.len() - exact;
    let total = new.iter().filter(|l| !tokens(&l.source).is_empty()).count();
    json!({
        "items": items.iter().map(|m| json!({"new": m.new, "old": m.old, "kind": m.kind, "ratio": m.ratio, "target": m.target, "tags_differ": m.tags_differ})).collect::<Vec<_>>(),
        "counts": {"exact": exact, "near": near, "none": total - exact - near, "total": total},
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(id: &str, who: &str, source: &str, target: &str) -> Line {
        Line { id: id.into(), who: who.into(), source: source.into(), target: target.into() }
    }

    fn old() -> Vec<Line> {
        vec![
            l("o1", "mc", "I did not expect to see you here.", "Nečekal jsem, že tě tu uvidím."),
            l("o2", "lisa", "Come on, we are going to be late for school!", "No tak, přijdeme pozdě do školy!"),
            l("o3", "mc", "Hello, [name]!", "Ahoj, [name]!"),
            l("o4", "mc", "Not translated yet.", ""),
            l("o5", "lisa", "Yes.", "Ano."),
        ]
    }

    #[test]
    fn the_same_original_under_a_new_id_is_carried_over_as_exact() {
        let new = vec![l("n1", "mc", "I DID NOT expect to see you here!", "")];
        let m = find(&old(), &new, 0.85);
        assert_eq!(m.len(), 1);
        assert_eq!((m[0].kind, m[0].old.as_str(), m[0].ratio), ("exact", "o1", 1.0)); // case and punctuation do not matter
        assert_eq!(m[0].target, "Nečekal jsem, že tě tu uvidím.");
    }

    #[test]
    fn a_small_edit_is_near_and_a_rewrite_is_nothing() {
        let new = vec![
            l("n1", "lisa", "Come on, we are going to be late for school today!", ""),
            l("n2", "lisa", "Please be quiet and leave the room right now.", ""),
        ];
        let m = find(&old(), &new, 0.85);
        assert_eq!(m.len(), 1, "{m:?}");
        assert_eq!((m[0].kind, m[0].new.as_str(), m[0].old.as_str()), ("near", "n1", "o2"));
        assert!(m[0].ratio >= 0.85 && m[0].ratio < 1.0);
    }

    #[test]
    fn the_speaker_must_match() {
        let new = vec![l("n1", "mc", "Yes.", ""), l("n2", "lisa", "Yes.", "")];
        let m = find(&old(), &new, 0.85);
        assert_eq!(m.iter().map(|x| x.new.as_str()).collect::<Vec<_>>(), vec!["n2"]);
    }

    #[test]
    fn different_variables_make_a_line_near_not_exact_and_say_so() {
        let new = vec![l("n1", "mc", "Hello, [player]!", "")];
        let m = find(&old(), &new, 0.5);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].kind, "near");
        assert!(m[0].tags_differ);
    }

    #[test]
    fn untranslated_old_lines_are_ignored_and_the_most_common_translation_wins() {
        let mut o = old();
        o.push(l("o6", "mc", "Fine.", "Dobře."));
        o.push(l("o7", "mc", "Fine!", "Fajn."));
        o.push(l("o8", "mc", "fine", "Dobře."));
        let new = vec![l("n1", "mc", "Not translated yet.", ""), l("n2", "mc", "Fine.", "")];
        let m = find(&o, &new, 0.85);
        assert_eq!(m.len(), 1);
        assert_eq!((m[0].new.as_str(), m[0].target.as_str()), ("n2", "Dobře."));
    }

    #[test]
    fn the_counts_cover_every_line_of_the_new_version() {
        let p = json!({
            "old": [{"id": "o1", "who": "mc", "source": "Good morning, everyone.", "target": "Dobré ráno všem."}],
            "new": [{"id": "n1", "who": "mc", "source": "Good morning, everyone."}, {"id": "n2", "who": "mc", "source": "A completely new line."}, {"id": "n3", "who": "mc", "source": "..."}],
        });
        let r = run(&p);
        assert_eq!(r["counts"], json!({"exact": 1, "near": 0, "none": 1, "total": 2})); // "..." has no words and is not counted
        assert_eq!(r["items"][0]["old"], "o1");
    }
}
