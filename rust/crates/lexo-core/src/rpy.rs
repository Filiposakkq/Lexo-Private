//! Port of `core/lexo/rpy.py`: the parser of Ren'Py translation files (`game/tl/<language>/*.rpy`).
//!
//! A *translation unit* is either a dialogue block (`translate czech <id>:` + a `# who "source"` comment + a `who "target"` line) or a strings pair
//! (`old "source"` / `new "target"` inside `translate czech strings:`). A unit is *pending* when its code line equals the source comment
//! (or `new` equals `old`, or `new ""`).
//!
//! Known edges against Python (no golden case for them yet): `\w` and `\s` of the `regex` crate follow Unicode properties, those of `re` follow
//! `str.isalnum` / `str.isspace`, which differ for combining marks and for U+001C to U+001F.
use crate::lang::Pack;
use crate::messages::{msg, Message};
use crate::util::{norm_ws, py_split_ws, py_strip, sha1_hex};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

struct Patterns {
    old: Regex,
    new: Regex,
    translate: Regex,
    game_ref: Regex,
    string: Regex,
    tag: Regex,
    word: Regex,
    line_split: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        old: Regex::new(r#"^\s*old\s+"(.*)"\s*$"#).unwrap(),
        new: Regex::new(r#"^\s*new\s+"(.*)"\s*$"#).unwrap(),
        translate: Regex::new(r"^translate\s+(\w+)\s+(\w+)\s*:\s*$").unwrap(),
        game_ref: Regex::new(r"^(game/\S+?:\d+)\s*$").unwrap(),
        string: Regex::new(r#""((?:[^"\\]|\\.)*)""#).unwrap(),
        tag: Regex::new(r"\{[^}]*\}|\[[^\]]*\]").unwrap(),
        word: Regex::new(r"[^\W\d_]+(?:'[^\W\d_]+)?").unwrap(),
        line_split: Regex::new(r"\r\n|\r|\n").unwrap(),
    })
}

/// The string literal of a line of code: `"..."` with escapes (group 1 = the text between the quotes).
pub fn string_re() -> &'static Regex {
    &patterns().string
}

/// Statement keywords that start lines containing quotes but are not dialogue.
const NON_TEXT: [&str; 26] = [
    "voice",
    "play",
    "queue",
    "stop",
    "show",
    "hide",
    "scene",
    "with",
    "window",
    "nvl",
    "pause",
    "call",
    "jump",
    "return",
    "image",
    "transform",
    "define",
    "default",
    "python",
    "renpy",
    "camera",
    "if",
    "elif",
    "else",
    "while",
    "menu",
];

/// Lines made only of symbols ("...", "?!") are dialogue but cannot be translated: false = count them as untranslated (the original behaviour).
pub const COUNT_SYMBOL_ONLY_AS_DONE: bool = false;

/// Common English words (without those that also exist in Czech: a, to, on, no, do, by, my, he, me...).
const EN_WORDS: &str = "
the and you your yours is are was were be been being am i i'm i'll i've i'd it it's its
that this these those what who whom whose which when where why how not don't doesn't didn't
can't won't isn't aren't wasn't couldn't wouldn't shouldn't have has had having will would
could should shall may might must of in for with at from about into over after before
but or so if then than too very just only also all any some more most much many each every
she her hers him his they them their we us our there here out up down off again
get got go going gone know think want need like see say said tell let come make take look
yes okay ok well oh hey hi hello please thanks thank sorry right really maybe
you're we're they're he's she's that's what's there's let's
";

pub fn en_words() -> &'static HashSet<&'static str> {
    static W: OnceLock<HashSet<&'static str>> = OnceLock::new();
    W.get_or_init(|| EN_WORDS.split_whitespace().collect())
}

/// Kinds of a pending line (the codes are sent to the UI, which words them): empty translation, still English, identical to the original, only symbols.
pub const K_EMPTY: &str = "empty";
pub const K_ENGLISH: &str = "english";
pub const K_SAME: &str = "same";
pub const K_SYMBOLS: &str = "symbols";

