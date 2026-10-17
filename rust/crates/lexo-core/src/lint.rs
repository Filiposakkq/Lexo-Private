//! Port of `core/lexo/lint.py`: run `renpy lint` of the game from the Ren'Py SDK and show what it reports.
//!
//! Ren'Py's own lint knows things this manager's checks do not (missing images, unknown labels, broken translations). It needs the SDK folder (the one with `renpy.py` / `renpy.sh`);
//! that path is a setting. The command is built as an argument list (no shell). The program that runs it is passed in (`Runner`), so the tests do not need Ren'Py.
//! The background thread with a polled status (`Linter` in Python) belongs to the application layer and comes with the operations.
use crate::messages::{msg, Message};
use crate::util::py_strip;
use regex::Regex;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const TIMEOUT: u64 = 300;
pub const MAX_OUTPUT: usize = 200_000;

/// The system the program runs on (the Python module reads it from `sys.platform`; a test passes it in).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Platform {
    Windows,
    Mac,
    Linux,
}

impl Platform {
    pub fn current() -> Platform {
        if cfg!(windows) {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::Mac
        } else {
            Platform::Linux
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Platform::Windows => "windows",
            Platform::Mac => "mac",
            Platform::Linux => "linux",
        }
    }
    pub fn is_windows(self) -> bool {
        self == Platform::Windows
    }
}

#[derive(Debug, PartialEq)]
pub struct LintError(pub Message);

impl std::fmt::Display for LintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.text())
    }
}

/// Runs a command in a folder: (exit code, output text).
pub type Runner<'a> = &'a dyn Fn(&[String], &Path) -> Result<(i32, String), LintError>;

fn list_dir_prefixed(lib: &Path, prefix: &str) -> Vec<PathBuf> {
    match std::fs::read_dir(lib) {
        Ok(rd) => rd.filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().starts_with(prefix)).map(|e| e.path()).collect(),
        Err(_) => Vec::new(),
    }
}

/// The Python that ships with the SDK: `lib/py3-*/` (Ren'Py 8), `lib/py2-*/` or `lib/<platform>-<arch>/` (Ren'Py 7 and older).
pub fn find_python(sdk: &Path, plat: Platform) -> Option<PathBuf> {
    let lib = sdk.join("lib");
    if !lib.is_dir() {
        return None;
    }
    let plat_name = plat.name();
    let others: Vec<&str> = ["windows", "linux", "mac"].into_iter().filter(|o| *o != plat_name).collect();
    let pick = |prefix: &str| {
        let mut found: Vec<PathBuf> =
            list_dir_prefixed(&lib, prefix).into_iter().filter(|d| !others.iter().any(|o| d.file_name().unwrap().to_string_lossy().contains(o))).collect(); // never the Python of another platform
        found.sort_by_key(|d| {
            let n = d.file_name().unwrap().to_string_lossy().into_owned();
            (!n.contains(plat_name), !n.contains("x86_64"), n)
        });
        found
    };
    let mut dirs = pick("py3-");
    dirs.extend(pick("py2-"));
    dirs.extend(pick(&format!("{plat_name}-")));
    for d in dirs {
        for name in ["python.exe", "python"] {
            if d.join(name).is_file() {
                return Some(d.join(name));
            }
        }
    }
    None
}

/// The `<Game>.py` next to `<Game>.exe` / `<Game>.sh` in a built game: it is the same file as `renpy.py` in an SDK, so the game's own engine can run commands such as
/// `translate` without a separate SDK. `None` when there is no such pair.
pub fn find_launcher(folder: &Path) -> Option<PathBuf> {
    let mut pys: Vec<PathBuf> = std::fs::read_dir(folder)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.to_string_lossy().to_lowercase() == "py") && p.is_file())
        .collect();
    pys.sort();
    pys.into_iter().find(|py| {
        let name = py.file_name().unwrap().to_string_lossy().into_owned();
        let stem = py.file_stem().unwrap().to_string_lossy().into_owned();
        name != "renpy.py" && [".exe", ".sh"].iter().any(|ext| folder.join(format!("{stem}{ext}")).is_file())
    })
}

