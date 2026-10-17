//! Port of `core/lexo/gametools.py`: the tools that prepare a game for translating: unpack `.rpa` archives, decompile `.rpyc` scripts, generate the translation files.
//!
//! Rules that hold for all of them: the game folder is only READ (anything written goes to a workspace folder the caller chooses; the one exception is `generate_translation`,
//! whose product lives inside the project and which refuses to run unless the caller says so); archive entries cannot write outside the target folder; external programs are
//! started with an argument list (no shell) and a time limit, their output is kept. The pieces that start programs take a `Runner`, so the tests do not need Ren'Py or unrpyc.
//!
//! Known differences from Python: the reason of a failed file operation is the text of the Rust error, not the `strerror` of Python (the wording of the system differs);
//! a file copied between drives keeps its modification time but not its access time (`shutil.copy2` keeps both).
use crate::fsutil::resolve;
use crate::lint::{self, LintError, Platform};
use crate::messages::{msg, Message};
use crate::rpa::{self, RpaError};
use regex::Regex;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const TIMEOUT: u64 = 1800;
pub const MAX_OUTPUT: usize = 200_000;
const CHUNK: usize = 1024 * 1024;
pub const DEFAULT_MAX_FILE: u64 = 2 * 1024 * 1024 * 1024; // one file inside an archive
pub const DEFAULT_MAX_TOTAL: u64 = 16 * 1024 * 1024 * 1024; // everything one extraction writes
const SCRIPT_SUFFIXES: [&str; 2] = [".rpyc", ".rpymc"];
const SCRIPT_SOURCE_SUFFIXES: [&str; 4] = [".rpy", ".rpyc", ".rpym", ".rpymc"];
const LINK_SKIP_ROOT: [&str; 2] = ["_progress_data", ".lexo"]; // data of translation tools lying next to the game: never part of a workspace
const LINK_SKIP_DIRS: [&str; 3] = ["cache", "saves", "tl"]; // Ren'Py rewrites these; they are created fresh in the workspace
const COPY_SUFFIXES: [&str; 6] = [".rpy", ".rpyc", ".rpymc", ".rpyb", ".pyc", ".pyo"]; // files the engine may rewrite in place: copied, never linked

#[derive(Debug, PartialEq)]
pub struct ToolError(pub Message);

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.text())
    }
}

impl From<RpaError> for ToolError {
    fn from(e: RpaError) -> Self {
        ToolError(e.0)
    }
}

impl From<LintError> for ToolError {
    fn from(e: LintError) -> Self {
        ToolError(e.0)
    }
}

/// Runs a command in a folder: (exit code, output text).
pub type Runner<'a> = &'a dyn Fn(&[String], &Path) -> Result<(i32, String), ToolError>;

/// The text of an I/O error without the number the system adds (`Access is denied. (os error 5)`).
fn reason(e: &std::io::Error) -> String {
    let t = e.to_string();
    match t.rfind(" (os error ") {
        Some(i) if t.ends_with(')') => t[..i].to_string(),
        _ => t,
    }
}

fn io_err(e: std::io::Error) -> ToolError {
    ToolError(msg("{reason}", &[("reason", json!(reason(&e)))]))
}

fn text_of(p: &Path) -> Value {
    json!(p.to_string_lossy())
}

// ---------------------------------------------------------------- paths the way Python orders and compares them
/// The parts of a path as `PurePath` compares them: lower-cased on Windows.
fn keys(p: &Path) -> Vec<String> {
    p.components().map(|c| if cfg!(windows) { c.as_os_str().to_string_lossy().to_lowercase() } else { c.as_os_str().to_string_lossy().into_owned() }).collect()
}

/// `inner` lies below `outer` (Python: `outer in inner.parents`).
fn below(inner: &Path, outer: &Path) -> bool {
    let (i, o) = (keys(inner), keys(outer));
    i.len() > o.len() && i.starts_with(&o)
}

fn same(a: &Path, b: &Path) -> bool {
    keys(a) == keys(b)
}

fn sort_paths(v: &mut [PathBuf]) {
    v.sort_by_cached_key(|p| keys(p));
}

