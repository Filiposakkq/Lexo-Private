//! Port of `core/lexo/app_game.py`: read-only facts about the game folder: gui, fonts, images, characters, scene preview data.
use crate::api::{ok, Answer, Args};
use crate::app::{line_gone, unknown_file, App};
use crate::app_base::{app_err, Res};
use crate::fsutil::resolve;
use crate::gameimages::GameImages;
use crate::gametools::{below, has_suffix, rel_text, rglob_suffix, sort_paths};
use crate::messages::{msg, Text};
use crate::util::{md5_hex, url_quote};
use regex::Regex;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::OnceLock;

const GUI_DEFAULTS: [(&str, i64); 10] = [
    ("text_size", 33),
    ("dialogue_width", 1116),
    ("textbox_height", 278),
    ("dialogue_ypos", 125),
    ("dialogue_xpos", 402),
    ("name_xpos", 360),
    ("name_ypos", 50),
    ("name_text_size", 56),
    ("screen_width", 1920),
    ("screen_height", 1080),
];

fn re(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).unwrap())
}

/// `game/script.rpy:123` -> (`game/script.rpy`, 123).
pub fn split_ref(reference: &str) -> Option<(String, usize)> {
    static R: OnceLock<Regex> = OnceLock::new();
    let c = re(&R, r"^(game/.+?):(\d+)$").captures(reference)?;
    Some((c[1].to_string(), c[2].parse().unwrap_or(0)))
}

fn read_lossy(path: &Path) -> std::io::Result<String> {
    Ok(String::from_utf8_lossy(&std::fs::read(path)?).into_owned())
}

/// The lines of a script the way `GameImages.script_lines` reads them.
pub fn script_lines(path: &Path) -> std::io::Result<Vec<String>> {
    Ok(crate::util::py_splitlines(&read_lossy(path)?))
}

