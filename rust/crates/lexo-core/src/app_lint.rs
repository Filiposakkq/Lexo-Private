//! Port of `reference/python/core/lexo/app_lint.py`: the game check (Ren'Py lint) run in the background.
use crate::api::{ok, plain, Answer, Args};
use crate::app::App;
use crate::app_base::{app_err, ApiError, Res};
use crate::fsutil::resolve;
use crate::lint::{self, Platform};
use crate::messages::{msg, py_str, Text};
use serde_json::{json, Value};
use std::path::Path;

impl App {
    /// Start the Ren'Py lint of the whole game.
    pub fn lint_start(&self) -> Res<Value> {
        self.need_project()?;
        let sdk = self.config.get("lint.sdk").map(py_str).unwrap_or_default();
        let sdk = crate::util::py_strip(&sdk);
        if sdk.is_empty() {
            return Err(app_err(msg("First set the Ren'Py SDK folder (the one with renpy.py).", &[])));
        }
        let Some(base) = self.game_root() else {
            return Err(app_err(msg("The game folder (game) above the translation was not found – lint needs the source files of the game.", &[])));
        };
        self.linter.start(Path::new(sdk), &base, lint::real_runner(), Platform::current()).map_err(|e| ApiError::App(Text::new_msg(e.0)))?;
        Ok(json!({"ok": true}))
    }

    pub fn lint_status(&self) -> Res<Value> {
        self.need_project()?;
        let mut st = self.linter.status();
        let base = self.game_root();
        let o = self.o()?;
        if let Some(issues) = st.get_mut("issues").and_then(Value::as_array_mut) {
            for it in issues {
                let mut rel = String::new();
                if let Some(base) = &base {
                    let p = resolve(&base.join(it["file"].as_str().unwrap_or("")));
                    if let Ok(r) = p.strip_prefix(resolve(&o.project.root)) {
                        let r = r.to_string_lossy().replace('\\', "/");
                        rel = o.project.files.keys().find(|k| k.replace('\\', "/") == r).cloned().unwrap_or_default();
                    }
                }
                it["rel"] = json!(rel);
            }
        }
        Ok(st)
    }
}

pub fn op_lint_start(app: &mut App, _a: &Args) -> Res<Answer> {
    plain(app.lint_start()?)
}

pub fn op_lint_status(app: &mut App, _a: &Args) -> Res<Answer> {
    ok(app.lint_status()?)
}