/// A name ends with a suffix (Windows ignores the case, like `Path.glob`).
fn has_suffix(name: &str, suffix: &str) -> bool {
    if cfg!(windows) {
        name.to_lowercase().ends_with(suffix)
    } else {
        name.ends_with(suffix)
    }
}

/// Everything below `root` (files and folders), without following links to folders: `Path.rglob("*")`.
fn rglob(root: &Path) -> Vec<PathBuf> {
    fn go(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        for e in rd.filter_map(|e| e.ok()) {
            let p = e.path();
            out.push(p.clone());
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                go(&p, out);
            }
        }
    }
    let mut out = Vec::new();
    go(root, &mut out);
    out
}

fn rglob_suffix(root: &Path, suffixes: &[&str]) -> Vec<PathBuf> {
    rglob(root).into_iter().filter(|p| p.file_name().is_some_and(|n| suffixes.iter().any(|s| has_suffix(&n.to_string_lossy(), s)))).collect()
}

fn rel_text(p: &Path, root: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

/// Copy a file and keep its modification time (`shutil.copy2`).
fn copy2(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::copy(src, dst)?;
    if let Ok(t) = fs::metadata(src).and_then(|m| m.modified()) {
        let _ = fs::OpenOptions::new().write(true).open(dst).and_then(|f| f.set_modified(t));
    }
    Ok(())
}

// ---------------------------------------------------------------- archives
/// The `.rpa` archives in or below `game_dir`, sorted.
pub fn find_archives(game_dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = rglob_suffix(game_dir, &[".rpa"]).into_iter().filter(|p| p.is_file()).collect();
    sort_paths(&mut v);
    v
}

/// `dest / name` when `name` stays inside `dest`, otherwise `None`. Handles `..`, absolute paths, drive letters and NUL.
pub fn safe_target(dest: &Path, name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains('\0') {
        return None;
    }
    let norm = name.replace('\\', "/");
    let mut cs = norm.chars();
    if norm.starts_with('/') || (cs.next().is_some_and(|c| c.is_ascii_alphabetic()) && cs.next() == Some(':')) {
        return None;
    }
    let parts: Vec<&str> = norm.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    if parts.is_empty() || parts.contains(&"..") {
        return None;
    }
    let dest = resolve(dest);
    let target = resolve(&parts.iter().fold(dest.clone(), |acc, p| acc.join(p)));
    below(&target, &dest).then_some(target)
}

/// `[(name, size)]` of an archive, sorted by name. Reads only the index.
pub fn list_archive(archive: &Path) -> Result<Vec<(String, u64)>, ToolError> {
    let (_ver, index) = rpa::read_index(archive)?;
    let mut v: Vec<(String, u64)> = index.into_iter().map(|(n, e)| (n, e.length)).collect();
    v.sort();
    Ok(v)
}

#[derive(Debug, Default)]
pub struct Extracted {
    pub written: Vec<String>,
    pub skipped: Vec<(String, Message)>,
    pub bytes: u64,
}

impl Extracted {
    pub fn to_json(&self) -> Value {
        json!({"written": self.written, "skipped": self.skipped.iter().map(|(n, m)| json!([n, m.wire()])).collect::<Vec<_>>(), "bytes": self.bytes})
    }
}

/// Unpack `archive` into `dest`. `only` = optional test of the entry name. Existing files are kept unless `overwrite`. An archive that cannot be used is an error, and so is a `dest` that is a file.
pub fn extract_rpa(
    archive: &Path,
    dest: &Path,
    only: Option<&dyn Fn(&str) -> bool>,
    overwrite: bool,
    max_file: u64,
    max_total: u64,
) -> Result<Extracted, ToolError> {
    if dest.exists() && !dest.is_dir() {
        return Err(ToolError(msg("The target is not a folder: {dest}", &[("dest", text_of(dest))])));
    }
    let (_ver, mut index) = rpa::read_index(archive)?;
    fs::create_dir_all(dest).map_err(io_err)?;
    index.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = Extracted::default();
    let mut src = fs::File::open(archive).map_err(io_err)?;
    let mut total = 0u64;
    for (name, entry) in &index {
        if only.is_some_and(|f| !f(name)) {
            continue;
        }
        let skip = |out: &mut Extracted, why: Message| out.skipped.push((name.clone(), why));
        let Some(target) = safe_target(dest, name) else {
            skip(&mut out, msg("the path would lead outside the target folder", &[]));
            continue;
        };
        if entry.length > max_file {
            skip(&mut out, msg("the file is too large", &[]));
            continue;
        }
        if total + entry.length > max_total {
            skip(&mut out, msg("the total limit of the extraction was exceeded", &[]));
            continue;
        }
        if target.exists() && !overwrite {
            skip(&mut out, msg("the file already exists", &[]));
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(io_err)?;
        }
        let mut tmp_name = target.file_name().unwrap_or_default().to_os_string();
        tmp_name.push(".part");
        let tmp = target.with_file_name(tmp_name);
        let written = (|| -> std::io::Result<()> {
            let mut file = fs::File::create(&tmp)?;
            file.write_all(&entry.prefix)?;
            src.seek(SeekFrom::Start(entry.offset))?;
            let mut left = entry.length.saturating_sub(entry.prefix.len() as u64);
            let mut buf = vec![0u8; CHUNK];
            while left > 0 {
                let n = src.read(&mut buf[..(CHUNK as u64).min(left) as usize])?;
                if n == 0 {
                    return Err(std::io::Error::other("the archive ends earlier than the index promises"));
                }
                file.write_all(&buf[..n])?;
                left -= n as u64;
            }
            drop(file);
            fs::rename(&tmp, &target)
        })();
        if let Err(e) = written {
            let _ = fs::remove_file(&tmp);
            skip(&mut out, msg("cannot be written: {reason}", &[("reason", json!(reason(&e)))]));
            continue;
        }
        out.written.push(name.clone());
        total += entry.length;
    }
    out.bytes = total;
    Ok(out)
}

// ---------------------------------------------------------------- workspace for a built game
#[derive(Debug, PartialEq)]
pub struct Workspace {
    pub linked: u64,
    pub copied: u64,
}

impl Workspace {
    pub fn to_json(&self) -> Value {
        json!({"linked": self.linked, "copied": self.copied})
    }
}

/// Build a working copy of a built game that costs (almost) no space: every file is a hard link to the original, except the folders Ren'Py rewrites (cache, saves, tl: not copied) and the
/// script/bytecode files (copied, so a rewrite cannot reach the original). A file is copied when a link is impossible (another drive), which can be large. The original is only read.
pub fn make_workspace(game_root: &Path, workspace: &Path) -> Result<Workspace, ToolError> {
    let (game_root, workspace) = (resolve(game_root), resolve(workspace));
    if same(&workspace, &game_root) || below(&workspace, &game_root) || below(&game_root, &workspace) {
        return Err(ToolError(msg("The workspace must not be inside the game folder.", &[])));
    }
    if !game_root.join("game").is_dir() {
        return Err(ToolError(msg("There is no game subfolder in the folder: {base}", &[("base", text_of(&game_root))])));
    }
    let mut done = Workspace { linked: 0, copied: 0 };
    fn go(src_dir: &Path, rel_dir: &Path, workspace: &Path, done: &mut Workspace) -> Result<(), ToolError> {
        let Ok(rd) = fs::read_dir(src_dir) else { return Ok(()) };
        let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let rel = rel_dir.join(e.file_name());
            let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
            if LINK_SKIP_ROOT.contains(&parts[0].as_str()) || (parts.len() > 1 && parts[0] == "game" && LINK_SKIP_DIRS.contains(&parts[1].as_str())) {
                continue;
            }
            let src = e.path();
            if src.is_dir() {
                fs::create_dir_all(workspace.join(&rel)).map_err(io_err)?;
                if e.file_type().is_ok_and(|t| t.is_dir()) {
                    go(&src, &rel, workspace, done)?;
                }
                continue;
            }
            if !src.is_file() {
                continue;
            }
            let dst = workspace.join(&rel);
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent).map_err(io_err)?;
            }
            if dst.exists() {
                continue;
            }
            let suffix = src.extension().map(|x| format!(".{}", x.to_string_lossy().to_lowercase())).unwrap_or_default();
            if !COPY_SUFFIXES.contains(&suffix.as_str()) && fs::hard_link(&src, &dst).is_ok() {
                done.linked += 1;
                continue;
            }
            copy2(&src, &dst).map_err(io_err)?;
            done.copied += 1;
        }
        Ok(())
    }
    go(&game_root, Path::new(""), &workspace, &mut done)?;
    Ok(done)
}

