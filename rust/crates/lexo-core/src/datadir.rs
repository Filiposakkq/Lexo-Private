//! Port of `core/lexo/datadir.py`: where the data of a project lives (docs/DATA-LAYOUT.md).
//!
//! `<translation folder>/.aside/` is shared, small, text files a team can put in git; `<config folder>/projects/<project id>/` is local data (backups, snapshots, the lock, text history).
//! The shared folder holds a one-line file `project-id`; the local folder is named after it, so the local data follows the project when the folder is moved.
use crate::clock::Clock;
use crate::fsutil::{atomic_write_text, token_hex};
use std::path::{Path, PathBuf};

pub const SHARED_NAME: &str = ".aside";
/// The name before the application was called Aside (and its layout file).
pub const LEGACY_SHARED_NAME: &str = ".lexo";
pub const LEGACY_LAYOUT_NAME: &str = "lexo.json";

/// A project folder from the time when the application was called Lexo: renames `.lexo` to `.aside` and `lexo.json` to `aside.json` (once; nothing changes when the new folder
/// exists already or the folder cannot be renamed).
pub fn adopt_legacy(root: &std::path::Path) {
    let (old, new) = (root.join(LEGACY_SHARED_NAME), root.join(SHARED_NAME));
    if old.is_dir() && !new.exists() {
        let _ = std::fs::rename(&old, &new);
    }
    let (legacy, current) = (new.join(LEGACY_LAYOUT_NAME), new.join(crate::layout::FILE_NAME));
    if legacy.is_file() && !current.exists() {
        let _ = std::fs::rename(&legacy, &current);
    }
}
pub const ID_FILE: &str = "project-id";

/// The id stored in the shared folder; created on first use. Empty when the folder cannot be written.
pub fn project_id(shared: &Path, clock: &dyn Clock) -> String {
    let path = shared.join(ID_FILE);
    if let Ok(text) = std::fs::read_to_string(&path) {
        let pid = crate::util::py_strip(&text);
        if pid.chars().all(char::is_alphanumeric) && !pid.is_empty() && pid.chars().count() >= 8 {
            return pid.to_string();
        }
    }
    let pid = token_hex(6);
    match atomic_write_text(&path, &format!("{pid}\n"), None, None, clock) {
        Ok(()) => pid,
        Err(_) => String::new(), // a read-only folder: the caller falls back to the folder name
    }
}

fn resolved(root: &Path) -> String {
    let p = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let s = p.to_string_lossy().into_owned();
    s.strip_prefix(r"\\?\").map(String::from).unwrap_or(s)
}

/// The local data folder of the project (created).
pub fn local_dir(shared: &Path, root: &Path, config_dir: &Path, clock: &dyn Clock) -> PathBuf {
    let mut pid = project_id(shared, clock);
    if pid.is_empty() {
        let safe: String = resolved(root).chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
        let tail: String = safe.chars().rev().take(40).collect::<Vec<_>>().into_iter().rev().collect();
        pid = format!("path-{tail}");
    }
    let d = config_dir.join("projects").join(pid);
    let _ = std::fs::create_dir_all(&d);
    d
}

#[cfg(test)]
mod adopt_tests {
    use super::adopt_legacy;

    #[test]
    fn the_old_folder_and_layout_file_are_renamed_once() {
        let tmp = std::env::temp_dir().join(format!("aside-adopt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join(".lexo")).unwrap();
        std::fs::write(tmp.join(".lexo/lexo.json"), "{}").unwrap();
        std::fs::write(tmp.join(".lexo/project-id"), "abc12345\n").unwrap();
        adopt_legacy(&tmp);
        assert!(!tmp.join(".lexo").exists() && tmp.join(".aside/aside.json").is_file());
        assert_eq!(std::fs::read_to_string(tmp.join(".aside/project-id")).unwrap(), "abc12345\n");
        adopt_legacy(&tmp); // nothing more to do
        assert!(tmp.join(".aside/aside.json").is_file());
        // when the new folder exists, the old one is left alone
        std::fs::create_dir_all(tmp.join(".lexo")).unwrap();
        adopt_legacy(&tmp);
        assert!(tmp.join(".lexo").is_dir());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
