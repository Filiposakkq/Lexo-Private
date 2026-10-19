//! A reader and checker for Hunspell dictionaries (`.dic` + `.aff`), for the Pro feature `spell.hunspell`.
//!
//! The dictionary is read as it is (nothing is expanded: a word is checked by taking the affixes off it, as Hunspell does). Supported: the flag formats (`ASCII`, `UTF-8`, `long`,
//! `num`), prefixes and suffixes with their conditions and the cross product, one continuation level of suffixes, `NEEDAFFIX`, `ONLYINCOMPOUND`, `TRY`, and the encodings UTF-8,
//! ISO-8859-1 and ISO-8859-2. Not supported: compound words, `REP`/`ICONV` tables and `KEEPCASE` (words are checked in the case they come in, or in lower case). Suggestions are words
//! at one edit from the unknown word (and, for languages with diacritics, the word with its diacritics put back) that the dictionary accepts.
use std::collections::{HashMap, HashSet};

const MAX_VARIANT_CANDIDATES: usize = 3000;

/// One element of a condition such as `[^aeiou]y`.
#[derive(Clone, Debug)]
enum Tok {
    Any,
    Is(char),
    Set(bool, Vec<char>),
}

impl Tok {
    fn matches(&self, c: char) -> bool {
        match self {
            Tok::Any => true,
            Tok::Is(x) => *x == c,
            Tok::Set(neg, set) => set.contains(&c) != *neg,
        }
    }
}

fn parse_cond(s: &str) -> Vec<Tok> {
    let chars: Vec<char> = s.chars().collect();
    let (mut out, mut i) = (Vec::new(), 0);
    while i < chars.len() {
        match chars[i] {
            '.' => out.push(Tok::Any),
            '[' => {
                let mut j = i + 1;
                let neg = chars.get(j) == Some(&'^');
                if neg {
                    j += 1;
                }
                let mut set = Vec::new();
                while j < chars.len() && chars[j] != ']' {
                    set.push(chars[j]);
                    j += 1;
                }
                out.push(Tok::Set(neg, set));
                i = j;
            }
            c => out.push(Tok::Is(c)),
        }
        i += 1;
    }
    out
}

#[derive(Clone, Debug)]
struct Rule {
    flag: u32,
    cross: bool,
    strip: String,
    add: String,
    cond: Vec<Tok>,
    conts: Vec<u32>,
}

impl Rule {
    /// Does the end (suffix) or the start (prefix) of `stem` fit the condition?
    fn fits(&self, stem: &str, suffix: bool) -> bool {
        if self.cond.is_empty() {
            return true;
        }
        let chars: Vec<char> = stem.chars().collect();
        if chars.len() < self.cond.len() {
            return false;
        }
        let part = if suffix { &chars[chars.len() - self.cond.len()..] } else { &chars[..self.cond.len()] };
        self.cond.iter().zip(part).all(|(t, c)| t.matches(*c))
    }
}

#[derive(Clone, Copy, PartialEq)]
enum FlagKind {
    Ascii,
    Long,
    Num,
}

pub struct Dict {
    roots: HashMap<String, Vec<u32>>,
    pfx: Vec<Rule>,
    sfx: Vec<Rule>,
    pfx_by: HashMap<char, Vec<usize>>,
    sfx_by: HashMap<char, Vec<usize>>,
    need_affix: Option<u32>,
    only_compound: Option<u32>,
    alphabet: Vec<char>,
    variants: HashMap<char, Vec<char>>,
}

fn decode(bytes: &[u8], enc: &str) -> Result<String, String> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let e = enc.to_uppercase().replace('_', "-");
    match e.as_str() {
        "UTF-8" | "UTF8" => Ok(String::from_utf8_lossy(bytes).into_owned()),
        "ISO8859-1" | "ISO-8859-1" | "LATIN1" => Ok(bytes.iter().map(|&b| b as char).collect()),
        "ISO8859-2" | "ISO-8859-2" | "LATIN2" => {
            const HIGH: &str = " Ą˘Ł¤ĽŚ§¨ŠŞŤŹ\u{AD}ŽŻ°ą˛ł´ľśˇ¸šşťź˝žżŔÁÂĂÄĹĆÇČÉĘËĚÍÎĎĐŃŇÓÔŐÖ×ŘŮÚŰÜÝŢßŕáâăäĺćçčéęëěíîďđńňóôőö÷řůúűüýţ˙";
            let high: Vec<char> = HIGH.chars().collect();
            Ok(bytes.iter().map(|&b| if b < 0xA0 { b as char } else { high[(b - 0xA0) as usize] }).collect())
        }
        other => Err(format!("The encoding {other} of the dictionary is not supported (use a UTF-8 dictionary).")),
    }
}