// ---------------------------------------------------------------- decompiling
/// Copy every `.rpyc` / `.rpymc` below `game_dir` into `workspace`, keeping the relative paths. The game is only read. Returns the number of copied files.
pub fn copy_scripts(game_dir: &Path, workspace: &Path) -> Result<u64, ToolError> {
    let (game_dir, workspace) = (resolve(game_dir), resolve(workspace));
    if same(&workspace, &game_dir) || below(&workspace, &game_dir) {
        return Err(ToolError(msg("The workspace must not be inside the game folder.", &[])));
    }
    let mut files: Vec<PathBuf> = rglob(&game_dir)
        .into_iter()
        .filter(|f| f.is_file())
        .filter(|f| f.extension().is_some_and(|x| SCRIPT_SUFFIXES.contains(&format!(".{}", x.to_string_lossy().to_lowercase()).as_str())))
        .collect();
    sort_paths(&mut files);
    let mut n = 0;
    for f in files {
        let target = workspace.join(f.strip_prefix(&game_dir).unwrap_or(&f));
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(io_err)?;
        }
        copy2(&f, &target).map_err(io_err)?;
        n += 1;
    }
    Ok(n)
}

/// Start a program, wait for it for `timeout` seconds and return (exit code, what it wrote to stdout followed by stderr).
pub fn run_process_for(argv: &[String], cwd: &Path, timeout: u64) -> Result<(i32, String), ToolError> {
    use std::process::{Command, Stdio};
    let Some((program, args)) = argv.split_first() else {
        return Err(ToolError(msg("The command could not be started: {reason}", &[("reason", json!("empty command"))])));
    };
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(cwd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000); // no console window
    let mut child = cmd.spawn().map_err(|e| ToolError(msg("The command could not be started: {reason}", &[("reason", json!(reason(&e)))])))?;
    fn drain(pipe: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<Vec<u8>> {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut buf);
            }
            buf
        })
    }
    let (out, err) = (drain(child.stdout.take()), drain(child.stderr.take()));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout);
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if std::time::Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ToolError(msg("The command took longer than {timeout} s and was stopped.", &[("timeout", json!(timeout))])));
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
            Err(e) => return Err(io_err(e)),
        }
    };
    let (out, err) = (out.join().unwrap_or_default(), err.join().unwrap_or_default());
    Ok((status.code().unwrap_or(-1), String::from_utf8_lossy(&out).into_owned() + &String::from_utf8_lossy(&err)))
}

