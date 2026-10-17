//! Port of `core/lexo/gameimages.py`: the pictures of the game, found by their Ren'Py image name, and the picture a line is shown on.
//!
//! Ren'Py games keep their images in `.rpa` archives or loose in `game/images`. A script line names an image (`scene ep1_cafe_maya1`), which is either a file called like that,
//! or an `image ep1_cafe_maya1:` statement whose first string is another image name or a path (animations list their frames). This module indexes the archives (read-only),
//! parses those statements, and answers "which picture is behind this script line": the last `scene` / `show` of an image that can be resolved before the line.
//!
//! It is an approximation: it follows the text of the script upwards (no jumps, no conditions, no layered images). The caches of the Python object (recent pictures and
//! scripts) are an optimisation of the application and are not part of this port yet.
use crate::rpa::{self, Entry as RpaEntry};
use crate::util::{py_split_ws, py_splitlines, py_strip};
use fancy_regex::Regex as FancyRegex;
use indexmap::IndexMap;
use regex::Regex;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const FONT_EXT: [&str; 4] = [".ttf", ".otf", ".woff", ".woff2"];
pub const IMG_EXT: [&str; 6] = [".jpg", ".jpeg", ".png", ".webp", ".avif", ".gif"];
const SKIP_DIRS: [&str; 4] = ["cache", "tl", "__pycache__", ".git"];
const SKIP_FIRST: [&str; 6] = ["screen ", "text ", "expression ", "layer ", "\"", "'"];
/// Lines upwards that are searched for the last scene / show.
const LOOK_UP: isize = 1500;

struct Patterns {
    character: FancyRegex,
    color: Regex,
    layered: Regex,
    composed: Regex,
    image_def: Regex,
    string: Regex,
    candidate: Regex,
    stmt: Regex,
    hide: Regex,
    at: Regex,
    cut: Regex,
    ws: Regex,
}

fn p() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        character: FancyRegex::new(r#"^\s*define\s+(\w+)\s*=\s*(Dynamic)?Character\(\s*(?:(?<q>["'])(?<name>.*?)\k<q>|(?<none>None))(?<rest>.*)$"#).unwrap(),
        color: Regex::new(r#"\b(?:who_color|color)\s*=\s*["'](#[0-9a-fA-F]{3,8})["']"#).unwrap(),
        layered: Regex::new(r"^\s*layeredimage\s+([A-Za-z0-9_.\- ]+?)\s*:").unwrap(),
        composed: Regex::new(r"\b(?:LiveComposite|Composite|im\.Composite|ConditionSwitch|DynamicImage|Fixed|contains)\b").unwrap(),
        image_def: Regex::new(r"^image\s+([A-Za-z0-9_ .\-]+?)\s*(=|:)\s*(.*)$").unwrap(),
        string: Regex::new(r#""([^"\\]*)"|'([^'\\]*)'"#).unwrap(),
        candidate: Regex::new(r"^[\w ./\-]+$").unwrap(),
        stmt: Regex::new(r"^\s*(scene|show)\s+(.+?)\s*:?\s*$").unwrap(),
        hide: Regex::new(r"(?i)^\s*hide\s+(\S+)").unwrap(),
        at: Regex::new(r"\sat\s+(.+?)(?:\s+(?:with|as|zorder|onlayer|behind)\b.*)?\s*:?\s*$").unwrap(),
        cut: Regex::new(r"\s+(?:with|at|as|zorder|onlayer|behind)\b.*$").unwrap(),
        ws: Regex::new(r"\s+").unwrap(),
    })
}

fn x_named(name: &str) -> Option<f64> {
    Some(match name {
        "left" | "topleft" => 0.0,
        "right" | "topright" => 1.0,
        "center" | "truecenter" | "top" => 0.5,
        _ => return None,
    })
}