/// Category of a pending (identical / empty) line based on its content.
pub fn classify(text: &str, pack: &Pack) -> &'static str {
    let p = patterns();
    if py_strip(text).is_empty() {
        return K_EMPTY;
    }
    let plain = p.tag.replace_all(text, " ").replace("\\n", " ").replace('’', "'");
    let words: Vec<&str> = p.word.find_iter(&plain).map(|m| m.as_str()).collect();
    if words.is_empty() {
        return K_SYMBOLS;
    }
    let letters = &pack.letters;
    if plain.chars().any(|ch| if letters.is_empty() { !ch.is_ascii() && ch.is_alphabetic() } else { letters.contains(&ch) }) {
        return K_SAME;
    }
    let hits = words.iter().filter(|w| en_words().contains(w.to_lowercase().as_str())).count();
    if hits > 0 || words.len() >= 5 {
        return K_ENGLISH;
    }
    K_SAME
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    /// 1-based line of the translated text (what the editor should open).
    pub line: usize,
    /// 1-based line of the source comment / old line (0 when unknown).
    pub src_line: usize,
    /// Approval key part: `translate czech <id>` or `strings`.
    pub block: String,
    /// `dialogue` or `string`.
    pub unit: &'static str,
    /// Text before the string literal (speaker + attributes), empty for strings.
    pub who: String,
    /// Text after the string literal (e.g. ` with vpunch`).
    pub suffix: String,
    /// Raw source text between the quotes (`None` when no source is known).
    pub source: Option<String>,
    /// Raw translated text between the quotes.
    pub target: String,
    /// Source line as written in the file (trimmed).
    pub src_raw: String,
    /// Translated line as written in the file (trimmed).
    pub cur_raw: String,
    pub src_who: String,
    pub src_suffix: String,
    /// One of the `K_*` codes when pending, empty when translated.
    pub category: &'static str,
    /// Original game script position Ren'Py recorded, e.g. `game/script.rpy:2155`.
    pub reference: String,
}

impl Entry {
    pub fn pending(&self) -> bool {
        !self.category.is_empty()
    }

    /// Identity used for manual approval: block + normalised source (not the line number).
    pub fn key(&self) -> (String, String) {
        (self.block.clone(), norm_ws(self.source.as_deref().unwrap_or(&self.target)))
    }
}