/// The encoding named by `SET` in the affix file (read from the bytes, which are ASCII up to that line).
fn encoding_of(aff: &[u8]) -> String {
    let head = String::from_utf8_lossy(&aff[..aff.len().min(4000)]).into_owned();
    head.lines().find_map(|l| l.trim_start_matches('\u{feff}').strip_prefix("SET ").map(|e| e.trim().to_string())).unwrap_or_else(|| "ISO8859-1".into())
}

struct Flags {
    kind: FlagKind,
    ids: HashMap<String, u32>,
}

impl Flags {
    fn id(&mut self, s: &str) -> u32 {
        let n = self.ids.len() as u32;
        *self.ids.entry(s.to_string()).or_insert(n)
    }

    fn parse(&mut self, s: &str) -> Vec<u32> {
        let parts: Vec<String> = match self.kind {
            FlagKind::Ascii => s.chars().map(String::from).collect(),
            FlagKind::Long => s.chars().collect::<Vec<_>>().chunks(2).map(|c| c.iter().collect()).collect(),
            FlagKind::Num => s.split(',').filter(|p| !p.is_empty()).map(String::from).collect(),
        };
        parts.iter().map(|p| self.id(p)).collect()
    }
}

/// The characters with an accent that have `base` as their plain letter (so that "e" can become "é" or "ě").
fn plain_letter(c: char) -> char {
    let mut b = [0u8; 4];
    let folded = lexo_core::util::fold(c.encode_utf8(&mut b));
    let mut it = folded.chars();
    match (it.next(), it.next()) {
        (Some(f), None) => f,
        _ => c,
    }
}

