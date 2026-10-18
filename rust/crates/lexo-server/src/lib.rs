//! The local HTTP server of Lexo (milestone M7): the UI files and the same routes as `free/lexo_free/server.py`, on top of `lexo-core`.
//!
//! Only the loopback interface is bound, the `Host` header is checked (DNS-rebinding protection) and every POST needs the per-run token that the page receives from the server.
//! The application (`App`) is not `Send`, so it lives on its own thread and the connection threads hand it jobs; one job runs at a time, like the lock of the Python server.
pub mod http;

use http::{content_type, parse_query, Request};
use lexo_core::api::{self, Answer};
use lexo_core::app::App;
use lexo_core::app_base::ApiError;
use lexo_core::messages::{msg, Text};
use serde_json::{json, Map, Value};
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

const MAX_BODY: usize = 5 * 1024 * 1024;

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
    pub base_url: Mutex<String>,
    focus: Mutex<Focus>,
    focus_changed: Condvar,
    /// window id -> time of its last ping (the application quits when the last one is gone).
    windows: Mutex<Vec<(String, Instant)>>,
    /// detail windows -> time of their last ping (the main window slims down while one is open).
    companions: Mutex<Vec<(String, Instant)>>,
    pub last_bye: Mutex<Option<Instant>>,
    /// A native folder dialog, when the shell around the server has one.
    pub pick_folder: Option<Box<dyn Fn() -> Option<String> + Send + Sync>>,
}

type Job = Box<dyn FnOnce(&mut App) + Send>;

/// The way to the thread that owns the application.
#[derive(Clone)]
struct AppHandle {
    tx: Sender<Job>,
}

impl AppHandle {
    /// Run `f` on the application thread and wait for its result; `Err` carries the text of a panic.
    fn run<R: Send + 'static>(&self, f: impl FnOnce(&mut App) -> R + Send + 'static) -> Result<R, String> {
        let (rtx, rrx) = channel();
        let job: Job = Box::new(move |app| {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(app)));
            let _ = rtx.send(r.map_err(|p| {
                p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_else(|| "panic".to_string())
            }));
        });
        self.tx.send(job).map_err(|_| "the application thread is gone".to_string())?;
        rrx.recv().map_err(|_| "the application thread is gone".to_string())?
    }
}

pub struct ServerOptions {
    pub ui_dir: PathBuf,
    pub port: u16,
    pub pick_folder: Option<Box<dyn Fn() -> Option<String> + Send + Sync>>,
}

pub struct RunningServer {
    pub addr: SocketAddr,
    pub session: Arc<Session>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    handle: AppHandle,
}

impl RunningServer {
    pub fn url(&self) -> String {
        self.session.base_url.lock().unwrap().clone()
    }

    /// Run `f` on the application (for the code that starts the server: open a project, change a setting ...).
    pub fn with_app<R: Send + 'static>(&self, f: impl FnOnce(&mut App) -> R + Send + 'static) -> Result<R, String> {
        self.handle.run(f)
    }

    /// Stop accepting connections and close the project. The threads end with the process.
    pub fn shutdown(self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = std::net::TcpStream::connect(self.addr); // wakes the accept loop
        let _ = self.handle.run(|app| app.close());
    }
}

/// Start the server. `make_app` builds the application on its own thread.
pub fn start(make_app: impl FnOnce() -> App + Send + 'static, options: ServerOptions) -> std::io::Result<RunningServer> {
    let listener = TcpListener::bind(("127.0.0.1", options.port))?;
    let addr = listener.local_addr()?;
    let (tx, rx) = channel::<Job>();
    std::thread::spawn(move || {
        let mut app = make_app();
        while let Ok(job) = rx.recv() {
            job(&mut app);
        }
        app.close();
    });
    let session = Arc::new(Session {
        token: lexo_core::fsutil::token_hex(16),
        ui_dir: options.ui_dir,
        base_url: Mutex::new(format!("http://127.0.0.1:{}/", addr.port())),
        focus: Mutex::new(Focus { seq: 0, file: String::new(), line: 0, text: None }),
        focus_changed: Condvar::new(),
        windows: Mutex::new(Vec::new()),
        companions: Mutex::new(Vec::new()),
        last_bye: Mutex::new(None),
        pick_folder: options.pick_folder,
    });
    let handle = AppHandle { tx };
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let (session, handle, stop) = (session.clone(), handle.clone(), stop.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                if stop.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
                let Ok(stream) = stream else { continue };
                let (session, handle) = (session.clone(), handle.clone());
                std::thread::spawn(move || {
                    if let Some(req) = Request::read(stream) {
                        serve(req, &session, &handle);
                    }
                });
            }
        });
    }
    Ok(RunningServer { addr, session, stop, handle })
}

