//! The routes of the server of Free (`reference/python/free/lexo_free/server.py`), independent of how a request arrives: a loopback socket (`lib.rs`) or the custom protocol of the Tauri window.
//! A request is a method, a target (`/api/entries?limit=5`), the token header and the body; the answer is a `Reply`.
use crate::backend::Backend;
use crate::http::{content_type, parse_query};
use lexo_core::api::{self, Answer};
use lexo_core::app_base::ApiError;
use lexo_core::messages::{msg, py_str, Message, Text};
use serde_json::{json, Map, Value};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

pub const MAX_BODY: usize = 5 * 1024 * 1024;

/// What only the application around the server can do: choose a folder, open one more window, go full screen.
pub trait Shell: Send + Sync {
    /// `None`: there is no dialog. `Some(None)`: the user cancelled. `Some(Some(path))`: the folder.
    fn pick_folder(&self) -> Option<Option<String>> {
        None
    }
    /// Open one more window at `url`.
    fn open_window(&self, _url: &str) -> Result<(), String> {
        Err("not available".into())
    }
    /// Toggle full screen of the window in front; `false` when that is not possible (the page then uses the browser's own way).
    fn toggle_fullscreen(&self) -> bool {
        false
    }
}

/// A shell that can do nothing.
pub struct NoShell;
impl Shell for NoShell {}

/// The line open in the main window; companion windows follow it.
#[derive(Clone, Debug, PartialEq)]
pub struct Focus {
    pub seq: i64,
    pub file: String,
    pub line: i64,
    pub text: Option<String>,
}

/// Per-run state of the server that is not the application's.
pub struct Session {
    pub token: String,
    pub ui_dir: PathBuf,
    /// The address of the page, to build the address of one more window.
    pub base_url: Mutex<String>,
    focus: Mutex<Focus>,
    focus_changed: Condvar,
    /// window id -> time of its last ping (the application quits when the last one is gone).
    windows: Mutex<Vec<(String, Instant)>>,
    /// detail windows -> time of their last ping (the main window slims down while one is open).
    companions: Mutex<Vec<(String, Instant)>>,
    pub last_bye: Mutex<Option<Instant>>,
    pub shell: Box<dyn Shell>,
}

impl Session {
    pub fn new(ui_dir: PathBuf, base_url: String, shell: Box<dyn Shell>) -> Session {
        Session {
            token: lexo_core::fsutil::token_hex(16),
            ui_dir,
            base_url: Mutex::new(base_url),
            focus: Mutex::new(Focus { seq: 0, file: String::new(), line: 0, text: None }),
            focus_changed: Condvar::new(),
            windows: Mutex::new(Vec::new()),
            companions: Mutex::new(Vec::new()),
            last_bye: Mutex::new(None),
            shell,
        }
    }

    /// How many windows have pinged lately (the Tauri shell quits when there are none for a while).
    pub fn open_windows(&self) -> usize {
        self.windows.lock().unwrap().iter().filter(|(_, t)| t.elapsed() < Duration::from_secs(30)).count()
    }
}

/// An answer: status, type, body, and whether the browser may keep it.
pub struct Reply {
    pub code: u16,
    pub ctype: String,
    pub body: Vec<u8>,
    pub cache: bool,
}

impl Reply {
    fn new(code: u16, ctype: &str, body: &[u8]) -> Reply {
        Reply { code, ctype: ctype.to_string(), body: body.to_vec(), cache: false }
    }
    fn json(code: u16, v: &Value) -> Reply {
        Reply::new(code, "application/json; charset=utf-8", &serde_json::to_vec(v).unwrap_or_default())
    }
    fn error(code: u16, m: Message) -> Reply {
        Reply::json(code, &json!({"ok": false, "error": m.wire()}))
    }
    pub fn forbidden() -> Reply {
        Reply::new(403, "text/plain", b"forbidden")
    }
    pub fn not_found() -> Reply {
        Reply::new(404, "text/plain", b"not found")
    }
    pub fn too_large() -> Reply {
        Reply::error(413, msg("The request is too large.", &[]))
    }
    pub fn bad_request() -> Reply {
        Reply::error(400, msg("Invalid request.", &[]))
    }
}

