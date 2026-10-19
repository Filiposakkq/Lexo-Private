//! The profile of a character (Pro feature `mt.profiles`): gender, how the character speaks and how they address other characters. It is kept in the NOTE of the glossary entry of the
//! speaker, as one last line that starts with `@profile `, so that it travels with the glossary (the CSV file, the event log, the merge of two translators) without a new kind of data:
//!
//! ```text
//! the free text of the note
//! @profile gender=f; style=sarcastic, short sentences; to:alex=informal; to:mr_smith=formal
//! ```
//!
//! The free part is what the glossary page shows and edits; the profile line is hidden there and kept when the note is saved (`keep`).
use std::collections::BTreeMap;

pub const MARK: &str = "@profile ";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Profile {
    /// `m`, `f`, `n` or empty.
    pub gender: String,
    /// How the character speaks, free text (no semicolon, no line break).
    pub style: String,
    /// The code of another speaker -> `informal` or `formal`.
    pub to: BTreeMap<String, String>,
}

impl Profile {
    pub fn is_empty(&self) -> bool {
        self.gender.is_empty() && self.style.is_empty() && self.to.is_empty()
    }
}

/// The note without its profile line, and the profile line itself (empty when there is none).
pub fn split(note: &str) -> (String, String) {
    let mut free: Vec<&str> = Vec::new();
    let mut line = String::new();
    for l in note.lines() {
        if l.starts_with(MARK) {
            line = l.to_string();
        } else {
            free.push(l);
        }
    }
    (free.join("\n").trim().to_string(), line)
}

fn clean(s: &str) -> String {
    s.replace([';', '\n', '\r'], ",").trim().to_string()
}

fn code_ok(c: &str) -> bool {
    !c.is_empty() && c.chars().all(|ch| !ch.is_whitespace() && !matches!(ch, ';' | '=' | ':'))
}

/// The profile of a note (empty when the note has none).
pub fn parse(note: &str) -> Profile {
    let line = split(note).1;
    let mut p = Profile::default();
    for part in line.strip_prefix(MARK).unwrap_or("").split(';') {
        let Some((k, v)) = part.split_once('=') else { continue };
        let (k, v) = (k.trim(), v.trim());
        match k {
            "gender" if ["m", "f", "n"].contains(&v) => p.gender = v.to_string(),
            "style" => p.style = v.to_string(),
            _ => {
                if let Some(code) = k.strip_prefix("to:") {
                    if code_ok(code) && (v == "informal" || v == "formal") {
                        p.to.insert(code.to_string(), v.to_string());
                    }
                }
            }
        }
    }
    p
}

/// The profile line (empty for an empty profile).
pub fn line_of(p: &Profile) -> String {
    if p.is_empty() {
        return String::new();
    }
    let mut parts: Vec<String> = Vec::new();
    if ["m", "f", "n"].contains(&p.gender.as_str()) {
        parts.push(format!("gender={}", p.gender));
    }
    let style = clean(&p.style);
    if !style.is_empty() {
        parts.push(format!("style={style}"));
    }
    for (code, form) in &p.to {
        if code_ok(code) && (form == "informal" || form == "formal") {
            parts.push(format!("to:{code}={form}"));
        }
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("{MARK}{}", parts.join("; "))
    }
}

/// The note: the free text and the profile line below it.
pub fn with_line(free: &str, line: &str) -> String {
    let free = free.trim();
    match (free.is_empty(), line.is_empty()) {
        (_, true) => free.to_string(),
        (true, false) => line.to_string(),
        (false, false) => format!("{free}\n{line}"),
    }
}

/// A note that is saved from the glossary page (which knows only the free text): a note without its own profile line gets the stored one back.
pub fn keep(stored_note: &str, new_note: &str) -> String {
    if split(new_note).1.is_empty() {
        with_line(new_note, &split(stored_note).1)
    } else {
        new_note.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(gender: &str, style: &str, to: &[(&str, &str)]) -> Profile {
        Profile { gender: gender.into(), style: style.into(), to: to.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect() }
    }

    #[test]
    fn a_profile_is_written_below_the_note_and_read_back() {
        let prof = p("f", "sarcastic, short sentences", &[("alex", "informal"), ("mr_smith", "formal")]);
        let note = with_line("The older sister.", &line_of(&prof));
        assert_eq!(note, "The older sister.\n@profile gender=f; style=sarcastic, short sentences; to:alex=informal; to:mr_smith=formal");
        assert_eq!(parse(&note), prof);
        assert_eq!(split(&note).0, "The older sister.");
    }

    #[test]
    fn an_empty_profile_leaves_no_line_and_bad_values_are_dropped() {
        assert_eq!(line_of(&Profile::default()), "");
        assert_eq!(with_line("only text", ""), "only text");
        let bad = parse("@profile gender=x; to:alex=rude; style=ok; to:=formal; nonsense");
        assert_eq!(bad, p("", "ok", &[]));
        // the style cannot break the line
        assert_eq!(line_of(&p("", "a; b\nc", &[])), "@profile style=a, b,c");
    }

    #[test]
    fn saving_a_note_from_the_glossary_page_keeps_the_hidden_profile() {
        let stored = "text\n@profile gender=m";
        assert_eq!(keep(stored, "new text"), "new text\n@profile gender=m");
        assert_eq!(keep(stored, ""), "@profile gender=m");
        assert_eq!(keep("text", "new"), "new"); // no profile stored: nothing to keep
        assert_eq!(keep(stored, "x\n@profile gender=f"), "x\n@profile gender=f");
        // a note that brings its own profile
    }
}
