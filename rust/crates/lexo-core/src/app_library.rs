//! The library of games shown on Home (new interface): the games the user has worked on or added, with what Home shows on a card. Kept in `library.json` in the settings folder of the application
//! (local to this computer; never in a project and never shared). A game gets into the library when its project is opened (every change of a project updates its card: the numbers and the time),
//! or when the user adds its folder. Removing a game removes only the card, never a file of the game. These calls are not in the contract of Core.
use crate::app::App;
use crate::app_base::{app_err, Res};
use crate::fsutil;
use crate::messages::{msg, py_str};
use crate::project::find_translation_folder;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const MAX_GAMES: usize = 500;
/// The time of the last work is moved at most this often (seconds), so that typing does not rewrite the file at every key.
const TOUCH_EVERY: i64 = 60;

fn stamp(app: &App) -> String {
    app.clock.now_utc().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// A translation folder sits in `game/tl/<language>`: its parent is called `tl`.
fn is_language_folder(dir: &Path) -> bool {
    dir.parent().and_then(|p| p.file_name()).is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("tl"))
}

impl App {
    fn library_file(&self) -> PathBuf {
        self.cfg_dir.join("library.json")
    }

    fn library_read(&self) -> Vec<Value> {
        let text = std::fs::read_to_string(self.library_file()).unwrap_or_default();
        let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap_or_else(|_| json!({}));
        v["games"].as_array().cloned().unwrap_or_default().into_iter().filter(|g| g["path"].as_str().is_some_and(|p| !p.is_empty())).collect()
    }

    fn library_write(&self, games: &[Value]) -> Res<()> {
        let path = self.library_file();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        fsutil::atomic_write_text(&path, &serde_json::to_string_pretty(&json!({"games": games})).unwrap_or_default(), Some("\n"), None, &*self.clock)
            .map_err(|e| app_err(msg("The library of games could not be saved: {reason}", &[("reason", json!(e.to_string()))])))
    }

    /// The games for Home, the pinned ones first, then the most recently used: `{games: [{path, name, version, language, total, done, corr, pinned, opened, state}]}`.
    /// `state`: `ready` (a translation is there), `needs_prepare` (a game without a translation), `choose` (several translations: open it to choose), `missing` (the folder is gone).
    pub fn library_list(&self) -> Res<Value> {
        let mut games: Vec<Value> = self
            .library_read()
            .into_iter()
            .map(|g| {
                let path = PathBuf::from(py_str(&g["path"]));
                let state = if !path.is_dir() {
                    "missing"
                } else {
                    match find_translation_folder(&path) {
                        Ok((f, _)) if f != path || is_language_folder(&f) => "ready",
                        Ok(_) => "needs_prepare",
                        Err(_) => "choose",
                    }
                };
                let s = &g["summary"];
                let name = py_str(&s["name"]);
                json!({
                    "path": g["path"], "name": if name.is_empty() { folder_name(&path) } else { name }, "version": s["version"], "language": s["language"],
                    "total": s["total"], "done": s["done"], "corr": s["corr"], "pinned": g["pinned"].as_bool().unwrap_or(false), "opened": g["opened"], "state": state,
                })
            })
            .collect();
        games.sort_by(|a, b| {
            b["pinned"]
                .as_bool()
                .cmp(&a["pinned"].as_bool())
                .then_with(|| py_str(&b["opened"]).cmp(&py_str(&a["opened"])))
                .then_with(|| py_str(&a["name"]).to_lowercase().cmp(&py_str(&b["name"]).to_lowercase()))
        });
        // Home has no project open, so what the title needs is here: is Pro really on (a licence that works or the development unlock)?
        let features = crate::features::status_of(&self.ext);
        let pro = features.as_object().is_some_and(|m| m.values().any(|f| f["available"] == true && f["limit"].is_null()));
        Ok(json!({"ok": true, "games": games, "pro": pro}))
    }

