//! The server answers like the server of Free: static files, the token on POST, the Host check, operations of Core, errors as JSON.
use lexo_core::app::App;
use lexo_core::config::Config;
use lexo_core::lang::Packs;
use lexo_server::{default_packs_dir, default_ui_dir, start, ServerOptions};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpStream;

fn request(port: u16, method: &str, path: &str, headers: &[(&str, &str)], body: &str) -> (u16, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let mut head = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: {}\r\n", body.len());
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    s.write_all(format!("{head}\r\n{body}").as_bytes()).unwrap();
    let mut out = Vec::new();
    s.read_to_end(&mut out).unwrap();
    let text = String::from_utf8_lossy(&out).into_owned();
    let code = text.split(' ').nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
    (code, text.split_once("\r\n\r\n").map(|x| x.1.to_string()).unwrap_or_default())
}

#[test]
fn the_new_interface_is_served_at_next_beside_the_classic_one() {
    std::env::set_var("LEXO_NO_PRO", "1");
    let tmp = std::env::temp_dir().join(format!("lexo-server-next-{}", std::process::id()));
    let next = tmp.join("next");
    std::fs::create_dir_all(next.join("assets")).unwrap();
    std::fs::write(next.join("index.html"), "<html>NEW __TOKEN__</html>").unwrap();
    std::fs::write(next.join("assets/app.mjs"), "export const x = 1;").unwrap();
    std::fs::write(tmp.join("secret.txt"), "no").unwrap();
    let cfg = tmp.join("cfg");
    let mut opts = ServerOptions::new(default_ui_dir());
    opts.ui_next_dir = Some(next.clone());
    let server = start(
        move || {
            let mut config = Config::new(Some(&cfg.join("config.json")));
            config.load();
            App::new(config, Packs { bundled: default_packs_dir(), user: None })
        },
        opts,
    )
    .unwrap();
    let port = server.addr.port();
    let token = server.session.token.clone();
    // the classic one stays at "/", the new one is at "/next/" (also "/next") and carries the token of the run
    let (code, body) = request(port, "GET", "/", &[], "");
    assert!(code == 200 && !body.contains("NEW"));
    for p in ["/next/", "/next"] {
        let (code, body) = request(port, "GET", p, &[], "");
        assert_eq!((code, body.contains(&format!("NEW {token}"))), (200, true), "{p}");
    }
    assert_eq!(request(port, "GET", "/next/assets/app.mjs", &[], "").0, 200);
    // nothing outside the folder, and nothing that is not a plain path
    for p in ["/next/../secret.txt", "/next/assets/../../secret.txt", "/next//etc", "/next/%2e%2e/secret.txt", "/next/missing.js"] {
        assert_eq!(request(port, "GET", p, &[], "").0, 404, "{p}");
    }
}