/// Where a shown picture stands, from its `at` clause (0 = left edge, 0.5 = centre, 1 = right edge). The game's own transforms are unknown: a name with left / right in it
/// is taken as such, anything else as the centre.
fn x_of(statement: &str) -> f64 {
    let hay = format!(" {statement}");
    let Some(m) = p().at.captures(&hay) else { return 0.5 };
    let first = m[1].split(',').next().unwrap_or("");
    let name = py_strip(first).to_lowercase();
    if let Some(x) = x_named(&name) {
        return x;
    }
    if name.contains("left") {
        0.2
    } else if name.contains("right") {
        0.8
    } else {
        0.5
    }
}

fn clean(name: &str) -> String {
    let t = py_strip(name).trim_matches(|c| c == '"' || c == '\'');
    p().ws.replace_all(t, " ").to_lowercase()
}

/// The file name of a path without its last extension (`Path.stem`).
fn stem_of(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or("");
    match name.rfind('.') {
        Some(i) if i > 0 => name[..i].to_string(),
        _ => name.to_string(),
    }
}

fn ends_with_any(s: &str, exts: &[&str]) -> bool {
    exts.iter().any(|e| s.ends_with(e))
}

/// Where the bytes of a picture or a font are.
#[derive(Clone, Debug, PartialEq)]
pub enum Ref {
    File { path: PathBuf, rel: String },
    Rpa { archive: PathBuf, entry: RpaEntry, name: String },
}

