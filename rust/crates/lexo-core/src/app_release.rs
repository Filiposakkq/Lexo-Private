//! The release builder (the Pro feature `release.build`): the translation as a zip the players unpack into their game.
//!
//! Core collects the `.rpy` files of the translation (and the files the translator attached: fonts, a gui file...), the numbers of the project and the history of the releases, asks the
//! Pro pack for the judgement (the checks, the changelog, the README, the script of the default language), and writes the zip, a file with its SHA-256 and the release notes next to it.
//! The result is a file; where it goes is the translator's business. The settings and the list of the releases are in `.aside/release.json` (shared with the project), the digests of the
//! lines of every release (for the changelog) in the local data. These calls are not in the contract of Core: they belong to the application around it.
use crate::app::App;
use crate::app_base::{app_err, Res};
use crate::fsutil;
use crate::messages::{msg, py_str};
use crate::project::DATA_DIR_NAME;
use crate::util::sha1_hex;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const FEATURE: &str = "release.build";
const MAX_EXTRAS: usize = 50;

/// The directories that never go into a release.
fn skipped_dir(name: &str) -> bool {
    name.starts_with('.') || matches!(name, "backups" | "_progress_data" | "__pycache__")
}

/// Every `.rpy` file under `root`, as (relative path with `/`, full path), sorted.
fn rpy_files(root: &Path, skip: &std::collections::BTreeSet<String>) -> Vec<(String, PathBuf)> {
    fn walk(dir: &Path, prefix: &str, skip: &std::collections::BTreeSet<String>, out: &mut Vec<(String, PathBuf)>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy().into_owned();
            let rel = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
            if e.path().is_dir() {
                if !skipped_dir(&name) {
                    walk(&e.path(), &rel, skip, out);
                }
            } else if name.to_lowercase().ends_with(".rpy") && !skip.contains(&name.to_lowercase()) {
                out.push((rel, e.path()));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, "", skip, &mut out);
    out.sort_by_key(|(r, _)| r.to_lowercase());
    out
}

/// The text of a `define config.<name> = "…"` (or `_("…")`) line of a script, empty when there is none.
fn config_value(text: &str, name: &str) -> String {
    let needle = format!("config.{name}");
    for line in text.lines() {
        let l = line.trim();
        if !(l.starts_with("define ") || l.starts_with("$ ")) {
            continue;
        }
        let Some(i) = l.find(&needle) else { continue };
        let rest = l[i + needle.len()..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else { continue };
        let rest = rest.trim_start().trim_start_matches("_(").trim_start();
        let Some(q) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else { continue };
        if let Some(end) = rest[1..].find(q) {
            return rest[1..1 + end].to_string();
        }
    }
    String::new()
}

fn sanitize(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '-' }).collect();
    s.trim_matches(['-', '.']).to_string()
}

fn default_settings() -> Value {
    json!({"extras": [], "language_script": true, "readme": true, "notes_md": true, "credits": "", "readme_extra": "", "out_dir": "", "files": {"mode": "partial", "include": [], "exclude": []}})
}

impl App {
    fn release_gate(&self) -> Res<()> {
        if self.ext.decide(FEATURE).available {
            Ok(())
        } else {
            Err(app_err(msg("“{title}” is a Pro feature.", &[("title", json!("Release builder"))])))
        }
    }

    fn release_path(&self) -> Res<PathBuf> {
        Ok(self.o()?.project.root.join(DATA_DIR_NAME).join("release.json"))
    }

    /// `{settings, releases}` from `.aside/release.json` (defaults when there is no file).
    fn release_file(&self) -> Res<Value> {
        let mut v: Value = std::fs::read_to_string(self.release_path()?)
            .ok()
            .and_then(|t| serde_json::from_str(t.trim_start_matches('\u{feff}')).ok())
            .unwrap_or_else(|| json!({}));
        let mut settings = default_settings();
        if let (Some(s), Some(d)) = (v["settings"].as_object(), settings.as_object_mut()) {
            for (k, val) in s {
                d.insert(k.clone(), val.clone());
            }
        }
        v["settings"] = settings;
        if !v["releases"].is_array() {
            v["releases"] = json!([]);
        }
        Ok(v)
    }

