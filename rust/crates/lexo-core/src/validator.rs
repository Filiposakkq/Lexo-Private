//! Port of `core/lexo/validator.py`: Ren'Py aware validation of translated strings.
//!
//! Reports *problems for a human to judge*; nothing is ever rewritten. Severities: error (Ren'Py would raise, or the script structure is broken),
//! warning (suspicious, probably a mistake), info (frequently intentional). Comparison is always *relative to the source*.
//! The language pack is a parameter (the Python module reads the "current" pack), and so is the spelling hints object (`SpellCheck`).
use crate::lang::Pack;
use crate::messages::{msg, Message};
use crate::rpy::{self, Entry, ParsedFile};
use crate::util::{is_letter, is_space, is_word, norm_ws, py_split_ws, py_strip, Counter};
use regex::Regex;
use serde_json::{json, Map, Value};
use std::collections::{BTreeSet, HashSet};
use std::sync::OnceLock;

/// Tags that must be closed.
const PAIRED_TAGS: [&str; 18] =
    ["i", "b", "u", "s", "plain", "color", "size", "font", "a", "alpha", "k", "outlinecolor", "rb", "rt", "art", "cps", "language", "shader"];
/// Tags that stand alone.
const SINGLE_TAGS: [&str; 9] = ["w", "p", "nw", "fast", "done", "clear", "image", "space", "vspace"];
/// Tags whose argument is part of the layout and should stay as in the source.
const ARG_TAGS: [&str; 11] = ["color", "size", "font", "a", "alpha", "outlinecolor", "image", "space", "vspace", "cps", "k"];
/// Change how the line advances.
const FLOW_TAGS: [&str; 4] = ["nw", "fast", "done", "clear"];
const PAUSE_TAGS: [&str; 2] = ["w", "p"];

struct Patterns {
    percent: Regex,
    digits: Regex,
    double_space: Regex,
    word: Regex,
    var_split: Regex,
    escape: Regex,
    brace_pair: Regex,
    brackets: Regex,
    var_open: Regex,
}

fn p() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        percent: Regex::new(r"%\(\w+\)[sdifr]|%[sdifr]").unwrap(),
        digits: Regex::new(r"\d+").unwrap(),
        double_space: Regex::new(r"\S {2,}\S").unwrap(),
        word: Regex::new(r"[^\W\d_]+(?:'[^\W\d_]+)?").unwrap(),
        var_split: Regex::new(r"[!:]").unwrap(),
        escape: Regex::new(r"\\.").unwrap(),
        brace_pair: Regex::new(r"\{[^}]*\}").unwrap(),
        brackets: Regex::new(r"\[[^\]]*\]").unwrap(),
        var_open: Regex::new(r"\[\[").unwrap(),
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub severity: &'static str,
    pub code: String,
    pub message: Message,
    /// `syntax`, `structure`, `length` or `text`.
    pub category: &'static str,
    pub extra: Map<String, Value>,
}

impl Issue {
    pub(crate) fn new(severity: &'static str, code: &str, message: Message) -> Issue {
        Issue { severity, code: code.into(), message, category: "syntax", extra: Map::new() }
    }
    pub(crate) fn cat(mut self, category: &'static str) -> Issue {
        self.category = category;
        self
    }
    pub(crate) fn extra(mut self, extra: Value) -> Issue {
        if let Value::Object(o) = extra {
            self.extra = o;
        }
        self
    }
    pub fn to_json(&self) -> Value {
        let mut o = Map::new();
        o.insert("severity".into(), json!(self.severity));
        o.insert("code".into(), json!(self.code));
        o.insert("message".into(), self.message.wire());
        o.insert("category".into(), json!(self.category));
        o.extend(self.extra.clone());
        Value::Object(o)
    }
}

