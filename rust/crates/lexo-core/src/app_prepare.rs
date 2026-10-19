//! Preparing a game for translating (the guide on Home): from a built game to a translation folder the application opens. The steps are those of `gametools.rs`, and the game is only read:
//! 1. a working copy of the game (a workspace in the settings of the application: hard links, so it costs almost no space), 2. the scripts inside the `.rpa` archives are unpacked,
//! 3. scripts that have only bytecode (`.rpyc`) are decompiled with **unrpyc** (a helper the user points at once; only when it is needed), 4. the game's own engine writes `game/tl/<language>`
//!
//! (`translate`). The work takes minutes, so it runs as a job on its own thread: `prepare/start` returns at once, `prepare/status` says how far it is. These calls are not in the contract of Core.
use crate::app::App;
use crate::app_base::{app_err, Res};
use crate::diskspace;
use crate::fsutil::resolve;
use crate::gametools::{self, below, find_archives, rglob_suffix, ToolError};
use crate::lint::{self, Platform};
use crate::messages::{msg, py_str, Message};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// The program that is run for a step (unrpyc, the engine). A plain function, so that a job can carry it to its own thread and a test can put a fake in its place.
pub type RunFn = fn(&[String], &Path) -> Result<(i32, String), ToolError>;

#[derive(Clone, Default)]
struct Job {
    current: String,
    done_steps: Vec<String>,
    done: bool,
    ok: bool,
    error: Value,
    result: Value,
}

fn jobs() -> &'static Mutex<BTreeMap<String, Job>> {
    static J: OnceLock<Mutex<BTreeMap<String, Job>>> = OnceLock::new();
    J.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn with_job(id: &str, f: impl FnOnce(&mut Job)) {
    if let Some(j) = jobs().lock().unwrap_or_else(|e| e.into_inner()).get_mut(id) {
        f(j);
    }
}

/// The folder that holds `game/`: the given folder, or the parent of a folder called `game`.
fn game_root_of(p: &Path) -> Option<PathBuf> {
    if p.join("game").is_dir() {
        return Some(resolve(p));
    }
    if p.is_dir() && p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("game")) {
        return p.parent().map(resolve);
    }
    None
}

fn valid_language(l: &str) -> bool {
    let mut cs = l.chars();
    cs.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && cs.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Scripts below `game` that are not translations: (sources `.rpy`, bytecode `.rpyc` that has no `.rpy` beside it).
fn scripts(game: &Path) -> (usize, usize) {
    let tl = game.join("tl");
    let outside = |p: &PathBuf| !below(p, &tl);
    let rpy: Vec<PathBuf> = rglob_suffix(game, &[".rpy"]).into_iter().filter(outside).collect();
    let rpyc_only = rglob_suffix(game, &[".rpyc"]).into_iter().filter(outside).filter(|p| !p.with_extension("rpy").is_file()).count();
    (rpy.len(), rpyc_only)
}

fn unrpyc_in(dir: &Path) -> Option<PathBuf> {
    let direct = dir.join("unrpyc.py");
    if direct.is_file() {
        return Some(direct);
    }
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase().starts_with("unrpyc")))
        .map(|p| p.join("unrpyc.py"))
        .find(|p| p.is_file())
}

impl App {
    fn prepare_settings_file(&self) -> PathBuf {
        self.cfg_dir.join("prepare.json")
    }

    /// The unrpyc the user chose before, or one that lies where it is likely to be (beside the game, in Downloads, on the Desktop).
    /// unrpyc comes with the application (`tools/unrpyc/v1` for Python 2.7, i.e. Ren'Py 7 and older, and `v2` for Python 3.9+, Ren'Py 8): the one that fits the Python of the game.
    fn bundled_unrpyc(&self, python: &Path) -> Option<PathBuf> {
        let py3 = python.components().any(|c| c.as_os_str().to_string_lossy().starts_with("py3-"));
        let tools = self.packs.bundled.parent()?.join("tools").join("unrpyc");
        let script = tools.join(if py3 { "v2" } else { "v1" }).join("unrpyc.py");
        script.is_file().then_some(script)
    }

