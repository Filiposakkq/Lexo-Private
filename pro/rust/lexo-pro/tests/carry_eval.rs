//! Measures `carry.over` on a REAL translation: the lines of a translated project are the "old version"; the "new version" is made from them by edits like an author makes them
//! (the translate ids change, some lines are edited a little or more, some are removed, some are new), and because it is known which new line comes from which old one, the
//! answer can be counted. Not run by default (it needs a translated project):
//!   LEXO_EVAL_PROJECT="D:\RenPy Stuff\Lexo-workspace\MidnightParadise-1.1b-pc-elite\game\tl\russian" cargo test --test carry_eval -- --ignored --nocapture
use lexo_core::api;
use lexo_core::app::App;
use lexo_core::config::Config;
use lexo_core::lang::Packs;
use lexo_core::memory::tokens;
use lexo_pro::carry_over::{find, Line};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let name = e.file_name();
        if name == ".aside" || name == ".lexo" {
            continue;
        }
        if e.file_type().unwrap().is_dir() {
            copy_tree(&e.path(), &to.join(&name));
        } else {
            std::fs::copy(e.path(), to.join(&name)).unwrap();
        }
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// The translated lines of a project: (who, source, target).
fn read_project(path: &Path) -> Vec<(String, String, String)> {
    let tmp: PathBuf = std::env::temp_dir().join(format!("carry-eval-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let lang = tmp.join("game/tl").join(path.file_name().unwrap());
    copy_tree(path, &lang);
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut config = Config::new(Some(&tmp.join("cfg/config.json")));
    config.load();
    std::env::set_var("LEXO_NO_PRO", "1");
    let mut app = App::new(config, Packs { bundled: repo.join("core/lexo/packs"), user: None });
    app.open_project(&lang.to_string_lossy(), false).expect("the project opens");
    let files = api::call(&mut app, "files", &json!({})).unwrap();
    let files: Value = files.json().unwrap().clone();
    let mut out = Vec::new();
    for f in files["files"].as_array().unwrap() {
        let name = f["name"].as_str().unwrap().to_string();
        let mut offset = 0;
        loop {
            let page = api::call(&mut app, "entries", &json!({"file": name, "status": "all", "limit": 2000, "offset": offset})).unwrap();
            let page: Value = page.json().unwrap().clone();
            let items = page["items"].as_array().unwrap();
            for it in items {
                let s = |k: &str| it[k].as_str().unwrap_or("").to_string();
                if it["unit"] == "dialogue" || it["unit"] == "string" {
                    out.push((s("who"), s("source"), s("target")));
                }
            }
            if items.len() < 2000 {
                break;
            }
            offset += 2000;
        }
    }
    app.close();
    drop(app);
    let _ = std::fs::remove_dir_all(&tmp);
    out
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Truth {
    Same,
    Tiny,
    Medium,
    Removed,
    Inserted,
}

fn edit_tiny(src: &str, rng: &mut Rng) -> String {
    let words: Vec<&str> = src.split(' ').collect();
    match rng.below(3) {
        0 => {
            // the last character: a full stop becomes an exclamation mark or the other way round
            let t = src.trim_end();
            match t.chars().last() {
                Some('.') => format!("{}!", &t[..t.len() - 1]),
                Some('!') => format!("{}.", &t[..t.len() - 1]),
                _ => format!("{t}."),
            }
        }
        1 if words.len() >= 3 => {
            // a typo fixed: one letter of a long word changes
            let i = (0..words.len()).filter(|&i| words[i].chars().count() >= 5).collect::<Vec<_>>();
            if i.is_empty() {
                return src.to_string();
            }
            let k = i[rng.below(i.len())];
            let mut w: Vec<String> = words.iter().map(|s| s.to_string()).collect();
            let cs: Vec<char> = w[k].chars().collect();
            let p = 1 + rng.below(cs.len() - 2);
            w[k] = cs
                .iter()
                .enumerate()
                .map(|(n, &c)| {
                    if n == p {
                        if c == 'e' {
                            'a'
                        } else {
                            'e'
                        }
                    } else {
                        c
                    }
                })
                .collect();
            w.join(" ")
        }
        _ if words.len() >= 3 => {
            let mut w: Vec<&str> = words.clone();
            let p = rng.below(w.len() - 1);
            w.swap(p, p + 1);
            w.join(" ")
        }
        _ => format!("{} really", src),
    }
}

fn edit_medium(src: &str, rng: &mut Rng) -> String {
    let mut w: Vec<String> = src.split(' ').map(String::from).collect();
    for _ in 0..(1 + rng.below(2)) {
        let p = rng.below(w.len());
        match rng.below(3) {
            0 => w[p] = "different".to_string(),
            1 if w.len() > 3 => {
                w.remove(p);
            }
            _ => w.insert(p, "actually".to_string()),
        }
    }
    w.join(" ")
}

fn edit_new(src: &str, rng: &mut Rng) -> String {
    // a line that looks like the others but is not the same: the words are shuffled and half of them are dropped
    let mut w: Vec<&str> = src.split(' ').collect();
    for i in (1..w.len()).rev() {
        w.swap(i, rng.below(i + 1));
    }
    let keep = (w.len() / 2).max(1);
    w.truncate(keep);
    format!("{} then", w.join(" "))
}

#[test]
#[ignore]
fn carry_over_on_a_real_translation() {
    let path = std::env::var("LEXO_EVAL_PROJECT").expect("set LEXO_EVAL_PROJECT to a translated tl folder");
    let rows = read_project(Path::new(&path));
    let translated: Vec<&(String, String, String)> = rows.iter().filter(|(_, s, t)| !t.trim().is_empty() && !tokens(s).is_empty()).collect();
    println!("{}: {} lines, {} translated", path, rows.len(), translated.len());
    assert!(translated.len() > 200, "too few translated lines to say anything");

    let old: Vec<Line> =
        translated.iter().enumerate().map(|(i, (w, s, t))| Line { id: format!("o{i}"), who: w.clone(), source: s.clone(), target: t.clone() }).collect();
    let by_id: HashMap<&str, &Line> = old.iter().map(|l| (l.id.as_str(), l)).collect();
    let mut rng = Rng(0x9E3779B97F4A7C15);
    let mut new: Vec<Line> = Vec::new();
    let mut truth: HashMap<String, (Truth, Option<usize>)> = HashMap::new();
    for (i, l) in old.iter().enumerate() {
        let r = rng.below(100);
        let words = l.source.split(' ').count();
        let (t, source) = match r {
            0..=61 => (Truth::Same, l.source.clone()),
            62..=71 => (Truth::Tiny, edit_tiny(&l.source, &mut rng)),
            72..=79 if words >= 4 => (Truth::Medium, edit_medium(&l.source, &mut rng)),
            72..=79 => (Truth::Tiny, edit_tiny(&l.source, &mut rng)),
            80..=87 => (Truth::Removed, String::new()),
            _ => (Truth::Inserted, edit_new(&l.source, &mut rng)),
        };
        if t == Truth::Removed {
            continue;
        }
        let id = format!("n{}", new.len());
        truth.insert(id.clone(), (t, if t == Truth::Inserted { None } else { Some(i) }));
        new.push(Line { id, who: l.who.clone(), source, target: String::new() });
    }
    // the new version is in another order than the old one
    for i in (1..new.len()).rev() {
        let j = rng.below(i + 1);
        new.swap(i, j);
    }
    let count = |t: Truth| truth.values().filter(|(x, _)| *x == t).count();
    println!(
        "new version: {} lines (same {}, tiny edit {}, bigger edit {}, new {}); {} lines were removed",
        new.len(),
        count(Truth::Same),
        count(Truth::Tiny),
        count(Truth::Medium),
        count(Truth::Inserted),
        old.len() + count(Truth::Inserted) - new.len()
    );

    for min_ratio in [0.75, 0.85, 0.92] {
        let started = std::time::Instant::now();
        let found = find(&old, &new, min_ratio);
        let by_new: HashMap<&str, &lexo_pro::carry_over::Match> = found.iter().map(|m| (m.new.as_str(), m)).collect();
        let correct = |m: &lexo_pro::carry_over::Match, from: usize| {
            let truth_line = &old[from];
            m.target == truth_line.target || by_id[m.old.as_str()].source.to_lowercase() == truth_line.source.to_lowercase()
        };
        println!("\nmin ratio {min_ratio}: {} answers in {:?}", found.len(), started.elapsed());
        for t in [Truth::Same, Truth::Tiny, Truth::Medium] {
            let total = count(t);
            let (mut got, mut right, mut exact) = (0, 0, 0);
            for (id, (x, from)) in &truth {
                if *x != t {
                    continue;
                }
                if let Some(m) = by_new.get(id.as_str()) {
                    got += 1;
                    exact += (m.kind == "exact") as usize;
                    right += correct(m, from.unwrap()) as usize;
                }
            }
            println!(
                "  {:?}: found {got} of {total} ({:.1} %), of them exact {exact}, with the right translation {right} ({:.1} % of the found)",
                t,
                100.0 * got as f64 / total as f64,
                100.0 * right as f64 / got.max(1) as f64
            );
        }
        let false_hits = truth.iter().filter(|(id, (x, _))| *x == Truth::Inserted && by_new.contains_key(id.as_str())).count();
        println!("  Inserted: wrongly matched {false_hits} of {} ({:.1} %)", count(Truth::Inserted), 100.0 * false_hits as f64 / count(Truth::Inserted) as f64);
    }
}