#[derive(Debug, Default, Clone)]
pub struct ParsedFile {
    pub entries: Vec<Entry>,
    /// (language, id, line)
    pub block_ids: Vec<(String, String, usize)>,
    /// (line, code, message)
    pub structure: Vec<(usize, &'static str, Message)>,
    pub digest: String,
    pub languages: HashSet<String>,
}

impl ParsedFile {
    pub fn pending(&self) -> Vec<&Entry> {
        self.entries.iter().filter(|e| e.pending()).collect()
    }
}

/// `(text before the literal, the literal, text after it, the line when there was a literal)`; without a literal the whole line comes first.
fn split_literal(code_line: &str) -> (String, String, String, String) {
    match patterns().string.captures(code_line) {
        None => (py_strip(code_line).to_string(), String::new(), String::new(), String::new()),
        Some(c) => {
            let whole = c.get(0).unwrap();
            (py_strip(&code_line[..whole.start()]).to_string(), c[1].to_string(), py_strip(&code_line[whole.end()..]).to_string(), code_line.to_string())
        }
    }
}

struct Comment {
    text: String,
    line: usize,
}

#[derive(Default)]
struct Pending {
    /// Normalised comment -> (comment text, line); a later equal comment replaces the earlier.
    comments: HashMap<String, Comment>,
    /// (normalised, text, line)
    comment_list: Vec<(String, String, usize)>,
    /// (line number, stripped line)
    code: Vec<(usize, String)>,
}

fn flush(pending: &mut Pending, entries: &mut Vec<Entry>, block: &str, block_ref: &str, pack: &Pack) {
    let mut used: HashSet<usize> = HashSet::new();
    for (idx, (lineno, c)) in pending.code.iter().enumerate() {
        let n = norm_ws(c);
        let (who, target, suffix, _) = split_literal(c);
        if let Some(cm) = pending.comments.get(&n) {
            entries.push(Entry {
                line: *lineno,
                src_line: cm.line,
                block: block.into(),
                unit: "dialogue",
                reference: block_ref.into(),
                who: who.clone(),
                suffix: suffix.clone(),
                source: Some(target.clone()),
                category: classify(&target, pack),
                target,
                src_raw: cm.text.clone(),
                cur_raw: c.clone(),
                src_who: who,
                src_suffix: suffix,
            });
            continue;
        }
        // translated: pair with a source comment by position, else by speaker
        let pick = if pending.comment_list.len() == pending.code.len() {
            Some(&pending.comment_list[idx])
        } else {
            pending.comment_list.iter().find(|cand| !used.contains(&cand.2) && split_literal(&cand.1).0 == who)
        };
        match pick {
            Some(pick) => {
                used.insert(pick.2);
                let (swho, ssrc, ssuffix, _) = split_literal(&pick.1);
                entries.push(Entry {
                    line: *lineno,
                    src_line: pick.2,
                    block: block.into(),
                    unit: "dialogue",
                    reference: block_ref.into(),
                    who,
                    suffix,
                    source: Some(ssrc),
                    target,
                    src_raw: pick.1.clone(),
                    cur_raw: c.clone(),
                    src_who: swho,
                    src_suffix: ssuffix,
                    category: "",
                });
            }
            None => entries.push(Entry {
                line: *lineno,
                src_line: 0,
                block: block.into(),
                unit: "dialogue",
                reference: block_ref.into(),
                who,
                suffix,
                source: None,
                target,
                cur_raw: c.clone(),
                ..Entry::default()
            }),
        }
    }
    *pending = Pending::default();
}

/// Parse the contents of one `.rpy` file.
pub fn parse_text(text: &str, digest: &str, pack: &Pack) -> ParsedFile {
    let p = patterns();
    let lines: Vec<&str> = p.line_split.split(text).collect();
    let mut out = ParsedFile { digest: digest.into(), ..ParsedFile::default() };
    let mut entries: Vec<Entry> = Vec::new();
    let mut pending = Pending::default();
    let mut block = String::new();
    // only the body of a `translate <lang> <id>:` block holds dialogue; a script without such blocks is not a translation file
    let mut collecting = false;
    let mut block_ref = String::new();
    let mut ref_pending = String::new();
    let mut old_ref = String::new();
    let mut old_text: Option<String> = None;
    let mut old_no = 0usize;

    fn close_old(old_text: &mut Option<String>, old_no: usize, out: &mut ParsedFile) {
        if old_text.take().is_some() {
            out.structure.push((old_no, "old-without-new", msg("After the \"old\" line the matching \"new\" line is missing.", &[])));
        }
    }

    for (i, raw_line) in lines.iter().enumerate() {
        let lineno = i + 1;
        let mut raw: &str = raw_line;
        let mut line: &str = py_strip(raw);
        if lineno == 1 {
            line = line.trim_start_matches('\u{feff}');
            raw = raw.trim_start_matches('\u{feff}');
        }
        if line.is_empty() {
            continue;
        }
        if line.starts_with("translate ") {
            flush(&mut pending, &mut entries, &block, &block_ref, pack);
            close_old(&mut old_text, old_no, &mut out);
            block = line.trim_end_matches(':').to_string();
            let words = py_split_ws(line);
            collecting = !(words.len() > 2 && (words[2] == "python" || words[2] == "style"));
            block_ref = std::mem::take(&mut ref_pending);
            if let Some(m) = p.translate.captures(line) {
                out.languages.insert(m[1].to_string());
                if &m[2] != "strings" {
                    out.block_ids.push((m[1].to_string(), m[2].to_string(), lineno));
                }
            }
            continue;
        }
        if line.starts_with('#') {
            let ctext = py_strip(line.trim_start_matches('#'));
            if let Some(m) = p.game_ref.captures(ctext) {
                ref_pending = m[1].to_string();
            }
            if ctext.contains('"') && !ctext.starts_with("game/") {
                pending.comments.insert(norm_ws(ctext), Comment { text: ctext.to_string(), line: lineno });
                pending.comment_list.push((norm_ws(ctext), ctext.to_string(), lineno));
            }
            continue;
        }
        // `old` / `new` are string-pair keywords only inside `translate <lang> strings:`; in a dialogue block they can be a character's name
        let in_strings = block.is_empty() || block.ends_with(" strings");
        if in_strings {
            if let Some(m) = p.old.captures(raw) {
                close_old(&mut old_text, old_no, &mut out);
                old_text = Some(m[1].to_string());
                old_no = lineno;
                old_ref = std::mem::take(&mut ref_pending);
                continue;
            }
            if let Some(m) = p.new.captures(raw) {
                let new_text = m[1].to_string();
                if old_text.is_none() {
                    out.structure.push((lineno, "new-without-old", msg("The \"new\" line has no \"old\" line before it.", &[])));
                }
                let same = old_text.as_deref() == Some(new_text.as_str());
                let category = if new_text.is_empty() || same { classify(&new_text, pack) } else { "" };
                entries.push(Entry {
                    line: lineno,
                    src_line: if old_text.is_some() { old_no } else { 0 },
                    block: "strings".into(),
                    unit: "string",
                    reference: old_ref.clone(),
                    source: old_text.clone(),
                    src_raw: old_text.as_ref().map(|o| format!("old \"{o}\"")).unwrap_or_default(),
                    cur_raw: format!("new \"{new_text}\""),
                    target: new_text,
                    category,
                    ..Entry::default()
                });
                old_text = None;
                continue;
            }
        }
        let first = py_split_ws(line)[0];
        if collecting && line.contains('"') && !NON_TEXT.contains(&first) && !line.starts_with('$') {
            pending.code.push((lineno, line.to_string()));
        }
    }
    flush(&mut pending, &mut entries, &block, &block_ref, pack);
    close_old(&mut old_text, old_no, &mut out);
    entries.sort_by_key(|e| e.line);
    out.entries = entries;
    out
}

/// Parse the bytes of a file: UTF-8 with an optional BOM; invalid bytes become U+FFFD and a `bad-encoding` problem is recorded.
pub fn parse_bytes(data: &[u8], pack: &Pack) -> ParsedFile {
    let body = data.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(data);
    let (text, bad) = match std::str::from_utf8(body) {
        Ok(s) => (s.to_string(), false),
        Err(_) => (String::from_utf8_lossy(body).into_owned(), true),
    };
    let mut out = parse_text(&text, &sha1_hex(data), pack);
    if bad {
        out.structure.push((
            1,
            "bad-encoding",
            msg("The file is not valid UTF-8 (some characters could not be read). Ren'Py reads scripts as UTF-8 – save it in the editor as “UTF-8”.", &[]),
        ));
    }
    out
}

/// (translated, untranslated, manually approved): approved pending lines count as translated.
pub fn count(entries: &[Entry], approved: &std::collections::BTreeSet<(String, String)>, symbols_as_done: Option<bool>) -> (usize, usize, usize) {
    let symbols_as_done = symbols_as_done.unwrap_or(COUNT_SYMBOL_ONLY_AS_DONE);
    let (mut done, mut todo, mut appr) = (0, 0, 0);
    for e in entries {
        if !e.pending() || (symbols_as_done && e.category == K_SYMBOLS) {
            done += 1;
        } else if approved.contains(&e.key()) {
            appr += 1;
        } else {
            todo += 1;
        }
    }
    (done + appr, todo, appr)
}
