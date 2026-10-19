//! Sharing the translation folder with git, with two buttons for people who do not use a command line: **Get changes** (pull) and **Send changes** (commit and push). Free, for every user of a
//! shared folder; it only runs the `git` program that is installed on the computer, in the translation folder, and never touches the settings of git.
//!
//! What it does, and what it deliberately does not do:
//! * **Get changes:** `git pull --rebase --autostash`. If that does not go through (a real conflict, no connection, a password is needed) the half-done rebase is cancelled (`rebase --abort`), the
//!   folder is left as it was, and the answer of git is shown. Aside never resolves a conflict by itself.
//! * **Send changes:** `git add -A` (the translation folder only), a commit named "Translation update by <name>" when something changed, then `git push`. If the server has newer changes the changes
//!   are got first (as above) and the push is tried once more.
//! * git is told never to ask in a terminal (`GIT_TERMINAL_PROMPT=0`), so a missing password is an error and not a hang; a saved login of the system (a credential manager, an SSH key) works.
//!
//! These calls are not in the contract of Core.
use crate::app::App;
use crate::app_base::{app_err, Res};
use crate::messages::msg;
use serde_json::{json, Value};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(120);

struct Out {
    ok: bool,
    code: Option<i32>,
    text: String,
}

/// Run `git <args>` in `dir`; `None` when git cannot be started (it is not installed).
fn git(dir: &Path, args: &[&str]) -> Option<Out> {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(dir).env("GIT_TERMINAL_PROMPT", "0").env("LC_ALL", "C").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000); // no console window
    let mut child = cmd.spawn().ok()?;
    let (mut so, mut se) = (child.stdout.take()?, child.stderr.take()?);
    let reader = std::thread::spawn(move || {
        let (mut a, mut b) = (String::new(), String::new());
        let _ = so.read_to_string(&mut a);
        let _ = se.read_to_string(&mut b);
        (a, b)
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if start.elapsed() > TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            Err(_) => break None,
        }
    };
    let (a, b) = reader.join().unwrap_or_default();
    let text =
        format!("{}{}", a.trim(), if b.trim().is_empty() { String::new() } else { format!("{}{}", if a.trim().is_empty() { "" } else { "\n" }, b.trim()) });
    Some(match status {
        Some(s) => Out { ok: s.success(), code: s.code(), text },
        None => Out { ok: false, code: None, text: "git did not answer in time".to_string() },
    })
}

/// The first lines of what git said, for a message.
fn brief(text: &str) -> String {
    text.lines().filter(|l| !l.trim().is_empty()).take(6).collect::<Vec<_>>().join("\n").chars().take(600).collect()
}

fn failed(what: &str, o: &Out) -> crate::app_base::ApiError {
    app_err(msg("{what} did not work: {reason}", &[("what", msg(what, &[]).wire()), ("reason", json!(brief(&o.text)))]))
}

impl App {
    fn git_dir(&self) -> Res<std::path::PathBuf> {
        Ok(self.o()?.project.root.clone())
    }

    /// `{available, repo, branch, remote, upstream, behind, ahead, changes}`: what the sharing buttons show.
    pub fn git_state(&mut self) -> Res<Value> {
        self.need_project()?;
        let dir = self.git_dir()?;
        if git(&dir, &["--version"]).is_none_or(|o| !o.ok) {
            return Ok(json!({"ok": true, "available": false, "repo": false}));
        }
        let inside = git(&dir, &["rev-parse", "--is-inside-work-tree"]).is_some_and(|o| o.ok && o.text.trim() == "true");
        if !inside {
            return Ok(json!({"ok": true, "available": true, "repo": false}));
        }
        let text = |args: &[&str]| git(&dir, args).filter(|o| o.ok).map(|o| o.text.trim().to_string()).unwrap_or_default();
        let counts = text(&["rev-list", "--left-right", "--count", "@{u}...HEAD"]);
        let mut it = counts.split_whitespace().map(|n| n.parse::<i64>().unwrap_or(0));
        let (behind, ahead) = (it.next().unwrap_or(0), it.next().unwrap_or(0));
        Ok(json!({
            "ok": true, "available": true, "repo": true,
            "branch": text(&["rev-parse", "--abbrev-ref", "HEAD"]),
            "remote": text(&["remote", "get-url", "origin"]),
            "upstream": !counts.is_empty(), "behind": behind, "ahead": ahead,
            "changes": text(&["status", "--porcelain", "--", "."]).lines().filter(|l| !l.trim().is_empty()).count(),
        }))
    }

    fn git_repo(&self) -> Res<std::path::PathBuf> {
        self.need_project()?;
        let dir = self.git_dir()?;
        match git(&dir, &["rev-parse", "--is-inside-work-tree"]) {
            None => Err(app_err(msg("git is not installed on this computer.", &[]))),
            Some(o) if o.ok && o.text.trim() == "true" => Ok(dir),
            Some(_) => Err(app_err(msg("This folder is not a git repository.", &[]))),
        }
    }