/// The runner the application uses: `run_process_for` with the time limit of 30 minutes.
pub fn run_process(argv: &[String], cwd: &Path) -> Result<(i32, String), ToolError> {
    run_process_for(argv, cwd, TIMEOUT)
}

pub fn build_unrpyc_command(
    python: &Path,
    unrpyc_script: &Path,
    target: &Path,
    clobber: bool,
    try_harder: bool,
    no_init_offset: bool,
) -> Result<(Vec<String>, PathBuf), ToolError> {
    if !python.is_file() {
        return Err(ToolError(msg("The Python for unrpyc does not exist: {python}", &[("python", text_of(python))])));
    }
    if !unrpyc_script.is_file() {
        return Err(ToolError(msg("The script unrpyc.py does not exist: {script}", &[("script", text_of(unrpyc_script))])));
    }
    if !target.is_dir() {
        return Err(ToolError(msg("The workspace folder does not exist: {target}", &[("target", text_of(target))])));
    }
    let mut argv = vec![python.to_string_lossy().into_owned(), unrpyc_script.to_string_lossy().into_owned()];
    for (flag, on) in [("-c", clobber), ("--try-harder", try_harder), ("--no-init-offset", no_init_offset)] {
        if on {
            argv.push(flag.into());
        }
    }
    argv.push(target.to_string_lossy().into_owned());
    Ok((argv, unrpyc_script.parent().map(Path::to_path_buf).unwrap_or_default()))
}

/// What a program that was run for the game said.
#[derive(Debug)]
pub struct Decompiled {
    pub ok: bool,
    pub exit_code: i32,
    pub raw: String,
    pub written: Vec<String>,
    pub seconds: f64,
}

