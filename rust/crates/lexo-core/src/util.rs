//! Port of `core/lexo/util.py` (the parts the ported modules need so far) and the few Python string rules the other modules rely on.
use sha1::{Digest, Sha1};
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

/// What Python's `str.isspace` (and so the `\s` of `re`, `str.strip()` and `str.split()`) counts as white space: Unicode White_Space plus the separators U+001C to U+001F.
pub fn is_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
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

/// Lower-case, diacritics stripped (for forgiving comparisons of user-typed words).
pub fn fold(text: &str) -> String {
    let stripped: String = text.to_lowercase().nfd().filter(|c| !is_combining_mark(*c)).collect();
    py_strip(&stripped).to_string()
}

/// The SHA-1 of `data` as 40 lower-case hex digits (`hashlib.sha1(data).hexdigest()`).
pub fn sha1_hex(data: &[u8]) -> String {
    Sha1::digest(data).iter().map(|b| format!("{b:02x}")).collect()
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
    fn fold_drops_case_and_diacritics() {
        assert_eq!(fold("  Příliš ŽLUŤOUČKÝ "), "prilis zlutoucky");
    }

    #[test]
    fn sha1_of_abc() {
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }
}
