//! Port of `core/lexo/config.py`: application settings, stored in one JSON file that does not depend on the working directory.
//!
//! Windows: `%APPDATA%\Aside\config.json` (the old name was `Lexo`, see `take_over_folder`); other: `~/.config/aside/config.json`; the folder can be overridden with the `LEXO_CONFIG_DIR` environment variable
//! (used by the tests). The file never contains secrets: git authenticates through the user's own git credentials.
use crate::clock::Clock;
use crate::fsutil::{atomic_write_text, WriteFailed};
use crate::messages::{msg, Message};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

pub fn defaults() -> Value {
    json!({
        "version": 1,
        "project_dir": "",
        "skip_files": ["common.rpy"],
        "count_symbol_only_as_done": false,
        "editor": {"preferred": "auto", "custom": ""},
        "validation": {
            "enabled": true,
            "show_info": false,
            "spelling": true,
            "textbox": {"enabled": true, "chars_per_line": 65, "max_lines": 4},
            "length": {"enabled": true, "min_source_chars": 12, "max_ratio": 2.2, "min_ratio": 0.45, "min_extra_chars": 25, "min_missing_chars": 18},
        },
        "goals": {"translate_per_week": 0, "proofread_per_week": 0},
        "snapshots": {"enabled": true, "interval_hours": 12, "keep": 20},
        "mt": {"provider": "", "api_key": "", "base_url": "", "model": "", "source_lang": "EN", "target_lang": "", "consent": false},
        "lint": {"sdk": ""},
        "web": {"repo": "", "path": "web/progress.json", "title": ""},
        "ui": {},
        "author": "",
    })
}

/// The folder of the settings and local data called `name` (`%APPDATA%\<name>` on Windows, `~/.config/<name in lower case>` elsewhere).
fn dir_named(name: &str) -> PathBuf {
    let home = || std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default();
    if cfg!(windows) {
        match std::env::var_os("APPDATA").filter(|e| !e.is_empty()) {
            Some(base) => PathBuf::from(base).join(name),
            None => home().join(name),
        }
    } else {
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".config")).join(name.to_lowercase())
    }
}

pub fn config_dir() -> PathBuf {
    if let Some(env) = std::env::var_os("LEXO_CONFIG_DIR").filter(|e| !e.is_empty()) {
        return PathBuf::from(env);
    }
    dir_named("Aside")
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// The application was called Lexo before it was called Aside: the first start of Aside takes over the settings and data of the old folder (a copy, the old folder stays for the old
/// application). Copies when `old` exists and `new` does not; returns whether it did. The copy is made next to `new` and renamed, so a failure leaves no half a folder.
pub fn take_over_folder(old: &Path, new: &Path) -> std::io::Result<bool> {
    if new.exists() || !old.is_dir() {
        return Ok(false);
    }
    let tmp = new.with_extension("copying");
    let _ = std::fs::remove_dir_all(&tmp);
    if let Err(e) = copy_tree(old, &tmp).and_then(|_| std::fs::rename(&tmp, new)) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }
    Ok(true)
}

/// Called once at the start of the application (not when `LEXO_CONFIG_DIR` chooses the folder).
pub fn take_over_legacy_config() {
    if std::env::var_os("LEXO_CONFIG_DIR").is_some_and(|e| !e.is_empty()) {
        return;
    }
    if let Err(e) = take_over_folder(&dir_named("Lexo"), &dir_named("Aside")) {
        eprintln!("The settings of Lexo could not be taken over: {e}");
    }
}

/// Python's `isinstance(v, type(default))` for JSON values (a bool is an int there).
fn same_type(default: &Value, v: &Value) -> bool {
    match (default, v) {
        (Value::Null, Value::Null)
        | (Value::Bool(_), Value::Bool(_))
        | (Value::String(_), Value::String(_))
        | (Value::Array(_), Value::Array(_))
        | (Value::Object(_), Value::Object(_)) => true,
        (Value::Number(d), Value::Bool(_)) => !d.is_f64(),
        (Value::Number(d), Value::Number(n)) => d.is_f64() == n.is_f64(),
        _ => false,
    }
}

/// Deep-merge `over` onto a copy of `base`, keeping only values of a compatible type; unknown keys of `over` are kept (a newer version may have written them).
pub fn merge(base: &Value, over: &Value) -> Value {
    let mut out = base.clone();
    let (Some(o), Some(b)) = (over.as_object(), out.as_object_mut()) else { return out };
    for (k, v) in o {
        match b.get(k) {
            None => {
                b.insert(k.clone(), v.clone());
            }
            Some(cur) if cur.is_object() && v.is_object() => {
                let merged = merge(cur, v);
                b.insert(k.clone(), merged);
            }
            Some(cur) => {
                // a number may replace an int, a float or a bool (but a bool replaces only a bool or an int)
                let number_for_number = matches!(cur, Value::Number(_) | Value::Bool(_)) && v.is_number();
                if same_type(cur, v) || number_for_number {
                    b.insert(k.clone(), v.clone());
                }
            }
        }
    }
    out
}

