//! Port of `core/lexo/util.py`.

/// What Python's `str.isspace` (and so the `\s` of `re`) counts as white space: Unicode White_Space plus the separators U+001C to U+001F.
fn is_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
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
}
