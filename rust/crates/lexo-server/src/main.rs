//! `lexo-server`: the local server with the UI, on the Rust Core.
//!
//!     lexo-server [--project <translation folder>] [--port N] [--ui <folder>] [--packs <folder>]
//!     lexo-server --selftest         a synthetic project in a temporary folder, for the UI self-test (free/lexo_free/ui/selftest.js)
//!
//! It prints the address and serves until it is stopped (Ctrl+C); the settings are in the usual folder of the settings (`LEXO_CONFIG_DIR`).
use lexo_core::app::App;
use lexo_core::config::Config;
use lexo_core::lang::Packs;
use lexo_server::{default_packs_dir, default_ui_dir, start, ServerOptions};
use serde_json::json;
use std::path::{Path, PathBuf};

const SCRIPT: &str = "# game/script.rpy:10
translate czech a1:

    # mc happy \"Hello there, [name].\"
    mc happy \"Ahoj tam, [name].\"

# game/script.rpy:20
translate czech a2:

    # mc \"Take the Dik Coins.\"
    mc \"Vezmi si peníze.\"

# game/script.rpy:30
translate czech a3:

    # de \"Hello [name].\"
    de \"Ahoj [jmeno].\"

# game/script.rpy:40
translate czech a4:

    # de \"Hmm...\"
    de \"Hmm...\"

# game/script.rpy:50
translate czech a5:

    # mc \"Where are you going today?\"
    mc \"Where are you going today?\"

# game/script.rpy:55
translate czech a6:

    # mc \"Yes.\"
    mc \"Ano.\"

# game/script.rpy:56
translate czech a7:

    # de \"Yes.\"
    de \"Jo.\"

# game/script.rpy:57
translate czech a8:

    # mc \"Where are you going tonight?\"
    mc \"Kam jdeš dnes večer?\"
";

const STRINGS: &str = "translate czech strings:

    # game/script.rpy:60
    old \"Start\"
    new \"Začít\"

    # game/script.rpy:61
    old \"Quit\"
    new \"Quit\"
";

/// The synthetic project of the UI self-test (`free/tests/ui/serve_selftest.py`): the folder to open (`.../.lexo-selftest/game/tl/czech`).
fn make_project() -> std::io::Result<PathBuf> {
    let base = std::env::temp_dir().join(format!("lexo-{}-{}", std::process::id(), lexo_core::fsutil::token_hex(3))).join(".lexo-selftest");
    let proj = base.join("game/tl/czech");
    std::fs::create_dir_all(&proj)?;
    let script: String = (1..80).map(|i| format!("label line_{i}:")).collect::<Vec<_>>().join("\n");
    std::fs::write(base.join("game/script.rpy"), script)?;
    std::fs::write(proj.join("script.rpy"), SCRIPT)?;
    std::fs::write(proj.join("options.rpy"), STRINGS)?;
    Ok(proj)
}

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
        ServerOptions { ui_dir: ui, port, pick_folder: None },
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
