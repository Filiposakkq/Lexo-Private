//! Port of `core/lexo/app.py` and the state of `CorePart`: the application state and operations (no HTTP, no UI) that the interface in `api.rs` exposes.
//!
//! The Python `App` is assembled from mixins by area; here `App` is one struct and each area is a file (`app_core.rs`, `app_validation.rs`, `app_entries.rs` ...) with its own
//! `impl App` block and the wrappers (`op_*`) that `api.rs` calls. What a project has once it is open is in `Open`, so a method can borrow the project and the store together.
use crate::app_base::{app_err, ApiError, Res};
use crate::clock::{Clock, SystemClock};
use crate::config::Config;
use crate::extensions::ExtensionHost;
use crate::gameimages::GameImages;
use crate::history::History;
use crate::lang::{Pack, Packs};
use crate::lock::ProjectLock;
use crate::messages::{msg, Text};
use crate::project::Project;
use crate::spelling::Spelling;
use crate::store::Store;
use crate::texthistory::TextHistory;
use crate::validator::Issue;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

/// A finding of the checks: the line, the finding and the index of the entry it belongs to (`None` for a problem of the file's structure).
pub type Found = (usize, Issue, Option<usize>);

/// The findings of one file and what they were computed from.
pub struct CachedIssues {
    pub digest: String,
    pub sig: String,
    pub issues: Rc<Vec<Found>>,
}

/// The state of an open project.
pub struct Open {
    pub project: Project,
    pub store: Store,
    pub history: History,
    /// Shared data: `<translation folder>/.lexo`.
    pub data_dir: PathBuf,
    /// Local data: backups, snapshots, lock, derived files (`datadir.rs`).
    pub local_dir: PathBuf,
    pub texthist: TextHistory,
    pub plock: Option<ProjectLock>,
    /// rel -> findings of the file (rebuilt when the file, the settings or the glossary change).
    pub val_cache: HashMap<String, CachedIssues>,
    /// Spelling hints built from the whole project's translated text, with the key they were built for.
    pub spell_cache: Option<(String, Rc<Spelling>)>,
    pub pack: Pack,
    /// The index of the game's pictures, built once per game folder: (the folder that holds `game/`, the index).
    pub gi: Option<(PathBuf, Rc<GameImages>)>,
}

pub struct App {
    pub config: Config,
    pub ext: ExtensionHost,
    pub packs: Packs,
    pub clock: Box<dyn Clock>,
    /// The folder of the settings (the local data folders of projects are below it).
    pub cfg_dir: PathBuf,
    pub open: Option<Open>,
    /// One-off messages for the UI (migration, load problems ...).
    pub notices: Vec<Text>,
    pub last_save_error: String,
}

impl App {
    pub fn new(config: Config, packs: Packs) -> App {
        let cfg_dir = config.path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        App {
            config,
            ext: ExtensionHost::installed(),
            packs,
            clock: Box::new(SystemClock),
            cfg_dir,
            open: None,
            notices: Vec::new(),
            last_save_error: String::new(),
        }
    }

    pub fn ready(&self) -> bool {
        self.open.is_some()
    }

    pub fn need_project(&self) -> Res<()> {
        if self.open.is_none() {
            return Err(app_err(msg("No project is open.", &[])));
        }
        Ok(())
    }

    pub fn o(&self) -> Res<&Open> {
        self.open.as_ref().ok_or_else(|| app_err(msg("No project is open.", &[])))
    }

    pub fn o_mut(&mut self) -> Res<&mut Open> {
        self.open.as_mut().ok_or_else(|| app_err(msg("No project is open.", &[])))
    }

    /// A setting by its dotted name, `default` when it is not there.
    pub fn cfg(&self, dotted: &str) -> Option<&serde_json::Value> {
        self.config.get(dotted)
    }

    pub fn cfg_bool(&self, dotted: &str, default: bool) -> bool {
        self.config.get(dotted).map_or(default, |v| v.as_bool().unwrap_or_else(|| crate::util::truthy(v)))
    }
}

pub fn unknown_file() -> ApiError {
    app_err(msg("Unknown file.", &[]))
}

pub fn line_gone() -> ApiError {
    app_err(msg("The line is no longer in the file – refresh the files.", &[]))
}
