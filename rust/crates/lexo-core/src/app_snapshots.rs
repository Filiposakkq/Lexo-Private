//! Port of `reference/python/core/lexo/app_snapshots.py`: snapshots (zip backups) of the translation folder.
//!
//! Known difference from Python: `maybe_auto_snapshot` makes the snapshot at once; Python does it in a background thread. The server that runs the application decides
//! whether to call it from another thread.
use crate::api::{ok, plain, Answer, Args};
use crate::app::App;
use crate::app_base::{app_err, ApiError, Res};
use crate::messages::{msg, Text};
use crate::snapshots;
use crate::texthistory;
use crate::VERSION;
use serde_json::{json, Value};

impl App {
    fn snap_cfg(&self, key: &str, default: Value) -> Value {
        self.config.get(&format!("snapshots.{key}")).cloned().unwrap_or(default)
    }

    /// Zip of all .rpy files (see `snapshots.rs`). Never fails; problems become a notice.
    pub fn snapshot(&mut self, reason: &str, force: bool) -> Option<Value> {
        let enabled = crate::util::truthy(&self.snap_cfg("enabled", json!(true)));
        if !force && !enabled {
            return None;
        }
        let keep = self.snap_cfg("keep", json!(20)).as_i64().unwrap_or(20);
        let clock = &*self.clock;
        let o = self.open.as_ref()?;
        if reason == "bulk" && snapshots::recent(&o.local_dir, 10.0, clock) {
            return None;
        }
        match snapshots::create(&o.project.root, &o.local_dir, reason, VERSION, clock) {
            Ok(Some(snap)) => {
                snapshots::prune(&o.local_dir, keep);
                Some(Value::Object(snap))
            }
            Ok(None) => None,
            Err(e) => {
                self.notices.push(Text::new_msg(msg("The backup was not created: {reason}", &[("reason", json!(e.0.text()))])));
                None
            }
        }
    }

    pub fn snapshot_for_bulk(&mut self, lines: usize, files: usize) {
        if lines >= 20 || files >= 2 {
            self.snapshot("bulk", false);
        }
    }

    /// At start: a snapshot when the last one is older than the interval.
    pub fn maybe_auto_snapshot(&mut self) -> Res<()> {
        let Some(o) = self.open.as_ref() else { return Ok(()) };
        if !self.cfg_bool("snapshots.enabled", true) {
            return Ok(());
        }
        let interval = self.snap_cfg("interval_hours", json!(12)).as_f64().unwrap_or(12.0);
        if !snapshots::due(&o.local_dir, interval, &*self.clock) {
            return Ok(());
        }
        self.snapshot("auto", false);
        Ok(())
    }

    pub fn snapshot_list(&self) -> Res<Value> {
        let o = self.o()?;
        let items: Vec<Value> = snapshots::list_snapshots(&o.local_dir).into_iter().map(Value::Object).collect();
        Ok(
            json!({"items": items, "enabled": self.snap_cfg("enabled", json!(true)), "interval_hours": self.snap_cfg("interval_hours", json!(12)), "keep": self.snap_cfg("keep", json!(20))}),
        )
    }

    pub fn snapshot_create(&mut self) -> Res<Value> {
        self.need_project()?;
        let keep = self.snap_cfg("keep", json!(20)).as_i64().unwrap_or(20);
        let o = self.o()?;
        let snap = snapshots::create(&o.project.root, &o.local_dir, "manual", VERSION, &*self.clock).map_err(|e| ApiError::App(Text::new_msg(e.0)))?;
        let Some(snap) = snap else { return Err(app_err(msg("There are no .rpy files in the folder.", &[]))) };
        snapshots::prune(&o.local_dir, keep);
        Ok(json!({"ok": true, "snapshot": snap}))
    }

    pub fn snapshot_files(&self, name: &str) -> Res<Value> {
        let o = self.o()?;
        let files = snapshots::compare(&o.project.root, &o.local_dir, name).map_err(|e| ApiError::App(Text::new_msg(e.0)))?;
        Ok(json!({"files": files.into_iter().map(|(f, s)| json!({"file": f, "state": s})).collect::<Vec<_>>()}))
    }

    /// Put files of a snapshot back. The current state is saved first, so the restore can be undone the same way.
    pub fn snapshot_restore(&mut self, name: &str, files: &Value) -> Res<Value> {
        self.need_project()?;
        let wanted: Option<Vec<String>> = files.as_array().map(|a| a.iter().map(crate::messages::py_str).collect());
        let clock = &*self.clock;
        let o = self.open.as_mut().unwrap();
        let before: Vec<(String, Vec<crate::rpy::Entry>)> = o.project.files.keys().map(|rel| (rel.clone(), o.project.entries(rel).to_vec())).collect();
        let res = snapshots::restore(&o.project.root, &o.local_dir, name, wanted.as_deref(), VERSION, clock);
        let _ = o.project.refresh(true, clock);
        o.val_cache.clear();
        let (restored, safety) = res.map_err(|e| ApiError::App(Text::new_msg(e.0)))?;
        let mut logged: Vec<Value> = Vec::new();
        for rel in o.project.files.keys() {
            if let Some((_, old)) = before.iter().find(|(r, _)| r == rel) {
                for c in texthistory::diff_files(old, o.project.entries(rel)) {
                    logged.push(json!({"file": rel, "key": [c.key.0, c.key.1], "old": c.old, "new": c.new}));
                }
            }
        }
        let names: Vec<String> = o.project.rows(&o.store).into_iter().map(|r| r.name).collect();
        o.store.ensure_files(&names, clock);
        self.log_text(&logged, "restore");
        let mut out = self.changed("restore")?;
        out.insert("restored".into(), json!(restored));
        out.insert("safety".into(), safety.and_then(|s| s.get("name").cloned()).unwrap_or(json!("")));
        Ok(Value::Object(out))
    }

    pub fn snapshot_delete(&self, name: &str) -> Res<Value> {
        let o = self.o()?;
        snapshots::delete(&o.local_dir, name).map_err(|e| ApiError::App(Text::new_msg(e.0)))?;
        Ok(json!({"ok": true}))
    }
}

// ---------------------------------------------------------------- the wrappers of api.rs
pub fn op_snapshots(app: &mut App, _a: &Args) -> Res<Answer> {
    ok(app.snapshot_list()?)
}

pub fn op_snapshots_files(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.snapshot_files(a.s("name"))?)
}

pub fn op_snapshots_create(app: &mut App, _a: &Args) -> Res<Answer> {
    plain(app.snapshot_create()?)
}

pub fn op_snapshots_restore(app: &mut App, a: &Args) -> Res<Answer> {
    plain(app.snapshot_restore(a.s("name"), a.a("files"))?)
}

pub fn op_snapshots_delete(app: &mut App, a: &Args) -> Res<Answer> {
    plain(app.snapshot_delete(a.s("name"))?)
}
