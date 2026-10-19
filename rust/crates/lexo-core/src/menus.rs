//! Port of `reference/python/core/lexo/menus.py`: menus of a Ren'Py script: which lines are choices, what question they answer and what follows each of them.
//!
//! The translation of a choice sits in `translate <lang> strings:` (far from the dialogue), so the translator needs the menu around it.
//! This module only reads the script text; it finds nothing it cannot see (no jumps, no `call screen`, no menus built in Python).
use crate::util::py_strip;
use regex::Regex;
use serde_json::{json, Value};
use std::sync::OnceLock;

struct Patterns {
    option: Regex,
    menu: Regex,
    say: Regex,
    literal: Regex,
    before_block: Regex,
    label: Regex,
}

fn p() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        option: Regex::new(r#"^\s*(?:"((?:[^"\\]|\\.)*)"|'((?:[^'\\]|\\.)*)')\s*(?:\bif\b\s+(.+?))?\s*:\s*(?:#.*)?$"#).unwrap(),
        menu: Regex::new(r"^\s*menu(?:\s+\w+)?\s*(?:\(.*\))?\s*:\s*(?:#.*)?$").unwrap(),
        // a line of dialogue: `"text"`, or `who "text"`, or `who attr attr "text" with x`; not a statement that merely contains a string
        say: Regex::new(r#"^\s*(?:[A-Za-z_][\w.]*(?:\s+[\w@.]+)*\s+)?(?:"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*')\s*(?:with\s+.+)?$"#).unwrap(),
        literal: Regex::new(r#"(?:"((?:[^"\\]|\\.)*)"|'((?:[^'\\]|\\.)*)')"#).unwrap(),
        before_block: Regex::new(r"^\s*(?:if|elif|else|while|for|with)\b").unwrap(),
        label: Regex::new(r"^\s*label\b").unwrap(),
    })
}

const NOT_SAY: [&str; 33] = [
    "if",
    "elif",
    "else",
    "while",
    "for",
    "show",
    "scene",
    "hide",
    "play",
    "stop",
    "queue",
    "with",
    "image",
    "define",
    "default",
    "label",
    "jump",
    "call",
    "return",
    "pass",
    "python",
    "init",
    "screen",
    "transform",
    "style",
    "translate",
    "window",
    "pause",
    "voice",
    "renpy",
    "camera",
    "text",
    "menu",
];
/// First lines of the body of a choice kept as they are (what a choice does when no dialogue follows).
const BODY_LINES: usize = 4;
/// Lines of dialogue shown after each choice.
const AFTER_LINES: usize = 2;
/// Lines of dialogue shown before a menu that has no question of its own.
const BEFORE_LINES: usize = 2;
const LOOK_BACK: isize = 60;

fn indent_of(text: &str) -> usize {
    text.chars().take_while(|c| *c == ' ' || *c == '\t').count()
}

fn blank(text: &str) -> bool {
    let s = py_strip(text);
    s.is_empty() || s.starts_with('#')
}

fn is_say(text: &str) -> bool {
    if blank(text) || !p().say.is_match(text) {
        return false;
    }
    // `text.strip().split(None, 1)[0].rstrip(":").lower()` is a word of NOT_SAY ("$" is one, too)
    let first = py_strip(text).split(crate::util::is_space).next().unwrap_or("").trim_end_matches(':').to_lowercase();
    first != "$" && !NOT_SAY.contains(&first.as_str())
}

/// The text between the quotes of the first string literal of a line.
pub fn literal_of(text: &str) -> String {
    match p().literal.captures(text) {
        Some(m) => m.get(1).or_else(|| m.get(2)).map(|g| g.as_str().to_string()).unwrap_or_default(),
        None => String::new(),
    }
}

