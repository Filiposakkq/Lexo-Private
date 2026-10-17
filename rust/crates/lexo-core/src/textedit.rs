//! Port of `core/lexo/textedit.py`: text-level helpers for batch edits of translations: find / replace in the plain modes (plain, whole word, by stem).
//! The regex mode and the safe fixes are Pro features: this module refuses the regex mode with the same message as Python.
//!
//! Everything here is pure (no file access): it turns a translated text into a proposed new text. Tags `{i}`, variables `[name]` and escapes (`\n`, `\"`)
//! are protected by default, so a replacement can never break them.
use crate::messages::{msg, Message};
use crate::util::is_letter;
use fancy_regex::Regex as FancyRegex;
use regex::Regex;
use std::sync::OnceLock;

const LETTER: &str = r"[^\W\d_]";
pub const MODES: [&str; 4] = ["plain", "word", "stem", "regex"];

fn token_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\{[^}]*\}|\[[^\]]*\]|\\.").unwrap())
}

/// Same length (in characters), with protected parts replaced by NUL so no pattern can match inside them.
pub fn mask(text: &str) -> String {
    token_re().replace_all(text, |m: &regex::Captures| "\0".repeat(m[0].chars().count())).into_owned()
}

/// A capital first letter of the model carries over to the new text.
pub fn case_like(model: &str, new: &str) -> String {
    let mut m = model.chars();
    let mut n = new.chars();
    match (m.next(), n.next()) {
        (Some(a), Some(b)) if a.is_uppercase() && b.is_lowercase() => b.to_uppercase().chain(n).collect(),
        _ => new.to_string(),
    }
}

fn stem_of(word: &str) -> String {
    let n = word.chars().count();
    word.chars().take(3.max(n.saturating_sub(2))).collect()
}

fn skip_chars(s: &str, n: usize) -> &str {
    s.char_indices().nth(n).map_or("", |(i, _)| &s[i..])
}

enum Kind {
    Literal(String),
    Stem { find: String, replace: String, stem: String, new_stem: String },
}

pub struct Replacer {
    pat: FancyRegex,
    kind: Kind,
}

impl Replacer {
    fn replacement(&self, whole: &str) -> String {
        match &self.kind {
            Kind::Literal(r) => r.clone(),
            Kind::Stem { find, replace, stem, new_stem } => {
                let new = if whole.to_lowercase().starts_with(&find.to_lowercase()) {
                    format!("{replace}{}", skip_chars(whole, find.chars().count()))
                // the whole word as typed, plus any ending after it
                } else {
                    format!("{new_stem}{}", skip_chars(whole, stem.chars().count()))
                    // another form: keep its ending, swap the stem
                };
                case_like(whole, &new)
            }
        }
    }

    /// `(new text, count)`; the pattern is applied outside protected parts when `protect` is set.
    pub fn apply(&self, text: &str, protect: bool) -> (String, usize) {
        let hay = if protect { mask(text) } else { text.to_string() };
        // positions of the masked text are mapped back to the text by character index (the two differ in byte length)
        let mut char_at_byte = vec![0usize; hay.len() + 1];
        for (ci, (bi, c)) in hay.char_indices().enumerate() {
            for k in 0..c.len_utf8() {
                char_at_byte[bi + k] = ci;
            }
            char_at_byte[bi + c.len_utf8()] = ci + 1;
        }
        let mut byte_at_char: Vec<usize> = text.char_indices().map(|(b, _)| b).collect();
        byte_at_char.push(text.len());
        let (mut out, mut last, mut n) = (String::new(), 0usize, 0usize);
        for m in self.pat.find_iter(&hay).map_while(Result::ok) {
            if m.start() == m.end() || m.as_str().contains('\0') {
                continue;
            }
            let (s, e) = (byte_at_char[char_at_byte[m.start()]], byte_at_char[char_at_byte[m.end()]]);
            out.push_str(&text[last..s]);
            out.push_str(&self.replacement(m.as_str()));
            last = e;
            n += 1;
        }
        out.push_str(&text[last..]);
        (out, n)
    }
}

/// A replacer for the search, or the message that says why it cannot be made.
pub fn make_replacer(find: &str, replace: &str, mode: &str, case: bool) -> Result<Replacer, Message> {
    if find.is_empty() {
        return Err(msg("Enter what to search for.", &[]));
    }
    if find.chars().count() > 300 || replace.chars().count() > 2000 {
        return Err(msg("The text to find or replace is too long.", &[]));
    }
    if !MODES.contains(&mode) {
        return Err(msg("Unknown search mode.", &[]));
    }
    let flags = if case { "" } else { "(?i)" };
    if mode == "regex" {
        // a Pro feature: the app asks the Pro pack (replace.regex), it is never made here
        return Err(msg("Regex find & replace is a Pro feature.", &[]));
    }
    let build = |body: String| {
        FancyRegex::new(&format!("{flags}{body}")).map_err(|e| msg("Unusable search: {reason}", &[("reason", serde_json::json!(e.to_string()))]))
    };
    if mode == "stem" {
        if find.chars().count() < 4 || !find.chars().all(is_letter) || replace.chars().count() < 2 || !replace.chars().all(is_letter) {
            return Err(msg("The “by stem” mode is only for one word (at least 4 letters) and one replacement word.", &[]));
        }
        let stem = stem_of(find);
        let pat = build(format!("(?<!{LETTER})({})({LETTER}*)", fancy_regex::escape(&stem)))?;
        let new_stem = stem_of(replace);
        return Ok(Replacer { pat, kind: Kind::Stem { find: find.into(), replace: replace.into(), stem, new_stem } });
    }
    let mut body = fancy_regex::escape(find).into_owned();
    if mode == "word" {
        body = format!("(?<!{LETTER}){body}(?!{LETTER})");
    }
    Ok(Replacer { pat: build(body)?, kind: Kind::Literal(replace.into()) })
}
