//! Port of `pro/lexo_pro/regex_replace.py`: regular expression find and replace over a list of texts (the Pro feature `replace.regex`).
//!
//! Works on plain data: the request carries the pattern, the replacement and the texts; the answer is the new text and the number of replacements for every text. Core decides which lines
//! are candidates and writes the confirmed result; tags, variables and escapes are protected exactly as in the plain modes (`textedit::sub_pattern`).
//!
//! Known differences from Python: the pattern is read by the `fancy-regex` engine (look-around and back-references work; a few Python-only constructs do not), and the wording of an
//! error of the pattern or of the replacement is the engine's own.
use fancy_regex::{Captures, Regex};
use lexo_core::extensions::ExtensionError;
use lexo_core::messages::{msg, py_str, Text};
use lexo_core::textedit::sub_pattern;
use lexo_core::util::truthy;
use serde_json::{json, Value};

fn error(m: lexo_core::messages::Message) -> ExtensionError {
    ExtensionError(Text::new_msg(m))
}

/// One piece of a replacement template of Python's `re`: text, or a group.
enum Piece {
    Text(String),
    Group(usize),
}

/// Python's template syntax: `\1` .. `\99`, `\g<name>`, `\g<1>`, `\n` `\t` ..., `\\`; a group that did not take part gives nothing.
fn parse_template(template: &str, re: &Regex) -> Result<Vec<Piece>, String> {
    let names: Vec<(String, usize)> = re.capture_names().enumerate().filter_map(|(i, n)| n.map(|n| (n.to_string(), i))).collect();
    let groups = re.captures_len();
    let group_of = |name: &str| -> Result<usize, String> {
        match name.parse::<usize>() {
            Ok(n) if n < groups => Ok(n),
            Ok(n) => Err(format!("invalid group reference {n}")),
            Err(_) => names.iter().find(|(k, _)| k == name).map(|(_, i)| *i).ok_or_else(|| format!("unknown group name '{name}'")),
        }
    };
    let chars: Vec<char> = template.chars().collect();
    let (mut out, mut text, mut i) = (Vec::new(), String::new(), 0);
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        if c != '\\' {
            text.push(c);
            continue;
        }
        let Some(&next) = chars.get(i) else { return Err("bad escape (end of pattern)".into()) };
        i += 1;
        match next {
            'g' => {
                if chars.get(i) != Some(&'<') {
                    return Err("missing <".into());
                }
                let end = chars[i..].iter().position(|c| *c == '>').ok_or("missing >, unterminated name")?;
                let name: String = chars[i + 1..i + end].iter().collect();
                i += end + 1;
                if name.is_empty() {
                    return Err("missing group name".into());
                }
                out.push(Piece::Text(std::mem::take(&mut text)));
                out.push(Piece::Group(group_of(&name)?));
            }
            '0' => {
                // an octal escape: up to two more octal digits
                let mut v = 0u32;
                let mut k = 0;
                while k < 2 && chars.get(i).is_some_and(|d| ('0'..='7').contains(d)) {
                    v = v * 8 + chars[i].to_digit(8).unwrap_or(0);
                    i += 1;
                    k += 1;
                }
                text.push(char::from_u32(v).unwrap_or('\0'));
            }
            '1'..='9' => {
                let mut digits = next.to_string();
                if let Some(d) = chars.get(i).filter(|d| d.is_ascii_digit()) {
                    // three octal digits are a character, two digits a group
                    if next <= '3' && *d <= '7' && chars.get(i + 1).is_some_and(|e| ('0'..='7').contains(e)) {
                        let v = digits.parse::<u32>().unwrap_or(0) * 64 + d.to_digit(8).unwrap_or(0) * 8 + chars[i + 1].to_digit(8).unwrap_or(0);
                        i += 2;
                        text.push(char::from_u32(v).unwrap_or('\0'));
                        continue;
                    }
                    digits.push(*d);
                    i += 1;
                }
                out.push(Piece::Text(std::mem::take(&mut text)));
                out.push(Piece::Group(group_of(&digits)?));
            }
            'n' => text.push('\n'),
            't' => text.push('\t'),
            'r' => text.push('\r'),
            'f' => text.push('\u{c}'),
            'v' => text.push('\u{b}'),
            'a' => text.push('\u{7}'),
            'b' => text.push('\u{8}'),
            '\\' => text.push('\\'),
            c if c.is_ascii_alphabetic() => return Err(format!("bad escape \\{c}")),
            c => {
                text.push('\\');
                text.push(c);
            }
        }
    }
    out.push(Piece::Text(text));
    Ok(out)
}

