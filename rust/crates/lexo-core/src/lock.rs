//! Port of `reference/python/core/lexo/lock.py`: keeps two copies of the app from editing one project (and its data) at the same time.
//!
//! `lock.json` in the data folder names the owner (pid, computer, random token) and is touched every few seconds while the app runs. Another copy that finds a *fresh*
//! lock of a different process refuses to open the project unless the user takes it over. A lock that was not refreshed for `STALE_AFTER` seconds (crash, power loss)
//! is treated as free. The same process may always re-open its own project.
//! The heartbeat thread of the Python module is the caller's job here: it calls `beat()` every `BEAT_EVERY` seconds.
use crate::clock::Clock;
use crate::fsutil::{atomic_write_bytes, token_hex};
use crate::messages::{msg, Message};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

pub const LOCK_NAME: &str = "lock.json";
pub const STALE_AFTER: f64 = 60.0;
pub const BEAT_EVERY: f64 = 15.0;

/// Who is running: the process id and the computer name (the Python module reads them from the system; a test passes them in).
#[derive(Clone, Debug)]
pub struct Owner {
    pub pid: i64,
    pub host: String,
}

impl Owner {
    pub fn this_process() -> Owner {
        let host = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_else(|_| "unknown".into());
        Owner { pid: std::process::id() as i64, host }
    }
}

/// Another running copy of the app owns the project. `holder` is what `lock.json` says.
#[derive(Debug, PartialEq)]
pub struct ProjectLocked {
    pub holder: Map<String, Value>,
    pub message: Message,
}

impl ProjectLocked {
    fn new(holder: Map<String, Value>, now: f64) -> ProjectLocked {
        let t = holder.get("time").and_then(Value::as_f64).unwrap_or(0.0);
        let age = (now - t).max(0.0) as i64;
        let show = |k: &str| holder.get(k).cloned().unwrap_or_else(|| json!("?"));
        let message = msg(
            "The project is being used by another copy of the application (computer {host}, process {pid}, last active {age} s ago). Two copies at once would overwrite each other's data.",
            &[("host", show("host")), ("pid", show("pid")), ("age", json!(age))],
        );
        ProjectLocked { holder, message }
    }
}

fn read(path: &Path) -> Option<Map<String, Value>> {
    match serde_json::from_str::<Value>(&std::fs::read_to_string(path).ok()?).ok()? {
        Value::Object(o) => Some(o),
        _ => None,
    }
}

pub struct ProjectLock {
    pub path: PathBuf,
    pub token: String,
    pub held: bool,
    pub lost: bool,
    pub owner: Owner,
}

impl ProjectLock {
    pub fn new(data_dir: &Path, owner: Owner) -> ProjectLock {
        ProjectLock { path: data_dir.join(LOCK_NAME), token: token_hex(8), held: false, lost: false, owner }
    }

    fn payload(&self, clock: &dyn Clock) -> Value {
        json!({"pid": self.owner.pid, "host": self.owner.host, "token": self.token, "time": clock.epoch()})
    }

    /// Take the lock. Fails when a live foreign owner holds it (and `force` is false).
    pub fn acquire(&mut self, force: bool, clock: &dyn Clock) -> Result<(), Box<ProjectLocked>> {
        if let Some(cur) = read(&self.path) {
            if !force && cur.get("token").and_then(Value::as_str) != Some(self.token.as_str()) {
                let mine =
                    cur.get("pid").and_then(Value::as_i64) == Some(self.owner.pid) && cur.get("host").and_then(Value::as_str) == Some(self.owner.host.as_str());
                let time = cur.get("time").and_then(Value::as_f64).unwrap_or(0.0);
                if !mine && clock.epoch() - time < STALE_AFTER {
                    return Err(Box::new(ProjectLocked::new(cur, clock.epoch())));
                }
            }
        }
        self.write(clock);
        self.held = true;
        self.lost = false;
        Ok(())
    }

    fn write(&self, clock: &dyn Clock) {
        // a read-only folder must not stop the app; the lock is only a safety net
        let _ = atomic_write_bytes(&self.path, self.payload(clock).to_string().as_bytes(), None, clock);
    }

    /// One heartbeat: notices a takeover, otherwise touches the file. Returns false once the lock is lost.
    pub fn beat(&mut self, clock: &dyn Clock) -> bool {
        if let Some(cur) = read(&self.path) {
            if cur.get("token").and_then(Value::as_str) != Some(self.token.as_str()) {
                self.lost = true; // somebody took the project over
                return false;
            }
        }
        self.write(clock);
        true
    }

    /// True while we still own the lock (re-reads the file, so it also notices a takeover immediately).
    pub fn check(&mut self) -> bool {
        if !self.held {
            return true;
        }
        if let Some(cur) = read(&self.path) {
            if cur.get("token").and_then(Value::as_str) != Some(self.token.as_str()) {
                self.lost = true;
            }
        }
        !self.lost
    }

    pub fn release(&mut self) {
        if self.held && !self.lost {
            if let Some(cur) = read(&self.path) {
                if cur.get("token").and_then(Value::as_str) == Some(self.token.as_str()) {
                    let _ = std::fs::remove_file(&self.path);
                }
            }
        }
        self.held = false;
    }
}