/// An unexpected error goes to `error.log` in the folder of the settings.
fn log_error(text: &str) {
    let dir = lexo_core::config::config_dir();
    if std::fs::create_dir_all(&dir).is_ok() {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open(dir.join("error.log")) {
            let _ = writeln!(f, "---\n{text}\n");
        }
    }
}

/// The routes, on top of a session and the application thread. Cheap to clone.
#[derive(Clone)]
pub struct Dispatcher {
    pub session: Arc<Session>,
    pub backend: Backend,
}

impl Dispatcher {
    /// Answer one request. The transport has checked who may ask (the Host header of the socket) and the size of the body.
    pub fn handle(&self, method: &str, target: &str, token: &str, body: Vec<u8>) -> Reply {
        match method {
            "GET" => self.get(target),
            "POST" => self.post(target, token, body),
            _ => Reply::not_found(),
        }
    }

    fn result(&self, r: Result<Result<Answer, ApiError>, String>) -> Reply {
        match r {
            Err(panic) => {
                log_error(&panic);
                Reply::error(500, msg("Unexpected error: {e}", &[("e", json!(panic))]))
            }
            Ok(Ok(Answer::Json(v))) => Reply::json(200, &if v.is_object() { v } else { json!({"ok": true, "result": v}) }),
            Ok(Ok(Answer::Binary(..))) => Reply::error(500, msg("Unexpected error: {e}", &[("e", json!("a binary answer on a JSON route"))])),
            Ok(Err(e)) => api_error(&e),
        }
    }

    fn static_file(&self, name: &str) -> Reply {
        let dir = &self.session.ui_dir;
        let path = dir.join(name);
        let inside = std::fs::canonicalize(&path).ok().zip(std::fs::canonicalize(dir).ok()).is_some_and(|(p, d)| p.starts_with(d) && p.is_file());
        if !inside {
            return Reply::not_found();
        }
        let Ok(mut data) = std::fs::read(&path) else { return Reply::not_found() };
        if name == "index.html" {
            data = String::from_utf8_lossy(&data).replace("__TOKEN__", &self.session.token).into_bytes();
        }
        Reply::new(200, content_type(name), &data)
    }