#[derive(Default, Debug)]
pub struct Tokens {
    /// (name, arg, full)
    pub open_tags: Vec<(String, String, String)>,
    pub close_tags: Vec<String>,
    /// (name, full)
    pub singles: Vec<(String, String)>,
    pub comments: Vec<String>,
    pub unknown: Vec<String>,
    pub variables: Vec<String>,
    pub placeholders: usize,
    pub percents: Vec<String>,
    /// ("open" | "close", name) in text order
    pub order: Vec<(&'static str, String)>,
    /// (code, message) syntax problems of this text
    pub problems: Vec<(&'static str, Message)>,
}

pub fn tokenize(text: &str) -> Tokens {
    let mut t = Tokens::default();
    let c: Vec<char> = text.chars().collect();
    let n = c.len();
    let starts = |i: usize, a: char, b: char| c.get(i) == Some(&a) && c.get(i + 1) == Some(&b);
    let mut i = 0;
    while i < n {
        let ch = c[i];
        if ch == '{' {
            if starts(i, '{', '{') {
                i += 2;
                continue;
            }
            let Some(j) = (i + 1..n).find(|k| c[*k] == '}') else {
                t.problems.push(("unterminated-tag", msg("Unterminated tag – the closing “{brace}” is missing.", &[("brace", json!("}"))])));
                break;
            };
            let body: String = c[i + 1..j].iter().collect();
            let full: String = c[i..=j].iter().collect();
            i = j + 1;
            if body.is_empty() {
                t.placeholders += 1;
            } else if body.starts_with('#') {
                t.comments.push(full);
            } else if let Some(rest) = body.strip_prefix('/') {
                let name = py_strip(rest).to_string();
                t.close_tags.push(name.clone());
                t.order.push(("close", name));
            } else {
                let (name, arg) = body.split_once('=').unwrap_or((&body, ""));
                let name = py_strip(name).to_string();
                if PAIRED_TAGS.contains(&name.as_str()) {
                    t.open_tags.push((name.clone(), arg.to_string(), full));
                    t.order.push(("open", name));
                } else if SINGLE_TAGS.contains(&name.as_str()) {
                    t.singles.push((name, full));
                } else {
                    t.unknown.push(full);
                }
            }
            continue;
        }
        if ch == '[' {
            if starts(i, '[', '[') {
                i += 2;
                continue;
            }
            let (mut depth, mut j) = (1, i + 1);
            while j < n && depth > 0 {
                if c[j] == '[' {
                    depth += 1;
                } else if c[j] == ']' {
                    depth -= 1;
                }
                j += 1;
            }
            if depth > 0 {
                t.problems.push(("unterminated-variable", msg("Unterminated variable – the closing “]” is missing.", &[])));
                break;
            }
            t.variables.push(py_strip(&c[i + 1..j - 1].iter().collect::<String>()).to_string());
            i = j;
            continue;
        }
        i += 1;
    }
    t.percents = p().percent.find_iter(text).map(|m| m.as_str().to_string()).collect();
    t
}

/// Problems with the order of paired tags: stray / mismatched closers (errors) and unclosed openers (warnings): (severity, code, tag name).
pub fn nesting_problems(tok: &Tokens) -> Vec<(&'static str, &'static str, String)> {
    let mut out = Vec::new();
    let mut stack: Vec<&str> = Vec::new();
    for (kind, name) in &tok.order {
        if *kind == "open" {
            stack.push(name);
        } else if stack.is_empty() {
            out.push(("error", "stray-close", name.clone()));
        } else if *stack.last().unwrap() != name {
            out.push(("error", "mismatched-close", name.clone()));
            if stack.contains(&name.as_str()) {
                while let Some(top) = stack.pop() {
                    if top == name {
                        break;
                    }
                }
            }
        } else {
            stack.pop();
        }
    }
    for name in stack {
        out.push(("warning", "unclosed", name.to_string()));
    }
    out
}

fn var_base(expr: &str) -> String {
    py_strip(p().var_split.splitn(expr, 2).next().unwrap_or("")).to_string()
}

fn tag(open: bool, name: &str) -> String {
    if open {
        format!("{{{name}}}")
    } else {
        format!("{{/{name}}}")
    }
}

/// Compare a translation with its source.
pub fn check_text(source: &str, target: &str) -> Vec<Issue> {
    let mut issues = Vec::new();
    let (s, t) = (tokenize(source), tokenize(target));
    let ts = |m: &Message| m.text();

    for (code, text) in &t.problems {
        if !s.problems.iter().any(|(c, m)| c == code && ts(m) == ts(text)) {
            issues.push(Issue::new("error", code, text.clone()));
        }
    }

    // ---- nesting of paired tags (only what is new compared with the source)
    let src_nest = Counter::from(nesting_problems(&s).into_iter().map(|(_, c, n)| format!("{c}\u{0}{n}")));
    let mut tgt_nest = Counter::default();
    for (_sev, code, name) in nesting_problems(&t) {
        let key = format!("{code}\u{0}{name}");
        tgt_nest.add(key.clone());
        if tgt_nest.get(&key) > src_nest.get(&key) {
            issues.push(match code {
                "stray-close" => Issue::new("error", code, msg("The closing tag {tag} has no matching opening one.", &[("tag", json!(tag(false, &name)))])),
                "mismatched-close" => {
                    Issue::new("error", code, msg("The tag {tag} is closed in the wrong order (wrong nesting).", &[("tag", json!(tag(false, &name)))]))
                }
                _ => Issue::new(
                    "warning",
                    code,
                    if s.close_tags.contains(&name) {
                        msg("The tag {tag} is not closed, but it is closed in the original.", &[("tag", json!(tag(true, &name)))])
                    } else {
                        msg("The tag {tag} is not closed.", &[("tag", json!(tag(true, &name)))])
                    },
                ),
            });
        }
    }

    // ---- paired tags: counts
    let so = Counter::from(s.open_tags.iter().map(|(n, _, _)| n.clone()));
    let to = Counter::from(t.open_tags.iter().map(|(n, _, _)| n.clone()));
    let sc = Counter::from(s.close_tags.iter().cloned());
    let tc = Counter::from(t.close_tags.iter().cloned());
    let names: BTreeSet<&String> = so.order.iter().chain(&to.order).chain(&sc.order).chain(&tc.order).collect();
    for name in names {
        if !PAIRED_TAGS.contains(&name.as_str()) {
            if tc.get(name) > sc.get(name) {
                issues.push(Issue::new("warning", "unknown-close-tag", msg("Unknown closing tag {tag}.", &[("tag", json!(tag(false, name)))])));
            }
            continue;
        }
        if to.get(name) < so.get(name) {
            issues.push(Issue::new(
                "warning",
                "tag-missing",
                msg(
                    "The translation is missing the tag {tag} (the original has it {a}×, the translation {b}×).",
                    &[("tag", json!(tag(true, name))), ("a", json!(so.get(name))), ("b", json!(to.get(name)))],
                ),
            ));
        } else if to.get(name) > so.get(name) {
            issues.push(Issue::new(
                "warning",
                "tag-extra",
                msg(
                    "The translation has an extra tag {tag} ({b}× against {a}× in the original).",
                    &[("tag", json!(tag(true, name))), ("b", json!(to.get(name))), ("a", json!(so.get(name)))],
                ),
            ));
        }
        if tc.get(name) < sc.get(name) && to.get(name) >= so.get(name) {
            issues.push(Issue::new("warning", "close-missing", msg("The translation is missing the closing tag {tag}.", &[("tag", json!(tag(false, name)))])));
        }
    }

    // ---- tag arguments (color=, size=, font=, a=...) should match the source
    let args = |tok: &Tokens| {
        Counter::from(tok.open_tags.iter().filter(|(n, a, _)| ARG_TAGS.contains(&n.as_str()) && !a.is_empty()).map(|(_, _, f)| f.replace(' ', "")))
    };
    let (src_args, tgt_args) = (args(&s), args(&t));
    for full in tgt_args.minus(&src_args) {
        // only report when the same tag name exists on both sides, otherwise tag-missing/extra covers it
        let name = full[1..].split('=').next().unwrap_or("");
        if so.get(name) > 0 && to.get(name) > 0 {
            issues.push(Issue::new("warning", "tag-arg", msg("The argument of the tag {tag} differs from the original.", &[("tag", json!(full))])));
        }
    }

    // ---- single tags
    let ss = Counter::from(s.singles.iter().map(|(n, _)| n.clone()));
    let tsg = Counter::from(t.singles.iter().map(|(n, _)| n.clone()));
    let single_names: BTreeSet<&String> = ss.order.iter().chain(&tsg.order).collect();
    for name in single_names {
        if ss.get(name) == tsg.get(name) {
            continue;
        }
        let sev = if FLOW_TAGS.contains(&name.as_str()) {
            "warning"
        } else if PAUSE_TAGS.contains(&name.as_str()) {
            "info"
        } else {
            "warning"
        };
        issues.push(Issue::new(
            sev,
            "single-tag",
            msg(
                "The number of tags {tag} differs (original {a}×, translation {b}×).",
                &[("tag", json!(tag(true, name))), ("a", json!(ss.get(name))), ("b", json!(tsg.get(name)))],
            ),
        ));
    }

    // ---- unknown tags that appear only in the translation
    let src_unknown: HashSet<&String> = s.unknown.iter().collect();
    for full in t.unknown.iter().collect::<BTreeSet<_>>().into_iter().filter(|f| !src_unknown.contains(*f)) {
        issues.push(Issue::new("warning", "unknown-tag", msg("Unknown tag {tag} (not in the original) – a typo?", &[("tag", json!(full))])));
    }

    // ---- {#...} disambiguation comments
    if !s.comments.is_empty() && !Counter::from(s.comments.iter().cloned()).same_as(&Counter::from(t.comments.iter().cloned())) {
        issues.push(Issue::new(
            "info",
            "comment-tag",
            msg("The tag {tag} of the original is missing from the translation or differs (Ren'Py ignores it when displaying).", &[("tag", json!("{#…}"))]),
        ));
    }

    // ---- positional {} placeholders of system strings
    if s.placeholders != t.placeholders {
        issues.push(Issue::new(
            "error",
            "placeholder",
            msg(
                "The number of placeholders {marks} differs (original {a}×, translation {b}×).",
                &[("marks", json!("{}")), ("a", json!(s.placeholders)), ("b", json!(t.placeholders))],
            ),
        ));
    }

    // ---- [variables]
    let (sv, tv) = (Counter::from(s.variables.iter().cloned()), Counter::from(t.variables.iter().cloned()));
    let s_bases: HashSet<String> = s.variables.iter().map(|v| var_base(v)).collect();
    let t_bases: HashSet<String> = t.variables.iter().map(|v| var_base(v)).collect();
    let sorted_keys = |c: &Counter| -> Vec<String> {
        let mut v = c.order.clone();
        v.sort();
        v
    };
    for expr in sorted_keys(&tv).into_iter().filter(|e| sv.get(e) == 0) {
        if !s_bases.contains(&var_base(&expr)) {
            issues.push(Issue::new(
                "error",
                "variable-unknown",
                msg("The variable [{expr}] is not in the original – a typo? The game might crash.", &[("expr", json!(expr))]),
            ));
        } else {
            issues.push(Issue::new(
                "warning",
                "variable-changed",
                msg("The variable [{expr}] differs from the original (e.g. another flag).", &[("expr", json!(expr))]),
            ));
        }
    }
    for expr in sorted_keys(&sv).into_iter().filter(|e| tv.get(e) == 0) {
        if !t_bases.contains(&var_base(&expr)) {
            issues.push(Issue::new("warning", "variable-missing", msg("The translation is missing the variable [{expr}].", &[("expr", json!(expr))])));
        }
    }
    for expr in sorted_keys(&sv).into_iter().filter(|e| tv.get(e) > 0) {
        let (a, b) = (tv.get(&expr), sv.get(&expr));
        let (code, sev) = if a > b {
            ("variable-repeat", "info")
        } else if a < b {
            ("variable-fewer", "info")
        } else {
            continue;
        };
        issues.push(Issue::new(
            sev,
            code,
            msg("The variable [{expr}] occurs {a}× in the translation (original {b}×).", &[("expr", json!(expr)), ("a", json!(a)), ("b", json!(b))]),
        ));
    }

    // ---- %-formats of system strings ("Page %s")
    if !Counter::from(s.percents.iter().cloned()).same_as(&Counter::from(t.percents.iter().cloned())) {
        let show = |v: &Vec<String>| if v.is_empty() { "—".to_string() } else { v.join(", ") };
        issues.push(Issue::new(
            "error",
            "percent-format",
            msg("The format marks % differ (original {a}, translation {b}).", &[("a", json!(show(&s.percents))), ("b", json!(show(&t.percents)))]),
        ));
    }

    // ---- escapes
    let (nl_s, nl_t) = (source.matches("\\n").count(), target.matches("\\n").count());
    if nl_s != nl_t {
        issues.push(Issue::new(
            "warning",
            "newline-count",
            msg("The number of \\n line breaks differs (original {a}×, translation {b}×).", &[("a", json!(nl_s)), ("b", json!(nl_t))]),
        ));
    }
    if p().escape.replace_all(target, "").contains('\\') {
        issues.push(Issue::new("error", "bad-escape", msg("A lone backslash “\\” – an invalid escape sequence.", &[])));
    }
    issues
}

// ---------------------------------------------------------------- text quality checks (category "text")

/// Common English function words that are not Czech words; "i", "most", "had" are Czech too and interjections (hello, okay...) are often kept on purpose, so neither counts.
const NOT_ENGLISH_EVIDENCE: [&str; 17] =
    ["i", "most", "had", "yes", "okay", "ok", "oh", "hey", "hi", "hello", "please", "thanks", "thank", "sorry", "well", "right", "really"];

fn english_evidence() -> &'static HashSet<&'static str> {
    static E: OnceLock<HashSet<&'static str>> = OnceLock::new();
    E.get_or_init(|| {
        let skip: HashSet<&str> = NOT_ENGLISH_EVIDENCE.iter().copied().chain(["maybe", "being"]).collect();
        rpy::en_words().iter().copied().filter(|w| !skip.contains(w)).collect()
    })
}

/// What the player reads: tags and [variables] removed, \n as a space.
pub fn plain_text(text: &str) -> String {
    let out = text.replace("{{", "{");
    let out = p().brace_pair.replace_all(&out, "").into_owned();
    let out = p().var_open.replace_all(&out, "[").into_owned();
    let out = p().brackets.replace_all(&out, " ").into_owned();
    out.replace("\\n", " ").replace("\\\"", "\"")
}

/// The matches of `\b([^\W\d_]{2,})\s+\1\b` (ignoring case): the first word of every pair of the same word twice in a row, in text order.
fn repeated_words(text: &str) -> Vec<String> {
    let c: Vec<char> = text.chars().collect();
    let n = c.len();
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        // a match starts at a word boundary with a letter run that ends where white space begins
        if is_letter(c[i]) && (i == 0 || !is_word(c[i - 1])) {
            let mut j = i;
            while j < n && is_letter(c[j]) {
                j += 1;
            }
            if j - i >= 2 && j < n && is_space(c[j]) {
                let mut k = j;
                while k < n && is_space(c[k]) {
                    k += 1;
                }
                let first: String = c[i..j].iter().collect();
                let end = k + (j - i);
                let same_len = end <= n && c[k..end].iter().collect::<String>().to_lowercase() == first.to_lowercase();
                if same_len && (end == n || !is_word(c[end])) {
                    out.push(first);
                    i = end; // finditer goes on after the match
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

fn words_lower(text: &str) -> Vec<String> {
    p().word.find_iter(text).map(|m| m.as_str().to_lowercase()).collect()
}

/// Cheap human-error checks on the visible text. Always relative to the source; warnings only.
pub fn check_quality(source: &str, target: &str, pack: &Pack) -> Vec<Issue> {
    let mut issues = Vec::new();
    let (ps, pt) = (plain_text(source), plain_text(target));

    // numbers must survive the translation
    let nums = |t: &str| Counter::from(p().digits.find_iter(t).map(|m| m.as_str().to_string()));
    let (ns, nt) = (nums(&ps), nums(&pt));
    if !ns.same_as(&nt) {
        // a number written out in words ("14" -> "čtrnáct") is normal in some languages (Pack.number_words_ok)
        let sev = if !ns.map.is_empty() && nt.map.is_empty() && pack.number_words_ok { "info" } else { "warning" };
        let show = |c: &Counter| if c.map.is_empty() { "—".to_string() } else { c.sorted_elements().join(", ") };
        issues.push(
            Issue::new(
                sev,
                "numbers-differ",
                msg("The numbers differ from the original (original: {a}; translation: {b}).", &[("a", json!(show(&ns))), ("b", json!(show(&nt)))]),
            )
            .cat("text"),
        );
    }

    // double spaces inside the text
    if p().double_space.is_match(target) && !p().double_space.is_match(source) {
        issues.push(
            Issue::new("warning", "double-space", msg("There is a double space in the translation.", &[]))
                .cat("text")
                .extra(json!({"fixable": true, "kind": "spaces"})),
        );
    }

    // the same word twice in a row (typo), unless the source does it as well or the language does it normally
    let src_rep: HashSet<String> = repeated_words(&ps).iter().map(|w| w.to_lowercase()).collect();
    if pack.check_repeats {
        if let Some(w) = repeated_words(&pt).iter().map(|w| w.to_lowercase()).find(|w| !pack.repeat_ok.contains(w) && !src_rep.contains(w)) {
            issues.push(Issue::new("warning", "repeated-word", msg("The same word twice in a row: “{w} {w}”.", &[("w", json!(w))])).cat("text"));
        }
    }

    // English left in a translated line
    let words = words_lower(&pt);
    let hits: Vec<&String> = words.iter().filter(|w| english_evidence().contains(w.as_str())).collect();
    if hits.len() >= 3 || (hits.len() >= 2 && hits.len() as f64 / words.len().max(1) as f64 >= 0.4) {
        let uniq: BTreeSet<&&String> = hits.iter().collect();
        let shown: Vec<&str> = uniq.into_iter().take(5).map(|w| w.as_str()).collect();
        issues.push(
            Issue::new(
                "warning",
                "english-left",
                msg("English is left in the translation ({words}). Did you forget to translate part of it?", &[("words", json!(shown.join(", ")))]),
            )
            .cat("text"),
        );
    }
    issues
}

/// Does the English `term` occur in `text` as a whole word (case-insensitive)?
pub fn term_in_text(text: &str, term: &str) -> bool {
    if term.is_empty() {
        return false;
    }
    let hay: Vec<char> = plain_text(text).to_lowercase().chars().collect();
    let needle: Vec<char> = term.to_lowercase().chars().collect();
    (0..hay.len()).any(|i| hay[i..].starts_with(&needle) && (i == 0 || !is_word(hay[i - 1])) && hay.get(i + needle.len()).is_none_or(|c| !is_word(*c)))
}

/// Is the Czech form of a glossary term present in the translation (any ending, matched by word stem)?
pub fn term_used(target: &str, czech: &str, pack: &Pack) -> bool {
    let cz_stems: Vec<String> = p().word.find_iter(czech).map(|m| pack.stem.apply(m.as_str())).collect();
    if cz_stems.is_empty() {
        return true;
    }
    let t_stems: HashSet<String> = p().word.find_iter(&plain_text(target)).map(|m| pack.stem.apply(m.as_str())).collect();
    cz_stems.iter().all(|cs| t_stems.iter().any(|ts| ts.chars().count() >= 3 && (ts.starts_with(cs.as_str()) || cs.starts_with(ts.as_str()))))
}

/// Glossary terms: when the original uses the English term, the translation should use the Czech one (any ending). `terms` = [(english, czech)].
pub fn check_terms(source: &str, target: &str, terms: &[(String, String)], pack: &Pack) -> Vec<Issue> {
    terms
        .iter()
        .filter(|(en, cz)| !en.is_empty() && !cz.is_empty() && term_in_text(source, en) && !term_used(target, cz, pack))
        .map(|(en, cz)| {
            Issue::new(
                "info",
                "term-inconsistent",
                msg(
                    "The term “{en}” is translated as “{cz}” in the Glossary, but I do not see that in the translation.",
                    &[("en", json!(en)), ("cz", json!(cz))],
                ),
            )
            .cat("text")
        })
        .collect()
}

fn hide_markup(text: &str, variable: &str) -> String {
    let out = text.replace("{{", "{");
    let out = p().brace_pair.replace_all(&out, "").into_owned();
    let out = p().var_open.replace_all(&out, "[").into_owned();
    p().brackets.replace_all(&out, variable).into_owned()
}

/// Characters a player would see: tags removed, variables counted as a short name.
pub fn visible_length(text: &str) -> usize {
    let out = hide_markup(text, "XXXXX").replace("\\n", " ").replace("\\\"", "\"");
    norm_ws(&out).chars().count()
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// `dict(DEFAULTS, **(cfg or {}))` for the numbers and flags of a check.
fn setting(cfg: Option<&Value>, key: &str, default: Value) -> Value {
    cfg.and_then(|c| c.get(key)).cloned().unwrap_or(default)
}

fn number(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.0)
}

/// Optional review aid: unusually long / short translation. Never an error. `cfg` holds any of `enabled`, `min_source_chars`, `max_ratio`, `min_ratio`,
/// `min_extra_chars`, `min_missing_chars`.
pub fn check_length(source: &str, target: &str, cfg: Option<&Value>) -> Vec<Issue> {
    if !truthy(&setting(cfg, "enabled", json!(true))) {
        return Vec::new();
    }
    let (min_source, max_ratio, min_ratio) =
        (number(&setting(cfg, "min_source_chars", json!(12))), number(&setting(cfg, "max_ratio", json!(2.2))), number(&setting(cfg, "min_ratio", json!(0.45))));
    let (min_extra, min_missing) = (number(&setting(cfg, "min_extra_chars", json!(25))), number(&setting(cfg, "min_missing_chars", json!(18))));
    let (ls, lt) = (visible_length(source), visible_length(target));
    if (ls as f64) < min_source || lt == 0 {
        return Vec::new();
    }
    let ratio = lt as f64 / ls as f64;
    let rounded: f64 = format!("{ratio:.2}").parse().unwrap_or(ratio);
    let extra = json!({"ratio": rounded, "source_len": ls, "target_len": lt});
    let params = |a: usize, b: usize| [("a", json!(a)), ("b", json!(b)), ("ratio", json!(format!("{ratio:.1}")))];
    if ratio > max_ratio && (lt as f64 - ls as f64) >= min_extra {
        return vec![Issue::new(
            "warning",
            "length-long",
            msg(
                "The translation is much longer than the original ({a} vs {b} characters, {ratio}×). Translations are often longer, but this is a lot.",
                &params(lt, ls),
            ),
        )
        .cat("length")
        .extra(extra)];
    }
    if ratio < min_ratio && (ls as f64 - lt as f64) >= min_missing {
        return vec![Issue::new(
            "warning",
            "length-short",
            msg("The translation is much shorter than the original ({a} vs {b} characters, {ratio}×). Is part of the text missing?", &params(lt, ls)),
        )
        .cat("length")
        .extra(extra)];
    }
    Vec::new()
}

/// Lines a text takes in the dialogue box: explicit \n breaks plus greedy word wrap at `per_line` characters.
pub fn lines_needed(text: &str, per_line: i64) -> i64 {
    let out = hide_markup(text, "XXXXX").replace("\\\"", "\"");
    let mut total = 0;
    for seg in out.split("\\n") {
        let (mut line_len, mut n) = (0i64, 1i64);
        for w in py_split_ws(seg) {
            let len = w.chars().count() as i64;
            let need = len + i64::from(line_len > 0);
            if line_len + need > per_line && line_len > 0 {
                n += 1;
                line_len = len;
            } else {
                line_len += need;
            }
        }
        total += n;
    }
    total
}

/// Will the translation fit the dialogue box? Only reported when the translation needs more lines than the limit AND more than the original did.
/// `cfg` holds any of `enabled`, `chars_per_line` (65), `max_lines` (4).
pub fn check_textbox(source: &str, target: &str, cfg: Option<&Value>) -> Vec<Issue> {
    let (enabled, per_line, max_lines) = (setting(cfg, "enabled", json!(true)), setting(cfg, "chars_per_line", json!(65)), setting(cfg, "max_lines", json!(4)));
    if !truthy(&enabled) || !truthy(&per_line) || !truthy(&max_lines) {
        return Vec::new();
    }
    let (per_line, max_lines) = (number(&per_line) as i64, number(&max_lines) as i64);
    let (ls, lt) = (lines_needed(source, per_line), lines_needed(target, per_line));
    if lt > max_lines && lt > ls {
        return vec![Issue::new(
            "warning",
            "textbox-overflow",
            msg(
                "The text may not fit the dialogue box: about {lines} lines (limit {limit}, original {orig}). Shorten it, or split it with {tag}.",
                &[("lines", json!(lt)), ("limit", json!(max_lines)), ("orig", json!(ls)), ("tag", json!("{p}"))],
            ),
        )
        .cat("length")
        .extra(json!({"lines": lt, "limit": max_lines}))];
    }
    Vec::new()
}

/// The spelling hints of the project's own vocabulary (see `spelling.rs`).
pub trait SpellCheck {
    fn check(&self, source: &str, target: &str) -> Vec<Issue>;
}

/// Everything that can be passed to the checks besides the entry.
#[derive(Default)]
pub struct Checks<'a> {
    pub length_cfg: Option<&'a Value>,
    pub terms: &'a [(String, String)],
    pub spell: Option<&'a dyn SpellCheck>,
    pub textbox: Option<&'a Value>,
}

/// All issues for one parsed Entry. Untranslated lines are not validated.
pub fn check_entry(entry: &Entry, pack: &Pack, checks: &Checks) -> Vec<Issue> {
    if entry.pending() {
        return Vec::new();
    }
    let mut issues = Vec::new();
    let Some(source) = entry.source.as_deref() else { return issues };
    // structure: speaker / attributes / trailing code must stay as in the comment
    if entry.unit == "dialogue" {
        if norm_ws(&entry.who) != norm_ws(&entry.src_who) {
            issues.push(
                Issue::new(
                    "error",
                    "speaker-changed",
                    msg("The speaker / attributes differ from the original: “{a}” → “{b}”.", &[("a", json!(entry.src_who)), ("b", json!(entry.who))]),
                )
                .cat("structure")
                .extra(json!({"fixable": true, "kind": "speaker"})),
            );
        }
        if norm_ws(&entry.suffix) != norm_ws(&entry.src_suffix) {
            issues.push(
                Issue::new(
                    "error",
                    "suffix-changed",
                    msg("The code after the text differs from the original: “{a}” → “{b}”.", &[("a", json!(entry.src_suffix)), ("b", json!(entry.suffix))]),
                )
                .cat("structure")
                .extra(json!({"fixable": true, "kind": "speaker"})),
            );
        }
    }
    if py_strip(&entry.target).is_empty() && !py_strip(source).is_empty() {
        return issues;
    }
    issues.extend(check_text(source, &entry.target));
    issues.extend(check_length(source, &entry.target, checks.length_cfg));
    issues.extend(check_quality(source, &entry.target, pack));
    if !checks.terms.is_empty() {
        issues.extend(check_terms(source, &entry.target, checks.terms, pack));
    }
    if let Some(spell) = checks.spell {
        issues.extend(spell.check(source, &entry.target));
    }
    if entry.unit == "dialogue" {
        issues.extend(check_textbox(source, &entry.target, checks.textbox));
    }
    issues
}

/// Issues for a whole ParsedFile: (line, issue, entry index or `None` for a structure problem), sorted by line.
pub fn check_parsed(parsed: &ParsedFile, pack: &Pack, checks: &Checks) -> Vec<(usize, Issue, Option<usize>)> {
    let mut out = Vec::new();
    for (idx, e) in parsed.entries.iter().enumerate() {
        for iss in check_entry(e, pack, checks) {
            out.push((e.line, iss, Some(idx)));
        }
    }
    for (line, code, text) in &parsed.structure {
        out.push((*line, Issue::new("error", code, text.clone()).cat("structure"), None));
    }
    out.sort_by_key(|r| r.0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(issues: &[Issue]) -> Vec<&str> {
        issues.iter().map(|i| i.code.as_str()).collect()
    }

    #[test]
    fn a_translation_that_keeps_tags_and_variables_has_no_text_issue() {
        assert!(check_text("Hello [name], {i}wait{/i}!", "Ahoj [name], {i}počkej{/i}!").is_empty());
    }

    #[test]
    fn broken_tags_and_variables_are_reported_relative_to_the_source() {
        assert_eq!(codes(&check_text("{i}a{/i}", "{i}a")), vec!["unclosed", "close-missing"]);
        assert_eq!(codes(&check_text("[x]", "[y]")), vec!["variable-unknown", "variable-missing"]);
        assert_eq!(codes(&check_text("a {i}b", "a {i}b")), Vec::<&str>::new());
        assert_eq!(codes(&check_text("Page %s", "Strana")), vec!["percent-format"]);
        assert_eq!(codes(&check_text("a", "b \\")), vec!["bad-escape"]);
    }

    #[test]
    fn the_same_word_twice_is_found_only_as_a_whole_pair() {
        assert_eq!(repeated_words("to je je dobře, ale ne ne ne"), vec!["je", "ne"]);
        assert!(repeated_words("a a").is_empty()); // one letter is not a word here
        assert_eq!(repeated_words("Ano ANO"), vec!["Ano"]);
        assert!(repeated_words("jede jedeme").is_empty());
    }

    #[test]
    fn wrapping_counts_lines_like_python() {
        assert_eq!(lines_needed("aaa bbb ccc", 7), 2);
        assert_eq!(lines_needed("a\\nb\\nc", 7), 3);
    }

    #[test]
    fn glossary_terms_match_whole_words() {
        assert!(term_in_text("The Sage came", "sage"));
        assert!(!term_in_text("A massage", "sage"));
    }
}
