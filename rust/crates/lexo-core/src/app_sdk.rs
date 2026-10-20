//! A Ren'Py SDK for a game that has no engine of its own (no `<Game>.py` with a `lib` folder beside it). The guide uses, in this order: the one the user chose, one that Aside downloaded earlier,
//! one that lies near the game. When there is none, the guide **asks the user** and, only after a click, downloads the official SDK from renpy.org (about 160 MB; the checksum published beside
//! it is checked). The SDK must fit the game: Python 3 (`py3`) is Ren'Py 8, Python 2 (`py2`) is Ren'Py 7 and older.
use crate::app::App;
use crate::app_base::{app_err, Res};
use crate::app_prepare::{jobs, with_job, Job};
use crate::diskspace;
use crate::lint::{self, Platform};
use crate::messages::{msg, Message};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Downloads a file: (url, where to write it, progress(received, total)). A plain function, so that a test can put a fake in its place.
pub type FetchFn = fn(&str, &Path, &dyn Fn(u64, u64)) -> Result<(), String>;

const BASE: &str = "https://www.renpy.org/dl";
/// (family, version, size of the zip in bytes): the last release of Ren'Py 8 and of Ren'Py 7.
pub const SDK_RELEASES: [(&str, &str, u64); 2] = [("py3", "8.5.3", 163_008_121), ("py2", "7.8.7", 164_677_000)];
/// Room needed beside the zip: the unpacked SDK takes about twice as much.
const NEED_BYTES: u64 = 1_000_000_000;

/// `py3` or `py2` by the folder of a Python (`lib/py3-windows-x86_64/python.exe`).
pub fn family_of(python: &Path) -> Option<&'static str> {
    python.components().find_map(|c| {
        let n = c.as_os_str().to_string_lossy();
        if n.starts_with("py3-") {
            Some("py3")
        } else if n.starts_with("py2-") {
            Some("py2")
        } else {
            None
        }
    })
}

/// Which Python the game was made for, by what is in its `lib` folder; `None` when it carries none.
pub fn game_family(root: &Path) -> Option<&'static str> {
    let names: Vec<String> = std::fs::read_dir(root.join("lib")).ok()?.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    if names.iter().any(|n| n.starts_with("py3-")) {
        Some("py3")
    } else if names.iter().any(|n| n.starts_with("py2-")) {
        Some("py2")
    } else {
        None
    }
}

/// The SDK in `dir` or one folder below it (an archive unpacks into `renpy-8.5.3-sdk/`): (folder, its Python).
fn sdk_in(dir: &Path) -> Option<(PathBuf, PathBuf)> {
    let direct = |d: &Path| if d.join("renpy.py").is_file() { lint::find_python(d, Platform::current()).map(|p| (d.to_path_buf(), p)) } else { None };
    if let Some(found) = direct(dir) {
        return Some(found);
    }
    let mut below: Vec<PathBuf> = std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    below.sort();
    below.iter().find_map(|d| direct(d))
}

fn human(bytes: u64) -> String {
    format!("{:.0} MB", bytes as f64 / (1024.0 * 1024.0))
}

pub fn no_engine() -> Message {
    msg("The game has no engine of its own, so a Ren'Py SDK is needed to prepare it: choose the folder of one, or let Aside download it.", &[])
}

impl App {
    fn sdk_downloads_dir(&self) -> PathBuf {
        self.cfg_dir.join("sdk")
    }

