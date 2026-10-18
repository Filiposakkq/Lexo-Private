//! Port of `pro/lexo_pro/publish.py`: publishing progress.json to the website repository: preview first, publish only on confirmation.
//!
//! Generating the data (export) and publishing it are separate steps:
//!
//!   `plan(...)`     read-only. Describes exactly what would happen: repository, branch, target file, what changes, the git commands, warnings (e.g. "push to main deploys the site").
//!   `publish(...)`  runs only with the token returned by `plan()` *and* the confirmed branch name. If the data or the repository changed after the preview, it refuses.
//!
//! Safety properties: nothing is pushed without `publish()`; only the one data file is added and committed (other changes in the repo are untouched); `git pull --ff-only` (no rebase /
//! autostash) so the repo's working tree is never rewritten; the file is written atomically and re-read to verify it; when the push fails, the local commit and the file are rolled back,
//! so a failed publish leaves the repository as it was.
//!
//! Known difference from Python: the confirmation token is made with SHA-1 (Core has no SHA-256); it only has to tell "the same preview" from another one, inside one run of the pack.
use lexo_core::clock::SystemClock;
use lexo_core::export;
use lexo_core::extensions::ExtensionError;
use lexo_core::fsutil::atomic_write_bytes;
use lexo_core::messages::{msg, py_str, Message, Text};
use lexo_core::pyfmt::json_dumps;
use lexo_core::util::{py_strip, sha1_hex};
use regex::Regex;
use serde_json::{json, Map, Value};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const COMMIT_MESSAGE: &str = "chore(translation): update progress data";
pub const PROTECTED_BRANCHES: [&str; 2] = ["main", "master"];

/// A message for the user (`PublishError`).
pub enum Failure {
    Msg(Message),
    Plain(String),
}

impl Failure {
    pub fn into_error(self) -> ExtensionError {
        ExtensionError(match self {
            Failure::Msg(m) => Text::new_msg(m),
            Failure::Plain(s) => Text::Plain(s),
        })
    }
}

impl From<Message> for Failure {
    fn from(m: Message) -> Failure {
        Failure::Msg(m)
    }
}

type Res<T> = Result<T, Failure>;

struct GitOut {
    ok: bool,
    stdout: String,
    stderr: String,
}

fn drain(pipe: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut p) = pipe {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    })
}

/// `git [-C repo] args` without a shell, without a question on the terminal, with a time limit.
fn git(repo: Option<&Path>, args: &[&str], timeout: u64) -> Res<GitOut> {
    let mut cmd = Command::new("git");
    if let Some(r) = repo {
        cmd.arg("-C").arg(r);
    }
    cmd.args(args).env("GIT_TERMINAL_PROMPT", "0").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000);
    let mut child = cmd.spawn().map_err(|_| Failure::Msg(msg("Git is not installed or not in PATH (https://git-scm.com).", &[])))?;
    let (out, err) = (drain(child.stdout.take()), drain(child.stderr.take()));
    let deadline = Instant::now() + Duration::from_secs(timeout);
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Failure::Msg(msg("Git did not answer in time (sign-in? network?).", &[])));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => return Err(Failure::Msg(msg("Git did not answer in time (sign-in? network?).", &[]))),
        }
    };
    Ok(GitOut {
        ok: status.success(),
        stdout: String::from_utf8_lossy(&out.join().unwrap_or_default()).into_owned(),
        stderr: String::from_utf8_lossy(&err.join().unwrap_or_default()).into_owned(),
    })
}

/// The last 300 characters of what git said.
fn tail(r: &GitOut) -> String {
    let t = if r.stderr.is_empty() { &r.stdout } else { &r.stderr };
    let t = py_strip(t);
    let n = t.chars().count();
    t.chars().skip(n.saturating_sub(300)).collect()
}

fn home() -> PathBuf {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default()
}

pub fn default_clone_dir(spec: &str) -> PathBuf {
    let last = spec.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    let name = last.strip_suffix(".git").unwrap_or(last);
    home().join(".progress-web").join(if name.is_empty() { "repo" } else { name })
}