impl App {
    /// The game's own `game` folder: the nearest folder called `game` above the translation folder (`None` when there is none).
    pub fn game_dir(&self) -> Option<PathBuf> {
        let root = &self.open.as_ref()?.project.root;
        root.ancestors().skip(1).find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase() == "game")).map(Path::to_path_buf)
    }

    /// Folder that contains `game/` (references look like `game/script.rpy:123`), found above the translation folder.
    pub fn game_root(&self) -> Option<PathBuf> {
        self.game_dir().and_then(|g| g.parent().map(Path::to_path_buf))
    }

    /// The text of the game's gui definitions: `game/gui.rpy`, else a file called `gui.rpy` elsewhere in the game, else the first script that has `define gui.` lines.
    fn gui_text(&self) -> String {
        let Some(game) = self.game_dir() else { return String::new() };
        let skip = ["tl", "cache", "saves", ".lexo", "_progress_data"];
        let mut scripts = rglob_suffix(&game, &[".rpy"]);
        scripts.retain(|p| {
            let rel = p.strip_prefix(&game).unwrap_or(p);
            let dirs: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
            !dirs[..dirs.len().saturating_sub(1)].iter().any(|d| skip.contains(&d.as_str()))
        });
        sort_paths(&mut scripts);
        let depth = |p: &PathBuf| p.strip_prefix(&game).map_or(0, |r| r.components().count());
        let mut named: Vec<&PathBuf> = scripts.iter().filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase() == "gui.rpy")).collect();
        named.sort_by_key(|p| depth(p)); // stable, like Python's sorted
        if let Some(path) = named.first() {
            return read_lossy(path).unwrap_or_default();
        }
        static R: OnceLock<Regex> = OnceLock::new();
        let marker = re(&R, r"(?m)^\s*define\s+gui\.(?:text_size|textbox_height|name_text_size)\b");
        for path in &scripts {
            if std::fs::metadata(path).map_or(true, |m| m.len() >= 2_000_000) {
                continue;
            }
            if let Ok(text) = read_lossy(path) {
                if marker.is_match(&text) {
                    return text;
                }
            }
        }
        String::new()
    }

    /// A short id of the open game folder. It is part of the URL of every picture and font, so the browser cannot show the picture of another game that has the same key.
    pub fn game_token(&self) -> String {
        match self.game_root() {
            Some(base) => md5_hex(base.to_string_lossy().as_bytes())[..8].to_string(),
            None => String::new(),
        }
    }

    /// Dialogue box metrics of the game (screen size, text size and width, box height, name position), for the scene preview. Falls back to the values of Being a DIK when the game
    /// sources are not next to the translation.
    pub fn game_gui(&self) -> Value {
        let mut out: Map<String, Value> = GUI_DEFAULTS.iter().map(|(k, v)| (k.to_string(), json!(v))).collect();
        out.insert("source".into(), msg("default values", &[]).wire());
        if self.open.is_none() || self.game_root().is_none() {
            return Value::Object(out);
        }
        let text = self.gui_text();
        if text.is_empty() {
            return Value::Object(out);
        }
        static D: OnceLock<Regex> = OnceLock::new();
        static I: OnceLock<Regex> = OnceLock::new();
        let (mut found, mut fractions): (Vec<(String, i64)>, Vec<(String, f64)>) = (Vec::new(), Vec::new());
        // numbers may be negative (name_ypos = -66) and a number with a decimal point is a FRACTION of the container in Ren'Py (dialogue_ypos = 0.33)
        for m in re(&D, r"(?m)^\s*define\s+gui\.(\w+)\s*=\s*(-?\d+(?:\.\d+)?)\b").captures_iter(&text) {
            let name = m[1].to_string();
            if found.iter().any(|(k, _)| *k == name) {
                continue;
            }
            if m[2].contains('.') {
                fractions.push((name.clone(), m[2].parse().unwrap_or(0.0)));
                found.push((name, 0));
            } else {
                found.push((name, m[2].parse().unwrap_or(0)));
            }
        }
        if let Some(m) = re(&I, r"gui\.init\(\s*(\d+)\s*,\s*(\d+)").captures(&text) {
            for (k, v) in [("screen_width", &m[1]), ("screen_height", &m[2])] {
                let n: i64 = v.parse().unwrap_or(0);
                match found.iter_mut().find(|(f, _)| f == k) {
                    Some(e) => e.1 = n,
                    None => found.push((k.to_string(), n)),
                }
            }
        }
        for (k, _) in GUI_DEFAULTS {
            if let Some((_, v)) = found.iter().find(|(f, _)| f == k) {
                out.insert(k.to_string(), json!(v));
            }
        }
        let get = |out: &Map<String, Value>, k: &str| out[k].as_i64().unwrap_or(0);
        let (width, height) = (get(&out, "screen_width"), get(&out, "screen_height"));
        let fraction = |k: &str| fractions.iter().find(|(f, _)| f == k).map(|(_, v)| *v);
        let tb_height = match fraction("textbox_height") {
            None => get(&out, "textbox_height"),
            Some(fr) => crate::pyfmt::round_half_even(fr * height as f64) as i64,
        };
        out.insert("textbox_height".into(), json!(tb_height));
        for (k, whole) in [("name_ypos", tb_height), ("dialogue_ypos", tb_height), ("name_xpos", width), ("dialogue_xpos", width), ("dialogue_width", width)] {
            if let Some(fr) = fraction(k) {
                out.insert(k.into(), json!(crate::pyfmt::round_half_even(fr * whole as f64) as i64));
            }
        }
        let any = GUI_DEFAULTS.iter().any(|(k, _)| found.iter().any(|(f, _)| f == k));
        out.insert("source".into(), if any { msg("gui.rpy of the game", &[]).wire() } else { msg("default values", &[]).wire() });
        Value::Object(out)
    }

    /// Font files for the dialogue text and the speaker name: gui.rpy, overridden by the translation folder (`translate <lang> python: gui.text_font = ...`,
    /// `translate <lang> style say_dialogue: font ...`), which is how Czech diacritics are usually added.
    pub fn game_fonts(&self) -> (String, String, Value) {
        let (mut text, mut name) = (String::new(), String::new());
        if self.game_root().is_none() {
            return (text, name, json!(""));
        }
        static G: OnceLock<Regex> = OnceLock::new();
        static P: OnceLock<Regex> = OnceLock::new();
        static S: OnceLock<Regex> = OnceLock::new();
        let gui = self.gui_text();
        for m in re(&G, r#"(?m)^\s*define\s+gui\.(text_font|name_text_font)\s*=\s*["']([^"']+)["']"#).captures_iter(&gui) {
            if &m[1] == "text_font" {
                text = m[2].to_string();
            } else {
                name = m[2].to_string();
            }
        }
        let mut found = !text.is_empty() || !name.is_empty();
        let mut styles: Vec<(String, String)> = Vec::new();
        let root = match self.open.as_ref() {
            Some(o) => o.project.root.clone(),
            None => return (text, name, json!("")),
        };
        let mut files = rglob_suffix(&root, &[".rpy"]);
        sort_paths(&mut files);
        for f in files {
            let Ok(txt) = read_lossy(&f) else { continue };
            if !txt.contains("font") {
                continue;
            }
            for m in re(&P, r#"(?m)^\s*gui\.(text_font|name_text_font)\s*=\s*["']([^"']+)["']"#).captures_iter(&txt) {
                if &m[1] == "text_font" {
                    text = m[2].to_string();
                } else {
                    name = m[2].to_string();
                }
                found = true;
            }
            for m in re(&S, r#"(?m)^translate\s+\w+\s+style\s+(\w+)\s*:\s*\n(?:[ \t]+[^\n]*\n)*?[ \t]+font\s+["']([^"']+)["']"#).captures_iter(&txt) {
                if !styles.iter().any(|(k, _)| *k == m[1]) {
                    styles.push((m[1].to_string(), m[2].to_string()));
                }
            }
        }
        let style = |k: &str| styles.iter().find(|(f, _)| f == k).map(|(_, v)| v.clone()).filter(|v| !v.is_empty());
        if let Some(v) = style("say_dialogue") {
            text = v;
        } else if let Some(v) = style("default") {
            if text.is_empty() {
                text = v;
            }
        }
        if let Some(v) = style("say_label") {
            name = v;
        }
        let source = if found || !styles.is_empty() { msg("gui.rpy and the translation", &[]).wire() } else { json!("") };
        (text, name, source)
    }

    /// The index of the game's pictures (archives + loose files + `image` statements); built once per game folder.
    pub fn game_images(&mut self) -> Option<Rc<GameImages>> {
        let base = self.game_root()?;
        if !base.join("game").is_dir() {
            return None;
        }
        let o = self.open.as_mut()?;
        if o.gi.as_ref().is_none_or(|(b, _)| *b != base) {
            let mut gi = GameImages::new(&base.join("game"));
            gi.build();
            o.gi = Some((base, Rc::new(gi)));
        }
        o.gi.as_ref().map(|(_, g)| g.clone())
    }

    /// Which picture is behind a line (the last `scene` / `show` before its place in the game script), plus the textbox metrics needed to draw the line over it.
    pub fn scene_info(&mut self, rel: &str, line: i64) -> Res<Value> {
        self.need_project()?;
        let (who, reference) = {
            let o = self.o()?;
            let ent = if o.project.files.contains_key(rel) { o.project.entries(rel).iter().find(|e| e.line as i64 == line) } else { None };
            let ent = ent.ok_or_else(line_gone)?;
            (ent.who.clone(), ent.reference.clone())
        };
        let gui = self.game_gui();
        let mut out = Map::new();
        out.insert("available".into(), json!(false));
        out.insert("gui".into(), gui);
        out.insert("who".into(), json!(who));
        out.insert("reason".into(), json!(""));
        let Some(gi) = self.game_images() else {
            out.insert("reason".into(), msg("The game folder (game) above the translation was not found, so the scene pictures cannot be shown.", &[]).wire());
            return Ok(Value::Object(out));
        };
        let Some((script_rel, script_line)) = split_ref(&reference) else {
            out.insert("reason".into(), msg("The line has no recorded position in the game script.", &[]).wire());
            return Ok(Value::Object(out));
        };
        let root = self.game_root().unwrap_or_default();
        let script = resolve(&root.join(&script_rel));
        if !below(&script, &resolve(&root)) || !(has_suffix(&script_rel.to_lowercase(), ".rpy") || script_rel.to_lowercase().ends_with(".rpym")) {
            out.insert("reason".into(), msg("Invalid script path.", &[]).wire());
            return Ok(Value::Object(out));
        }
        let Ok(lines) = script_lines(&script) else {
            out.insert("reason".into(), msg("The script {name} was not found (are only .rpyc files at hand?).", &[("name", json!(script_rel))]).wire());
            return Ok(Value::Object(out));
        };
        let sc = gi.scene_for(&lines, script_line);
        let (text_font, name_font, source) = self.game_fonts();
        let _ = source;
        let token = self.game_token();
        for (kind, font) in [("text", &text_font), ("name", &name_font)] {
            let fname = if font.is_empty() {
                String::new()
            } else {
                Path::new(font.as_str()).file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default()
            };
            let value = if gi.fonts.contains_key(&fname) {
                json!({"file": fname, "url": format!("/api/game/font?g={token}&k={}", url_quote(&fname))})
            } else {
                Value::Null
            };
            out.insert(format!("fonts_{kind}"), value);
        }
        out.insert("speaker".into(), gi.characters.get(&who).map_or(Value::Null, |c| json!({"name": c.name, "color": c.color, "dynamic": c.dynamic})));
        let found = sc["found"].as_bool().unwrap_or(false);
        let layered = sc.get("layered").and_then(Value::as_bool).unwrap_or(false);
        out.insert("available".into(), json!(true));
        out.insert("found".into(), json!(found));
        out.insert("layered".into(), json!(layered));
        out.insert("image".into(), sc["image"].clone());
        out.insert("missing".into(), sc["missing"].clone());
        out.insert("script".into(), json!(format!("{script_rel}:{}", sc["script_line"].as_i64().unwrap_or(0))));
        out.insert("flat".into(), sc.get("flat").cloned().unwrap_or(json!("")));
        out.insert("index_errors".into(), json!(gi.errors.iter().take(3).collect::<Vec<_>>()));
        out.insert("images".into(), json!(gi.stems.len()));
        let key = sc["key"].as_str().unwrap_or("");
        if found && !key.is_empty() {
            out.insert("url".into(), json!(format!("/api/game/image?g={token}&k={}", url_quote(key))));
        } else if !found {
            out.insert(
                "reason".into(),
                if layered {
                    msg("The scene is built from layered or composed images, which Lexo does not draw yet.", &[]).wire()
                } else {
                    msg("I found no picture (scene / show) before this line that can be traced in the game archives.", &[]).wire()
                },
            );
        }
        let layers: Vec<Value> = sc["layers"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|x| json!({"image": x["image"], "x": x["x"], "url": format!("/api/game/image?g={token}&k={}", url_quote(x["key"].as_str().unwrap_or("")))}))
            .collect();
        out.insert("layers".into(), Value::Array(layers));
        Ok(Value::Object(out))
    }

    /// `(bytes, mime)` of a font file of the game, by file name (only fonts the index knows).
    pub fn game_font(&mut self, name: &str) -> Res<(Vec<u8>, &'static str)> {
        self.need_project()?;
        let gi = self.game_images().ok_or_else(|| app_err(msg("The game fonts are not available.", &[])))?;
        gi.font_bytes(name).map_err(|e| crate::app_base::ApiError::App(Text::Msg(e.0)))
    }

    pub fn game_characters(&mut self) -> Res<Value> {
        self.need_project()?;
        let chars: Map<String, Value> = match self.game_images() {
            Some(gi) => gi.characters.iter().map(|(k, c)| (k.clone(), json!({"name": c.name, "color": c.color, "dynamic": c.dynamic}))).collect(),
            None => Map::new(),
        };
        Ok(json!({"characters": chars}))
    }

    /// Advance widths (from hmtx) of a game font, so the scene preview can place characters like Ren'Py does.
    pub fn game_font_metrics(&mut self, name: &str) -> Res<Value> {
        let (data, _mime) = self.game_font(name)?;
        let m = crate::fontmetrics::advances(&data).ok_or_else(|| app_err(msg("The character widths cannot be read from the font.", &[])))?;
        let adv: Map<String, Value> = m.adv.iter().map(|(k, v)| (k.to_string(), json!(v))).collect();
        Ok(json!({"upm": m.upm, "adv": adv}))
    }

    /// `(bytes, mime)` of a picture the index knows by its stem key (never an arbitrary path).
    pub fn game_image(&mut self, key: &str) -> Res<(Vec<u8>, &'static str)> {
        self.need_project()?;
        let gi = self.game_images().ok_or_else(|| app_err(msg("The game pictures are not available.", &[])))?;
        gi.image_bytes(key).map_err(|e| crate::app_base::ApiError::App(Text::Msg(e.0)))
    }
}

// ---------------------------------------------------------------- the wrappers of api.rs
pub fn op_game_characters(app: &mut App, _a: &Args) -> Res<Answer> {
    ok(app.game_characters()?)
}

pub fn op_game_fontmetrics(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.game_font_metrics(a.s("k"))?)
}

pub fn op_game_scene(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.scene_info(a.s("file"), a.i("line"))?)
}

pub fn op_game_font(app: &mut App, a: &Args) -> Res<Answer> {
    let (data, mime) = app.game_font(a.s("k"))?;
    Ok(Answer::Binary(data, mime.to_string()))
}

pub fn op_game_image(app: &mut App, a: &Args) -> Res<Answer> {
    let (data, mime) = app.game_image(a.s("k"))?;
    Ok(Answer::Binary(data, mime.to_string()))
}

#[allow(dead_code)]
fn unused(_: &str) -> crate::app_base::ApiError {
    unknown_file()
}

#[allow(dead_code)]
fn paths_note(p: &Path, root: &Path) -> String {
    rel_text(p, root)
}