    /// An SDK that fits the game: (folder, Python, where it comes from: `chosen`, `downloaded` or `found`).
    pub(crate) fn sdk_find(&self, root: &Path, family: Option<&str>) -> Option<(PathBuf, PathBuf, &'static str)> {
        let fits = |python: &Path| family.is_none_or(|f| family_of(python) == Some(f));
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(self.prepare_settings_file()).unwrap_or_default()).unwrap_or(Value::Null);
        let mut chosen: Vec<String> = ["py3", "py2"].iter().filter_map(|f| saved["sdk"][*f].as_str().map(str::to_string)).collect();
        chosen.extend(self.config.get("lint.sdk").and_then(|v| v.as_str().map(str::to_string)));
        for p in chosen.iter().filter(|p| !p.is_empty()) {
            if let Some((dir, py)) = sdk_in(Path::new(p)).filter(|(_, py)| fits(py)) {
                return Some((dir, py, "chosen"));
            }
        }
        for f in ["py3", "py2"] {
            if let Some((dir, py)) = sdk_in(&self.sdk_downloads_dir().join(f)).filter(|(_, py)| fits(py)) {
                return Some((dir, py, "downloaded"));
            }
        }
        let mut near: Vec<PathBuf> = Vec::new();
        for up in [root.parent(), root.parent().and_then(Path::parent)].into_iter().flatten() {
            near.extend(
                std::fs::read_dir(up)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_dir() && p.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase().starts_with("renpy"))),
            );
        }
        near.sort();
        near.iter().find_map(|d| sdk_in(d).filter(|(_, py)| fits(py))).map(|(dir, py)| (dir, py, "found"))
    }

    /// What can be downloaded for a game of this family: `[{family, version, size, human}]`.
    pub(crate) fn sdk_offers(family: Option<&str>) -> Value {
        json!(SDK_RELEASES
            .iter()
            .filter(|(f, _, _)| family.is_none_or(|g| g == *f))
            .map(|(f, v, s)| json!({"family": f, "version": v, "size": s, "human": human(*s)}))
            .collect::<Vec<_>>())
    }

    /// Remember an SDK the user points at (the folder that holds `renpy.py`, or the one that holds that folder).
    pub fn prepare_set_sdk(&mut self, path: &str) -> Res<Value> {
        let p = PathBuf::from(crate::util::py_strip(path).trim_matches('"'));
        let Some((dir, py)) = sdk_in(&p) else {
            return Err(app_err(msg("A Ren'Py SDK was not found there: {path}", &[("path", json!(path))])));
        };
        let family = family_of(&py).unwrap_or("py3");
        self.prepare_settings_set(&["sdk", family], json!(dir.to_string_lossy()))?;
        Ok(json!({"ok": true, "sdk": dir.to_string_lossy(), "family": family}))
    }

    /// Download the SDK of a family (`py3` = Ren'Py 8, `py2` = Ren'Py 7) as a job: `{job}`. The caller has asked the user first.
    pub fn prepare_sdk_download(&mut self, family: &str) -> Res<Value> {
        self.prepare_sdk_download_with(family, fetch_http)
    }

    pub fn prepare_sdk_download_with(&mut self, family: &str, fetch: FetchFn) -> Res<Value> {
        let Some(&(_, version, size)) = SDK_RELEASES.iter().find(|(f, _, _)| *f == family) else {
            return Err(app_err(msg("This kind of SDK is not known: {family}", &[("family", json!(family))])));
        };
        let root = self.sdk_downloads_dir();
        std::fs::create_dir_all(&root).map_err(|e| app_err(msg("The download failed: {reason}", &[("reason", json!(e.to_string()))])))?;
        if let Some(free) = diskspace::free_bytes(&root).filter(|f| *f < NEED_BYTES.max(size * 3)) {
            return Err(app_err(msg(
                "There is not enough room for the SDK on the disk of the settings: it needs about {need} and {free} is free.",
                &[("need", json!(human(NEED_BYTES.max(size * 3)))), ("free", json!(human(free)))],
            )));
        }
        let id = format!("sdk-{}", crate::fsutil::token_hex(4));
        {
            let mut all = jobs().lock().unwrap_or_else(|e| e.into_inner());
            if all.values().any(|j| !j.done) {
                return Err(app_err(msg("A game is being prepared; wait until it is done.", &[])));
            }
            all.insert(id.clone(), Job { current: "download".into(), ..Job::default() });
        }
        let (job, family, ver) = (id.clone(), family.to_string(), version.to_string());
        std::thread::spawn(move || run_download(&job, &root, &family, &ver, fetch));
        Ok(json!({"ok": true, "job": id, "version": version}))
    }
}