impl Decompiled {
    pub fn to_json(&self) -> Value {
        json!({"ok": self.ok, "exit_code": self.exit_code, "raw": self.raw, "written": self.written, "seconds": crate::pyfmt::round_digits(self.seconds, 1)})
    }
}

/// Run unrpyc on `workspace` (a folder with copied `.rpyc` files). `written` = the `.rpy` files (relative paths) that did not exist before.
pub fn decompile(
    python: &Path,
    unrpyc_script: &Path,
    workspace: &Path,
    clobber: bool,
    try_harder: bool,
    no_init_offset: bool,
    runner: Runner,
) -> Result<Decompiled, ToolError> {
    let (argv, cwd) = build_unrpyc_command(python, unrpyc_script, workspace, clobber, try_harder, no_init_offset)?;
    let before: BTreeSet<PathBuf> = rglob_suffix(workspace, &[".rpy"]).into_iter().collect();
    let t0 = std::time::Instant::now();
    let (code, text) = runner(&argv, &cwd)?;
    let after: BTreeSet<PathBuf> = rglob_suffix(workspace, &[".rpy"]).into_iter().collect();
    let mut written: Vec<String> = after.difference(&before).map(|p| rel_text(p, workspace)).collect();
    written.sort();
    Ok(Decompiled { ok: code == 0, exit_code: code, raw: text.chars().take(MAX_OUTPUT).collect(), written, seconds: t0.elapsed().as_secs_f64() })
}

// ---------------------------------------------------------------- the whole way from a built game to translation files
#[derive(Debug, PartialEq)]
pub struct ScriptsExtracted {
    pub archives: u64,
    pub written: u64,
    pub skipped: u64,
}

impl ScriptsExtracted {
    pub fn to_json(&self) -> Value {
        json!({"archives": self.archives, "written": self.written, "skipped": self.skipped})
    }
}

/// Unpack the scripts that sit inside the `.rpa` archives of the workspace's game folder into that folder (next to the archives). Existing files are kept.
pub fn extract_scripts(workspace: &Path) -> Result<ScriptsExtracted, ToolError> {
    let game = workspace.join("game");
    let mut done = ScriptsExtracted { archives: 0, written: 0, skipped: 0 };
    let mut archives: Vec<PathBuf> = fs::read_dir(&game)
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| has_suffix(&n.to_string_lossy(), ".rpa"))).collect())
        .unwrap_or_default();
    sort_paths(&mut archives);
    for arc in archives {
        let Ok(list) = list_archive(&arc) else { continue };
        let wanted: BTreeSet<String> =
            list.into_iter().map(|(n, _)| n).filter(|n| SCRIPT_SOURCE_SUFFIXES.iter().any(|s| n.to_lowercase().ends_with(s))).collect();
        if wanted.is_empty() {
            continue;
        }
        let res = extract_rpa(&arc, &game, Some(&|n: &str| wanted.contains(n)), false, DEFAULT_MAX_FILE, DEFAULT_MAX_TOTAL)?;
        done.archives += 1;
        done.written += res.written.len() as u64;
        done.skipped += res.skipped.len() as u64;
    }
    Ok(done)
}

/// unrpyc loses the indentation of the lines that follow a line ending in a backslash inside a python block, which makes Ren'Py refuse the file. Indent such lines to the block's level.
/// Returns (new text, number of lines changed); other text is left alone.
pub fn repair_continuations(text: &str) -> (String, usize) {
    use crate::util::{is_space, py_strip};
    let (mut out, mut changed, mut in_block, mut cont) = (Vec::new(), 0, false, false);
    for line in text.split('\n') {
        let mut line = line.to_string();
        let top = !py_strip(&line).is_empty() && !line.starts_with([' ', '\t']);
        let ends_colon_or_slash = |l: &str, c: char| l.trim_end_matches(is_space).ends_with(c);
        if cont && top {
            line = format!("    {line}");
            changed += 1;
        } else if top {
            in_block = ends_colon_or_slash(&line, ':') && line.contains("python");
        }
        cont = in_block && ends_colon_or_slash(&line, '\\');
        out.push(line);
    }
    (out.join("\n"), changed)
}

