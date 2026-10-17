//! Port of `core/lexo/util.py` (the parts the ported modules need so far) and the few Python string rules the other modules rely on.
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

/// What Python's `str.isspace` (and so the `\s` of `re`, `str.strip()` and `str.split()`) counts as white space: Unicode White_Space plus the separators U+001C to U+001F.
pub fn is_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// `\w` of Python's `re`: a letter, a digit or an underscore (the `regex` crate's own `\w` is used outside ASCII, see the notes in `rpy.rs`).
pub fn is_word(c: char) -> bool {
    if c.is_ascii() {
        return c == '_' || c.is_ascii_alphanumeric();
    }
    static R: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    R.get_or_init(|| regex::Regex::new(r"\A\w\z").unwrap()).is_match(c.encode_utf8(&mut [0; 4]))
}

/// `[^\W\d_]` of Python's `re`: a letter (a word character that is neither a digit nor an underscore).
pub fn is_letter(c: char) -> bool {
    if c.is_ascii() {
        return c.is_ascii_alphabetic();
    }
    static R: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    R.get_or_init(|| regex::Regex::new(r"\A[^\W\d_]\z").unwrap()).is_match(c.encode_utf8(&mut [0; 4]))
}

/// `text.strip()`
pub fn py_strip(text: &str) -> &str {
    text.trim_matches(is_space)
}

/// `text.split()`: the words between runs of white space.
pub fn py_split_ws(text: &str) -> Vec<&str> {
    text.split(is_space).filter(|p| !p.is_empty()).collect()
}

/// `re.sub(r"\s+", " ", text).strip()`: runs of white space become one space, the ends are trimmed.
pub fn norm_ws(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending = false;
    for c in text.chars() {
        if is_space(c) {
            pending = true;
        } else {
            if pending && !out.is_empty() {
                out.push(' ');
            }
            pending = false;
            out.push(c);
        }
    }
    out
}

/// `str.splitlines()`: lines end at `\n`, `\r\n`, `\r`, `\v`, `\f`, U+001C to U+001E, U+0085, U+2028 and U+2029; a final line end does not make an empty line.
pub fn py_splitlines(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push(std::mem::take(&mut cur));
            }
            '\n' | '\u{b}' | '\u{c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}' => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Lower-case, diacritics stripped (for forgiving comparisons of user-typed words).
pub fn fold(text: &str) -> String {
    let stripped: String = text.to_lowercase().nfd().filter(|c| !is_combining_mark(*c)).collect();
    py_strip(&stripped).to_string()
}

/// The SHA-1 of `data` as 40 lower-case hex digits (`hashlib.sha1(data).hexdigest()`).
pub fn sha1_hex(data: &[u8]) -> String {
    Sha1::digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

/// A multiset that remembers the order of first appearance (Python's `Counter`).
#[derive(Clone, Debug, Default)]
pub struct Counter {
    pub order: Vec<String>,
    pub map: HashMap<String, usize>,
}

impl Counter {
    pub fn from<I: IntoIterator<Item = String>>(items: I) -> Counter {
        let mut c = Counter::default();
        for i in items {
            c.add(i);
        }
        c
    }
    pub fn add(&mut self, key: String) {
        if !self.map.contains_key(&key) {
            self.order.push(key.clone());
        }
        *self.map.entry(key).or_insert(0) += 1;
    }
    pub fn get(&self, key: &str) -> usize {
        self.map.get(key).copied().unwrap_or(0)
    }
    pub fn same_as(&self, other: &Counter) -> bool {
        self.map.len() == other.map.len() && self.map.iter().all(|(k, v)| other.get(k) == *v)
    }
    /// `self - other`: the keys with a positive difference, in the order of `self`.
    pub fn minus(&self, other: &Counter) -> Vec<String> {
        self.order.iter().filter(|k| self.get(k) > other.get(k)).cloned().collect()
    }
    /// `sorted(counter.elements())`
    pub fn sorted_elements(&self) -> Vec<String> {
        let mut v: Vec<String> = self.order.iter().flat_map(|k| std::iter::repeat_n(k.clone(), self.get(k))).collect();
        v.sort();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The expected values are what the Python function answers (checked with `core/lexo/util.py`).
    #[test]
    fn norm_ws_matches_python() {
        let cases = [
            ("  a \t\n b  ", "a b"),
            ("a\u{a0}\u{a0}b", "a b"),
            ("x\u{1f}y", "x y"),
            ("\u{3000}a\u{2003}b\u{2029}", "a b"),
            ("\u{85}a\u{b}b\u{c}c", "a b c"),
            ("", ""),
            ("   ", ""),
            ("no  change", "no change"),
        ];
        for (input, want) in cases {
            assert_eq!(norm_ws(input), want, "{input:?}");
        }
    }

    #[test]
    fn strip_and_split_follow_python() {
        assert_eq!(py_strip("\u{1f} a b \u{a0}"), "a b");
        assert_eq!(py_split_ws(" a\u{1f}b  c "), vec!["a", "b", "c"]);
        assert!(py_split_ws("   ").is_empty());
    }

    #[test]
    fn splitlines_follows_python() {
        assert_eq!(py_splitlines("a\r\nb\rc\n\nd\u{2028}e\n"), vec!["a", "b", "c", "", "d", "e"]);
        assert!(py_splitlines("").is_empty());
        assert_eq!(py_splitlines("x"), vec!["x"]);
    }

    #[test]
    fn fold_drops_case_and_diacritics() {
        assert_eq!(fold("  Příliš ŽLUŤOUČKÝ "), "prilis zlutoucky");
    }

    #[test]
    fn sha1_of_abc() {
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }
}