/// (argv, cwd) that runs a Ren'Py command (default `lint`) for the game folder `base` (the folder that contains game/).
pub fn build_command(sdk: &Path, base: &Path, command: &[&str], plat: Platform) -> Result<(Vec<String>, PathBuf), LintError> {
    if !sdk.is_dir() {
        return Err(LintError(msg("The Ren'Py SDK folder does not exist: {sdk}", &[("sdk", json!(sdk.to_string_lossy()))])));
    }
    let py = find_python(sdk, plat);
    let mut entry = sdk.join("renpy.py");
    if py.is_some() && !entry.is_file() {
        entry = find_launcher(sdk).unwrap_or(entry); // a built game carries its own engine
    }
    if let Some(py) = py.filter(|_| entry.is_file()) {
        let mut argv = vec![py.to_string_lossy().into_owned(), "-EO".into(), entry.to_string_lossy().into_owned(), base.to_string_lossy().into_owned()];
        argv.extend(command.iter().map(|c| c.to_string()));
        return Ok((argv, sdk.to_path_buf()));
    }
    let sh = sdk.join("renpy.sh");
    if sh.is_file() && !plat.is_windows() {
        let mut argv = vec![sh.to_string_lossy().into_owned(), base.to_string_lossy().into_owned()];
        argv.extend(command.iter().map(|c| c.to_string()));
        return Ok((argv, sdk.to_path_buf()));
    }
    Err(LintError(msg(
        "I did not find renpy.py and the built-in Python (lib/py3-…, lib/windows-x86_64 …) nor renpy.sh in the SDK. Is it the folder of an unpacked Ren'Py SDK?",
        &[],
    )))
}

/// One finding of lint: `game/script.rpy:123 text`.
#[derive(Debug, PartialEq)]
pub struct Issue {
    pub file: String,
    pub line: i64,
    pub message: String,
}

/// Lines like `game/script.rpy:123 text` -> findings; everything is also kept raw by the caller.
pub fn parse(output: &str) -> Vec<Issue> {
    static R: OnceLock<Regex> = OnceLock::new();
    let rx = R.get_or_init(|| Regex::new(r"^(?P<file>[^\s:][^:]*?\.rpym?):(?P<line>\d+)\s+(?P<msg>.+?)\s*$").unwrap());
    crate::util::py_splitlines(output)
        .iter()
        .filter_map(|ln| {
            let m = rx.captures(py_strip(ln))?;
            Some(Issue { file: m["file"].replace('\\', "/"), line: m["line"].parse().unwrap_or(0), message: m["msg"].to_string() })
        })
        .collect()
}

/// The result of one lint run.
#[derive(Debug)]
pub struct Linted {
    pub ok: bool,
    pub exit_code: i32,
    pub issues: Vec<Issue>,
    pub raw: String,
    pub seconds: f64,
}

impl Linted {
    pub fn to_json(&self) -> Value {
        json!({
            "ok": self.ok, "exit_code": self.exit_code, "raw": self.raw, "seconds": crate::pyfmt::round_digits(self.seconds, 1),
            "issues": self.issues.iter().map(|i| json!({"file": i.file, "line": i.line, "message": i.message})).collect::<Vec<_>>(),
        })
    }
}

/// Run lint synchronously. `runner(argv, cwd)` runs the command (the application starts the real program).
pub fn run(sdk: &Path, base: &Path, runner: Runner, plat: Platform) -> Result<Linted, LintError> {
    let (argv, cwd) = build_command(sdk, base, &["lint"], plat)?;
    let t0 = std::time::Instant::now();
    let (code, text) = runner(&argv, &cwd)?;
    let text: String = text.chars().take(MAX_OUTPUT).collect();
    Ok(Linted { ok: code == 0, exit_code: code, issues: parse(&text), raw: text, seconds: t0.elapsed().as_secs_f64() })
}