fn expand(pieces: &[Piece], caps: &Captures) -> String {
    let mut out = String::new();
    for p in pieces {
        match p {
            Piece::Text(t) => out.push_str(t),
            Piece::Group(n) => out.push_str(caps.get(*n).map_or("", |m| m.as_str())),
        }
    }
    out
}

pub fn substitute(p: &Value) -> Result<Value, ExtensionError> {
    let find = py_str(&p["pattern"]);
    let replace = py_str(&p["replace"]);
    let case = p.get("case").is_some_and(truthy);
    let protect = p.get("protect").is_none_or(truthy);
    if find.is_empty() {
        return Err(error(msg("Enter what to search for.", &[])));
    }
    if find.chars().count() > 300 || replace.chars().count() > 2000 {
        return Err(error(msg("The text to find or replace is too long.", &[])));
    }
    let flags = if case { "" } else { "(?i)" };
    let re = Regex::new(&format!("{flags}{find}")).map_err(|e| error(msg("Invalid regular expression: {e}", &[("e", json!(e.to_string()))])))?;
    let template = parse_template(&replace, &re).map_err(|e| error(msg("Invalid regular expression: {e}", &[("e", json!(e))])))?; // bad group references are refused early
    let mut results = Vec::new();
    for text in p["texts"].as_array().into_iter().flatten() {
        let text = py_str(text);
        let (new, n) =
            sub_pattern(&re, &text, protect, 0, &mut |c| Ok(expand(&template, c))).map_err(|e| error(msg("Invalid replacement: {e}", &[("e", json!(e))])))?;
        results.push(json!({"new": new, "n": n}));
    }
    Ok(json!({"results": results}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(pattern: &str, replace: &str, texts: &[&str], case: bool, protect: bool) -> Result<Vec<(String, u64)>, ExtensionError> {
        let v = substitute(&json!({"pattern": pattern, "replace": replace, "case": case, "protect": protect, "texts": texts}))?;
        Ok(v["results"].as_array().unwrap().iter().map(|r| (r["new"].as_str().unwrap().to_string(), r["n"].as_u64().unwrap())).collect())
    }

    #[test]
    fn groups_case_and_the_count() {
        assert_eq!(
            sub(r"(\w+)ovi", r"\1u", &["Petrovi a Honzovi", "nothing"], false, true).unwrap(),
            vec![("Petru a Honzu".to_string(), 2), ("nothing".to_string(), 0)]
        );
        assert_eq!(sub("hello", "x", &["Hello"], false, true).unwrap()[0].1, 1);
        assert_eq!(sub("hello", "x", &["Hello"], true, true).unwrap()[0].1, 0);
        assert_eq!(sub(r"(?P<w>a)", r"\g<w>\g<w>", &["bab"], false, true).unwrap()[0].0, "baab");
    }

    #[test]
    fn it_cannot_match_across_a_tag_or_inside_one() {
        assert_eq!(sub("a.*b", "X", &["a{i}b"], false, true).unwrap()[0], ("a{i}b".to_string(), 0));
        assert_eq!(sub("i", "X", &["a{i}[name]b i"], false, true).unwrap()[0].0, "a{i}[name]b X");
        assert_eq!(sub("i", "X", &["{i}"], false, false).unwrap()[0].1, 1);
    }

    #[test]
    fn bad_input_is_a_message_for_the_user() {
        for (pattern, replace) in [("(", "x"), ("a", r"\9"), ("", "x"), ("a", r"\q")] {
            assert!(sub(pattern, replace, &["text"], false, true).is_err(), "{pattern} {replace}");
        }
    }
}
