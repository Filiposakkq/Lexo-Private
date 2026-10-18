//! The local server of Lexo (milestones M7 and M8): the UI files and the same routes as `free/lexo_free/server.py`, on top of `lexo-core`.
//!
//! The routes (`dispatch.rs`) do not care how a request arrives. Here a loopback socket brings them (the development server and the UI self-test); the Tauri application
//! (`lexo-app`) brings them through the custom protocol of its window, without any socket. Only the loopback interface is bound, the `Host` header is checked (DNS-rebinding
//! protection) and every POST needs the per-run token that the page receives from the server.
pub mod backend;
pub mod dispatch;
pub mod http;
pub mod selftest;

use backend::Backend;
use dispatch::{Dispatcher, NoShell, Reply, Session, Shell, MAX_BODY};
use http::Request;
use lexo_core::app::App;
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct ServerOptions {
    pub ui_dir: PathBuf,
    pub port: u16,
    pub shell: Box<dyn Shell>,
}

impl ServerOptions {
    pub fn new(ui_dir: PathBuf) -> ServerOptions {
        ServerOptions { ui_dir, port: 0, shell: Box::new(NoShell) }
    }
}

pub struct RunningServer {
    pub addr: SocketAddr,
    pub session: Arc<Session>,
    pub dispatcher: Dispatcher,
    stop: Arc<std::sync::atomic::AtomicBool>,
}

impl RunningServer {
    pub fn url(&self) -> String {
        self.session.base_url.lock().unwrap().clone()
    }

    /// Run `f` on the application (for the code that starts the server: open a project, change a setting ...).
    pub fn with_app<R: Send + 'static>(&self, f: impl FnOnce(&mut App) -> R + Send + 'static) -> Result<R, String> {
        self.dispatcher.backend.run(f)
    }

    /// Stop accepting connections and close the project. The threads end with the process.
    pub fn shutdown(self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = std::net::TcpStream::connect(self.addr); // wakes the accept loop
        let _ = self.dispatcher.backend.run(|app| app.close());
    }
}

/// Start the server. `make_app` builds the application on its own thread.
pub fn start(make_app: impl FnOnce() -> App + Send + 'static, options: ServerOptions) -> std::io::Result<RunningServer> {
    let listener = TcpListener::bind(("127.0.0.1", options.port))?;
    let addr = listener.local_addr()?;
    let session = Arc::new(Session::new(options.ui_dir, format!("http://127.0.0.1:{}/", addr.port()), options.shell));
    let dispatcher = Dispatcher { session: session.clone(), backend: Backend::spawn(make_app) };
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let (dispatcher, stop) = (dispatcher.clone(), stop.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                if stop.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
                let Ok(stream) = stream else { continue };
                let dispatcher = dispatcher.clone();
                std::thread::spawn(move || {
                    if let Some(req) = Request::read(stream) {
                        serve(req, &dispatcher);
                    }
                });
            }
        });
    }
    Ok(RunningServer { addr, session, dispatcher, stop })
}

fn host_ok(req: &Request) -> bool {
    let host = req.header("host").split(':').next().unwrap_or("").to_lowercase();
    ["127.0.0.1", "localhost", "[::1]"].contains(&host.as_str())
}

fn send(req: &mut Request, reply: Reply) {
    req.respond(reply.code, &reply.ctype, &reply.body, reply.cache);
}

fn serve(mut req: Request, dispatcher: &Dispatcher) {
    if !host_ok(&req) {
        if req.method == "POST" {
            req.drain(MAX_BODY);
        }
        return send(&mut req, Reply::forbidden());
    }
    let body = if req.method == "POST" {
        let Some(length) = req.content_length() else { return send(&mut req, Reply::bad_request()) };
        let token_ok = req.header("x-token") == dispatcher.session.token || req.target.starts_with("/api/bye");
        if !token_ok {
            req.drain(MAX_BODY);
        } else if length > MAX_BODY {
            return send(&mut req, Reply::too_large());
        }
        if token_ok {
            match req.body(length) {
                Some(b) => b,
                None => return send(&mut req, Reply::bad_request()),
            }
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    let reply = dispatcher.handle(&req.method, &req.target, req.header("x-token"), body);
    send(&mut req, reply);
}

/// The folder with the UI files: `LEXO_UI_DIR`, or the folder of this repository.
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
