//! Port of `core/lexo/editor.py`: open a translation file at an exact line in VS Code or Notepad++.
//!
//! Commands are always built as an argument list (never a shell string) and the file must be a `.rpy` file inside the project, so a hostile file name cannot inject anything.
//!   VS Code:    `Code.exe -g "<file>:<line>"`        Notepad++:  `notepad++.exe -n<line> "<file>"`
//! When the preferred editor is missing the other known editor is tried, then the system default (which cannot jump to a line: the message says so).
//! The programs that look for an editor, start it and open a file are passed in, so the tests start nothing.
use crate::lint::Platform;
use crate::messages::{msg, Message};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

/// Characters that cmd.exe would take for something else.
const SHELL_UNSAFE: &str = "&|<>^%\"\r\n";

/// The environment variables the search for an editor reads.
pub type Env = Map<String, Value>;

fn env_path(env: &Env, key: &str) -> PathBuf {
    PathBuf::from(env.get(key).and_then(Value::as_str).unwrap_or(""))
}

fn first_existing(paths: &[PathBuf]) -> Option<String> {
    paths.iter().find(|p| !p.as_os_str().is_empty() && p.is_file()).map(|p| p.to_string_lossy().into_owned())
}

/// Looks a program up on the PATH.
pub type Which<'a> = &'a dyn Fn(&str) -> Option<String>;

/// The real `shutil.which`: the PATH of the process (with the extensions of PATHEXT on Windows).
pub fn which(name: &str) -> Option<String> {
    let p = Path::new(name);
    if p.components().count() > 1 {
        return p.is_file().then(|| name.to_string());
    }
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT".into()).split(';').map(|s| s.to_string()).collect()
    } else {
        vec![String::new()]
    };
    for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        for ext in std::iter::once(String::new()).chain(exts.iter().cloned()) {
            let cand = dir.join(format!("{name}{ext}"));
            if cand.is_file() {
                return Some(cand.to_string_lossy().into_owned());
            }
        }
    }
    None
}

/// Path of Code.exe (preferred: runs without cmd.exe) or the `code` launcher.
pub fn find_vscode(which: Which, env: &Env, plat: Platform) -> Option<String> {
    if let Some(launcher) = which("code") {
        let lp = PathBuf::from(&launcher);
        let ext = lp.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if ext == "cmd" || ext == "bat" {
            let resolved = std::fs::canonicalize(&lp).unwrap_or(lp.clone());
            let exe = resolved.parent().and_then(Path::parent).map(|d| d.join("Code.exe"));
            return exe.and_then(|e| first_existing(&[e])).or(Some(launcher));
        }
        return Some(launcher);
    }
    if plat.is_windows() {
        return first_existing(&[
            env_path(env, "LOCALAPPDATA").join("Programs/Microsoft VS Code/Code.exe"),
            env_path(env, "ProgramFiles").join("Microsoft VS Code/Code.exe"),
            env_path(env, "ProgramFiles(x86)").join("Microsoft VS Code/Code.exe"),
        ]);
    }
    which("codium")
}

pub fn find_notepadpp(which: Which, env: &Env) -> Option<String> {
    if let Some(found) = which("notepad++") {
        return Some(found);
    }
    first_existing(&[
        env_path(env, "ProgramFiles").join("Notepad++/notepad++.exe"),
        env_path(env, "ProgramFiles(x86)").join("Notepad++/notepad++.exe"),
        env_path(env, "LOCALAPPDATA").join("Programs/Notepad++/notepad++.exe"),
    ])
}

pub fn build_command(kind: &str, exe: &str, path: &Path, line: i64) -> Option<Vec<String>> {
    match kind {
        "vscode" => Some(vec![exe.into(), "-g".into(), format!("{}:{line}", path.to_string_lossy())]),
        "notepadpp" => Some(vec![exe.into(), format!("-n{line}"), path.to_string_lossy().into_owned()]),
        _ => None,
    }
}