    /// `git pull --rebase --autostash`; on a failure the rebase is cancelled and the folder stays as it was. One case is mended: a new member opened the project before getting the shared data, so
    /// Aside made its own `.aside` files, and the shared ones would overwrite them. Those files (and only files inside `.aside/`) are moved to the local data of the project and the pull is tried again.
    fn git_pull_inner(&self, dir: &Path) -> Res<String> {
        let none = || app_err(msg("git is not installed on this computer.", &[]));
        let mut o = git(dir, &["pull", "--rebase", "--autostash"]).ok_or_else(none)?;
        if !o.ok && o.text.contains("untracked working tree files would be overwritten") {
            let files: Vec<String> = o.text.lines().filter(|l| l.starts_with('\t')).map(|l| l.trim().to_string()).collect();
            let only_ours = !files.is_empty() && files.iter().all(|f| f.starts_with(".aside/") || f.contains("/.aside/"));
            let root = git(dir, &["rev-parse", "--show-toplevel"]).filter(|r| r.ok).map(|r| std::path::PathBuf::from(r.text.trim()));
            if let (true, Some(root)) = (only_ours, root) {
                let backup = self.o()?.local_dir.join("git-backup");
                for f in &files {
                    let from = root.join(f);
                    let to = backup.join(f);
                    if let Some(p) = to.parent() {
                        let _ = std::fs::create_dir_all(p);
                    }
                    if std::fs::rename(&from, &to).is_err() && std::fs::copy(&from, &to).is_ok() {
                        let _ = std::fs::remove_file(&from);
                    }
                }
                o = git(dir, &["pull", "--rebase", "--autostash"]).ok_or_else(none)?;
            }
        }
        if !o.ok {
            let _ = git(dir, &["rebase", "--abort"]);
            return Err(failed("Getting the changes", &o));
        }
        Ok(brief(&o.text))
    }

    /// Get the changes of the others (Free): pull, then read the files again.
    pub fn git_pull(&mut self) -> Res<Value> {
        let dir = self.git_repo()?;
        let said = self.git_pull_inner(&dir)?;
        self.refresh(true, false, true)?;
        let mut state = self.git_state()?;
        state["said"] = json!(said);
        Ok(state)
    }

    /// Send your changes (Free): add the translation folder, commit when something changed, push; when the server is ahead, get its changes and try once more.
    pub fn git_push(&mut self) -> Res<Value> {
        let dir = self.git_repo()?;
        let me = self.author_id();
        let who = self.team_members().ok().and_then(|m| m.get(&me).cloned()).filter(|n| !n.is_empty()).unwrap_or_else(|| me.clone());
        let run = |args: &[&str], what: &str| -> Res<Out> {
            let o = git(&dir, args).ok_or_else(|| app_err(msg("git is not installed on this computer.", &[])))?;
            if o.ok {
                Ok(o)
            } else {
                Err(failed(what, &o))
            }
        };
        run(&["add", "-A", "--", "."], "Adding the changes")?;
        let staged = git(&dir, &["diff", "--cached", "--quiet", "--", "."]).is_some_and(|o| o.code == Some(1));
        if staged {
            let message = format!("Translation update by {who}");
            let has_name = git(&dir, &["config", "user.name"]).is_some_and(|o| o.ok && !o.text.trim().is_empty());
            let (name_arg, mail_arg) = (format!("user.name={who}"), format!("user.email={me}@aside.invalid"));
            let mut args: Vec<&str> = Vec::new();
            if !has_name {
                args.extend(["-c", name_arg.as_str(), "-c", mail_arg.as_str()]);
                // only when git does not know who you are
            }
            args.extend(["commit", "-m", message.as_str()]);
            run(&args, "Saving the changes")?;
        }
        let has_upstream = git(&dir, &["rev-parse", "--abbrev-ref", "@{u}"]).is_some_and(|o| o.ok);
        let push_args: Vec<&str> = if has_upstream { vec!["push"] } else { vec!["push", "-u", "origin", "HEAD"] };
        let mut pushed = git(&dir, &push_args).ok_or_else(|| app_err(msg("git is not installed on this computer.", &[])))?;
        if !pushed.ok && (pushed.text.contains("rejected") || pushed.text.contains("fetch first") || pushed.text.contains("non-fast-forward")) {
            self.git_pull_inner(&dir)?; // the server has newer changes: get them first, then once more
            self.refresh(true, false, true)?;
            pushed = git(&dir, &push_args).ok_or_else(|| app_err(msg("git is not installed on this computer.", &[])))?;
        }
        if !pushed.ok {
            return Err(failed("Sending the changes", &pushed));
        }
        let mut state = self.git_state()?;
        state["committed"] = json!(staged);
        Ok(state)
    }
}