    /// Add a game by its folder (a game folder, its `game` folder or a translation folder). It is only put on Home; nothing is opened or written in the game.
    pub fn library_add(&mut self, path: &str) -> Res<Value> {
        let p = PathBuf::from(crate::util::py_strip(path).trim_matches('"'));
        if !p.is_dir() {
            return Err(app_err(msg("The folder was not found: {path}", &[("path", json!(path))])));
        }
        let mut games = self.library_read();
        let key = fsutil::resolve(&p);
        let key_s = key.to_string_lossy().into_owned();
        if games.iter().any(|g| py_str(&g["path"]) == key_s) {
            return self.library_list();
        }
        if games.len() >= MAX_GAMES {
            return Err(app_err(msg("The library holds at most {n} games.", &[("n", json!(MAX_GAMES))])));
        }
        games.push(json!({"path": key_s, "pinned": false, "added": stamp(self), "opened": "", "summary": {"name": folder_name(&key)}}));
        self.library_write(&games)?;
        self.library_list()
    }

    /// The paths a card may be kept under: the text as given and the resolved one (a card is stored resolved, which on Windows also expands short names such as `RUNNER~1`).
    fn library_keys(path: &str) -> [String; 2] {
        let given = crate::util::py_strip(path).trim_matches('"').to_string();
        let resolved = fsutil::resolve(Path::new(&given)).to_string_lossy().into_owned();
        [given, resolved]
    }

    /// Take a game off Home (the card only: no file of the game is touched).
    pub fn library_remove(&mut self, path: &str) -> Res<Value> {
        let mut games = self.library_read();
        let keys = Self::library_keys(path);
        games.retain(|g| !keys.contains(&py_str(&g["path"])));
        self.library_write(&games)?;
        self.library_list()
    }

    pub fn library_pin(&mut self, path: &str, pinned: bool) -> Res<Value> {
        let mut games = self.library_read();
        let keys = Self::library_keys(path);
        for g in games.iter_mut().filter(|g| keys.contains(&py_str(&g["path"]))) {
            g["pinned"] = json!(pinned);
        }
        self.library_write(&games)?;
        self.library_list()
    }

    /// Called after every save of the derived data of the open project (`persist`): the project is on Home, with its numbers and the time of the last work. A problem here never breaks the work.
    pub fn library_remember(&mut self) {
        let Some(o) = self.open.as_ref() else { return };
        let root = o.project.root.to_string_lossy().into_owned();
        let rows = o.project.rows(&o.store);
        let (total, done, corr): (i64, i64, i64) = rows.iter().fold((0, 0, 0), |a, r| (a.0 + r.total, a.1 + r.done, a.2 + r.corr));
        let game = self.release_game();
        let name = py_str(&game["name"]);
        let summary = json!({"name": if name.is_empty() { folder_name(Path::new(&root)) } else { name }, "version": game["version"], "language": if o.store.layout.language.is_empty() { folder_name(Path::new(&root)) } else { o.store.layout.language.clone() }, "total": total, "done": done, "corr": corr});
        let now = stamp(self);
        let mut games = self.library_read();
        let epoch = |s: &str| chrono::DateTime::parse_from_rfc3339(s).map(|d| d.timestamp()).unwrap_or(0);
        // the card of the game folder that was added by hand is the same game as its translation folder, once that is opened
        let same = |g: &Value| {
            let gp = py_str(&g["path"]);
            gp == root
                || (Path::new(&root).starts_with(&gp)
                    && find_translation_folder(Path::new(&gp)).is_ok_and(|(f, _)| fsutil::resolve(&f).to_string_lossy() == root.as_str()))
        };
        match games.iter_mut().find(|g| same(g)) {
            Some(g) => {
                g["path"] = json!(root);
                let recent = epoch(&now) - epoch(&py_str(&g["opened"])) < TOUCH_EVERY;
                if g["summary"] == summary && recent {
                    return;
                }
                g["summary"] = summary;
                if !recent {
                    g["opened"] = json!(now);
                }
            }
            None => games.push(json!({"path": root, "pinned": false, "added": now, "opened": now, "summary": summary})),
        }
        let _ = self.library_write(&games);
    }
}

fn folder_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.to_string_lossy().into_owned())
}