    /// Which unrpyc, and where it comes from: `chosen` (the user pointed at one), `bundled` (the one of the application that fits the game's Python) or `found` (one that lies beside the game,
    /// in Downloads or on the Desktop).
    fn prepare_unrpyc(&self, root: &Path) -> Option<(PathBuf, &'static str)> {
        let text = std::fs::read_to_string(self.prepare_settings_file()).unwrap_or_default();
        let saved =
            serde_json::from_str::<Value>(&text).ok().map(|v| py_str(&v["unrpyc"])).filter(|p| !p.is_empty()).map(PathBuf::from).filter(|p| p.is_file());
        if let Some(p) = saved {
            return Some((p, "chosen"));
        }
        if let Some(b) = lint::find_python(root, Platform::current()).and_then(|py| self.bundled_unrpyc(&py)) {
            return Some((b, "bundled"));
        }
        let home = ["USERPROFILE", "HOME"].into_iter().find_map(std::env::var_os).map(PathBuf::from);
        let mut places: Vec<PathBuf> = vec![root.to_path_buf()];
        places.extend(root.parent().map(Path::to_path_buf));
        if let Some(h) = home {
            places.extend([h.join("Downloads"), h.join("Desktop")]);
        }
        places.iter().find_map(|d| unrpyc_in(d)).map(|p| (p, "found"))
    }

