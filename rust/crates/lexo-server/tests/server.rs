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