impl Ref {
    pub fn kind(&self) -> &'static str {
        match self {
            Ref::File { .. } => "file",
            Ref::Rpa { .. } => "rpa",
        }
    }

    pub fn rel(&self) -> &str {
        match self {
            Ref::File { rel, .. } => rel,
            Ref::Rpa { name, .. } => name,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Character {
    pub name: String,
    pub color: String,
    pub dynamic: bool,
}

#[derive(Default)]
pub struct GameImages {
    pub dir: PathBuf,
    /// lower-case file name without extension -> ref
    pub stems: IndexMap<String, Ref>,
    /// lower-case relative path -> ref
    pub paths: IndexMap<String, Ref>,
    /// names of layered / composed images (not drawn yet)
    pub layered: BTreeSet<String>,
    pub composed: BTreeSet<String>,
    /// lower-case image name -> candidate strings
    pub defs: IndexMap<String, Vec<String>>,
    /// lower-case file name -> ref
    pub fonts: IndexMap<String, Ref>,
    /// speaker code -> character
    pub characters: IndexMap<String, Character>,
    sorted_stems: Vec<String>,
    pub errors: Vec<String>,
}

impl GameImages {
    pub fn new(game_dir: &Path) -> GameImages {
        GameImages { dir: game_dir.to_path_buf(), ..GameImages::default() }
    }

    // ------------------------------------------------------------ index
    pub fn build(&mut self) {
        let (mut archives, mut scripts): (Vec<PathBuf>, Vec<PathBuf>) = (Vec::new(), Vec::new());
        let mut images: Vec<(String, PathBuf)> = Vec::new();
        let mut fonts: Vec<(String, String, PathBuf)> = Vec::new();
        self.walk(&self.dir.clone(), &mut archives, &mut scripts, &mut images, &mut fonts);
        for (rel, full) in images {
            self.add(&rel, Ref::File { path: full, rel: rel.clone() });
        }
        for (low, rel, full) in fonts {
            self.fonts.entry(low).or_insert(Ref::File { path: full, rel });
        }
        for arc in &archives {
            match rpa::read_index(arc) {
                Err(e) => self.errors.push(format!("{}: {e}", arc.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())),
                Ok((_, idx)) => {
                    for (name, entry) in idx {
                        let low = name.to_lowercase();
                        if ends_with_any(&low, &IMG_EXT) {
                            self.add(&name, Ref::Rpa { archive: arc.clone(), entry, name: name.clone() });
                        } else if ends_with_any(&low, &FONT_EXT) {
                            let file = name.rsplit('/').next().unwrap_or("").to_lowercase();
                            self.fonts.entry(file).or_insert(Ref::Rpa { archive: arc.clone(), entry, name: name.clone() });
                        }
                    }
                }
            }
        }
        self.parse_defs(&scripts);
        self.propagate_layers();
        let mut keys: Vec<String> = self.stems.keys().cloned().collect();
        keys.sort();
        self.sorted_stems = keys;
    }

    #[allow(clippy::type_complexity)]
    fn walk(
        &self,
        dir: &Path,
        archives: &mut Vec<PathBuf>,
        scripts: &mut Vec<PathBuf>,
        images: &mut Vec<(String, PathBuf)>,
        fonts: &mut Vec<(String, String, PathBuf)>,
    ) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let (mut dirs, mut files): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
        for e in rd.filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.path().is_dir() {
                if !SKIP_DIRS.contains(&name.to_lowercase().as_str()) {
                    dirs.push(name);
                }
            } else {
                files.push(name);
            }
        }
        dirs.sort();
        files.sort();
        for fn_ in files {
            let (low, full) = (fn_.to_lowercase(), dir.join(&fn_));
            let rel = full.strip_prefix(&self.dir).unwrap_or(&full).to_string_lossy().replace('\\', "/");
            if low.ends_with(".rpa") {
                archives.push(full);
            } else if low.ends_with(".rpy") {
                scripts.push(full);
            } else if ends_with_any(&low, &IMG_EXT) {
                images.push((rel, full));
            } else if ends_with_any(&low, &FONT_EXT) {
                fonts.push((low, rel, full));
            }
        }
        for d in dirs {
            self.walk(&dir.join(d), archives, scripts, images, fonts);
        }
    }

    fn add(&mut self, rel: &str, r: Ref) {
        let low = rel.to_lowercase();
        self.paths.entry(low.clone()).or_insert_with(|| r.clone());
        self.stems.entry(stem_of(&low)).or_insert(r);
    }

    fn parse_defs(&mut self, scripts: &[PathBuf]) {
        for f in scripts {
            let Ok(bytes) = std::fs::read(f) else { continue };
            let lines = py_splitlines(&String::from_utf8_lossy(&bytes));
            let mut i = 0;
            while i < lines.len() {
                let line = &lines[i];
                if line.starts_with(['d', ' ', '\t']) {
                    if let Ok(Some(cm)) = p().character.captures(line) {
                        let code = cm[1].to_string();
                        if !self.characters.contains_key(&code) {
                            let rest = cm.name("rest").map_or("", |m| m.as_str());
                            let color = p().color.captures(rest).map(|c| c[1].to_string()).unwrap_or_default();
                            let dynamic = cm.get(2).is_some();
                            let nm = cm.name("name").map_or("", |m| m.as_str());
                            self.characters.insert(code, Character { name: if dynamic { format!("[{nm}]") } else { nm.to_string() }, color, dynamic });
                        }
                    }
                }
                if let Some(lm) = p().layered.captures(line) {
                    self.layered.insert(clean(&lm[1]));
                }
                let m = p().image_def.captures(line);
                i += 1;
                let Some(m) = m else { continue };
                let (name, sep, rest) = (clean(&m[1]), m[2].to_string(), m[3].to_string());
                let mut texts = vec![rest.clone()];
                if sep == ":" || py_strip(&rest).is_empty() {
                    let mut j = i;
                    while j < lines.len() && j - i < 15 && (py_strip(&lines[j]).is_empty() || lines[j].starts_with([' ', '\t'])) {
                        texts.push(lines[j].clone());
                        j += 1;
                    }
                }
                if texts.iter().any(|t| p().composed.is_match(t)) {
                    self.composed.insert(name); // several parts: never take one of them for the whole picture
                    continue;
                }
                let mut cands: Vec<String> = Vec::new();
                for t in &texts {
                    for g in p().string.captures_iter(t) {
                        let c = py_strip(g.get(1).or_else(|| g.get(2)).map_or("", |m| m.as_str())).to_string();
                        if !c.is_empty() && p().candidate.is_match(&c) && !["dissolve", "fade", "none"].contains(&c.to_lowercase().as_str()) {
                            cands.push(c);
                        }
                    }
                }
                if !cands.is_empty() && !self.defs.contains_key(&name) {
                    cands.truncate(6);
                    self.defs.insert(name, cands);
                }
            }
        }
    }

    /// An image made of other images is layered when one of them is: `image kwhblink:` (a blink animation of `kwh`) is layered when `kwh` is a Composite. Repeated until
    /// nothing changes (a few levels at most).
    fn propagate_layers(&mut self) {
        for _ in 0..5 {
            let fresh: Vec<String> = self
                .defs
                .iter()
                .filter(|(name, cands)| {
                    !self.composed.contains(*name)
                        && cands.iter().any(|c| !c.contains('/') && !ends_with_any(&c.to_lowercase(), &IMG_EXT) && self.is_layered(c, &py_split_ws(c)))
                })
                .map(|(n, _)| n.clone())
                .collect();
            if fresh.is_empty() {
                return;
            }
            for n in fresh {
                self.defs.shift_remove(&n); // its parts are never the picture of the whole
                self.composed.insert(n);
            }
        }
    }

    // ------------------------------------------------------------ lookup
    fn stem_ref(&self, key: &str) -> Option<(String, Ref)> {
        self.stems.get(key).map(|r| (key.to_string(), r.clone()))
    }

    /// (stem key, ref) of the picture an image name stands for, or `None`.
    pub fn resolve(&self, name: &str) -> Option<(String, Ref)> {
        self.resolve_depth(name, 0)
    }

    fn resolve_depth(&self, name: &str, depth: usize) -> Option<(String, Ref)> {
        let n = clean(name);
        if n.is_empty() || depth > 4 {
            return None;
        }
        for cand in [n.clone(), n.replace(' ', "_")] {
            if let Some(hit) = self.stem_ref(&cand) {
                return Some(hit);
            }
        }
        let mut cands: Vec<&String> = self.defs.get(&n).map(|v| v.iter().collect()).unwrap_or_default();
        cands.extend(self.defs.get(&n.replace(' ', "_")).map(|v| v.iter().collect::<Vec<_>>()).unwrap_or_default());
        for cand in cands {
            let low = cand.to_lowercase();
            let hit = if low.contains('/') || ends_with_any(&low, &IMG_EXT) {
                self.stem_ref(&stem_of(&low)).or_else(|| self.paths.get(&low).map(|r| (low.clone(), r.clone())))
            } else {
                self.resolve_depth(cand, depth + 1)
            };
            if hit.is_some() {
                return hit;
            }
        }
        let base = n.replace(' ', "_");
        let i = self.sorted_stems.partition_point(|s| s.as_str() < base.as_str());
        let tail = Regex::new(&format!(r"\A{}[_\-]?(blink\d*|\d+|[a-z])?\d*\z", regex::escape(&base))).ok()?;
        let near: Vec<&String> = self.sorted_stems[i..]
            .iter()
            .take(40)
            .filter(|s| s.starts_with(&format!("{base}_")) || s.starts_with(&base))
            .filter(|s| tail.is_match(s))
            .collect();
        near.into_iter().min_by_key(|s| (s.chars().count(), (*s).clone())).and_then(|s| self.stem_ref(s))
    }

    /// (bytes, mime) for a stem key returned by `resolve`.
    pub fn image_bytes(&self, key: &str) -> Result<(Vec<u8>, &'static str), rpa::RpaError> {
        let r = self.stems.get(&clean(key)).ok_or_else(|| rpa::RpaError(crate::messages::msg("the picture does not exist", &[])))?;
        let data = match r {
            Ref::Rpa { archive, entry, .. } => rpa::read_entry(archive, entry, rpa::DEFAULT_ENTRY_LIMIT)?,
            Ref::File { path, .. } => std::fs::read(path)
                .map_err(|e| rpa::RpaError(crate::messages::msg("the archive cannot be read: {reason}", &[("reason", json!(e.to_string()))])))?,
        };
        let ext = format!(".{}", r.rel().rsplit('.').next().unwrap_or("").to_lowercase());
        let mime = sniff(&data).unwrap_or(match ext.as_str() {
            ".png" => "image/png",
            ".webp" => "image/webp",
            ".avif" => "image/avif",
            ".gif" => "image/gif",
            _ => "image/jpeg",
        });
        Ok((data, mime))
    }

    /// (bytes, mime) of a font by its file name (only names the index knows).
    pub fn font_bytes(&self, name: &str) -> Result<(Vec<u8>, &'static str), rpa::RpaError> {
        let key = name.rsplit(['/', '\\']).next().unwrap_or("").to_lowercase();
        let missing = || rpa::RpaError(crate::messages::msg("the font does not exist", &[]));
        let r = self.fonts.get(&key).filter(|_| key == name.to_lowercase()).ok_or_else(missing)?;
        let data = match r {
            Ref::Rpa { archive, entry, .. } => rpa::read_entry(archive, entry, 16 * 1024 * 1024)?,
            Ref::File { path, .. } => std::fs::read(path).map_err(|_| missing())?,
        };
        let mime = match key.rsplit('.').next().unwrap_or("") {
            "otf" => "font/otf",
            "woff" => "font/woff",
            "woff2" => "font/woff2",
            _ => "font/ttf",
        };
        Ok((data, mime))
    }

    // ------------------------------------------------------------ the picture behind a script line
    /// (line, image name) of the `scene` statements of a script, in order: the boundaries of the scenes.
    pub fn scene_starts(lines: &[String]) -> Vec<(usize, String)> {
        let mut out = Vec::new();
        for (n, text) in lines.iter().enumerate() {
            if let Some(m) = p().stmt.captures(text) {
                if &m[1] == "scene" {
                    let what = py_strip(&p().cut.replace(&m[2], "")).to_string();
                    if !what.is_empty() && !starts_with_skip(&what) {
                        out.push((n + 1, py_split_ws(&what).first().copied().unwrap_or("").to_string()));
                    }
                }
            }
        }
        out
    }

    /// The picture behind `line`: the last `scene` (the background) and the `show` pictures put over it since, minus the `hide`n ones.
    pub fn scene_for(&self, lines: &[String], line: usize) -> Value {
        let mut missing: Vec<String> = Vec::new();
        // after the background, newest first: (kind, tag, layer)
        let mut events: Vec<(&str, String, Option<Value>)> = Vec::new();
        let mut bg: Option<Map<String, Value>> = None;
        let start = (line as isize - 1).min(lines.len() as isize) - 1;
        let stop = (-1isize).max(line as isize - 1 - LOOK_UP);
        let mut n = start;
        while n > stop {
            let text = &lines[n as usize];
            let here = n as usize;
            n -= 1;
            if let Some(h) = p().hide.captures(text) {
                if bg.is_none() {
                    events.push(("hide", h[1].to_lowercase(), None));
                }
                continue;
            }
            let Some(m) = p().stmt.captures(text) else { continue };
            let what = py_strip(&p().cut.replace(&m[2], "")).to_string();
            if what.is_empty() || starts_with_skip(&what) {
                continue;
            }
            let tokens: Vec<&str> = py_split_ws(&what);
            if &m[1] == "show" {
                if bg.is_some() {
                    continue;
                }
                if self.is_layered(&what, &tokens) {
                    // not drawn: it hides an older picture of the same tag, and the preview says so
                    events.push(("show", tokens[0].to_lowercase(), None));
                    continue;
                }
                let mut hit_done = false;
                for cand in [what.as_str(), tokens[0]] {
                    if let Some(hit) = self.resolve(cand) {
                        events.push(("show", tokens[0].to_lowercase(), Some(json!({"image": cand, "key": hit.0, "x": x_of(&m[2]), "script_line": here + 1}))));
                        hit_done = true;
                        break;
                    }
                }
                if !hit_done && missing.len() < 5 {
                    missing.push(what);
                }
                continue;
            }
            if self.is_layered(&what, &tokens) {
                // the whole scene is layered: no honest background exists
                bg = Some(obj(json!({"found": false, "image": what, "key": "", "script_line": here + 1, "layered": true})));
                break;
            }
            let flat = match tokens[0].to_lowercase().as_str() {
                "black" => Some("#000000"),
                "white" => Some("#ffffff"),
                _ => None,
            };
            if let Some(f) = flat {
                bg = Some(obj(json!({"found": true, "image": tokens[0], "key": "", "flat": f, "script_line": here + 1})));
                break;
            }
            for cand in [what.as_str(), tokens[0]] {
                if let Some(hit) = self.resolve(cand) {
                    bg = Some(obj(json!({"found": true, "image": cand, "key": hit.0, "script_line": here + 1})));
                    break;
                }
            }
            if bg.is_some() {
                break;
            }
            if missing.len() < 5 {
                missing.push(what);
            }
            events.clear(); // an unresolved scene still clears the screen: older shows would be wrong
        }
        // the stack of shown pictures, oldest first; a `show` replaces the picture with the same tag in place
        let mut stack: Vec<(String, Option<Value>)> = Vec::new();
        for (kind, tag, layer) in events.into_iter().rev() {
            if kind == "hide" {
                stack.retain(|(t, _)| *t != tag);
            } else if let Some(slot) = stack.iter_mut().find(|(t, _)| *t == tag) {
                slot.1 = layer;
            } else {
                stack.push((tag, layer));
            }
        }
        let layered = stack.iter().any(|(_, v)| v.is_none());
        let layers: Vec<Value> = stack.into_iter().filter_map(|(_, v)| v).collect();
        if let Some(b) = &bg {
            if b.get("layered") == Some(&json!(true)) {
                let mut out = b.clone();
                out.insert("key".into(), json!(""));
                out.insert("missing".into(), json!(missing));
                out.insert("layers".into(), json!([]));
                out.insert("layered".into(), json!(true));
                return Value::Object(out);
            }
        }
        let mut out = match bg {
            Some(b) => b,
            None => {
                if layers.is_empty() {
                    return json!({"found": false, "image": "", "key": "", "script_line": 0, "missing": missing, "layers": [], "layered": layered});
                }
                obj(json!({"found": true, "image": "", "key": "", "script_line": layers[0]["script_line"]}))
            }
        };
        out.insert("missing".into(), json!(missing));
        let stripped: Vec<Value> = layers
            .iter()
            .map(|l| Value::Object(l.as_object().unwrap().iter().filter(|(k, _)| *k != "script_line").map(|(k, v)| (k.clone(), v.clone())).collect()))
            .collect();
        out.insert("layers".into(), Value::Array(stripped));
        out.insert("layered".into(), json!(layered));
        Value::Object(out)
    }

    /// Is the image of a `scene` / `show` statement made of layers (`layeredimage`, a composite, a condition switch)?
    pub fn is_layered(&self, what: &str, _tokens: &[&str]) -> bool {
        let cleaned = clean(what);
        let words: Vec<&str> = cleaned.split(' ').filter(|w| !w.is_empty()).collect();
        // the name of the image, or its first words (the rest are attributes: `show anna smile`, `scene bg alley night`)
        (1..=words.len()).any(|k| {
            let head = words[..k].join(" ");
            self.layered.contains(&head) || self.composed.contains(&head)
        })
    }

    /// The names of the images by kind, for the comparison of two implementations.
    pub fn summary(&self) -> Value {
        fn keys(m: &IndexMap<String, Ref>) -> Vec<&String> {
            let mut v: Vec<&String> = m.keys().collect();
            v.sort();
            v
        }
        let chars: BTreeMap<&String, Value> =
            self.characters.iter().map(|(k, c)| (k, json!({"name": c.name, "color": c.color, "dynamic": c.dynamic}))).collect();
        json!({
            "stems": keys(&self.stems), "paths": keys(&self.paths), "fonts": keys(&self.fonts), "layered": self.layered, "composed": self.composed,
            "defs": self.defs.iter().map(|(k, v)| (k.clone(), json!(v))).collect::<Map<String, Value>>(), "characters": chars, "errors": self.errors,
        })
    }
}

fn obj(v: Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

fn starts_with_skip(what: &str) -> bool {
    let low = what.to_lowercase();
    SKIP_FIRST.iter().any(|s| low.starts_with(s)) || low.starts_with('(')
}

fn sniff(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some("image/webp")
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        Some("image/gif")
    } else {
        None
    }
}