    /// Remember where `unrpyc.py` is (the file, or the folder that holds it).
    pub fn prepare_set_unrpyc(&mut self, path: &str) -> Res<Value> {
        let p = PathBuf::from(crate::util::py_strip(path).trim_matches('"'));
        let file = if p.is_dir() { unrpyc_in(&p) } else { Some(p.clone()).filter(|f| f.is_file()) };
        let Some(file) = file.filter(|f| f.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("unrpyc.py"))) else {
            return Err(app_err(msg("unrpyc.py was not found there: {path}", &[("path", json!(path))])));
        };
        let io = |e: std::io::Error| app_err(msg("The setting could not be saved: {reason}", &[("reason", json!(e.to_string()))]));
        std::fs::create_dir_all(&self.cfg_dir).map_err(io)?;
        std::fs::write(self.prepare_settings_file(), serde_json::to_string_pretty(&json!({"unrpyc": file.to_string_lossy()})).unwrap_or_default())
            .map_err(io)?;
        Ok(json!({"ok": true, "unrpyc": file.to_string_lossy()}))
    }

    /// What the guide shows before it starts: what is in the game and what is missing. `ready` says that it can start.
    pub fn prepare_inspect(&self, path: &str) -> Res<Value> {
        let p = PathBuf::from(crate::util::py_strip(path).trim_matches('"'));
        let Some(root) = game_root_of(&p) else {
            return Err(app_err(msg("There is no game folder in this folder: {path}", &[("path", json!(path))])));
        };
        let game = root.join("game");
        let archives = find_archives(&game).len();
        let (rpy, rpyc_only) = scripts(&game);
        let translations: Vec<String> = std::fs::read_dir(game.join("tl"))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        let python = lint::find_python(&root, Platform::current());
        let launcher = lint::find_launcher(&root);
        let unrpyc = self.prepare_unrpyc(&root);
        let needs_unrpyc = rpyc_only > 0 || archives > 0;
        let mut problems: Vec<Message> = Vec::new();
        if python.is_none() || launcher.is_none() {
            problems.push(msg(
                "The game has no engine of its own (no program of the game with its .py file and a lib folder beside it), so it cannot be prepared here.",
                &[],
            ));
        }
        if needs_unrpyc && unrpyc.is_none() {
            problems.push(msg("unrpyc is needed to read the scripts of this game: choose the file unrpyc.py.", &[]));
        }
        let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        Ok(json!({
            "ok": true, "root": root.to_string_lossy(), "name": name, "archives": archives, "scripts": rpy, "bytecode_only": rpyc_only, "translations": translations,
            "engine": python.is_some() && launcher.is_some(), "unrpyc": unrpyc.as_ref().map(|u| u.0.to_string_lossy().into_owned()), "unrpyc_source": unrpyc.as_ref().map(|u| u.1),
            "unrpyc_version": unrpyc.as_ref().filter(|u| u.1 == "bundled").and_then(|u| u.0.parent()?.file_name().map(|n| n.to_string_lossy().into_owned())), "needs_unrpyc": needs_unrpyc,
            "problems": problems.iter().map(Message::wire).collect::<Vec<_>>(), "ready": problems.is_empty(),
            "default_workspace": self.prepare_default_workspace(&root).to_string_lossy(),
        }))
    }

    /// What the chosen way costs: `{mode, workspace, links, need, free, enough}`. `mode` is `copy` (a working copy, in `workspace` or beside the game) or `inplace` (no copy: the game is
    /// worked on where it is). A copy is linked when the disks are the same, so it takes almost no room; on another disk everything is copied.
    pub fn prepare_plan(&self, path: &str, mode: &str, workspace: &str) -> Res<Value> {
        let p = PathBuf::from(crate::util::py_strip(path).trim_matches('"'));
        let Some(root) = game_root_of(&p) else {
            return Err(app_err(msg("There is no game folder in this folder: {path}", &[("path", json!(path))])));
        };
        if mode == "inplace" {
            return Ok(
                json!({"ok": true, "mode": "inplace", "workspace": root.to_string_lossy(), "links": false, "need": 0, "free": diskspace::free_bytes(&root), "enough": true}),
            );
        }
        let ws = self.prepare_target(&root, workspace)?;
        let (links, need) = gametools::workspace_cost(&root, &ws);
        let free = diskspace::free_bytes(&ws);
        // a margin: the engine writes its own files (caches, the translation) too
        let enough = free.is_none_or(|f| f > need.saturating_add(need / 10).saturating_add(200 * 1024 * 1024));
        Ok(json!({"ok": true, "mode": "copy", "workspace": ws.to_string_lossy(), "links": links, "need": need, "free": free, "enough": enough}))
    }

    /// The folder of a working copy: the one the user chose, or the default; never inside the game (and not a file).
    fn prepare_target(&self, root: &Path, chosen: &str) -> Res<PathBuf> {
        let chosen = crate::util::py_strip(chosen).trim_matches('"').to_string();
        let ws = if chosen.is_empty() { self.prepare_default_workspace(root) } else { resolve(Path::new(&chosen)) };
        if ws == root || ws.starts_with(root) || root.starts_with(&ws) {
            return Err(app_err(msg("The working copy must not be inside the game folder.", &[])));
        }
        if ws.is_file() {
            return Err(app_err(msg("The working copy cannot go there: {path} is a file.", &[("path", json!(ws.to_string_lossy()))])));
        }
        Ok(ws)
    }

    /// Where the working copy goes when the user does not choose: **beside the game** (`<game>-aside`), on the same disk, so that its files can be linked instead of copied. When the folder
    /// of the game cannot hold it, the settings folder of the application.
    fn prepare_default_workspace(&self, root: &Path) -> PathBuf {
        let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "game".into());
        match root.parent() {
            Some(parent) if parent.is_dir() => parent.join(format!("{name}-aside")),
            _ => self.prepare_workspace(root),
        }
    }

    fn prepare_workspace(&self, root: &Path) -> PathBuf {
        let name: String = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        let id = crate::util::sha1_hex(root.to_string_lossy().as_bytes());
        self.cfg_dir.join("workspaces").join(format!("{}-{}", if name.is_empty() { "game" } else { &name }, &id[..6]))
    }

    /// Start the preparation as a job: `{job}`.
    pub fn prepare_start(&mut self, path: &str, language: &str, mode: &str, workspace: &str) -> Res<Value> {
        self.prepare_start_with(path, language, mode, workspace, gametools::run_process)
    }

    /// The same with the program runner chosen by the caller (tests put a fake in its place).
    pub fn prepare_start_with(&mut self, path: &str, language: &str, mode: &str, workspace: &str, runner: RunFn) -> Res<Value> {
        if !valid_language(language) {
            return Err(app_err(msg("The language name may contain only letters, digits and an underscore and must not start with a digit.", &[])));
        }
        let info = self.prepare_inspect(path)?;
        if !info["ready"].as_bool().unwrap_or(false) {
            // the same checks as `prepare_inspect`, as a message to refuse with
            let root = game_root_of(Path::new(&py_str(&info["root"]))).unwrap_or_default();
            let refusal = if lint::find_python(&root, Platform::current()).is_none() || lint::find_launcher(&root).is_none() {
                msg(
                    "The game has no engine of its own (no program of the game with its .py file and a lib folder beside it), so it cannot be prepared here.",
                    &[],
                )
            } else {
                msg("unrpyc is needed to read the scripts of this game: choose the file unrpyc.py.", &[])
            };
            return Err(app_err(refusal));
        }
        let root = PathBuf::from(py_str(&info["root"]));
        let inplace = mode == "inplace";
        let workspace = if inplace { root.clone() } else { self.prepare_target(&root, workspace)? };
        if !inplace {
            let plan = self.prepare_plan(path, "copy", &workspace.to_string_lossy())?;
            if !plan["enough"].as_bool().unwrap_or(true) {
                return Err(app_err(msg(
                    "There is not enough room on that disk for the working copy: it needs about {need} and {free} is free. Choose a folder on a disk with more room, or work in the game itself (without a copy).",
                    &[("need", json!(human(plan["need"].as_u64().unwrap_or(0)))), ("free", json!(human(plan["free"].as_u64().unwrap_or(0))))],
                )));
            }
        }
        let python = lint::find_python(&root, Platform::current()).unwrap_or_default();
        let unrpyc = info["unrpyc"].as_str().map(PathBuf::from);
        let id = format!("{}-{}", crate::fsutil::token_hex(4), crate::util::sha1_hex(language.as_bytes()).get(..4).unwrap_or(""));
        {
            let mut all = jobs().lock().unwrap_or_else(|e| e.into_inner());
            if all.values().any(|j| !j.done) {
                return Err(app_err(msg("A game is being prepared; wait until it is done.", &[])));
            }
            all.insert(id.clone(), Job { current: "workspace".into(), ..Job::default() });
        }
        let language = language.to_string();
        let job = id.clone();
        std::thread::spawn(move || run_job(&job, &root, &workspace, &language, &python, unrpyc.as_deref(), inplace, runner));
        Ok(json!({"ok": true, "job": id}))
    }

    /// How far the job is: `{current, steps, done, ok, error, result}` (the steps are `workspace`, `extract`, `decompile`, `generate`).
    pub fn prepare_status(&self, job: &str) -> Res<Value> {
        let all = jobs().lock().unwrap_or_else(|e| e.into_inner());
        let Some(j) = all.get(job) else {
            return Err(app_err(msg("This preparation is not known (the application was restarted).", &[])));
        };
        Ok(json!({"ok": true, "current": j.current, "steps": j.done_steps, "done": j.done, "success": j.ok, "error": j.error, "result": j.result}))
    }
}

