//! The Lexo application: a Tauri window around the UI of Free, on the Rust Core (milestone M8).
//!
//! The window loads its page from the custom protocol `lexo://` (on Windows `http://lexo.localhost/`), and the page's own `fetch('/api/...')` goes to the same protocol: the handler
//! below gives the request to the routes of `lexo-server` (`Dispatcher`), so the UI runs unchanged and no socket is opened. The things only the shell can do (the folder dialog, one more
//! window, full screen) are the `Shell` of this program.
//!
//!     lexo-app [<translation folder>]        open that folder (else the one remembered in the settings)
//!     LEXO_SELFTEST=1 lexo-app               a synthetic project, the UI self-test runs in the window; the result goes to stdout and the exit code says if it passed
#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

use lexo_core::app::App;
use lexo_core::config::Config;
use lexo_core::lang::Packs;
use lexo_server::backend::Backend;
use lexo_server::dispatch::{Dispatcher, Reply, Session, Shell, MAX_BODY};
use lexo_server::{default_packs_dir, default_ui_dir};
use serde_json::{json, Value};
use std::sync::Arc;
use tauri::http::{Request, Response};
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::DialogExt;

/// Where the pages of the window come from (the scheme of `register_*_uri_scheme_protocol`, as the webview sees it).
#[cfg(windows)]
const BASE_URL: &str = "http://lexo.localhost/";
#[cfg(not(windows))]
const BASE_URL: &str = "lexo://localhost/";

const SCRIPT_SELFTEST_REPORT: &str = "/__selftest_report";

/// In the self-test the page reports its result to the program, which prints it and ends.
const SELFTEST_JS: &str = "(function(){ if(!/[?&]selftest/.test(location.search)) return; var t=setInterval(function(){ var s=window.__selftest; \
    if(s&&s.done){ clearInterval(t); fetch('/__selftest_report',{method:'POST',body:JSON.stringify({results:s.results||[],errors:s.errors||[]})}); } },500); })();";

struct TauriShell {
    handle: AppHandle,
    windows: std::sync::atomic::AtomicUsize,
}

impl Shell for TauriShell {
    fn pick_folder(&self) -> Option<Option<String>> {
        let chosen = self.handle.dialog().file().set_title("Choose the translation folder (game/tl/<language>)").blocking_pick_folder();
        Some(chosen.and_then(|p| p.into_path().ok()).map(|p| p.to_string_lossy().into_owned()))
    }

    fn open_window(&self, url: &str) -> Result<(), String> {
        let url: Url = url.parse().map_err(|e| format!("{e}"))?;
        let n = self.windows.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 2;
        let handle = self.handle.clone();
        self.handle
            .run_on_main_thread(move || {
                let _ = WebviewWindowBuilder::new(&handle, format!("w{n}"), WebviewUrl::CustomProtocol(url)).title("Lexo").inner_size(1200.0, 800.0).build();
            })
            .map_err(|e| e.to_string())
    }

    fn toggle_fullscreen(&self) -> bool {
        let windows = self.handle.webview_windows();
        let Some(w) = windows.values().find(|w| w.is_focused().unwrap_or(false)).or_else(|| windows.values().next()) else { return false };
        let on = w.is_fullscreen().unwrap_or(false);
        w.set_fullscreen(!on).is_ok()
    }
}

