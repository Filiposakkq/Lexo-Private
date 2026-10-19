//! Preparing a game for translating (the guide on Home): what is inspected, what is refused, and the whole way with a fake program in place of unrpyc and of the engine of the game.
use lexo_core::app::App;
use lexo_core::app_prepare::RunFn;
use lexo_core::config::Config;
use lexo_core::gametools::ToolError;
use lexo_core::lang::Packs;
use serde_json::Value;
use std::path::Path;
use std::time::{Duration, Instant};

fn app_in(tmp: &Path) -> App {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut config = Config::new(Some(&tmp.join("cfg/config.json")));
    config.load();
    App::new(config, Packs { bundled: repo.join("rust/resources/packs"), user: None })
}

/// A built game: its own engine (a launcher and a Python in lib), a script as source or as bytecode only.
fn game(root: &Path, bytecode_only: bool, py_dir: &str) {
    let plat = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    };
    let py = root.join("lib").join(format!("{py_dir}-{plat}-x86_64"));
    std::fs::create_dir_all(&py).unwrap();
    for f in ["python.exe", "python"] {
        std::fs::write(py.join(f), "").unwrap();
    }
    for f in ["Fake.exe", "Fake.sh", "Fake.py"] {
        std::fs::write(root.join(f), "").unwrap();
    }
    std::fs::create_dir_all(root.join("game")).unwrap();
    std::fs::write(root.join("game/options.rpy"), "define config.name = _(\"Fake Game\")\n").unwrap();
    std::fs::write(root.join("game").join(if bytecode_only { "script.rpyc" } else { "script.rpy" }), "label start:\n    e \"Hi\"\n").unwrap();
}

fn fake(argv: &[String], _cwd: &Path) -> Result<(i32, String), ToolError> {
    if argv.iter().any(|a| a.ends_with("unrpyc.py")) {
        let target = Path::new(argv.last().unwrap());
        std::fs::write(target.join("script.rpy"), "label start:\n    e \"Hi\"\n").unwrap();
        return Ok((0, "decompiled".into()));
    }
    if argv.iter().any(|a| a == "translate") {
        let (base, lang) = (&argv[argv.len() - 3], &argv[argv.len() - 1]);
        let tl = Path::new(base).join("game/tl").join(lang);
        std::fs::create_dir_all(&tl).unwrap();
        std::fs::write(tl.join("script.rpy"), "translate czech a_1:\n\n    # e \"Hi\"\n    e \"Hi\"\n").unwrap();
        return Ok((0, "translated".into()));
    }
    Ok((1, "unknown command".into()))
}