fn tool_err(e: ToolError) -> Value {
    e.0.wire()
}

/// The whole way, on the thread of the job. The game is only read; everything is written in `workspace`.
#[allow(clippy::too_many_arguments)]
fn run_job(id: &str, root: &Path, workspace: &Path, language: &str, python: &Path, unrpyc: Option<&Path>, inplace: bool, runner: RunFn) {
    let step = |name: &str| {
        let name = name.to_string();
        with_job(id, move |j| {
            if !j.current.is_empty() && j.current != name {
                j.done_steps.push(j.current.clone());
            }
            j.current = name;
        })
    };
    let fail = |e: Value| {
        with_job(id, move |j| {
            j.done = true;
            j.ok = false;
            j.error = e;
        })
    };
    let run = |argv: &[String], cwd: &Path| runner(argv, cwd);
    let tl = workspace.join("game").join("tl").join(language);
    if tl.is_dir() && !rglob_suffix(&tl, &[".rpy"]).is_empty() {
        // prepared before: the translation is there, nothing is generated again (it would be refused: a language folder is never overwritten)
        with_job(id, |j| {
            j.done = true;
            j.ok = true;
            j.result = json!({"folder": tl.to_string_lossy(), "reused": true});
        });
        return;
    }
    step("workspace");
    if !inplace {
        // (in the game itself there is nothing to copy)
        if let Err(e) = gametools::make_workspace(root, workspace) {
            return fail(tool_err(e));
        }
    }
    step("extract");
    if let Err(e) = gametools::extract_scripts(workspace) {
        return fail(tool_err(e));
    }
    let game = workspace.join("game");
    // scripts that have only bytecode need unrpyc (the archives may have just brought some)
    let (_, rpyc_only) = scripts(&game);
    if rpyc_only > 0 {
        step("decompile");
        let Some(unrpyc) = unrpyc else {
            return fail(msg("unrpyc is needed to read the scripts of this game: choose the file unrpyc.py.", &[]).wire());
        };
        match gametools::decompile(python, unrpyc, &game, false, true, false, &run) {
            Ok(d) if d.ok => {
                gametools::repair_decompiled(&game, &d.written);
            }
            Ok(d) => {
                return fail(
                    msg(
                        "unrpyc did not finish: {output}",
                        &[("output", json!(d.raw.chars().rev().take(500).collect::<String>().chars().rev().collect::<String>()))],
                    )
                    .wire(),
                )
            }
            Err(e) => return fail(tool_err(e)),
        }
    }
    step("generate");
    let mut result = None;
    let mut quarantined: Vec<String> = Vec::new();
    for _attempt in 0..3 {
        let g = match gametools::generate_translation(workspace, workspace, language, true, &run, Platform::current()) {
            Ok(g) => g,
            Err(e) => return fail(tool_err(e)),
        };
        let (ok, raw) = (g.ok, g.raw.clone());
        result = Some(g);
        if ok {
            break;
        }
        let moved = gametools::quarantine_unparsable(&game, &raw, &[&format!("tl/{language}/")]);
        if moved.is_empty() {
            break;
        }
        quarantined.extend(moved);
        let tl = game.join("tl").join(language);
        if tl.is_dir() && rglob_suffix(&tl, &[".rpy"]).is_empty() {
            let _ = std::fs::remove_dir_all(&tl);
        }
    }
    match result {
        Some(g) if g.ok => with_job(id, move |j| {
            j.done_steps.push("generate".into());
            j.current = String::new();
            j.done = true;
            j.ok = true;
            j.result = json!({"folder": g.folder, "files": g.files.len(), "seconds": g.seconds, "quarantined": quarantined, "reused": false});
        }),
        Some(g) => fail(
            msg(
                "The game did not write the translation: {output}",
                &[("output", json!(g.raw.chars().rev().take(500).collect::<String>().chars().rev().collect::<String>()))],
            )
            .wire(),
        ),
        None => fail(msg("The game did not write the translation.", &[]).wire()),
    }
}

/// 1.5 GB, 320 MB: a size for a message.
fn human(bytes: u64) -> String {
    let (kb, mb, gb) = (1024f64, 1024f64 * 1024.0, 1024f64 * 1024.0 * 1024.0);
    let b = bytes as f64;
    if b >= gb {
        format!("{:.1} GB", b / gb)
    } else if b >= mb {
        format!("{:.0} MB", b / mb)
    } else {
        format!("{:.0} kB", b / kb)
    }
}
