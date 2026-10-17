//! Port of `core/lexo/app_missed.py`: texts the generator missed (show text, input prompts, titles in variables ...): find them in the game's scripts and add them to the
//! translation.
use crate::api::{ok, Answer, Args};
use crate::app::App;
use crate::app_base::{app_err, Res};
use crate::messages::{msg, msgn, py_str};
use crate::missed;
use crate::util::{py_strip, truthy};
use serde_json::{json, Value};
use std::path::PathBuf;

impl App {
    /// The folder with the game's scripts (.rpy outside tl): `<game>/` for a translation in `<game>/tl/<language>`.
    fn script_folder(&self) -> Option<PathBuf> {
        self.game_dir().filter(|b| missed::has_scripts(b))
    }

    fn missed_settings_json(&self) -> Res<Value> {
        let (functions, ignored) = missed::load_settings(&self.o()?.data_dir);
        Ok(json!({"functions": functions, "ignored": ignored}))
    }

    /// Look for texts in the game's scripts that the translation does not have. Nothing is changed. `level`: `likely` (default) or `all` (adds the weaker guesses).
    pub fn missed_scan(&self, level: &str) -> Res<Value> {
        self.need_project()?;
        let o = self.o()?;
        let (functions, ignored) = missed::load_settings(&o.data_dir);
        let settings = json!({"functions": functions, "ignored": ignored});
        let Some(folder) = self.script_folder() else {
            return Ok(json!({"items": [], "truncated": false, "folder": "", "counts": {"likely": 0, "maybe": 0}, "settings": settings,
                "message": msg("The game's scripts (.rpy) were not found next to the translation. Unpack and decompile the game first.", &[]).wire()}));
        };
        let known = missed::known_texts(&o.project.root);
        let (items, truncated) = missed::find_missed(
            &folder,
            &known.strings,
            Some(&known.dialogue),
            Some(&known.places),
            if level == "all" { 2000 } else { 100000 },
            &functions,
            &ignored,
        );
        let likely = items.iter().filter(|i| i.level == "likely").count();
        let counts = json!({"likely": likely, "maybe": items.len() - likely});
        let shown: Vec<Value> = items.iter().filter(|i| level == "all" || i.level == "likely").map(|i| i.to_json()).collect();
        Ok(json!({"items": shown, "truncated": truncated, "folder": folder.to_string_lossy(), "counts": counts, "settings": settings, "message": ""}))
    }

    /// Functions of the game that are called again and again with a text (candidates for the project's list of functions that show text).
    pub fn missed_functions(&self) -> Res<Value> {
        self.need_project()?;
        let o = self.o()?;
        let (functions, ignored) = missed::load_settings(&o.data_dir);
        let settings = json!({"functions": functions, "ignored": ignored});
        let Some(folder) = self.script_folder() else { return Ok(json!({"suggested": [], "settings": settings})) };
        let strings = missed::known_strings(&o.project.root);
        let suggested: Vec<Value> = missed::suggest_functions(&folder, &strings, &functions, &ignored, 3)
            .iter()
            .map(|(n, c, e)| json!({"name": n, "calls": c, "example": e}))
            .collect();
        Ok(json!({"suggested": suggested, "settings": settings}))
    }

    fn save_missed_settings(&self, functions: &[String], ignored: &[String]) -> Res<Value> {
        let o = self.o()?;
        match missed::save_settings(&o.data_dir, functions, ignored, &*self.clock) {
            Ok((f, i)) => Ok(json!({"functions": f, "ignored": i})),
            Err(e) => Err(app_err(msg("The settings of the search could not be saved: {reason}", &[("reason", e.reason.wire())]))),
        }
    }

    /// Set the project's own functions that show text (names like `notify` or `ui.popup`); the ignored texts stay.
    pub fn missed_settings(&self, functions: &Value) -> Res<Value> {
        self.need_project()?;
        let (_, ignored) = missed::load_settings(&self.o()?.data_dir);
        let names: Vec<String> = functions.as_array().into_iter().flatten().map(|f| py_strip(&py_str(f)).to_string()).filter(|f| !f.is_empty()).collect();
        self.save_missed_settings(&names, &ignored)
    }

    /// Leave these texts out of the list for good (the project remembers them in missed.json); `clear` forgets all of them.
    pub fn missed_ignore(&self, texts: &Value, clear: bool) -> Res<Value> {
        self.need_project()?;
        let (functions, ignored) = missed::load_settings(&self.o()?.data_dir);
        if clear {
            return self.save_missed_settings(&functions, &[]);
        }
        let new: Vec<String> = texts.as_array().into_iter().flatten().map(py_str).filter(|t| !py_strip(t).is_empty()).collect();
        if new.is_empty() {
            return Err(app_err(msg("Nothing was chosen.", &[])));
        }
        let mut all = ignored;
        all.extend(new);
        self.save_missed_settings(&functions, &all)
    }

    /// Write the chosen texts to `<translation folder>/extra_strings.rpy` (untranslated, so they appear as lines to do) and reload the files. `into_files` (experimental): put
    /// each text into the translation file of its own script instead.
    pub fn missed_add(&mut self, items: &Value, into_files: bool) -> Res<Value> {
        self.need_project()?;
        let chosen: Vec<missed::Item> = items
            .as_array()
            .into_iter()
            .flatten()
            .filter(|i| i.is_object() && !py_strip(&i.get("text").map(py_str).unwrap_or_default()).is_empty())
            .map(|i| missed::Item {
                text: i.get("text").map(py_str).unwrap_or_default(),
                file: i.get("file").map(py_str).unwrap_or_default(),
                line: i.get("line").filter(|v| truthy(v)).and_then(|v| v.as_i64().or_else(|| py_str(v).parse().ok())).unwrap_or(0),
            })
            .collect();
        if chosen.is_empty() {
            return Err(app_err(msg("Nothing was chosen.", &[])));
        }
        self.snapshot("bulk", false);
        let o = self.o()?;
        let language = o.project.root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let res = missed::write_strings(&o.project.root, &language, &chosen, Some(&o.local_dir.join("backups")), into_files, &*self.clock)
            .map_err(|e| app_err(msg("The strings file could not be written: {reason}", &[("reason", e.reason.wire())])))?;
        self.refresh(true, false, false)?;
        let files: serde_json::Map<String, Value> = res.files.iter().map(|(f, n)| (f.clone(), json!(n))).collect();
        Ok(json!({
            "file": res.file, "files": files, "added": res.added, "skipped": res.skipped,
            "message": msgn(res.added as i64, "{n} text was added to the translation files.", "{n} texts were added to the translation files.", &[]).wire(),
        }))
    }
}

pub fn op_missed(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.missed_scan(a.s("level"))?)
}

pub fn op_missed_functions(app: &mut App, _a: &Args) -> Res<Answer> {
    ok(app.missed_functions()?)
}

pub fn op_missed_settings(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.missed_settings(a.a("functions"))?)
}

pub fn op_missed_ignore(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.missed_ignore(a.a("texts"), a.f("clear"))?)
}

pub fn op_missed_add(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.missed_add(a.a("items"), a.f("into_files"))?)
}

#[allow(dead_code)]
fn unused(app: &App) -> Res<Value> {
    app.missed_settings_json()
}