fn wait(app: &App, job: &str) -> Value {
    let end = Instant::now() + Duration::from_secs(30);
    loop {
        let s = app.prepare_status(job).unwrap();
        if s["done"] == true || Instant::now() > end {
            return s;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_game_is_inspected_refused_when_it_cannot_be_prepared_and_prepared_step_by_step() {
    let tmp = std::env::temp_dir().join(format!("prepare-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let mut app = app_in(&tmp);
    let runner: RunFn = fake;

    // not a game at all, a bad language name, a game without an engine of its own
    assert!(app.prepare_inspect(&tmp.join("nowhere").to_string_lossy()).is_err());
    let plain = tmp.join("Plain");
    std::fs::create_dir_all(plain.join("game")).unwrap();
    std::fs::write(plain.join("game/script.rpy"), "label start:\n").unwrap();
    let i = app.prepare_inspect(&plain.to_string_lossy()).unwrap();
    assert_eq!((i["engine"].clone(), i["ready"].clone(), i["scripts"].clone()), (false.into(), false.into(), 1.into()), "{i}");
    assert!(app.prepare_start_with(&plain.to_string_lossy(), "czech", "copy", "", runner).is_err());

    // a game whose scripts are sources: nothing needs unrpyc; the folder of the `game` folder is accepted as well
    let a = tmp.join("Source Game");
    game(&a, false, "py3");
    let i = app.prepare_inspect(&a.join("game").to_string_lossy()).unwrap();
    assert_eq!(
        (i["engine"].clone(), i["needs_unrpyc"].clone(), i["ready"].clone(), i["bytecode_only"].clone()),
        (true.into(), false.into(), true.into(), 0.into()),
        "{i}"
    );
    assert!(app.prepare_start_with(&a.to_string_lossy(), "1bad", "copy", "", runner).is_err(), "the language name is checked");
    let plan = app.prepare_plan(&a.to_string_lossy(), "copy", "").unwrap();
    assert_eq!(
        plan["workspace"].as_str().unwrap().replace('\\', "/"),
        format!("{}-aside", a.to_string_lossy().replace('\\', "/")),
        "the copy is beside the game: {plan}"
    );
    assert_eq!((plan["links"].clone(), plan["enough"].clone()), (true.into(), true.into()), "the same disk: linked, little to copy: {plan}");
    assert!(plan["need"].as_u64().unwrap() < 10_000, "{plan}");
    let plan = app.prepare_plan(&a.to_string_lossy(), "inplace", "").unwrap();
    assert_eq!((plan["need"].clone(), plan["enough"].clone()), (0.into(), true.into()), "{plan}");
    // the copy must not go inside the game
    assert!(app.prepare_start_with(&a.to_string_lossy(), "czech", "copy", &a.join("game").join("copy").to_string_lossy(), runner).is_err());
    let job = app.prepare_start_with(&a.to_string_lossy(), "czech", "copy", "", runner).unwrap()["job"].as_str().unwrap().to_string();
    let done = wait(&app, &job);
    assert_eq!((done["done"].clone(), done["success"].clone()), (true.into(), true.into()), "{done}");
    let folder = done["result"]["folder"].as_str().unwrap();
    assert!(Path::new(folder).join("script.rpy").is_file(), "the translation is in the workspace, not in the game");
    assert!(!a.join("game/tl").exists(), "the game itself is only read");
    assert!(done["steps"].as_array().unwrap().iter().any(|s| s == "workspace"));
    // prepared before: it is not made again
    let job = app.prepare_start_with(&a.to_string_lossy(), "czech", "copy", "", runner).unwrap()["job"].as_str().unwrap().to_string();
    assert_eq!(wait(&app, &job)["result"]["reused"], true);

    // the translation folder opens in the application, and then Home has it
    app.open_project(folder, false).unwrap();
    assert!(app.library_list().unwrap()["games"].as_array().unwrap().iter().any(|g| g["state"] == "ready"));

    // a game that has only bytecode needs unrpyc: the one that comes with the application is used, of the version that fits the Python of the game (3 -> v2, 2 -> v1)
    let b = tmp.join("Bytecode Game");
    game(&b, true, "py3");
    let i = app.prepare_inspect(&b.to_string_lossy()).unwrap();
    assert_eq!((i["bytecode_only"].clone(), i["needs_unrpyc"].clone(), i["ready"].clone()), (1.into(), true.into(), true.into()), "{i}");
    assert_eq!((i["unrpyc_source"].clone(), i["unrpyc_version"].clone()), ("bundled".into(), "v2".into()), "{i}");
    let old = tmp.join("Old Game");
    game(&old, true, "py2");
    let i = app.prepare_inspect(&old.to_string_lossy()).unwrap();
    assert_eq!((i["unrpyc_source"].clone(), i["unrpyc_version"].clone(), i["ready"].clone()), ("bundled".into(), "v1".into(), true.into()), "{i}");
    let v1 = i["unrpyc"].as_str().unwrap().to_string();
    assert!(Path::new(&v1).is_file() && v1.replace('\\', "/").ends_with("tools/unrpyc/v1/unrpyc.py"), "{v1}");
    let job = app.prepare_start_with(&b.to_string_lossy(), "czech", "copy", "", runner).unwrap()["job"].as_str().unwrap().to_string();
    let done = wait(&app, &job);
    assert_eq!(done["success"], true, "{done}");
    assert!(done["steps"].as_array().unwrap().iter().any(|s| s == "decompile"));

    // one the user points at wins over the bundled one
    assert!(app.prepare_set_unrpyc(&tmp.join("nothing.py").to_string_lossy()).is_err());
    let mine = tmp.join("tools/unrpyc");
    std::fs::create_dir_all(&mine).unwrap();
    std::fs::write(mine.join("unrpyc.py"), "").unwrap();
    app.prepare_set_unrpyc(&mine.to_string_lossy()).unwrap();
    let i = app.prepare_inspect(&b.to_string_lossy()).unwrap();
    assert_eq!((i["unrpyc_source"].clone(), i["ready"].clone()), ("chosen".into(), true.into()), "{i}");

    // in the game itself: no copy at all, the translation is written into the game (the way many translators work)
    let c = tmp.join("In Place Game");
    game(&c, false, "py3");
    let job = app.prepare_start_with(&c.to_string_lossy(), "czech", "inplace", "", runner).unwrap()["job"].as_str().unwrap().to_string();
    let done = wait(&app, &job);
    assert_eq!(done["success"], true, "{done}");
    assert!(c.join("game/tl/czech/script.rpy").is_file(), "written in the game");
    assert!(!tmp.join("In Place Game-aside").exists(), "and nothing was copied");
}