    fn release_save(&self, v: &Value) -> Res<()> {
        let path = self.release_path()?;
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        fsutil::atomic_write_text(&path, &serde_json::to_string_pretty(v).unwrap_or_default(), Some("\n"), None, &*self.clock)
            .map_err(|e| app_err(msg("The release settings could not be saved: {reason}", &[("reason", json!(e.to_string()))])))
    }

    /// The name and the version of the game, read from its scripts (`config.name`, `config.version`).
    fn release_game(&self) -> Value {
        let mut name = String::new();
        let mut version = String::new();
        if let Some(game) = self.game_dir() {
            for f in ["options.rpy", "script.rpy"] {
                if let Ok(text) = std::fs::read_to_string(game.join(f)) {
                    if name.is_empty() {
                        name = config_value(&text, "name");
                    }
                    if version.is_empty() {
                        version = config_value(&text, "version");
                    }
                }
            }
        }
        if name.is_empty() {
            name = self.game_root().and_then(|r| r.file_name().map(|n| n.to_string_lossy().into_owned())).unwrap_or_default();
        }
        json!({"name": name, "version": version})
    }

    /// What the release builder shows when it opens: the settings, the releases so far, the game, the language, a suggested version.
    pub fn release_state(&self) -> Res<Value> {
        self.need_project()?;
        self.release_gate()?;
        let file = self.release_file()?;
        let o = self.o()?;
        let language = o.project.root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let releases: Vec<Value> = file["releases"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|mut r| {
                if let Some(m) = r.as_object_mut() {
                    m.remove("files"); // the digests of the files are for the changelog only
                }
                r
            })
            .collect();
        let last = file["releases"].as_array().and_then(|a| a.last()).map(|r| py_str(&r["version"])).unwrap_or_default();
        Ok(json!({"ok": true, "settings": file["settings"], "releases": releases, "game": self.release_game(), "language": language, "last_version": last}))
    }

    /// Saves the settings (the attached files, the options, the credits, the folder of the output).
    pub fn release_set(&mut self, settings: &Value) -> Res<Value> {
        self.need_project()?;
        self.release_gate()?;
        let mut clean = default_settings();
        let m = clean.as_object_mut().unwrap();
        for k in ["language_script", "readme", "notes_md"] {
            if let Some(b) = settings[k].as_bool() {
                m.insert(k.into(), json!(b));
            }
        }
        for (k, max) in [("credits", 2000), ("readme_extra", 4000), ("out_dir", 500)] {
            let s = py_str(&settings[k]);
            if s.chars().count() > max {
                return Err(app_err(msg("The name or note is too long.", &[])));
            }
            m.insert(k.into(), json!(s.trim()));
        }
        // which files: a preset (complete / partial / all) and the files you added or took out by hand
        let mode = py_str(&settings["files"]["mode"]);
        let mode = if ["complete", "partial", "all"].contains(&mode.as_str()) { mode } else { "partial".to_string() };
        let paths = |k: &str| -> Vec<Value> {
            settings["files"][k].as_array().into_iter().flatten().map(py_str).filter(|p| !p.is_empty() && p.len() < 400).take(5000).map(Value::from).collect()
        };
        m.insert("files".into(), json!({"mode": mode, "include": paths("include"), "exclude": paths("exclude")}));
        let extras = settings["extras"].as_array().cloned().unwrap_or_default();
        if extras.len() > MAX_EXTRAS {
            return Err(app_err(msg("At most {n} files can be attached.", &[("n", json!(MAX_EXTRAS))])));
        }
        let mut list = Vec::new();
        for e in extras {
            let (from, to) = (py_str(&e["from"]).trim().to_string(), py_str(&e["to"]).trim().replace('\\', "/"));
            if from.is_empty() || to.is_empty() {
                return Err(app_err(msg("An attached file needs its place on this computer and its place in the game.", &[])));
            }
            list.push(json!({"from": from, "to": to}));
        }
        m.insert("extras".into(), json!(list));
        let mut file = self.release_file()?;
        file["settings"] = clean;
        self.release_save(&file)?;
        Ok(json!({"ok": true, "settings": file["settings"]}))
    }