pub fn repo_url(spec: &str) -> Option<String> {
    let spec = py_strip(spec);
    let short = Regex::new(r"^[\w.-]+/[\w.-]+$").unwrap();
    let drive = Regex::new(r"^[A-Za-z]:").unwrap();
    if short.is_match(spec) && !drive.is_match(spec) {
        return Some(format!("https://github.com/{spec}.git"));
    }
    Regex::new(r"^(https?://|git@|file:)").unwrap().is_match(spec).then(|| spec.to_string())
}

/// `(path, url)`: a local folder if there is one, or the default clone folder and the URL to clone.
pub fn locate_repo(spec: &str) -> (Option<PathBuf>, Option<String>) {
    let expanded = match spec.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with(['/', '\\']) => home().join(rest.trim_start_matches(['/', '\\'])),
        _ => PathBuf::from(spec),
    };
    if expanded.is_dir() {
        return (Some(expanded), None);
    }
    match repo_url(spec) {
        None => (None, None),
        Some(url) => (Some(default_clone_dir(&url)), Some(url)),
    }
}

fn sorted(v: &Value) -> Value {
    match v {
        Value::Object(o) => {
            let mut keys: Vec<&String> = o.keys().collect();
            keys.sort();
            Value::Object(keys.into_iter().map(|k| (k.clone(), sorted(&o[k]))).collect::<Map<_, _>>())
        }
        Value::Array(a) => Value::Array(a.iter().map(sorted).collect()),
        other => other.clone(),
    }
}

/// Content identity for the confirmation token: the data without its timestamp.
pub fn canonical(data: &Value) -> String {
    let mut d = data.clone();
    if let Some(o) = d.as_object_mut() {
        o.shift_remove("updated");
    }
    json_dumps(&sorted(&d), false)
}