    fn get(&self, target: &str) -> Reply {
        let session = &self.session;
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        let q = parse_query(query);
        *session.last_bye.lock().unwrap() = None;
        let get = |k: &str| q.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str()).unwrap_or("");
        if path == "/" || path == "/index.html" {
            return self.static_file("index.html");
        }
        let flat = ["/styles.css", "/crest.svg", "/favicon.svg", "/aside.png", "/selftest.js"];
        let script =
            path.strip_prefix("/js/").is_some_and(|n| n.ends_with(".js") && n.len() > 3 && n.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)));
        if flat.contains(&path) || script {
            return self.static_file(&path[1..]);
        }
        let Some(r) = path.strip_prefix("/api/") else { return Reply::not_found() };
        if r == "focus" {
            // with ?after=N&wait=1 it answers as soon as the focus changes (or after 20 s)
            let after = get("after");
            let mut focus = session.focus.lock().unwrap();
            if get("wait") == "1" && !after.is_empty() && after.trim_start_matches('-').chars().all(|c| c.is_ascii_digit()) {
                let after: i64 = after.parse().unwrap_or(0);
                let deadline = Instant::now() + Duration::from_secs(20);
                while focus.seq <= after {
                    let left = deadline.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        break;
                    }
                    focus = session.focus_changed.wait_timeout(focus, left).unwrap().0;
                }
            }
            let f = focus.clone();
            drop(focus);
            return Reply::json(200, &json!({"ok": true, "seq": f.seq, "file": f.file, "line": f.line, "text": f.text}));
        }
        if r == "ping" {
            let (wid, k) = (get("w"), get("k"));
            if !wid.is_empty() {
                let wid: String = wid.chars().take(40).collect();
                touch(&mut session.windows.lock().unwrap(), &wid);
                if k == "1" {
                    touch(&mut session.companions.lock().unwrap(), &wid);
                }
            }
            let n = session.companions.lock().unwrap().iter().filter(|(_, t)| t.elapsed() < Duration::from_secs(12)).count();
            return Reply::json(200, &json!({"ok": true, "companions": n}));
        }
        if r == "licence" {
            return self.licence("status", json!({}));
        }
        let Some(op) = api::route("GET", r) else { return Reply::error(404, msg("Unknown address.", &[])) };
        let params: Map<String, Value> = q.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
        let (name, binary) = (op.name, op.kind == "binary");
        let params = Value::Object(params);
        let result = self.backend.run(move |app| api::call(app, name, &params));
        if !binary {
            return self.result(result);
        }
        match result {
            Ok(Ok(Answer::Binary(data, mime))) => Reply { code: 200, ctype: mime, body: data, cache: true },
            Ok(Err(e)) if e.is_app_error() => Reply::new(404, "text/plain", e.text().text().as_bytes()),
            Ok(Err(ApiError::Missing(_))) | Ok(Ok(_)) => Reply::not_found(),
            Ok(Err(_)) | Err(_) => Reply::new(500, "text/plain", "chyba".as_bytes()),
        }
    }

    fn post(&self, target: &str, token: &str, body: Vec<u8>) -> Reply {
        let session = &self.session;
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        if path == "/api/bye" {
            // sendBeacon cannot set headers
            let q = parse_query(query);
            let get = |k: &str| q.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()).unwrap_or_default();
            if get("t") == session.token {
                let wid: String = get("w").chars().take(40).collect();
                session.windows.lock().unwrap().retain(|(k, _)| *k != wid);
                session.companions.lock().unwrap().retain(|(k, _)| *k != wid);
                let mut windows = session.windows.lock().unwrap();
                windows.retain(|(_, t)| t.elapsed() <= Duration::from_secs(30)); // windows that stopped pinging
                if windows.is_empty() {
                    *session.last_bye.lock().unwrap() = Some(Instant::now());
                }
            }
            return Reply::new(200, "text/plain", b"ok");
        }
        if token != session.token {
            return Reply::error(403, msg("Forbidden (invalid token).", &[]));
        }
        let text = if body.is_empty() { Some("{}".to_string()) } else { String::from_utf8(body).ok() };
        let Some(parsed) = text.and_then(|t| serde_json::from_str::<Value>(&t).ok()) else { return Reply::bad_request() };
        let d: Map<String, Value> = parsed.as_object().cloned().unwrap_or_default();
        let r = path.strip_prefix("/api/").unwrap_or("");
        if let Some(op) = api::route("POST", r) {
            let (name, params) = (op.name, Value::Object(d));
            return self.result(self.backend.run(move |app| api::call(app, name, &params)));
        }
        // what only the application around Core can do: windows, the focus between windows, a folder dialog
        match r {
            "project/pick" => self.project_pick(),
            "window/open" => self.open_window(&d),
            "window/fullscreen" => {
                Reply::json(200, &if session.shell.toggle_fullscreen() { json!({"ok": true}) } else { json!({"ok": false, "fallback": true}) })
            }
            "licence/activate"
            | "licence/refresh"
            | "licence/deactivate"
            | "licence/devices"
            | "licence/release"
            | "licence/rename"
            | "licence/recover"
            | "licence/recover-confirm" => self.licence(r.strip_prefix("licence/").unwrap_or(""), Value::Object(d)),
            "carry/pick" => match self.session.shell.pick_folder() {
                None => Reply::error(400, msg("Choosing a folder is not available in this mode – enter the path by hand.", &[])),
                Some(None) => Reply::json(200, &json!({"ok": true, "cancelled": true})),
                Some(Some(path)) => Reply::json(200, &json!({"ok": true, "cancelled": false, "path": path})),
            },
            "carry/preview" | "carry/apply" => {
                let (apply, d) = (r == "carry/apply", Value::Object(d));
                self.result(self.backend.run(move |app| -> Result<Answer, ApiError> {
                    let (path, ratio) = (d["old"].as_str().unwrap_or("").to_string(), d["min_ratio"].as_f64().unwrap_or(0.8));
                    let out = if apply {
                        app.carry_apply(&path, ratio, d["selections"].as_array().map_or(&[][..], |v| v.as_slice()), d["mark"].as_bool().unwrap_or(true))?
                    } else {
                        app.carry_preview(&path, ratio)?
                    };
                    Ok(Answer::Json(out))
                }))
            }
            "release/state" | "release/settings" | "release/preview" | "release/build" => {
                let (what, d) = (r.to_string(), Value::Object(d));
                self.result(self.backend.run(move |app| -> Result<Answer, ApiError> {
                    let text = |k: &str| d[k].as_str().unwrap_or("").to_string();
                    let out = match what.as_str() {
                        "release/state" => app.release_state()?,
                        "release/settings" => app.release_set(&d["settings"])?,
                        "release/preview" => app.release_preview(&text("version"), &text("notes"))?,
                        _ => app.release_build(&text("version"), &text("notes"), d["force"].as_bool().unwrap_or(false))?,
                    };
                    Ok(Answer::Json(out))
                }))
            }
            "style/targets" | "style/batch" => {
                let (batch, d) = (r == "style/batch", Value::Object(d));
                self.result(self.backend.run(move |app| -> Result<Answer, ApiError> {
                    let out = if batch { app.style_batch(&d["items"])? } else { app.style_targets(d["file"].as_str().unwrap_or(""), &d["lines"])? };
                    Ok(Answer::Json(out))
                }))
            }
            "spell/state" | "spell/add" | "spell/remove" | "spell/ignore" | "spell/check" | "spell/replace" => {
                let (route, d) = (r.to_string(), Value::Object(d));
                self.result(self.backend.run(move |app| -> Result<Answer, ApiError> {
                    let text = |k: &str| d[k].as_str().unwrap_or("").to_string();
                    let out = match route.as_str() {
                        "spell/state" => app.spell_state()?,
                        "spell/add" => app.spell_dict_add(&text("path"))?,
                        "spell/remove" => app.spell_dict_remove()?,
                        "spell/ignore" => app.spell_ignore(&d["words"])?,
                        "spell/check" => app.spell_check(&text("file"), &d["lines"])?,
                        _ => app.spell_replace_plan(&text("word"), &text("to"))?,
                    };
                    let mut out = out;
                    if let Some(o) = out.as_object_mut() {
                        o.entry("ok").or_insert(json!(true)); // the UI treats an answer without `ok` as a failure
                    }
                    Ok(Answer::Json(out))
                }))
            }
            "profile/list" | "profile/set" => {
                let (set, d) = (r == "profile/set", Value::Object(d));
                self.result(self.backend.run(move |app| -> Result<Answer, ApiError> {
                    let out = if set {
                        let text = |k: &str| d[k].as_str().unwrap_or("").to_string();
                        app.profile_set(&text("code"), &text("gender"), &text("style"), &d["to"])?
                    } else {
                        app.profiles()?
                    };
                    Ok(Answer::Json(out))
                }))
            }
            "memory/fill-preview" | "memory/fill-apply" => {
                let (apply, d) = (r == "memory/fill-apply", Value::Object(d));
                self.result(self.backend.run(move |app| -> Result<Answer, ApiError> {
                    let ratio = d["min_ratio"].as_f64().unwrap_or(0.85);
                    let out = if apply {
                        app.fill_apply(ratio, d["selections"].as_array().map_or(&[][..], |v| v.as_slice()), d["mark"].as_bool().unwrap_or(true))?
                    } else {
                        app.fill_preview(ratio)?
                    };
                    Ok(Answer::Json(out))
                }))
            }
            "focus" => {
                if !self.backend.run(|app| app.ready()).unwrap_or(false) {
                    return Reply::error(400, msg("No project is open.", &[]));
                }
                self.set_focus(&d)
            }
            _ => Reply::error(404, msg("Unknown address.", &[])),
        }
    }

    /// The licence of the Pro pack (state, key, regular check, freeing this computer); without the pack the state is `none`. These calls are not in the contract of Core: they belong to the
    /// application around it, like the folder dialog.
    fn licence(&self, method: &str, payload: Value) -> Reply {
        let method = method.to_string();
        let result = self.backend.run(move |app| -> Result<Answer, ApiError> {
            if !app.ext.present() {
                return Ok(Answer::Json(json!({"ok": true, "state": "none"})));
            }
            let mut answer = app.ext.licence(&method, &payload).map_err(|e| ApiError::App(e.0))?;
            if let Some(o) = answer.as_object_mut() {
                o.insert("ok".into(), json!(true));
            }
            Ok(Answer::Json(answer))
        });
        self.result(result)
    }

    fn project_pick(&self) -> Reply {
        let Some(chosen) = self.session.shell.pick_folder() else {
            return Reply::error(400, msg("Choosing a folder is not available in this mode – enter the path by hand.", &[]));
        };
        let result = self.backend.run(move |app| -> Result<Answer, ApiError> {
            let Some(chosen) = chosen else { return Ok(Answer::Json(json!({"ok": true, "cancelled": true}))) };
            let (ok, problem) = lexo_core::project::check_project_dir(&chosen);
            if !ok {
                return Err(ApiError::App(Text::new_msg(problem.unwrap_or_else(|| msg("The folder cannot be used.", &[])))));
            }
            app.open_project(&chosen, false)?;
            Ok(Answer::Json(json!({"ok": true, "cancelled": false, "path": chosen})))
        });
        self.result(result)
    }

    fn set_focus(&self, d: &Map<String, Value>) -> Reply {
        let line = d.get("line").map(py_str).unwrap_or_default();
        let line = if !line.is_empty() && line.chars().all(|c| c.is_ascii_digit()) { line.parse().unwrap_or(0) } else { 0 };
        let mut focus = self.session.focus.lock().unwrap();
        focus.seq += 1;
        focus.file = d.get("file").map(py_str).unwrap_or_default().chars().take(500).collect();
        focus.line = line;
        focus.text = d.get("text").and_then(Value::as_str).map(|t| t.chars().take(20000).collect()); // what is being typed in the main window
        let f = focus.clone();
        self.session.focus_changed.notify_all();
        drop(focus);
        Reply::json(200, &json!({"ok": true, "seq": f.seq, "file": f.file, "line": f.line, "text": f.text}))
    }

    /// One more app window with its own list / filter state, e.g. to read the scene next to the editor.
    fn open_window(&self, d: &Map<String, Value>) -> Reply {
        let text = |k: &str| d.get(k).map(py_str).unwrap_or_default();
        let mut q: Vec<(String, String)> = vec![("win".into(), "2".into())];
        if text("companion") == "1" {
            q.push(("companion".into(), "1".into()));
        }
        if ["check", "proof", "scene", "memory", "history", "script"].contains(&text("tab").as_str()) {
            q.push(("tab".into(), text("tab")));
        }
        if ["overview", "files", "entries", "validation", "glossary", "publish", "settings"].contains(&text("view").as_str()) {
            q.push(("view".into(), text("view")));
        }
        if d.get("file").is_some_and(lexo_core::util::truthy) {
            q.push(("file".into(), text("file").chars().take(500).collect()));
        }
        if !text("line").is_empty() && text("line").chars().all(|c| c.is_ascii_digit()) {
            q.push(("line".into(), text("line").parse::<u64>().unwrap_or(0).to_string()));
        }
        if ["all", "todo", "translated", "approved", "review", "proofed", "fix", "stale"].contains(&text("status").as_str()) {
            q.push(("status".into(), text("status")));
        }
        let query = q.iter().map(|(k, v)| format!("{}={}", lexo_core::util::url_quote(k), lexo_core::util::url_quote(v))).collect::<Vec<_>>().join("&");
        let url = format!("{}?{query}", self.session.base_url.lock().unwrap());
        match self.session.shell.open_window(&url) {
            Ok(()) => Reply::json(200, &json!({"ok": true, "url": url})),
            Err(e) => {
                let m = if e == "not available" {
                    msg("A new window cannot be opened from the application here.", &[])
                } else {
                    msg("The window could not be opened ({e}).", &[("e", json!(e))])
                };
                Reply::json(200, &json!({"ok": false, "fallback": true, "error": m.wire()}))
            }
        }
    }
}

fn touch(list: &mut Vec<(String, Instant)>, id: &str) {
    match list.iter_mut().find(|(k, _)| k == id) {
        Some(e) => e.1 = Instant::now(),
        None => list.push((id.to_string(), Instant::now())),
    }
}

/// An error of an operation as the transport reports it.
pub fn api_error(e: &ApiError) -> Reply {
    match e {
        ApiError::Busy { text, path } => Reply::json(409, &json!({"ok": false, "error": text.wire(), "locked": true, "path": path})),
        ApiError::App(_) | ApiError::Locked(_) => Reply::json(400, &json!({"ok": false, "error": e.text().wire()})),
        ApiError::Missing(k) | ApiError::Unknown(k) => Reply::error(400, msg("Missing value: {e}", &[("e", json!(format!("'{k}'")))])),
        ApiError::Write(w) => Reply::error(
            409,
            msg(
                "Cannot save: {reason} ({name})",
                &[("reason", w.reason.wire()), ("name", json!(w.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()))],
            ),
        ),
    }
}