    /// Everything the pack needs to judge a release, and what goes into the zip: `(payload for the pack, files (path, full path), extras (to, full path))`.
    #[allow(clippy::type_complexity)]
    fn release_input(&mut self, version: &str, notes: &str) -> Res<(Value, Vec<(String, PathBuf)>, Vec<(String, PathBuf)>, Vec<Value>)> {
        self.need_project()?;
        let overview = self.overview()?;
        let file = self.release_file()?;
        let settings = file["settings"].clone();
        let game = self.release_game();
        let o = self.o()?;
        let language = o.project.root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let prefix = format!("game/tl/{language}");
        let all_files = rpy_files(&o.project.root, o.project.skip());
        // how far every file is translated, and which files the preset and your hand picks select
        let rows: std::collections::BTreeMap<String, crate::project::Row> = o.project.rows(&o.store).into_iter().map(|r| (r.name.clone(), r)).collect();
        let sel = &settings["files"];
        let mode = py_str(&sel["mode"]);
        let set_of = |k: &str| -> std::collections::BTreeSet<String> { sel[k].as_array().into_iter().flatten().map(py_str).collect() };
        let (include, exclude) = (set_of("include"), set_of("exclude"));
        let mut listed: Vec<Value> = Vec::new();
        let mut zip_files: Vec<(String, PathBuf)> = Vec::new();
        let mut file_list: Vec<Value> = Vec::new();
        let mut files: Vec<(String, PathBuf)> = Vec::new();
        let (mut sel_total, mut sel_done) = (0i64, 0i64);
        for (rel, full) in &all_files {
            let (total, done, todo) = rows.get(rel).map_or((0, 0, 0), |r| (r.total, r.done, r.todo));
            let state = if total == 0 {
                "other"
            } else if todo == 0 {
                "complete"
            } else if done > 0 {
                "partial"
            } else {
                "none"
            };
            let by_mode = match mode.as_str() {
                "all" => true,
                "complete" => state == "complete",
                _ => state == "complete" || state == "partial",
            };
            let selected = if include.contains(rel) {
                true
            } else if exclude.contains(rel) {
                false
            } else {
                by_mode
            };
            let path = format!("{prefix}/{rel}");
            file_list.push(json!({"file": rel, "path": path, "total": total, "done": done, "state": state, "by_mode": by_mode, "selected": selected}));
            if !selected {
                continue;
            }
            let bytes = std::fs::read(full).map_err(|e| app_err(msg("The file could not be read: {reason}", &[("reason", json!(format!("{rel}: {e}")))])))?;
            listed.push(json!({"path": path, "sha1": sha1_hex(&bytes), "bytes": bytes.len(), "state": state}));
            zip_files.push((path, full.clone()));
            files.push((rel.clone(), full.clone()));
            sel_total += total;
            sel_done += done;
        }
        // the digests of the lines (for the changelog of the next release)
        let mut lines = Map::new();
        for (rel, _) in &files {
            if o.project.files.contains_key(rel) {
                for e in o.project.entries(rel) {
                    if e.source.is_some() && o.project.is_translated(&o.store, rel, e) && !e.target.trim().is_empty() {
                        let key = e.key();
                        lines.insert(
                            sha1_hex(format!("{rel}|{}|{}", key.0, key.1).as_bytes())[..12].to_string(),
                            json!(sha1_hex(e.target.as_bytes())[..8].to_string()),
                        );
                    }
                }
            }
        }
        let t = &overview["totals"];
        let v = &overview["validation"];
        // the numbers are those of the files that go into the release
        let stats = json!({"total": sel_total, "translated": sel_done, "approved": t["approved"], "proofread": t["proofread"], "fix": t["proof_fix"], "stale": t["proof_stale"],
            "errors": v["error"], "warnings": v["warning"]});
        // the files that are attached
        let mut extras: Vec<Value> = Vec::new();
        let mut zip_extras: Vec<(String, PathBuf)> = Vec::new();
        for e in settings["extras"].as_array().into_iter().flatten() {
            let (from, to) = (py_str(&e["from"]), py_str(&e["to"]));
            let meta = std::fs::metadata(&from).ok().filter(|m| m.is_file());
            extras.push(json!({"from": from, "to": to, "exists": meta.is_some(), "size": meta.as_ref().map_or(0, |m| m.len())}));
            if meta.is_some() {
                zip_extras.push((to, PathBuf::from(from)));
            }
        }
        // the previous release: what the changelog is compared with
        let prev = file["releases"].as_array().and_then(|a| a.last()).cloned();
        let previous = match prev {
            Some(mut p) => {
                let digests = std::fs::read_to_string(o.local_dir.join("releases").join(format!("{}-lines.json", sanitize(&py_str(&p["version"])))))
                    .ok()
                    .and_then(|t| serde_json::from_str::<Value>(&t).ok());
                p["lines"] = digests.unwrap_or(Value::Null);
                p
            }
            None => Value::Null,
        };
        let today = self.clock.now_local().date().format("%Y-%m-%d").to_string();
        let payload = json!({"language": language, "game": game, "version": version, "notes": notes, "today": today, "files": listed, "lines": lines, "stats": stats, "previous": previous,
            "extras": extras, "options": settings});
        Ok((payload, zip_files, zip_extras, file_list))
    }