/// `shlex.split(text, posix=...)`: words separated by white space, with quotes. Without `posix` a quote only opens a quoted word at the start of a word and stays in it.
pub fn shlex_split(text: &str, posix: bool) -> Result<Vec<String>, String> {
    let chars: Vec<char> = text.chars().collect();
    let (mut out, mut i) = (Vec::new(), 0usize);
    let unclosed = || "No closing quotation".to_string();
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        let mut token = String::new();
        if !posix {
            if chars[i] == '"' || chars[i] == '\'' {
                let q = chars[i];
                token.push(q);
                i += 1;
                loop {
                    let Some(&c) = chars.get(i) else { return Err(unclosed()) };
                    token.push(c);
                    i += 1;
                    if c == q {
                        break;
                    }
                }
            } else {
                while i < chars.len() && !chars[i].is_whitespace() {
                    token.push(chars[i]);
                    i += 1;
                }
            }
            out.push(token);
            continue;
        }
        while i < chars.len() && !chars[i].is_whitespace() {
            match chars[i] {
                '\'' => {
                    i += 1;
                    loop {
                        let Some(&c) = chars.get(i) else { return Err(unclosed()) };
                        i += 1;
                        if c == '\'' {
                            break;
                        }
                        token.push(c);
                    }
                }
                '"' => {
                    i += 1;
                    loop {
                        let Some(&c) = chars.get(i) else { return Err(unclosed()) };
                        i += 1;
                        if c == '"' {
                            break;
                        }
                        if c == '\\' && matches!(chars.get(i), Some('"') | Some('\\')) {
                            token.push(chars[i]);
                            i += 1;
                        } else {
                            token.push(c);
                        }
                    }
                }
                '\\' => {
                    i += 1;
                    match chars.get(i) {
                        Some(&c) => {
                            token.push(c);
                            i += 1;
                        }
                        None => return Err("No escaped character".into()),
                    }
                }
                c => {
                    token.push(c);
                    i += 1;
                }
            }
        }
        out.push(token);
    }
    Ok(out)
}

/// Expand a user template like `"C:\Tools\ed.exe" --line {line} {file}` into an argv list.
pub fn custom_command(template: &str, path: &Path, line: i64, plat: Platform) -> Result<Vec<String>, Message> {
    let mut argv: Vec<String> = shlex_split(template, !plat.is_windows())
        .map_err(|e| Message::new(&e, None, Default::default()))?
        .into_iter()
        .map(|a| a.trim_matches('"').to_string())
        .collect();
    if argv.is_empty() {
        return Err(msg("The custom command is empty.", &[]));
    }
    if !argv.iter().any(|a| a.contains("{file}")) {
        argv.push("{file}".into());
    }
    Ok(argv.into_iter().map(|a| a.replace("{file}", &path.to_string_lossy()).replace("{line}", &line.to_string())).collect())
}

fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).map(|c| PathBuf::from(c.to_string_lossy().trim_start_matches(r"\\?\"))).unwrap_or_else(|_| p.to_path_buf())
}

fn validate_target(path: &Path, root: Option<&Path>) -> Result<PathBuf, Message> {
    let p = canonical(path);
    if !p.is_file() {
        return Err(msg("The file does not exist: {path}", &[("path", json!(p.to_string_lossy()))]));
    }
    if p.extension().map(|e| e.to_string_lossy().to_lowercase()) != Some("rpy".into()) {
        return Err(msg("Only .rpy files can be opened.", &[]));
    }
    if let Some(root) = root {
        let r = canonical(root);
        if r != p && !p.starts_with(&r) {
            return Err(msg("The file is not inside the project folder.", &[]));
        }
    }
    Ok(p)
}

/// Finds the executable of an editor (`vscode` / `notepadpp`), or nothing.
pub type Finder<'a> = &'a dyn Fn(&str) -> Option<String>;
/// Starts a command; an error says why it could not be started.
pub type Starter<'a> = &'a mut dyn FnMut(&[String]) -> Result<(), String>;
/// Opens a file with the program of the system.
pub type Opener<'a> = &'a mut dyn FnMut(&Path) -> Result<(), String>;

fn text_of(v: &Value) -> String {
    crate::messages::render(v).as_str().map(String::from).unwrap_or_else(|| crate::messages::py_str(v))
}

/// Open `path` at `line`: {ok, editor, command, message, jumped}.
#[allow(clippy::too_many_arguments)]
pub fn open_in_editor(
    path: &Path,
    line: i64,
    cfg: &Value,
    root: Option<&Path>,
    runner: Starter,
    finder: Finder,
    opener: Opener,
    which_fn: Which,
    plat: Platform,
) -> Value {
    let get = |k: &str, d: &str| cfg.get(k).and_then(Value::as_str).unwrap_or(d).to_string();
    let (pref, custom) = (get("preferred", "auto"), get("custom", ""));
    let p = match validate_target(path, root) {
        Ok(p) => p,
        Err(m) => return json!({"ok": false, "editor": "", "command": [], "message": m.wire(), "jumped": false}),
    };
    let line = line.max(1);
    let order: Vec<&str> = match pref.as_str() {
        "vscode" => vec!["vscode", "notepadpp"],
        "notepadpp" => vec!["notepadpp", "vscode"],
        "custom" => vec!["custom", "vscode", "notepadpp"],
        "system" => vec![],
        _ => vec!["vscode", "notepadpp"],
    };
    let mut tried: Vec<Value> = Vec::new();
    for kind in order {
        let argv: Vec<String> = if kind == "custom" {
            if custom.trim().is_empty() {
                tried.push(msg("the custom command is not set", &[]).wire());
                continue;
            }
            match custom_command(&custom, &p, line, plat) {
                Ok(a) => {
                    let exe = a[0].clone();
                    if !(Path::new(&exe).is_file() || which_fn(&exe).is_some()) {
                        tried.push(msg("custom command: {exe} not found", &[("exe", json!(exe))]).wire());
                        continue;
                    }
                    a
                }
                Err(e) => {
                    tried.push(json!(format!("{kind}: {}", e.text())));
                    continue;
                }
            }
        } else {
            let label = if kind == "vscode" { "VS Code" } else { "Notepad++" };
            let Some(exe) = finder(kind) else {
                tried.push(msg("{label} not found", &[("label", json!(label))]).wire());
                continue;
            };
            let ext = Path::new(&exe).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if (ext == "cmd" || ext == "bat") && p.to_string_lossy().chars().any(|c| SHELL_UNSAFE.contains(c)) {
                tried.push(msg("{label}: the path contains characters that are dangerous for cmd.exe", &[("label", json!(label))]).wire());
                continue;
            }
            build_command(kind, &exe, &p, line).unwrap_or_default()
        };
        match runner(&argv) {
            Ok(()) => {
                let name = match kind {
                    "vscode" => json!("VS Code"),
                    "notepadpp" => json!("Notepad++"),
                    _ => msg("custom editor", &[]).wire(),
                };
                return json!({"ok": true, "editor": kind, "command": argv, "jumped": true, "message": msg("Opened in {name} at line {line}.", &[("name", name), ("line", json!(line))]).wire()});
            }
            Err(e) => tried.push(json!(format!("{kind}: {e}"))),
        }
    }
    // fallback: system default, no line number possible
    match opener(&p) {
        Ok(()) => {
            let list = if tried.is_empty() { "not set".to_string() } else { tried.iter().map(text_of).collect::<Vec<_>>().join("; ") };
            return json!({
                "ok": true, "editor": "system", "command": [p.to_string_lossy()], "jumped": false,
                "message": msg(
                    "VS Code and Notepad++ could not be started ({tried}). The file is open in the default program, but without jumping to line {line} – set the editor in Settings.",
                    &[("tried", json!(list)), ("line", json!(line))],
                ).wire(),
            });
        }
        Err(e) => tried.push(msg("default program: {e}", &[("e", json!(e))]).wire()),
    }
    let list = tried.iter().map(text_of).collect::<Vec<_>>().join("; ");
    json!({"ok": false, "editor": "", "command": [], "jumped": false, "message": msg("The file could not be opened: {tried}", &[("tried", json!(list))]).wire()})
}