fn run_download(id: &str, root: &Path, family: &str, version: &str, fetch: FetchFn) {
    let fail = |m: Message| {
        let wire = m.wire();
        with_job(id, move |j| {
            j.done = true;
            j.ok = false;
            j.error = wire;
        })
    };
    let step = |name: &'static str| {
        with_job(id, move |j| {
            if j.current != name {
                j.done_steps.push(std::mem::replace(&mut j.current, name.into()));
            }
        })
    };
    let target = root.join(family);
    if let Some((dir, _)) = sdk_in(&target) {
        with_job(id, |j| {
            j.done = true;
            j.ok = true;
            j.result = json!({"sdk": dir.to_string_lossy(), "family": family, "reused": true});
        });
        return;
    }
    let part = root.join(format!("{family}.part"));
    let _ = std::fs::remove_dir_all(&part);
    if let Err(e) = std::fs::create_dir_all(&part) {
        return fail(msg("The download failed: {reason}", &[("reason", json!(e.to_string()))]));
    }
    let name = format!("renpy-{version}-sdk.zip");
    let (zip_path, sums_path) = (part.join(&name), part.join("checksums.txt"));
    let url = format!("{BASE}/{version}");
    let failed = |e: String| msg("The download failed: {reason}", &[("reason", json!(e))]);
    if let Err(e) = fetch(&format!("{url}/checksums.txt"), &sums_path, &|_, _| {}) {
        let _ = std::fs::remove_dir_all(&part);
        return fail(failed(e));
    }
    let progress = |received: u64, total: u64| with_job(id, move |j| j.result = json!({"received": received, "total": total}));
    if let Err(e) = fetch(&format!("{url}/{name}"), &zip_path, &progress) {
        let _ = std::fs::remove_dir_all(&part);
        return fail(failed(e));
    }
    step("verify");
    let expected = std::fs::read_to_string(&sums_path).ok().and_then(|t| sha256_of(&t, &name));
    let Some(expected) = expected else {
        let _ = std::fs::remove_dir_all(&part);
        return fail(msg("The checksum of the download could not be found.", &[]));
    };
    if file_sha256(&zip_path).as_deref() != Some(expected.as_str()) {
        let _ = std::fs::remove_dir_all(&part);
        return fail(msg("The download is damaged (the checksum does not match). Try again.", &[]));
    }
    step("unpack");
    let unpacked = part.join("sdk");
    if let Err(e) = unzip(&zip_path, &unpacked) {
        let _ = std::fs::remove_dir_all(&part);
        return fail(msg("The SDK could not be unpacked: {reason}", &[("reason", json!(e))]));
    }
    let _ = std::fs::remove_file(&zip_path);
    let _ = std::fs::remove_dir_all(&target);
    if let Err(e) = std::fs::rename(&unpacked, &target) {
        let _ = std::fs::remove_dir_all(&part);
        return fail(msg("The SDK could not be unpacked: {reason}", &[("reason", json!(e.to_string()))]));
    }
    let _ = std::fs::remove_dir_all(&part);
    let Some((dir, _)) = sdk_in(&target) else {
        return fail(msg("The SDK could not be unpacked: {reason}", &[("reason", json!("renpy.py is missing"))]));
    };
    with_job(id, |j| {
        j.done_steps.push(std::mem::take(&mut j.current));
        j.done = true;
        j.ok = true;
        j.result = json!({"sdk": dir.to_string_lossy(), "family": family, "reused": false});
    });
}

/// The SHA-256 of `name` from the `# sha256` section of `checksums.txt` (the lines are `<hash> <file>`).
fn sha256_of(text: &str, name: &str) -> Option<String> {
    let mut in_section = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            in_section = line.eq_ignore_ascii_case("# sha256");
        } else if in_section {
            let mut parts = line.split_whitespace();
            if let (Some(hash), Some(file)) = (parts.next(), parts.next()) {
                if file.trim_start_matches('*') == name && hash.len() == 64 {
                    return Some(hash.to_lowercase());
                }
            }
        }
    }
    None
}

fn file_sha256(path: &Path) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn unzip(zip_path: &Path, into: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(into).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = entry.enclosed_name() else { continue }; // never outside the folder
        let out = into.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut w = std::fs::File::create(&out).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut w).map_err(|e| e.to_string())?;
        w.flush().map_err(|e| e.to_string())?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode));
        }
    }
    Ok(())
}

/// The real download (https, redirects followed), written to `dest` in pieces.
fn fetch_http(url: &str, dest: &Path, progress: &dyn Fn(u64, u64)) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new().timeout_connect(std::time::Duration::from_secs(30)).timeout_read(std::time::Duration::from_secs(60)).build();
    let resp = agent.get(url).call().map_err(|e| e.to_string())?;
    let total = resp.header("Content-Length").and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    let mut reader = resp.into_reader();
    let mut out = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    let (mut buf, mut received) = (vec![0u8; 1 << 16], 0u64);
    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        received += n as u64;
        progress(received, total);
    }
    Ok(())
}