    /// The checks, the changelog and the texts of a release, and what would go into it. Nothing is written.
    pub fn release_preview(&mut self, version: &str, notes: &str) -> Res<Value> {
        self.release_gate()?;
        let (payload, files, extras, file_list) = self.release_input(version.trim(), notes)?;
        let mut plan = self.ext_call(FEATURE, "plan", &payload)?;
        plan["ok_to_build"] = plan["ok"].clone();
        plan["files"] = json!(files.len());
        plan["file_list"] = json!(file_list);
        plan["extras"] = json!(extras.len());
        plan["stats"] = payload["stats"].clone();
        plan["game"] = payload["game"].clone();
        plan["language"] = payload["language"].clone();
        plan["previous"] =
            if payload["previous"].is_object() { json!({"version": payload["previous"]["version"], "date": payload["previous"]["date"]}) } else { Value::Null };
        plan["ok"] = json!(true);
        plan["can_build"] = json!(plan["ok_to_build"] == true);
        Ok(plan)
    }

    /// Writes the zip (and a file with its SHA-256 and the release notes) and records the release. With `force` the errors of the check of the translation do not stop it.
    pub fn release_build(&mut self, version: &str, notes: &str, force: bool) -> Res<Value> {
        self.release_gate()?;
        let version = version.trim();
        let (payload, files, extras, _list) = self.release_input(version, notes)?;
        let plan = self.ext_call(FEATURE, "plan", &payload)?;
        if plan["ok"] != true && !(force && plan["can_force"] == true) {
            return Err(app_err(msg("The release cannot be built yet: look at the checks.", &[])));
        }
        let mut seen = std::collections::BTreeSet::new();
        for (to, _) in files.iter().chain(extras.iter()) {
            if !seen.insert(to.to_lowercase()) {
                return Err(app_err(msg("Two files would be in the same place of the archive: {path}", &[("path", json!(to))])));
            }
        }
        let language = py_str(&payload["language"]);
        let game_name = sanitize(&py_str(&payload["game"]["name"]));
        let base = format!("{}{}-v{version}", if game_name.is_empty() { String::new() } else { format!("{game_name}-") }, sanitize(&language));
        let (out_dir, local_dir) = {
            let o = self.o()?;
            let configured = py_str(&payload["options"]["out_dir"]);
            (if configured.is_empty() { o.local_dir.join("releases") } else { PathBuf::from(configured) }, o.local_dir.clone())
        };
        std::fs::create_dir_all(&out_dir)
            .map_err(|e| app_err(msg("The folder of the release could not be made: {reason}", &[("reason", json!(e.to_string()))])))?;
        let zip_path = out_dir.join(format!("{base}.zip"));
        // the archive: the texts, the translation, the attached files, in a fixed order
        let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut put = |name: &str, data: &[u8]| -> Res<()> {
            z.start_file(name, opts)
                .and_then(|_| z.write_all(data).map_err(zip::result::ZipError::Io))
                .map_err(|e| app_err(msg("The archive could not be written: {reason}", &[("reason", json!(e.to_string()))])))
        };
        if let Some(t) = plan["readme"].as_str() {
            put("README.txt", t.as_bytes())?;
        }
        put("CHANGELOG.txt", py_str(&plan["changelog"]).as_bytes())?;
        if let Some(t) = plan["language_script"].as_str() {
            put(&format!("game/zz_{}_default_language.rpy", sanitize(&language)), t.as_bytes())?;
        }
        for (to, full) in files.iter().chain(extras.iter()) {
            let data = std::fs::read(full).map_err(|e| app_err(msg("The file could not be read: {reason}", &[("reason", json!(format!("{to}: {e}")))])))?;
            put(to, &data)?;
        }
        let bytes = z.finish().map_err(|e| app_err(msg("The archive could not be written: {reason}", &[("reason", json!(e.to_string()))])))?.into_inner();
        fsutil::atomic_write_bytes(&zip_path, &bytes, None, &*self.clock)
            .map_err(|e| app_err(msg("The archive could not be saved: {reason}", &[("reason", json!(e.to_string()))])))?;
        let sha256: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
        let sha_path = out_dir.join(format!("{base}.zip.sha256"));
        let _ = std::fs::write(&sha_path, format!("{sha256} *{base}.zip\n"));
        let notes_path = plan["release_notes"].as_str().map(|t| {
            let p = out_dir.join(format!("{base}-release-notes.md"));
            let _ = std::fs::write(&p, t);
            p
        });
        // the release is recorded (the digests of the lines go to the local data)
        let _ = std::fs::create_dir_all(local_dir.join("releases"));
        let _ = std::fs::write(
            local_dir.join("releases").join(format!("{}-lines.json", sanitize(version))),
            serde_json::to_string(&payload["lines"]).unwrap_or_default(),
        );
        let files_map: Map<String, Value> = payload["files"].as_array().into_iter().flatten().map(|f| (py_str(&f["path"]), f["sha1"].clone())).collect();
        let record = json!({"version": version, "date": payload["today"], "game_version": payload["game"]["version"], "files": files_map, "stats": payload["stats"],
            "zip": format!("{base}.zip"), "sha256": sha256, "size": bytes.len(), "notes": notes, "forced": force && plan["ok"] != true});
        let mut file = self.release_file()?;
        file["releases"].as_array_mut().unwrap().push(record);
        self.release_save(&file)?;
        Ok(
            json!({"ok": true, "version": version, "zip": zip_path.to_string_lossy(), "size": bytes.len(), "sha256": sha256, "sha_file": sha_path.to_string_lossy(),
            "notes_file": notes_path.map(|p| p.to_string_lossy().into_owned()), "files": files.len(), "extras": extras.len(), "changelog": plan["changelog"]}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_and_the_version_of_a_game_are_read_from_its_script() {
        let text = "define config.name = _(\"Midnight Paradise\")\ndefine config.version = '1.1b'\n# define config.version = \"0\"\n$ config.version = \"9\"\n";
        assert_eq!(config_value(text, "name"), "Midnight Paradise");
        assert_eq!(config_value(text, "version"), "1.1b"); // the first definition wins; a comment is not one
        assert_eq!(config_value("init python:\n    pass\n", "version"), "");
    }

    #[test]
    fn file_names_are_made_safe() {
        assert_eq!(sanitize("Midnight Paradise: 1.1b"), "Midnight-Paradise--1.1b");
        assert_eq!(sanitize("czech"), "czech");
        assert_eq!(sanitize("../../x"), "x");
    }
}