fn summ(d: &Value) -> Value {
    match (d.get("translation").and_then(|t| t.get("percent")), d.get("proofreading").and_then(|t| t.get("percent"))) {
        (Some(t), Some(p)) => json!({"translation": t, "proofreading": p, "updated": d.get("updated").cloned().unwrap_or(Value::Null)}),
        _ => Value::Null,
    }
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// Read-only description of a publish. `web` is the config's web section, `text` the JSON text.
pub fn plan(web: &Value, data: &Value) -> Res<Map<String, Value>> {
    let repo = web.get("repo").map(py_str).unwrap_or_default();
    if repo.is_empty() {
        return Err(msg("The website is not set up – fill in the repository in Settings.", &[]).into());
    }
    let target = export::clean_target(&web.get("path").map(py_str).unwrap_or_default()).map_err(|m| Failure::Plain(m.text()))?;
    let (path, url) = locate_repo(&repo);
    let Some(path) = path else { return Err(msg("The repository “{repo}” is neither a folder nor a GitHub address.", &[("repo", json!(repo))]).into()) };
    let will_clone = url.is_some() && !path.join(".git").is_dir();
    let mut info = json!({
        "repo": repo, "repo_path": path.to_string_lossy(), "url": url, "target": target, "will_clone": will_clone, "branch": "", "remote": "", "upstream": "", "dirty": [], "current": null,
        "changed": true, "blockers": [], "warnings": [], "commands": [], "new": summ(data), "live": false,
    });
    let digest = sha1_hex(format!("{}|{}|{}", canonical(data), path.to_string_lossy(), target).as_bytes());
    info["token"] = json!(&digest[..16]);
    let push = |info: &mut Value, key: &str, v: Value| info[key].as_array_mut().unwrap().push(v);
    if will_clone {
        let url = url.clone().unwrap_or_default();
        push(&mut info, "warnings", msg(
            "The repository is not downloaded yet. When publishing, it is cloned from {url} and the data is sent to its default branch (usually main = the live website).",
            &[("url", json!(url))],
        ).wire());
        info["live"] = json!(true); // the default branch of a new clone is usually the live website
        info["commands"] = json!([
            format!("git clone {url} \"{}\"", path.to_string_lossy()),
            format!("write {target}"),
            format!("git add -- {target}"),
            format!("git commit -m \"{COMMIT_MESSAGE}\" -- {target}"),
            "git push"
        ]);
        return Ok(info.as_object().cloned().unwrap_or_default());
    }
    if !path.join(".git").exists() {
        return Err(msg("The folder is not a git repository: {path}", &[("path", json!(path.to_string_lossy()))]).into());
    }
    let branch = git(Some(&path), &["rev-parse", "--abbrev-ref", "HEAD"], 120)?;
    if !branch.ok {
        return Err(msg("The git repository cannot be read: {output}", &[("output", json!(tail(&branch)))]).into());
    }
    let branch_name = py_strip(&branch.stdout).to_string();
    info["branch"] = json!(branch_name);
    if branch_name == "HEAD" {
        push(&mut info, "blockers", msg("The repository is in a detached HEAD – switch to a branch.", &[]).wire());
    }
    let up = git(Some(&path), &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"], 120)?;
    if up.ok {
        info["upstream"] = json!(py_strip(&up.stdout));
    } else {
        push(
            &mut info,
            "blockers",
            msg("The branch {branch} has no remote branch (upstream) set – a push would go nowhere.", &[("branch", json!(branch_name))]).wire(),
        );
    }
    let rem = git(Some(&path), &["remote", "get-url", "origin"], 120)?;
    info["remote"] = json!(if rem.ok { py_strip(&rem.stdout).to_string() } else { String::new() });
    let st = git(Some(&path), &["status", "--porcelain"], 120)?;
    if st.ok {
        let dirty: Vec<String> = st.stdout.lines().map(|l| l.chars().skip(3).collect::<String>()).filter(|l| l.replace('\\', "/") != target).take(20).collect();
        info["dirty"] = json!(dirty);
    }
    let file = path.join(&target);
    if file.is_file() {
        let parsed = std::fs::read(&file)
            .ok()
            .and_then(|b| String::from_utf8(b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&b).to_vec()).ok())
            .and_then(|t| serde_json::from_str::<Value>(&t).ok());
        match parsed {
            Some(cur) => {
                info["current"] = summ(&cur);
                info["changed"] = json!(canonical(&cur) != canonical(data));
            }
            None => push(&mut info, "warnings", msg("The existing file in the repository cannot be read; it will be overwritten.", &[]).wire()),
        }
    } else {
        push(&mut info, "warnings", msg("The file in the repository does not exist yet, it will be created.", &[]).wire());
    }
    if PROTECTED_BRANCHES.contains(&branch_name.as_str()) {
        push(
            &mut info,
            "warnings",
            msg("A push to the branch {branch} deploys the website live (Cloudflare Workers).", &[("branch", json!(branch_name))]).wire(),
        );
        info["live"] = json!(true);
    }
    let dirty_n = info["dirty"].as_array().map_or(0, Vec::len);
    if dirty_n > 0 {
        push(
            &mut info,
            "warnings",
            msg("There are other uncommitted changes in the repository ({n}); publishing does not touch them.", &[("n", json!(dirty_n))]).wire(),
        );
    }
    if info["changed"] == json!(false) {
        push(&mut info, "warnings", msg("The published data does not differ from the current data (except the time) – nothing to publish.", &[]).wire());
    }
    info["commands"] = json!(strs(&["git pull --ff-only"])
        .into_iter()
        .chain([format!("write {target}"), format!("git add -- {target}"), format!("git commit -m \"{COMMIT_MESSAGE}\" -- {target}"), "git push".to_string()])
        .collect::<Vec<_>>());
    Ok(info.as_object().cloned().unwrap_or_default())
}

fn restore(dest: &Path, before: &Option<Vec<u8>>) {
    match before {
        None => {
            if dest.exists() {
                let _ = std::fs::remove_file(dest);
            }
        }
        Some(b) => {
            let _ = atomic_write_bytes(dest, b, None, &SystemClock);
        }
    }
}

fn blockers_text(info: &Map<String, Value>) -> String {
    info["blockers"].as_array().into_iter().flatten().map(|b| lexo_core::messages::py_str(&lexo_core::messages::render(b))).collect::<Vec<_>>().join(" ")
}

/// Run the publish. Fails with a message (nothing, or only a reverted state, is left behind).
pub fn publish(web: &Value, data: &Value, text: &str, token: &str, confirmed_branch: &str) -> Res<Value> {
    let mut info = plan(web, data)?;
    if token != info["token"].as_str().unwrap_or("") {
        return Err(msg("The data or the repository changed since the preview – open the preview again.", &[]).into());
    }
    if !info["blockers"].as_array().is_none_or(Vec::is_empty) {
        return Err(Failure::Plain(blockers_text(&info)));
    }
    let path = PathBuf::from(info["repo_path"].as_str().unwrap_or(""));
    let target = info["target"].as_str().unwrap_or("").to_string();
    let mut log: Vec<Value> = Vec::new();
    let will_clone = info["will_clone"] == json!(true);
    if will_clone {
        if confirmed_branch != "*clone*" {
            return Err(msg("Cloning and publishing was not confirmed.", &[]).into());
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let r = git(None, &["clone", info["url"].as_str().unwrap_or(""), &path.to_string_lossy()], 600)?;
        if !r.ok {
            return Err(Failure::Plain(format!("git clone selhal: {}", tail(&r))));
        }
        log.push(msg("Repository cloned.", &[]).wire());
        info = plan(web, data)?;
        if !info["blockers"].as_array().is_none_or(Vec::is_empty) {
            return Err(Failure::Plain(blockers_text(&info)));
        }
    } else if confirmed_branch != info["branch"].as_str().unwrap_or("") {
        return Err(
            msg("The confirmed branch “{a}” does not match the current one “{b}”.", &[("a", json!(confirmed_branch)), ("b", info["branch"].clone())]).into()
        );
    }
    if info["changed"] == json!(false) && !will_clone {
        return Ok(json!({"ok": true, "message": msg("No change – the website is already up to date.", &[]).wire(), "log": log}));
    }
    let r = git(Some(&path), &["pull", "--ff-only"], 300)?;
    if !r.ok {
        return Err(msg("git pull --ff-only failed (nothing changed): {output}", &[("output", json!(tail(&r)))]).into());
    }
    let dest = path.join(&target);
    let before = std::fs::read(&dest).ok().filter(|_| dest.is_file());
    let payload = text.as_bytes();
    let written: Result<(), Failure> = (|| {
        serde_json::from_slice::<Value>(payload)
            .map_err(|e| msg("The file in the repository cannot be written: {reason}", &[("reason", json!(e.to_string()))]))?;
        atomic_write_bytes(&dest, payload, None, &SystemClock)
            .map_err(|e| msg("The file in the repository cannot be written: {reason}", &[("reason", e.reason.wire())]))?;
        let back: Value = std::fs::read(&dest).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);
        if back.get("schema") != data.get("schema") {
            return Err(msg("The written file did not pass the check.", &[]).into());
        }
        Ok(())
    })();
    if let Err(e) = written {
        restore(&dest, &before);
        return Err(e);
    }
    let rollback = |sha: Option<&str>| {
        if let Some(sha) = sha {
            if let Ok(head) = git(Some(&path), &["rev-parse", "HEAD"], 120) {
                if py_strip(&head.stdout) == sha {
                    let _ = git(Some(&path), &["reset", "--soft", "HEAD~1"], 120);
                }
            }
        }
        let _ = git(Some(&path), &["reset", "-q", "--", &target], 120);
        restore(&dest, &before);
    };
    let r = git(Some(&path), &["add", "--", &target], 120)?;
    if !r.ok {
        rollback(None);
        return Err(Failure::Plain(format!("git add selhal: {}", tail(&r))));
    }
    if git(Some(&path), &["diff", "--cached", "--quiet", "--", &target], 120)?.ok {
        rollback(None);
        return Ok(json!({"ok": true, "message": msg("No change – the website is already up to date.", &[]).wire(), "log": log}));
    }
    let r = git(Some(&path), &["commit", "-m", COMMIT_MESSAGE, "--", &target], 120)?;
    if !r.ok {
        rollback(None);
        return Err(msg("git commit failed (the file was restored to its previous state): {output}", &[("output", json!(tail(&r)))]).into());
    }
    let sha = py_strip(&git(Some(&path), &["rev-parse", "HEAD"], 120)?.stdout).to_string();
    let r = git(Some(&path), &["push"], 300)?;
    if !r.ok {
        rollback(Some(&sha));
        return Err(
            msg("git push failed. The local commit and the file were restored to their previous state: {output}", &[("output", json!(tail(&r)))]).into()
        );
    }
    let branch = match info["branch"].as_str().filter(|b| !b.is_empty()) {
        Some(b) => json!(b),
        None => msg("default", &[]).wire(),
    };
    Ok(
        json!({"ok": true, "message": msg("Published to the branch {branch} ({sha}).", &[("branch", branch), ("sha", json!(&sha[..sha.len().min(7)]))]).wire(), "log": log, "commit": sha}),
    )
}