fn to_response(reply: Reply) -> Response<Vec<u8>> {
    Response::builder()
        .status(reply.code)
        .header("Content-Type", reply.ctype)
        .header("Cache-Control", if reply.cache { "private, max-age=600" } else { "no-store" })
        .header("X-Content-Type-Options", "nosniff")
        .header("Content-Security-Policy", "default-src 'self' lexo: http://lexo.localhost; style-src 'self' 'unsafe-inline'; img-src 'self' data: lexo: http://lexo.localhost; script-src 'self' lexo: http://lexo.localhost")
        .body(reply.body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

fn handle_request(dispatcher: &Dispatcher, selftest: bool, request: Request<Vec<u8>>) -> Reply {
    let target = request.uri().path_and_query().map_or("/".to_string(), |p| p.as_str().to_string());
    let method = request.method().as_str().to_string();
    let token = request.headers().get("x-token").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let body = request.into_body();
    if selftest && method == "POST" && target == SCRIPT_SELFTEST_REPORT {
        let report: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        let failed: Vec<&Value> = report["results"].as_array().into_iter().flatten().filter(|r| r["pass"] != json!(true)).collect();
        let total = report["results"].as_array().map_or(0, Vec::len);
        println!("UI self-test: {}/{} checks passed", total - failed.len(), total);
        for r in &failed {
            println!("  FAIL  {} {}", r["name"], r["detail"]);
        }
        let errors = report["errors"].as_array().cloned().unwrap_or_default();
        for e in &errors {
            println!("  JS ERROR  {e}");
        }
        std::process::exit(i32::from(!failed.is_empty() || !errors.is_empty() || total == 0));
    }
    if body.len() > MAX_BODY {
        return Reply::too_large();
    }
    dispatcher.handle(&method, &target, &token, body)
}

fn main() {
    let selftest = std::env::var_os("LEXO_SELFTEST").is_some();
    let arg_project = std::env::args().skip(1).find(|a| !a.starts_with("--"));
    let (project, cfg_dir) = if selftest {
        let proj = lexo_server::selftest::make_project().expect("the synthetic project");
        let cfg = std::env::temp_dir().join(format!("lexo-cfg-{}", std::process::id()));
        (Some(proj.to_string_lossy().into_owned()), Some(cfg))
    } else {
        (arg_project, None)
    };
    let ui_dir = default_ui_dir();
    let packs = default_packs_dir();
    let backend = Backend::spawn(move || {
        let mut config = Config::new(cfg_dir.as_ref().map(|d| d.join("config.json")).as_deref());
        config.load();
        let user = cfg_dir.as_ref().map_or_else(|| lexo_core::config::config_dir().join("packs"), |d| d.join("packs"));
        let mut app = App::new(config, Packs { bundled: packs, user: Some(user) });
        // the folder given, or the one remembered in the settings
        let remembered = app.config.get("project_dir").and_then(Value::as_str).map(String::from);
        if let Some(path) = project.or(remembered).filter(|p| lexo_core::project::check_project_dir(p).0) {
            match app.open_project(&path, false) {
                Ok(_) => {
                    let _ = app.maybe_auto_snapshot();
                    if selftest {
                        let _ = lexo_core::api::call(&mut app, "glossary/set", &json!({"items": [{"code": "Dik Coins", "name": "dikmince", "type": "term"}]}));
                    }
                }
                Err(e) => eprintln!("The project could not be opened: {e}"),
            }
        }
        app
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol("lexo", move |ctx, request, responder| {
            // the dispatcher is managed in `setup`; the first request comes after the window is made
            let Some(dispatcher) = ctx.app_handle().try_state::<Dispatcher>().map(|s| s.inner().clone()) else {
                return responder.respond(to_response(Reply::not_found()));
            };
            std::thread::spawn(move || responder.respond(to_response(handle_request(&dispatcher, selftest, request))));
        })
        .setup(move |app| {
            let shell = TauriShell { handle: app.handle().clone(), windows: std::sync::atomic::AtomicUsize::new(0) };
            let session = Arc::new(Session::new(ui_dir, BASE_URL.to_string(), Box::new(shell)));
            let dispatcher = Dispatcher { session, backend };
            app.manage(dispatcher);
            let query = if selftest { "?selftest=1&lang=cs" } else { "" }; // the self-test checks Czech texts
            let url: Url = format!("{BASE_URL}{query}").parse()?;
            let mut window =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::CustomProtocol(url)).title("Lexo").inner_size(1400.0, 900.0).min_inner_size(900.0, 600.0);
            if selftest {
                window = window.initialization_script(SELFTEST_JS);
            }
            window.build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("the Lexo application could not start");
}
