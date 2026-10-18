//! `lexo-server`: the local server with the UI, on the Rust Core.
//!
//!     lexo-server [--project <translation folder>] [--port N] [--ui <folder>] [--packs <folder>]
//!     lexo-server --selftest         a synthetic project in a temporary folder, for the UI self-test (free/lexo_free/ui/selftest.js)
//!
//! It prints the address and serves until it is stopped (Ctrl+C); the settings are in the usual folder of the settings (`LEXO_CONFIG_DIR`).
use lexo_core::app::App;
use lexo_core::config::Config;
use lexo_core::lang::Packs;
use lexo_server::selftest::make_project;
use lexo_server::{default_packs_dir, default_ui_dir, start, ServerOptions};
use serde_json::json;
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let selftest = args.iter().any(|a| a == "--selftest");
    let ui = value("--ui").map(PathBuf::from).unwrap_or_else(default_ui_dir);
    let packs = value("--packs").map(PathBuf::from).unwrap_or_else(default_packs_dir);
    let port: u16 = value("--port").and_then(|p| p.parse().ok()).unwrap_or(0);
    let (project, cfg_dir) = if selftest {
        let proj = make_project().expect("the synthetic project");
        let cfg = std::env::temp_dir().join(format!("lexo-cfg-{}-{}", std::process::id(), lexo_core::fsutil::token_hex(3)));
        (Some(proj.to_string_lossy().into_owned()), Some(cfg))
    } else {
        (value("--project"), None)
    };
    let project_for_app = project.clone();
    let server = start(
        move || {
            let mut config = Config::new(cfg_dir.as_ref().map(|d| d.join("config.json")).as_deref());
            config.load();
            let user = lexo_core::config::config_dir().join("packs");
            let user = cfg_dir.as_ref().map_or(user, |d| d.join("packs"));
            let mut app = App::new(config, Packs { bundled: packs, user: Some(user) });
            if !selftest {
                app.mt_http = Some(Box::new(lexo_core::net::post)); // the real network (the self-test must not touch it)
            }
            if let Some(p) = &project_for_app {
                match app.open_project(p, false) {
                    Ok(_) => {
                        if selftest {
                            let _ =
                                lexo_core::api::call(&mut app, "glossary/set", &json!({"items": [{"code": "Dik Coins", "name": "dikmince", "type": "term"}]}));
                        }
                    }
                    Err(e) => eprintln!("The project could not be opened: {e}"),
                }
            }
            app
        },
        ServerOptions { ui_dir: ui, port, shell: Box::new(lexo_server::dispatch::NoShell) },
    )
    .expect("the server could not be started");
    let mut url = server.url();
    if selftest {
        url.push_str("?selftest=1&lang=cs"); // the self-test checks Czech texts
    }
    println!("{url}");
    use std::io::Write;
    let _ = std::io::stdout().flush();
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
        let _ = Path::new(".");
    }
}