pub struct Config {
    pub path: PathBuf,
    pub data: Value,
    pub load_error: Option<Message>,
}

impl Config {
    pub fn new(path: Option<&Path>) -> Config {
        Config { path: path.map(Path::to_path_buf).unwrap_or_else(|| config_dir().join("config.json")), data: defaults(), load_error: None }
    }

    pub fn load(&mut self) -> &mut Config {
        self.load_error = None;
        match std::fs::read(&self.path) {
            Ok(bytes) => {
                let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
                match std::str::from_utf8(body).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str::<Value>(t).map_err(|e| e.to_string())) {
                    Ok(raw) => self.data = merge(&defaults(), &raw),
                    Err(e) => self.fail(e),
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => self.data = defaults(),
            Err(e) => self.fail(e.to_string()),
        }
        self
    }

    /// Keep the broken file for inspection; run with defaults instead of crashing.
    fn fail(&mut self, e: String) {
        self.load_error = Some(msg("The settings cannot be loaded ({e}); using the defaults.", &[("e", json!(e))]));
        self.data = defaults();
    }

    pub fn save(&self, clock: &dyn Clock) -> Result<(), WriteFailed> {
        let text = serde_json::to_string_pretty(&self.data).unwrap_or_default() + "\n";
        atomic_write_text(&self.path, &text, Some("\n"), Some(&self.path.parent().unwrap_or(Path::new(".")).join("backups")), clock)
    }

    pub fn get(&self, dotted: &str) -> Option<&Value> {
        let mut cur = &self.data;
        for part in dotted.split('.') {
            cur = cur.as_object()?.get(part)?;
        }
        Some(cur)
    }

    pub fn set(&mut self, dotted: &str, value: Value) {
        let parts: Vec<&str> = dotted.split('.').collect();
        let mut cur = &mut self.data;
        for part in &parts[..parts.len() - 1] {
            if !cur.is_object() {
                *cur = Value::Object(Map::new());
            }
            cur = cur.as_object_mut().unwrap().entry(part.to_string()).or_insert_with(|| Value::Object(Map::new()));
        }
        if let Some(o) = cur.as_object_mut() {
            o.insert(parts[parts.len() - 1].to_string(), value);
        }
    }

    /// Merge a (validated) patch and save.
    pub fn update(&mut self, patch: &Value, clock: &dyn Clock) -> Result<(), WriteFailed> {
        self.data = merge(&self.data, patch);
        self.save(clock)
    }

    /// The settings for the UI. The machine translation key never leaves the app: only whether one is stored.
    pub fn public(&self) -> Value {
        let mut d = self.data.clone();
        if let Some(mt) = d.get_mut("mt").and_then(Value::as_object_mut) {
            let has_key = mt.get("api_key").is_some_and(crate::util::truthy);
            mt.insert("has_key".into(), json!(has_key));
            mt.insert("api_key".into(), json!(""));
        }
        d
    }
}

#[cfg(test)]
mod take_over_tests {
    use super::take_over_folder;

    #[test]
    fn the_new_folder_takes_a_copy_of_the_old_one_only_once() {
        let tmp = std::env::temp_dir().join(format!("aside-takeover-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let (old, new) = (tmp.join("Lexo"), tmp.join("Aside"));
        // nothing to take over
        assert!(!take_over_folder(&old, &new).unwrap() && !new.exists());
        std::fs::create_dir_all(old.join("projects/p1/backups")).unwrap();
        std::fs::write(old.join("config.json"), "{\"author\": \"A\"}").unwrap();
        std::fs::write(old.join("projects/p1/backups/x.rpy"), "x").unwrap();
        assert!(take_over_folder(&old, &new).unwrap());
        assert_eq!(std::fs::read_to_string(new.join("config.json")).unwrap(), "{\"author\": \"A\"}");
        assert!(new.join("projects/p1/backups/x.rpy").is_file() && old.join("config.json").is_file(), "a copy, the old folder stays");
        // an existing new folder is never touched
        std::fs::write(new.join("config.json"), "{}").unwrap();
        assert!(!take_over_folder(&old, &new).unwrap());
        assert_eq!(std::fs::read_to_string(new.join("config.json")).unwrap(), "{}");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