// ---------------------------------------------------------------- answering
fn json_body(v: &Value) -> Vec<u8> {
    serde_json::to_vec(v).unwrap_or_default()
}

fn send_json(req: &mut Request, code: u16, v: &Value) {
    req.respond(code, "application/json; charset=utf-8", &json_body(v), false);
}

fn error(req: &mut Request, code: u16, m: lexo_core::messages::Message) {
    send_json(req, code, &json!({"ok": false, "error": m.wire()}));
}

fn host_ok(req: &Request) -> bool {
    let host = req.header("host").split(':').next().unwrap_or("").to_lowercase();
    ["127.0.0.1", "localhost", "[::1]"].contains(&host.as_str())
}

/// The answer of an operation, or the error as the transport reports it.
fn send_result(req: &mut Request, r: Result<Result<Answer, ApiError>, String>) {
    match r {
        Err(panic) => {
            log_error(&panic);
            error(req, 500, msg("Unexpected error: {e}", &[("e", json!(panic))]));
        }
        Ok(Ok(Answer::Json(v))) => send_json(req, 200, &if v.is_object() { v } else { json!({"ok": true, "result": v}) }),
        Ok(Ok(Answer::Binary(..))) => error(req, 500, msg("Unexpected error: {e}", &[("e", json!("a binary answer on a JSON route"))])),
        Ok(Err(e)) => send_api_error(req, &e),
    }
}

fn send_api_error(req: &mut Request, e: &ApiError) {
    match e {
        ApiError::Busy { text, path } => send_json(req, 409, &json!({"ok": false, "error": text.wire(), "locked": true, "path": path})),
        ApiError::App(_) | ApiError::Locked(_) => send_json(req, 400, &json!({"ok": false, "error": e.text().wire()})),
        ApiError::Missing(k) | ApiError::Unknown(k) => error(req, 400, msg("Missing value: {e}", &[("e", json!(format!("'{k}'")))])),
        ApiError::Write(w) => error(
            req,
            409,
            msg(
                "Cannot save: {reason} ({name})",
                &[("reason", w.reason.wire()), ("name", json!(w.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()))],
            ),
        ),
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

fn static_file(req: &mut Request, session: &Session, name: &str) {
    let path = session.ui_dir.join(name);
    let inside = std::fs::canonicalize(&path).ok().zip(std::fs::canonicalize(&session.ui_dir).ok()).is_some_and(|(p, d)| p.starts_with(d) && p.is_file());
    if !inside {
        return req.respond(404, "text/plain", b"not found", false);
    }
    let Ok(mut data) = std::fs::read(&path) else { return req.respond(404, "text/plain", b"not found", false) };
    if name == "index.html" {
        let text = String::from_utf8_lossy(&data).replace("__TOKEN__", &session.token);
        data = text.into_bytes();
    }
    req.respond(200, content_type(name), &data, false);
}

fn serve(mut req: Request, session: &Arc<Session>, handle: &AppHandle) {
    if !host_ok(&req) {
        if req.method == "POST" {
            req.drain(MAX_BODY);
        }
        return req.respond(403, "text/plain", b"forbidden", false);
    }
    match req.method.as_str() {
        "GET" => do_get(&mut req, session, handle),
        "POST" => do_post(&mut req, session, handle),
        _ => req.respond(404, "text/plain", b"not found", false),
    }
}

fn do_get(req: &mut Request, session: &Arc<Session>, handle: &AppHandle) {
    let target = req.target.clone();
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    let q = parse_query(query);
    *session.last_bye.lock().unwrap() = None;
    let get = |k: &str| q.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str()).unwrap_or("");
    if path == "/" || path == "/index.html" {
        return static_file(req, session, "index.html");
    }
    let flat = ["/styles.css", "/crest.svg", "/favicon.svg", "/selftest.js"];
    let script =
        path.strip_prefix("/js/").is_some_and(|n| n.ends_with(".js") && n.len() > 3 && n.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)));
    if flat.contains(&path) || script {
        return static_file(req, session, &path[1..]);
    }
    let Some(r) = path.strip_prefix("/api/") else { return req.respond(404, "text/plain", b"not found", false) };
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
        return send_json(req, 200, &json!({"ok": true, "seq": f.seq, "file": f.file, "line": f.line, "text": f.text}));
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
        return send_json(req, 200, &json!({"ok": true, "companions": n}));
    }
    let Some(op) = api::route("GET", r) else { return error(req, 404, msg("Unknown address.", &[])) };
    let params: Map<String, Value> = q.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
    let (name, binary) = (op.name, op.kind == "binary");
    let params = Value::Object(params);
    let result = handle.run(move |app| api::call(app, name, &params));
    if !binary {
        return send_result(req, result);
    }
    match result {
        Ok(Ok(Answer::Binary(data, mime))) => req.respond(200, &mime, &data, true),
        Ok(Err(ApiError::App(t))) => req.respond(404, "text/plain", t.text().as_bytes(), false),
        Ok(Err(e)) if e.is_app_error() => req.respond(404, "text/plain", e.text().text().as_bytes(), false),
        Ok(Err(ApiError::Missing(_))) | Ok(Ok(_)) => req.respond(404, "text/plain", b"not found", false),
        Ok(Err(_)) | Err(_) => req.respond(500, "text/plain", "chyba".as_bytes(), false),
    }
}

