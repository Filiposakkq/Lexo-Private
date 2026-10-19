//! Publishing progress.json to a website repository, against throw-away local git repositories (the tests of `reference/python/pro/tests/test_publish.py`, at the level of the pack).
use lexo_pro::publish::{plan, publish};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

fn git(cwd: &Path, args: &[&str]) -> String {
    let out = Command::new("git").args(args).current_dir(cwd).output().expect("git");
    assert!(out.status.success(), "git {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

struct Fixture {
    tmp: PathBuf,
    remote: PathBuf,
    work: PathBuf,
    web: Value,
}

impl Fixture {
    fn new(name: &str) -> Fixture {
        let tmp = std::env::temp_dir().join(format!("lexo-pro-publish-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let (remote, work) = (tmp.join("origin.git"), tmp.join("work"));
        git(&tmp, &["init", "--bare", "-b", "main", remote.to_str().unwrap()]);
        git(&tmp, &["clone", remote.to_str().unwrap(), work.to_str().unwrap()]);
        for (k, v) in [("user.name", "Test"), ("user.email", "t@example.com"), ("commit.gpgsign", "false")] {
            git(&work, &["config", k, v]);
        }
        std::fs::create_dir_all(work.join("web")).unwrap();
        std::fs::write(work.join("web/progress.json"), "{\"schema\": 1, \"translation\": {\"percent\": 1.0}}\n").unwrap();
        git(&work, &["add", "."]);
        git(&work, &["commit", "-m", "init"]);
        git(&work, &["push", "-u", "origin", "main"]);
        let web = json!({"repo": work.to_str().unwrap(), "path": "web/progress.json", "title": "T"});
        Fixture { tmp, remote, work, web }
    }

    fn head(&self) -> String {
        git(&self.remote, &["rev-parse", "main"])
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.tmp);
    }
}

fn data(done: f64) -> Value {
    json!({"schema": 2, "updated": "2026-10-18T10:00+02:00", "translation": {"percent": done}, "proofreading": {"percent": 0.0}, "episodes": [], "seasons": [{"n": 3, "status": "planned"}]})
}

fn text(d: &Value) -> String {
    serde_json::to_string_pretty(d).unwrap() + "\n"
}

fn warnings(info: &serde_json::Map<String, Value>) -> String {
    info["warnings"].as_array().unwrap().iter().map(|w| lexo_core::messages::py_str(&lexo_core::messages::render(w))).collect::<Vec<_>>().join(" | ")
}

fn failure(r: Result<Value, lexo_pro::publish::Failure>) -> String {
    match r {
        Ok(v) => panic!("expected a failure, got {v}"),
        Err(e) => e.into_error().0.text(),
    }
}

#[test]
fn the_preview_changes_nothing_and_a_publish_needs_the_token_and_the_branch() {
    let f = Fixture::new("preview");
    let before = (f.head(), git(&f.work, &["rev-parse", "HEAD"]), git(&f.work, &["status", "--porcelain"]));
    let info = plan(&f.web, &data(40.0)).ok().unwrap();
    assert_eq!((info["branch"].as_str(), info["upstream"].as_str()), (Some("main"), Some("origin/main")));
    assert_eq!(info["changed"], json!(true));
    assert!(warnings(&info).contains("deploys the website live"));
    assert!(info["commands"].as_array().unwrap().contains(&json!("git push")));
    assert_eq!(before, (f.head(), git(&f.work, &["rev-parse", "HEAD"]), git(&f.work, &["status", "--porcelain"])));

    let token = info["token"].as_str().unwrap().to_string();
    failure(publish(&f.web, &data(40.0), "{}", "wrong", "main"));
    failure(publish(&f.web, &data(40.0), "{}", &token, "develop"));
    assert!(failure(publish(&f.web, &data(41.0), "{}", &token, "main")).contains("changed since the preview")); // the data changed after the preview
    assert_eq!(f.head(), before.0);
}

#[test]
fn a_publish_commits_only_the_data_file_and_a_second_one_is_a_noop() {
    let f = Fixture::new("publish");
    std::fs::write(f.work.join("notes.txt"), "keep me").unwrap(); // an unrelated dirty file
    let d = data(40.0);
    let info = plan(&f.web, &d).ok().unwrap();
    assert!(warnings(&info).contains("other uncommitted changes"));
    let res = publish(&f.web, &d, &text(&d), info["token"].as_str().unwrap(), "main").ok().unwrap();
    assert_eq!(res["ok"], json!(true));
    assert_eq!(
        git(&f.remote, &["show", "--name-only", "--format=%s", "main"]).lines().map(str::trim).collect::<Vec<_>>(),
        vec!["chore(translation): update progress data", "", "web/progress.json"]
    );
    let published: Value = serde_json::from_str(&git(&f.remote, &["show", "main:web/progress.json"])).unwrap();
    assert_eq!(published["schema"], json!(2));
    assert_eq!(published["seasons"][0], json!({"n": 3, "status": "planned"}));
    assert_eq!(git(&f.work, &["status", "--porcelain"]), "?? notes.txt");
    assert_eq!(std::fs::read_to_string(f.work.join("notes.txt")).unwrap(), "keep me");

    let head = f.head();
    let again = plan(&f.web, &d).ok().unwrap();
    assert_eq!(again["changed"], json!(false));
    let res = publish(&f.web, &d, &text(&d), again["token"].as_str().unwrap(), "main").ok().unwrap();
    assert!(lexo_core::messages::py_str(&lexo_core::messages::render(&res["message"])).contains("No change"));
    assert_eq!(f.head(), head);
}

#[test]
fn a_failed_push_rolls_everything_back() {
    let f = Fixture::new("rollback");
    let hook = f.remote.join("hooks/pre-receive");
    std::fs::write(&hook, "#!/bin/sh\necho rejected by test >&2\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // git runs a hook only when it is executable
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let local_before = git(&f.work, &["rev-parse", "HEAD"]);
    let file_before = std::fs::read(f.work.join("web/progress.json")).unwrap();
    let d = data(40.0);
    let info = plan(&f.web, &d).ok().unwrap();
    let msg = failure(publish(&f.web, &d, &text(&d), info["token"].as_str().unwrap(), "main"));
    assert!(msg.contains("push failed"), "{msg}");
    assert_eq!(git(&f.work, &["rev-parse", "HEAD"]), local_before);
    assert_eq!(std::fs::read(f.work.join("web/progress.json")).unwrap(), file_before);
    assert_eq!(git(&f.work, &["status", "--porcelain"]), "");
}

#[test]
fn blockers_and_setup_problems() {
    let f = Fixture::new("blockers");
    git(&f.work, &["checkout", "-b", "feature"]);
    let info = plan(&f.web, &data(40.0)).ok().unwrap();
    assert!(!info["blockers"].as_array().unwrap().is_empty());
    failure(publish(&f.web, &data(40.0), "{}", info["token"].as_str().unwrap(), "feature"));

    let mut web = f.web.clone();
    web["repo"] = json!("");
    assert!(failure(plan(&web, &data(1.0)).map(Value::Object)).contains("not set up"));
    web["repo"] = json!("no-such-owner-xyz/no-such-repo-xyz");
    let clone = plan(&web, &data(1.0)).ok().unwrap();
    assert_eq!(clone["will_clone"], json!(true));
    assert!(!Path::new(clone["repo_path"].as_str().unwrap()).exists());
    assert!(warnings(&clone).contains("cloned"));
    failure(publish(&web, &data(1.0), "{}", clone["token"].as_str().unwrap(), "main")); // a clone needs its own confirmation marker
    web["repo"] = json!(f.tmp.to_str().unwrap());
    assert!(failure(plan(&web, &data(1.0)).map(Value::Object)).contains("not a git repository"));
    for bad in ["../x.json", "/abs/x.json", "C:/x.json", "web/progress.txt", ""] {
        let mut web = f.web.clone();
        web["path"] = json!(bad);
        assert!(plan(&web, &data(1.0)).is_err(), "{bad}");
    }
}