impl Dict {
    pub fn load(dic: &[u8], aff: &[u8]) -> Result<Dict, String> {
        let enc = encoding_of(aff);
        let aff_text = decode(aff, &enc)?;
        let dic_text = decode(dic, &enc)?;
        let mut flags = Flags { kind: FlagKind::Ascii, ids: HashMap::new() };
        let (mut pfx, mut sfx): (Vec<Rule>, Vec<Rule>) = (Vec::new(), Vec::new());
        let mut cross: HashMap<(bool, String), bool> = HashMap::new();
        let (mut need, mut only, mut try_chars) = (None, None, String::new());
        // the flag format comes first: later lines depend on it
        for line in aff_text.lines() {
            let t: Vec<&str> = line.split_whitespace().collect();
            if t.first() == Some(&"FLAG") {
                flags.kind = match t.get(1).map(|s| s.to_uppercase()).as_deref() {
                    Some("LONG") => FlagKind::Long,
                    Some("NUM") => FlagKind::Num,
                    _ => FlagKind::Ascii,
                };
            }
        }
        for line in aff_text.lines() {
            let line = line.trim_start_matches('\u{feff}');
            let t: Vec<&str> = line.split_whitespace().collect();
            match t.first().copied() {
                Some("NEEDAFFIX") | Some("PSEUDOROOT") if t.len() > 1 => need = flags.parse(t[1]).first().copied(),
                Some("ONLYINCOMPOUND") if t.len() > 1 => only = flags.parse(t[1]).first().copied(),
                Some("TRY") if t.len() > 1 => try_chars = t[1].to_string(),
                Some(kind @ ("PFX" | "SFX")) if t.len() >= 4 => {
                    let suffix = kind == "SFX";
                    let is_header = t.len() <= 4 && matches!(t[2], "Y" | "N") && t[3].chars().all(|c| c.is_ascii_digit());
                    if is_header {
                        cross.insert((suffix, t[1].to_string()), t[2] == "Y");
                        continue;
                    }
                    if t.len() < 4 {
                        continue;
                    }
                    let flag = flags.id(t[1]);
                    let strip = if t[2] == "0" { String::new() } else { t[2].to_string() };
                    let (add_raw, conts) = match t[3].split_once('/') {
                        Some((a, f)) => (a, flags.parse(f)),
                        None => (t[3], Vec::new()),
                    };
                    let add = if add_raw == "0" { String::new() } else { add_raw.to_string() };
                    let cond = t.get(4).filter(|c| **c != ".").map(|c| parse_cond(c)).unwrap_or_default();
                    let rule = Rule { flag, cross: *cross.get(&(suffix, t[1].to_string())).unwrap_or(&true), strip, add, cond, conts };
                    if suffix {
                        sfx.push(rule)
                    } else {
                        pfx.push(rule)
                    }
                }
                _ => {}
            }
        }
        let mut roots: HashMap<String, Vec<u32>> = HashMap::new();
        let mut chars: HashMap<char, usize> = HashMap::new();
        for (n, line) in dic_text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || (n == 0 && line.chars().all(|c| c.is_ascii_digit())) {
                continue;
            }
            let token = line.split_whitespace().next().unwrap_or("");
            let (word, fl) = token.split_once('/').unwrap_or((token, ""));
            if word.is_empty() {
                continue;
            }
            for c in word.chars() {
                *chars.entry(c).or_insert(0) += 1;
            }
            let f = flags.parse(fl);
            roots.entry(word.to_string()).or_default().extend(f);
        }
        if roots.is_empty() {
            return Err("The dictionary has no words.".into());
        }
        let index = |rules: &[Rule], suffix: bool| {
            let mut m: HashMap<char, Vec<usize>> = HashMap::new();
            for (i, r) in rules.iter().enumerate() {
                let key = if suffix { r.add.chars().last() } else { r.add.chars().next() }.unwrap_or('\0');
                m.entry(key).or_default().push(i);
            }
            m
        };
        // the letters for insertions: the dictionary's own `TRY`, else the letters it uses most
        let mut alphabet: Vec<char> = try_chars.chars().filter(|c| c.is_alphabetic()).collect();
        if alphabet.is_empty() {
            let mut by: Vec<(char, usize)> = chars.iter().filter(|(c, _)| c.is_alphabetic()).map(|(c, n)| (*c, *n)).collect();
            by.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            alphabet = by.into_iter().take(45).map(|(c, _)| c).collect();
        }
        let mut variants: HashMap<char, Vec<char>> = HashMap::new();
        let mut seen: Vec<char> = chars.keys().copied().filter(|c| c.is_alphabetic()).collect();
        seen.sort();
        for c in seen {
            let base = plain_letter(c);
            if base != c {
                variants.entry(base).or_default().push(c);
            }
        }
        let (pfx_by, sfx_by) = (index(&pfx, false), index(&sfx, true));
        Ok(Dict { roots, pfx, sfx, pfx_by, sfx_by, need_affix: need, only_compound: only, alphabet, variants })
    }

    pub fn len(&self) -> usize {
        self.roots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    fn root_has(&self, w: &str, flag: Option<u32>) -> bool {
        let Some(f) = self.roots.get(w) else { return false };
        match flag {
            Some(x) => f.contains(&x),
            None => !self.need_affix.is_some_and(|n| f.contains(&n)) && !self.only_compound.is_some_and(|n| f.contains(&n)),
        }
    }

    /// `w` is a root with a suffix (or two) taken off. `outer` is the flag of the suffix that was put on after this one: this suffix must name it among its continuation flags.
    fn suffixed(&self, w: &str, outer: Option<u32>, depth: u8) -> bool {
        let Some(last) = w.chars().last() else { return false };
        for key in [last, '\0'] {
            for &i in self.sfx_by.get(&key).map(|v| v.as_slice()).unwrap_or(&[]) {
                let r = &self.sfx[i];
                if outer.is_some_and(|o| !r.conts.contains(&o)) || !w.ends_with(&r.add) {
                    continue;
                }
                let stem = format!("{}{}", &w[..w.len() - r.add.len()], r.strip);
                if stem.is_empty() || !r.fits(&stem, true) {
                    continue;
                }
                if self.root_has(&stem, Some(r.flag)) || (depth < 2 && self.suffixed(&stem, Some(r.flag), depth + 1)) {
                    return true;
                }
            }
        }
        false
    }

    fn known(&self, w: &str) -> bool {
        if self.root_has(w, None) || self.suffixed(w, None, 0) {
            return true;
        }
        let Some(first) = w.chars().next() else { return false };
        for key in [first, '\0'] {
            for &i in self.pfx_by.get(&key).map(|v| v.as_slice()).unwrap_or(&[]) {
                let p = &self.pfx[i];
                if !w.starts_with(&p.add) {
                    continue;
                }
                let s1 = format!("{}{}", p.strip, &w[p.add.len()..]);
                if s1.is_empty() || !p.fits(&s1, false) {
                    continue;
                }
                if self.root_has(&s1, Some(p.flag)) {
                    return true;
                }
                // a prefix and a suffix on one word (the cross product)
                if p.cross && self.sfx_cross(&s1, p.flag) {
                    return true;
                }
            }
        }
        false
    }

    /// A word with a suffix that may be combined with prefixes; the root must carry the flag of the prefix too.
    fn sfx_cross(&self, w: &str, pfx_flag: u32) -> bool {
        let Some(last) = w.chars().last() else { return false };
        for key in [last, '\0'] {
            for &i in self.sfx_by.get(&key).map(|v| v.as_slice()).unwrap_or(&[]) {
                let r = &self.sfx[i];
                if !r.cross || !w.ends_with(&r.add) {
                    continue;
                }
                let stem = format!("{}{}", &w[..w.len() - r.add.len()], r.strip);
                if !stem.is_empty() && r.fits(&stem, true) && self.roots.get(&stem).is_some_and(|f| f.contains(&r.flag) && f.contains(&pfx_flag)) {
                    return true;
                }
            }
        }
        false
    }

    /// Is the word right? A word is tried as it is and in lower case; a word in capitals also with a capital first letter only. Words with `-` or `'` may be right part by part.
    pub fn check(&self, word: &str) -> bool {
        if word.chars().all(|c| !c.is_alphabetic()) {
            return true;
        }
        if self.check_plain(word) {
            return true;
        }
        let parts: Vec<&str> = word.split(['-', '\'', '’']).filter(|p| !p.is_empty()).collect();
        parts.len() > 1 && parts.iter().all(|p| p.chars().count() <= 1 || self.check_plain(p))
    }

    fn check_plain(&self, word: &str) -> bool {
        if self.known(word) {
            return true;
        }
        let lower = word.to_lowercase();
        if lower != word && self.known(&lower) {
            return true;
        }
        let upper = word.chars().count() > 1 && word.chars().all(|c| !c.is_lowercase());
        if upper {
            let mut cs = lower.chars();
            let title: String = cs.next().map(|f| f.to_uppercase().chain(cs).collect()).unwrap_or_default();
            return self.known(&title);
        }
        false
    }

    /// Up to `max` words the dictionary accepts that are one edit away from `word` or that are `word` with its diacritics put right.
    pub fn suggest(&self, word: &str, max: usize) -> Vec<String> {
        let lower = word.to_lowercase();
        let chars: Vec<char> = lower.chars().collect();
        let (mut out, mut seen): (Vec<String>, HashSet<String>) = (Vec::new(), HashSet::new());
        // the case of the word is kept; a word typed in lower case may also be a name that the dictionary has with a capital
        let title = word.chars().next().is_some_and(|c| c.is_uppercase());
        let all_caps = word.chars().count() > 1 && word.chars().all(|c| !c.is_lowercase());
        let titled = |s: &str| -> String {
            let mut cs = s.chars();
            cs.next().map(|f| f.to_uppercase().chain(cs).collect()).unwrap_or_default()
        };
        let mut take = |cand: String, out: &mut Vec<String>| {
            let mut forms: Vec<String> = if all_caps {
                vec![cand.to_uppercase()]
            } else if title {
                vec![titled(&cand)]
            } else {
                vec![cand.clone(), titled(&cand)]
            };
            // the capital form is only for a word that is right with a capital alone
            if let Some(i) = forms.iter().position(|f| f != word && self.check(f)) {
                let f = forms.swap_remove(i);
                if seen.insert(f.clone()) {
                    out.push(f);
                }
            }
        };
        // the same letters with other accents: "prelozit" -> "přeložit"
        let mut combos: Vec<String> = vec![String::new()];
        for c in &chars {
            let mut options = vec![*c];
            options.extend(self.variants.get(&plain_letter(*c)).into_iter().flatten().copied().filter(|v| v != c));
            if plain_letter(*c) != *c && !options.contains(&plain_letter(*c)) {
                options.push(plain_letter(*c));
            }
            let mut next = Vec::new();
            for base in &combos {
                for o in &options {
                    if next.len() >= MAX_VARIANT_CANDIDATES {
                        break;
                    }
                    next.push(format!("{base}{o}"));
                }
            }
            combos = next;
        }
        for c in combos {
            take(c, &mut out);
        }
        let n = chars.len();
        let mut edits: Vec<String> = Vec::new();
        for i in 0..n {
            let mut d = chars.clone();
            d.remove(i);
            edits.push(d.into_iter().collect());
        }
        for i in 0..n.saturating_sub(1) {
            let mut d = chars.clone();
            d.swap(i, i + 1);
            edits.push(d.into_iter().collect());
        }
        for i in 0..n {
            for a in &self.alphabet {
                let mut d = chars.clone();
                d[i] = *a;
                edits.push(d.iter().collect());
            }
        }
        for i in 0..=n {
            for a in &self.alphabet {
                let mut d = chars.clone();
                d.insert(i, *a);
                edits.push(d.iter().collect());
            }
        }
        for e in edits {
            take(e, &mut out);
            if out.len() >= max * 3 {
                break;
            }
        }
        out.truncate(max);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AFF: &str = "SET UTF-8\nTRY esianrtolcdugmphbyfvkwzESIANRTOLCDUGMPHBYFVKWZ'\nFLAG long\nNEEDAFFIX ZZ\n\
PFX Re Y 1\nPFX Re 0 re .\n\
SFX Pl Y 2\nSFX Pl 0 s [^sy]\nSFX Pl y ies [^aeiou]y\n\
SFX Ed Y 1\nSFX Ed 0 ed/Pl .\n";
    // "walked" = walk + Ed; "walkeds" is not a word in real English, but the continuation shows the chain: walk + ed + s
    const DIC: &str = "6\nwalk/PlEdRe\ncity/Pl\nhello\nHello\nbase/ZZPl\nPraha\n";

    fn dict() -> Dict {
        Dict::load(DIC.as_bytes(), AFF.as_bytes()).unwrap()
    }

    #[test]
    fn roots_and_affixes_are_accepted() {
        let d = dict();
        for w in ["walk", "walks", "walked", "walkeds", "rewalk", "rewalked", "cities", "city", "hello", "Praha", "praha"] {
            // "praha" in lower case is only right when the dictionary has it in lower case: it does not
            assert_eq!(d.check(w), w != "praha", "{w}");
        }
    }

    #[test]
    fn a_wrong_word_is_refused_and_the_flag_needaffix_makes_a_bare_root_wrong() {
        let d = dict();
        assert!(!d.check("walkd"));
        assert!(!d.check("base"), "needs an affix");
        assert!(d.check("bases"));
    }

    #[test]
    fn words_in_capitals_and_with_parts_are_checked() {
        let d = dict();
        assert!(d.check("HELLO") && d.check("Walk"));
        assert!(d.check("walk-hello") && !d.check("walk-helo"));
        assert!(d.check("walk's") || d.check("walk’s"), "an apostrophe splits the word");
    }

    #[test]
    fn suggestions_are_words_one_edit_away() {
        let d = dict();
        assert!(d.suggest("helo", 5).contains(&"hello".to_string()));
        assert!(d.suggest("wlak", 5).contains(&"walk".to_string()));
        assert_eq!(d.suggest("Helo", 3).first().map(String::as_str), Some("Hello"));
    }

    #[test]
    fn missing_diacritics_are_put_back() {
        let d = Dict::load("3\nprůběh\npřeložit\nžlutý\n".as_bytes(), b"SET UTF-8\n").unwrap();
        assert_eq!(d.suggest("preložit", 3), vec!["přeložit"]);
        assert!(d.suggest("zluty", 3).contains(&"žlutý".to_string()));
        assert!(d.suggest("prubeh", 3).contains(&"průběh".to_string()));
    }

    #[test]
    fn an_old_encoding_is_read_and_an_unknown_one_is_refused() {
        // "žlutý" in ISO-8859-2: ž = 0xBE, ý = 0xFD
        let mut dic = b"1\n".to_vec();
        dic.extend([0xBE, b'l', b'u', b't', 0xFD, b'\n']);
        let d = Dict::load(&dic, b"SET ISO8859-2\n").unwrap();
        assert!(d.check("žlutý"));
        assert!(Dict::load(b"1\nabc\n", b"SET KOI8-R\n").is_err());
        assert!(Dict::load(b"0\n", b"SET UTF-8\n").is_err());
    }
}