fn touch(list: &mut Vec<(String, Instant)>, id: &str) {
    match list.iter_mut().find(|(k, _)| k == id) {
        Some(e) => e.1 = Instant::now(),
        None => list.push((id.to_string(), Instant::now())),
    }
}

fn do_post(req: &mut Request, session: &Arc<Session>, handle: &AppHandle) {
    let target = req.target.clone();
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
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
        return req.respond(200, "text/plain", b"ok", false);
    }
    if req.header("x-token") != session.token {
        req.drain(MAX_BODY);
        return error(req, 403, msg("Forbidden (invalid token).", &[]));
    }
    let Some(length) = req.content_length() else { return error(req, 400, msg("Invalid request.", &[])) };
    if length > MAX_BODY {
        return error(req, 413, msg("The request is too large.", &[]));
    }
    let body: Value = match req
        .body(length)
        .and_then(|b| String::from_utf8(b).ok())
        .map(|t| if t.is_empty() { "{}".to_string() } else { t })
        .and_then(|t| serde_json::from_str(&t).ok())
    {
        Some(v) => v,
        None => return error(req, 400, msg("Invalid request.", &[])),
    };
    let d: Map<String, Value> = body.as_object().cloned().unwrap_or_default();
    let r = path.strip_prefix("/api/").unwrap_or("");
    if let Some(op) = api::route("POST", r) {
        let (name, params) = (op.name, Value::Object(d));
        return send_result(req, handle.run(move |app| api::call(app, name, &params)));
    }
    // what only the application around Core can do: windows, the focus between windows, a folder dialog
    match r {
        "project/pick" => {
            let Some(pick) = session.pick_folder.as_ref() else {
                return error(req, 400, msg("Choosing a folder is not available in this mode – enter the path by hand.", &[]));
            };
            let chosen = pick();
            let result = handle.run(move |app| -> Result<Value, ApiError> {
                let Some(chosen) = chosen else { return Ok(json!({"ok": true, "cancelled": true})) };
                let (ok, problem) = lexo_core::project::check_project_dir(&chosen);
                if !ok {
                    return Err(ApiError::App(Text::new_msg(problem.unwrap_or_else(|| msg("The folder cannot be used.", &[])))));
                }
                app.open_project(&chosen, false)?;
                Ok(json!({"ok": true, "cancelled": false, "path": chosen}))
            });
            send_result(req, result.map(|r| r.map(Answer::Json)));
        }
        "window/open" => {
            send_json(req, 200, &json!({"ok": false, "fallback": true, "error": msg("A new window cannot be opened from the application here.", &[]).wire()}))
        }
        "window/fullscreen" => send_json(req, 200, &json!({"ok": false, "fallback": true})),
        "focus" => {
            let ready = handle.run(|app| app.ready()).unwrap_or(false);
            if !ready {
                return error(req, 400, msg("No project is open.", &[]));
            }
            let line = d.get("line").map(lexo_core::messages::py_str).unwrap_or_default();
            let line = if !line.is_empty() && line.chars().all(|c| c.is_ascii_digit()) { line.parse().unwrap_or(0) } else { 0 };
            let mut focus = session.focus.lock().unwrap();
            focus.seq += 1;
            focus.file = d.get("file").map(lexo_core::messages::py_str).unwrap_or_default().chars().take(500).collect();
            focus.line = line;
            focus.text = d.get("text").and_then(Value::as_str).map(|t| t.chars().take(20000).collect()); // what is being typed in the main window
            let f = focus.clone();
            session.focus_changed.notify_all();
            drop(focus);
            send_json(req, 200, &json!({"ok": true, "seq": f.seq, "file": f.file, "line": f.line, "text": f.text}));
        }
        _ => error(req, 404, msg("Unknown address.", &[])),
    }
}

/// The folder with the UI files: the argument, `LEXO_UI_DIR`, or the folder of this repository.
pub fn default_ui_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("LEXO_UI_DIR") {
        return PathBuf::from(d);
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../free/lexo_free/ui")
}

/// The folder with the language packs shipped with the application.
pub fn default_packs_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("LEXO_PACKS_DIR") {
        return PathBuf::from(d);
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../core/lexo/packs")
}