#[test]
fn without_a_new_interface_next_is_not_found() {
    std::env::set_var("LEXO_NO_PRO", "1");
    let tmp = std::env::temp_dir().join(format!("lexo-server-nonext-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let cfg = tmp.join("cfg");
    let server = start(
        move || {
            let mut config = Config::new(Some(&cfg.join("config.json")));
            config.load();
            App::new(config, Packs { bundled: default_packs_dir(), user: None })
        },
        ServerOptions::new(default_ui_dir()),
    )
    .unwrap();
    assert_eq!(request(server.addr.port(), "GET", "/next/", &[], "").0, 404);
}

#[test]
fn the_server_serves_the_ui_and_the_operations() {
    std::env::set_var("LEXO_NO_PRO", "1");
    let tmp = std::env::temp_dir().join(format!("lexo-server-test-{}", std::process::id()));
    let tl = tmp.join("g/game/tl/czech");
    std::fs::create_dir_all(&tl).unwrap();
    std::fs::write(tl.join("script.rpy"), "# game/script.rpy:4\ntranslate czech a1:\n\n    # mc \"Hello.\"\n    mc \"Ahoj.\"\n").unwrap();
    let cfg = tmp.join("cfg");
    let (tl2, cfg2) = (tl.to_string_lossy().into_owned(), cfg.clone());
    let server = start(
        move || {
            let mut config = Config::new(Some(&cfg2.join("config.json")));
            config.load();
            let mut app = App::new(config, Packs { bundled: default_packs_dir(), user: None });
            app.open_project(&tl2, false).unwrap();
            app
        },
        ServerOptions::new(default_ui_dir()),
    )
    .unwrap();
    let port = server.addr.port();
    let token = server.session.token.clone();

    let (code, body) = request(port, "GET", "/", &[], "");
    assert_eq!(code, 200);
    assert!(body.contains(&token) && !body.contains("__TOKEN__"), "the page carries the token of the run");
    assert_eq!(request(port, "GET", "/js/../../x.js", &[], "").0, 404);
    assert_eq!(request(port, "GET", "/nothing", &[], "").0, 404);

    let (code, body) = request(port, "GET", "/api/state", &[], "");
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!((code, v["ok"].clone(), v["ready"].clone()), (200, json!(true), json!(true)));

    let (code, body) = request(port, "GET", "/api/entries?status=all&limit=5", &[], "");
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!((code, v["total"].clone()), (200, json!(1)));

    assert_eq!(request(port, "GET", "/api/entries", &[("Host", "evil.example")], "").0, 403, "a Host that is not the loopback is refused");
    assert_eq!(request(port, "POST", "/api/undo", &[], "{}").0, 403, "a POST needs the token");
    let (code, body) = request(port, "POST", "/api/undo", &[("X-Token", &token)], "{}");
    assert_eq!(code, 400, "{body}");
    assert!(serde_json::from_str::<Value>(&body).unwrap()["ok"] == json!(false));
    let (code, body) =
        request(port, "POST", "/api/entry/save", &[("X-Token", &token)], &json!({"file": "script.rpy", "line": 5, "text": "Ahoj, světe."}).to_string());
    assert_eq!(code, 200, "{body}");
    assert_eq!(request(port, "POST", "/api/entry/save", &[("X-Token", &token)], "{}").0, 400, "a missing parameter is a 400");
    assert_eq!(request(port, "POST", "/api/entry/save", &[("X-Token", &token)], "not json").0, 400);
    assert!(std::fs::read_to_string(tl.join("script.rpy")).unwrap().contains("Ahoj, světe."));

    let (code, body) = request(port, "POST", "/api/focus", &[("X-Token", &token)], &json!({"file": "script.rpy", "line": 5}).to_string());
    assert_eq!((code, serde_json::from_str::<Value>(&body).unwrap()["seq"].clone()), (200, json!(1)));
    let (_, body) = request(port, "GET", "/api/focus?after=0&wait=1", &[], "");
    assert_eq!(serde_json::from_str::<Value>(&body).unwrap()["line"], json!(5));
    server.shutdown();
    let _ = std::fs::remove_dir_all(&tmp);
}

/// A shell that remembers the address of the window it was asked to open.
struct Recorder(std::sync::Arc<std::sync::Mutex<Vec<String>>>);
impl lexo_server::dispatch::Shell for Recorder {
    fn open_window(&self, url: &str) -> Result<(), String> {
        self.0.lock().unwrap().push(url.to_string());
        Ok(())
    }
}

#[test]
fn a_second_window_opens_in_the_classic_or_the_new_interface() {
    std::env::set_var("LEXO_NO_PRO", "1");
    let tmp = std::env::temp_dir().join(format!("lexo-server-window-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let cfg = tmp.join("cfg");
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut opts = ServerOptions::new(default_ui_dir());
    opts.shell = Box::new(Recorder(seen.clone()));
    let server = start(
        move || {
            let mut config = Config::new(Some(&cfg.join("config.json")));
            config.load();
            App::new(config, Packs { bundled: default_packs_dir(), user: None })
        },
        opts,
    )
    .unwrap();
    let port = server.addr.port();
    let token = server.session.token.clone();
    let post = |body: Value| request(port, "POST", "/api/window/open", &[("X-Token", &token), ("Content-Type", "application/json")], &body.to_string());
    let (code, _) = post(json!({"view": "entries", "companion": "1", "tab": "scene", "status": "all", "file": "a b.rpy", "line": 12}));
    assert_eq!(code, 200);
    let (code, answer) = post(json!({"view": "entries", "companion": "1", "tab": "scene", "ui": "next", "file": "a.rpy", "line": 7}));
    assert_eq!(code, 200);
    assert!(answer.contains("\"ok\":true"), "{answer}");
    let urls = seen.lock().unwrap().clone();
    assert_eq!(urls.len(), 2, "{urls:?}");
    assert!(urls[0].starts_with(&format!("http://127.0.0.1:{port}/?win=2&companion=1&tab=scene&view=entries&file=a%20b.rpy&line=12")), "{}", urls[0]);
    assert!(urls[1].starts_with(&format!("http://127.0.0.1:{port}/next/?win=2&companion=1&tab=scene&view=entries&file=a.rpy&line=7")), "{}", urls[1]);
}