/// Apply `repair_continuations` to the decompiled files (relative paths under `game_dir`). Returns {file: lines changed} for the ones that changed.
pub fn repair_decompiled(game_dir: &Path, files: &[String]) -> Map<String, Value> {
    let mut done = Map::new();
    for rel in files {
        let path = game_dir.join(rel);
        let Ok(bytes) = fs::read(&path) else { continue };
        let Ok(text) = String::from_utf8(bytes) else { continue };
        let text = text.replace("\r\n", "\n").replace('\r', "\n"); // Python reads text with universal line ends
        let (new, n) = repair_continuations(&text);
        if n > 0 {
            if fs::write(&path, new.as_bytes()).is_err() {
                continue;
            }
            done.insert(rel.clone(), json!(n));
        }
    }
    done
}

fn parse_error_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"File "game/(?P<file>[^"]+\.rpym?)", line \d+"#).unwrap())
}

/// Move aside (rename to `<name>.unparsable`) the `.rpy` files that Ren'Py's output names as having errors, so the rest can still be translated. `protect` = relative prefixes that are never
/// touched (the folder being generated). Returns the moved relative paths.
pub fn quarantine_unparsable(game_dir: &Path, raw: &str, protect: &[&str]) -> Vec<String> {
    let names: BTreeSet<&str> = parse_error_re().captures_iter(raw).map(|c| c.name("file").unwrap().as_str()).collect();
    let mut moved = Vec::new();
    for rel in names {
        let path = game_dir.join(rel);
        if path.is_file() && !protect.iter().any(|p| rel.starts_with(p)) {
            let mut name = path.file_name().unwrap_or_default().to_os_string();
            name.push(".unparsable");
            if fs::rename(&path, path.with_file_name(name)).is_ok() {
                moved.push(rel.to_string());
            }
        }
    }
    moved
}

// ---------------------------------------------------------------- translation files
pub struct Plan {
    pub folder: String,
    pub exists: bool,
}

impl Plan {
    pub fn to_json(&self) -> Value {
        json!({"folder": self.folder, "exists": self.exists})
    }
}

/// What `generate_translation` would do, without doing it.
pub fn plan_translation(base: &Path, language: &str) -> Result<Plan, ToolError> {
    let mut cs = language.chars();
    let valid = cs.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && cs.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid {
        return Err(ToolError(msg("The language name may contain only letters, digits and an underscore and must not start with a digit.", &[])));
    }
    let folder = base.join("game").join("tl").join(language);
    Ok(Plan { folder: folder.to_string_lossy().into_owned(), exists: folder.exists() })
}

#[derive(Debug)]
pub struct Generated {
    pub ok: bool,
    pub exit_code: i32,
    pub raw: String,
    pub folder: String,
    pub files: Vec<String>,
    pub seconds: f64,
}

impl Generated {
    pub fn to_json(&self) -> Value {
        json!({"ok": self.ok, "exit_code": self.exit_code, "raw": self.raw, "folder": self.folder, "files": self.files, "seconds": crate::pyfmt::round_digits(self.seconds, 1)})
    }
}

/// Let the Ren'Py SDK generate `game/tl/<language>` for the project `base` (the folder that contains game/). It writes INTO the project, so the caller must pass
/// `write_into_project`, and an existing language folder is never touched.
pub fn generate_translation(sdk: &Path, base: &Path, language: &str, write_into_project: bool, runner: Runner, plat: Platform) -> Result<Generated, ToolError> {
    let plan = plan_translation(base, language)?;
    if !base.join("game").is_dir() {
        return Err(ToolError(msg("There is no game subfolder in the folder: {base}", &[("base", text_of(base))])));
    }
    if !write_into_project {
        return Err(ToolError(msg(
            "Generating writes into the project (game/tl/{language}); it must be confirmed explicitly.",
            &[("language", json!(language))],
        )));
    }
    if plan.exists {
        return Err(ToolError(msg("The folder {folder} already exists and I will not overwrite it.", &[("folder", json!(plan.folder))])));
    }
    let (argv, cwd) = lint::build_command(sdk, base, &["translate", language], plat)?;
    let t0 = std::time::Instant::now();
    let (code, text) = runner(&argv, &cwd)?;
    let folder = PathBuf::from(&plan.folder);
    let mut files: Vec<String> = if folder.is_dir() { rglob_suffix(&folder, &[".rpy"]).iter().map(|p| rel_text(p, &folder)).collect() } else { Vec::new() };
    files.sort();
    Ok(Generated {
        ok: code == 0 && !files.is_empty(),
        exit_code: code,
        raw: text.chars().take(MAX_OUTPUT).collect(),
        folder: plan.folder,
        files,
        seconds: t0.elapsed().as_secs_f64(),
    })
}

/// Everything between "a built game" and "game/tl/<language> exists", in a workspace; the game itself is only read: 1. `make_workspace` (links), 2. `extract_scripts` (from the archives),
/// 3. `decompile` (unrpyc) so that every script has a `.rpy`, 4. `generate_translation` with the game's own engine. `progress(step_name)` is called before each step. Returns a map with the
/// result of each step and `ok`. Stops at the first failing step.
#[allow(clippy::too_many_arguments)]
pub fn prepare_translation(
    game_root: &Path,
    workspace: &Path,
    language: &str,
    unrpyc_python: &Path,
    unrpyc_script: &Path,
    runner: Runner,
    progress: Option<&dyn Fn(&str)>,
    plat: Platform,
) -> Result<Map<String, Value>, ToolError> {
    let note = |step: &str| {
        if let Some(f) = progress {
            f(step)
        }
    };
    let mut out = Map::new();
    out.insert("ok".into(), json!(false));
    out.insert("workspace".into(), text_of(workspace));
    note("workspace");
    out.insert("workspace_files".into(), make_workspace(game_root, workspace)?.to_json());
    note("extract");
    out.insert("extract".into(), extract_scripts(workspace)?.to_json());
    note("decompile");
    let game = workspace.join("game");
    let dec = decompile(unrpyc_python, unrpyc_script, &game, false, true, false, runner)?;
    out.insert("decompile".into(), dec.to_json());
    if !dec.ok {
        return Ok(out);
    }
    out.insert("repaired".into(), Value::Object(repair_decompiled(&game, &dec.written)));
    note("generate");
    let mut quarantined: Vec<String> = Vec::new();
    out.insert("quarantined".into(), json!([]));
    let mut generated = None;
    for _attempt in 0..3 {
        let g = generate_translation(workspace, workspace, language, true, runner, plat)?;
        out.insert("generate".into(), g.to_json());
        let (ok, raw) = (g.ok, g.raw.clone());
        generated = Some(g);
        if ok {
            break;
        }
        let moved = quarantine_unparsable(&game, &raw, &[&format!("tl/{language}/")]);
        if moved.is_empty() {
            break;
        }
        quarantined.extend(moved);
        out.insert("quarantined".into(), json!(quarantined));
        let tl = game.join("tl").join(language);
        if tl.is_dir() && rglob_suffix(&tl, &[".rpy"]).is_empty() {
            // the failed run left an empty folder
            let _ = fs::remove_dir_all(&tl);
        }
    }
    out.insert("ok".into(), json!(generated.is_some_and(|g| g.ok)));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continuation_lines_are_indented_to_the_block() {
        let text = "init python:\n    x = 1 + \\\n2\nlabel a:\n    return";
        let (new, n) = repair_continuations(text);
        assert_eq!(n, 1);
        assert_eq!(new, "init python:\n    x = 1 + \\\n    2\nlabel a:\n    return");
    }

    #[test]
    fn names_that_lead_outside_are_refused() {
        let d = std::env::temp_dir();
        for bad in ["", "../x", "a/../../x", "/abs", "C:/x", "c:x", "a\0b", "./", "\\\\server\\x"] {
            assert!(safe_target(&d, bad).is_none(), "{bad:?}");
        }
        assert!(safe_target(&d, "a/./b\\c.rpy").is_some());
    }

    #[test]
    fn a_program_is_run_and_its_output_kept() {
        let exe = std::env::current_exe().unwrap();
        let (code, text) = run_process_for(&[exe.to_string_lossy().into_owned(), "--list".into()], exe.parent().unwrap(), 60).unwrap();
        assert_eq!(code, 0);
        assert!(text.contains("a_program_is_run_and_its_output_kept"));
        assert!(run_process_for(&["no-such-program-anywhere".into()], Path::new("."), 5).is_err());
    }
}