/// The last lines of dialogue before the menu on line index `head`, on the way the story reaches it: lines of the same block, and, when the menu is the first
/// thing in an `if` / `else` block, the lines before that `if` (the body of an earlier branch or of an earlier menu is skipped).
fn said_before(lines: &[String], head: usize) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    let mut level = indent_of(&lines[head]);
    let start = head as isize - 1;
    let stop = (-1isize).max(head as isize - 1 - LOOK_BACK);
    let mut i = start;
    while i > stop {
        let text = &lines[i as usize];
        i -= 1;
        if blank(text) {
            continue;
        }
        let ind = indent_of(text);
        if ind > level {
            continue;
        }
        if ind < level {
            if p().before_block.is_match(text) {
                level = ind;
                continue;
            }
            break;
        }
        if is_say(text) {
            out.insert(0, (i + 2) as usize);
            if out.len() >= BEFORE_LINES {
                break;
            }
        } else if p().menu.is_match(text) || p().label.is_match(text) {
            break;
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq)]
pub struct MenuOption {
    /// 1-based line of the choice.
    pub n: usize,
    pub text: String,
    pub cond: String,
    /// Line numbers of the dialogue after the choice.
    pub after: Vec<usize>,
    pub body: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Menu {
    pub menu_line: usize,
    pub question: Vec<usize>,
    pub before: Vec<usize>,
    pub options: Vec<MenuOption>,
    /// Index of the choice, -1 when it is not known.
    pub current: i64,
}

impl Menu {
    pub fn to_json(&self) -> Value {
        json!({
            "menu_line": self.menu_line, "question": self.question, "before": self.before, "current": self.current,
            "options": self.options.iter().map(|o| json!({"n": o.n, "text": o.text, "cond": o.cond, "after": o.after, "body": o.body})).collect::<Vec<_>>(),
        })
    }
}

/// The menu a line of the script is part of, or `None`: the choice itself (see `find_menu`), the question, the first lines of the body of a choice, or the lines
/// said just before the menu. Only lines that the menu shows count.
pub fn menu_around(lines: &[String], line: usize, choice: Option<&str>) -> Option<Menu> {
    let found = find_menu(lines, line, choice);
    if found.is_some() || !(1..=lines.len()).contains(&line) || !is_say(&lines[line - 1]) {
        return found;
    }
    let mut head: Option<usize> = None;
    let mut own = indent_of(&lines[line - 1]);
    let mut i = line as isize - 2;
    let stop = (-1isize).max(line as isize - 2 - 400);
    while i > stop {
        let text = &lines[i as usize];
        if blank(text) {
            i -= 1;
            continue;
        }
        let ind = indent_of(text);
        if ind >= own {
            i -= 1;
            continue;
        }
        if p().menu.is_match(text) {
            head = Some(i as usize);
            break;
        }
        if p().option.is_match(text) {
            head = find_menu(lines, i as usize + 1, None).map(|m| m.menu_line - 1);
            break;
        }
        if p().label.is_match(text) {
            break;
        }
        own = ind; // an `if` inside the choice: keep going up
        i -= 1;
    }
    if head.is_none() {
        // said just before a menu
        let mut seen = 0;
        for (i, text) in lines.iter().enumerate().take(lines.len().min(line + 80)).skip(line) {
            if blank(text) {
                continue;
            }
            seen += 1;
            if seen > 10 || p().label.is_match(text) {
                break;
            }
            if p().menu.is_match(text) {
                head = Some(i);
                break;
            }
        }
    }
    let inner = head.and_then(|h| find_menu(lines, h + 1, None))?;
    let shown = inner.question.iter().chain(inner.before.iter()).chain(inner.options.iter().flat_map(|o| o.after.iter()));
    if shown.into_iter().any(|n| *n == line) {
        Some(inner)
    } else {
        None
    }
}

/// The menu a choice belongs to, or `None`. `line` (1-based) is where Ren'Py recorded the choice: the line of the choice itself, or (Ren'Py 7 and others) the
/// line of the `menu:` statement, the same for all its choices; `choice` is the original text of the choice, which then tells which one it is.
pub fn find_menu(lines: &[String], line: usize, choice: Option<&str>) -> Option<Menu> {
    if !(1..=lines.len()).contains(&line) {
        return None;
    }
    let mut head: Option<usize> = None;
    if p().menu.is_match(&lines[line - 1]) {
        head = Some(line - 1);
    } else if p().option.is_match(&lines[line - 1]) {
        let own = indent_of(&lines[line - 1]);
        let mut i = line as isize - 2;
        let stop = (-1isize).max(line as isize - 2 - 400);
        while i > stop {
            let text = &lines[i as usize];
            if !blank(text) && indent_of(text) < own {
                // the first line with less indent: the `menu:` line, or nothing
                head = if p().menu.is_match(text) { Some(i as usize) } else { None };
                break;
            }
            i -= 1;
        }
    }
    let head = head?;
    let level = lines[head + 1..].iter().find(|x| !blank(x)).map(|x| indent_of(x)).unwrap_or(indent_of(&lines[head]) + 1);
    if level <= indent_of(&lines[head]) {
        return None;
    }
    let mut question: Vec<usize> = Vec::new();
    let mut options: Vec<MenuOption> = Vec::new();
    // the `after` list holds a 0 for "a menu inside this choice began": the lines after it are not this choice's
    let mut after_raw: Vec<Vec<usize>> = Vec::new();
    for (i, text) in lines.iter().enumerate().skip(head + 1) {
        if blank(text) {
            continue;
        }
        let ind = indent_of(text);
        if ind < level {
            break;
        }
        if ind == level {
            if let Some(m) = p().option.captures(text) {
                let t = m.get(1).or_else(|| m.get(2)).map(|g| g.as_str()).unwrap_or("");
                options.push(MenuOption {
                    n: i + 1,
                    text: t.into(),
                    cond: m.get(3).map(|g| g.as_str()).unwrap_or("").into(),
                    after: Vec::new(),
                    body: Vec::new(),
                });
                after_raw.push(Vec::new());
            } else if options.is_empty() && is_say(text) {
                question.push(i + 1);
            }
        } else if let Some(last) = options.last_mut() {
            if last.body.len() < BODY_LINES {
                last.body.push(text.trim_end_matches(crate::util::is_space).to_string());
            }
        }
        if ind > level && !options.is_empty() {
            let after = after_raw.last_mut().unwrap();
            if after.len() < AFTER_LINES {
                if p().menu.is_match(text) {
                    after.push(0);
                } else if is_say(text) && !after.contains(&0) {
                    after.push(i + 1);
                }
            }
        }
    }
    for (o, a) in options.iter_mut().zip(after_raw) {
        o.after = a.into_iter().filter(|n| *n != 0).collect();
    }
    let before = if question.is_empty() { said_before(lines, head) } else { Vec::new() };
    let mut current = options.iter().position(|o| o.n == line).map(|k| k as i64).unwrap_or(-1);
    if current < 0 {
        if let Some(c) = choice {
            let want = crate::util::py_split_ws(c).join(" ");
            current = options.iter().position(|o| crate::util::py_split_ws(&o.text).join(" ") == want).map(|k| k as i64).unwrap_or(-1);
        }
    }
    Some(Menu { menu_line: head + 1, question, before, options, current })
}
