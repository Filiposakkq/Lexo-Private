//! The two buttons for sharing a folder with git ("Get changes", "Send changes") against real repositories: a bare "server" and two clones.
use lexo_core::app::App;
use lexo_core::config::Config;
use lexo_core::lang::Packs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git").args(args).current_dir(dir).output().expect("git runs");
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

fn app_of(tmp: &Path, who: &str, folder: &Path) -> App {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut config = Config::new(Some(&tmp.join(format!("cfg-{who}/config.json"))));
    config.load();
    config.set("author", serde_json::json!(format!("{who}-0001")));
    let mut app = App::new(config, Packs { bundled: repo.join("rust/resources/packs"), user: None });
    app.open_project(&folder.to_string_lossy(), false).expect("the project opens");
    app
}

#[test]
fn changes_are_sent_and_got_and_a_conflict_is_left_alone() {
    if Command::new("git").arg("--version").output().is_err() {
        return; // no git on this computer: nothing to test
    }
    // git must know who commits in this test (the application only fills it in when it is empty)
    for (k, v) in
        [("GIT_AUTHOR_NAME", "Test"), ("GIT_AUTHOR_EMAIL", "t@example.invalid"), ("GIT_COMMITTER_NAME", "Test"), ("GIT_COMMITTER_EMAIL", "t@example.invalid")]
    {
        std::env::set_var(k, v);
    }
    let tmp = std::env::temp_dir().join(format!("git-sync-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let server = tmp.join("server.git");
    std::fs::create_dir_all(&server).unwrap();
    git(&server, &["init", "--bare", "-q"]);
    let (a, b): (PathBuf, PathBuf) = (tmp.join("a"), tmp.join("b"));
    git(&tmp, &["clone", "-q", server.to_str().unwrap(), "a"]);
    let tl_a = a.join("game/tl/czech");
    std::fs::create_dir_all(&tl_a).unwrap();
    std::fs::write(tl_a.join("script.rpy"), "translate czech a_1:\n\n    # e \"Hello.\"\n    e \"Ahoj.\"\n\n").unwrap();
    git(&a, &["add", "-A"]);
    git(&a, &["commit", "-q", "-m", "start"]);
    git(&a, &["push", "-q", "-u", "origin", "HEAD"]);
    git(&tmp, &["clone", "-q", server.to_str().unwrap(), "b"]);
    let tl_b = b.join("game/tl/czech");

    let mut app_a = app_of(&tmp, "ann", &tl_a);
    let mut app_b = app_of(&tmp, "bob", &tl_b);

    // a folder that is not a repository, and the state of one that is
    let plain = tmp.join("plain/game/tl/czech");
    std::fs::create_dir_all(&plain).unwrap();
    std::fs::write(plain.join("script.rpy"), "translate czech a_1:\n\n    # e \"Hi.\"\n    e \"Ahoj.\"\n\n").unwrap();
    let mut app_p = app_of(&tmp, "pat", &plain);
    assert_eq!(app_p.git_state().unwrap()["repo"], false);
    assert!(app_p.git_pull().is_err() && app_p.git_push().is_err());
    let s = app_a.git_state().unwrap();
    // opening the project made its data folder (`.aside`), which is new for git
    assert_eq!((s["repo"].clone(), s["upstream"].clone()), (true.into(), true.into()), "{s}");

    // Ann changes a line and sends it: committed (named after her id: the name is not set) and pushed
    app_a.entry_save("script.rpy", 4, "Ahoj, světe.").unwrap();
    assert!(app_a.git_state().unwrap()["changes"].as_i64().unwrap() >= 1);
    let s = app_a.git_push().unwrap();
    assert_eq!((s["committed"].clone(), s["ahead"].clone(), s["changes"].clone()), (true.into(), 0.into(), 0.into()), "{s}");
    assert!(git(&a, &["log", "-1", "--format=%s"]).starts_with("Translation update by "));

    // Bob gets it: the file is changed and read again
    let s = app_b.git_pull().unwrap();
    assert_eq!(s["behind"], 0, "{s}");
    assert!(std::fs::read_to_string(tl_b.join("script.rpy")).unwrap().contains("Ahoj, světe."));
    // Bob opened the folder before getting Ann's shared data, so he had made his own `.aside` files: they were put aside, and Ann's are his now
    let id = |d: &Path| std::fs::read_to_string(d.join(".aside/project-id")).unwrap().trim().to_string(); // git may change the line ends
    assert_eq!(id(&tl_b), id(&tl_a));

    // nothing to send: no commit, no error
    let s = app_b.git_push().unwrap();
    assert_eq!(s["committed"], false, "{s}");

    // both change the same line: the first sends, the second cannot (a conflict): the folder is left as it was, nothing is half done
    app_a.entry_save("script.rpy", 4, "Ahoj A.").unwrap();
    app_a.git_push().unwrap();
    app_b.entry_save("script.rpy", 4, "Ahoj B.").unwrap();
    let err = app_b.git_push().unwrap_err().to_string();
    assert!(err.contains("did not work"), "{err}");
    assert!(!tl_b.join("../../../.git/rebase-merge").exists() && !tl_b.join("../../../.git/rebase-apply").exists(), "the rebase was cancelled");
    assert!(std::fs::read_to_string(tl_b.join("script.rpy")).unwrap().contains("Ahoj B."), "Bob's own text is still there");
}
