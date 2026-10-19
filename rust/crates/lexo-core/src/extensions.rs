//! Port of `reference/python/core/lexo/extensions.py`: the extension host, how a separately delivered Pro pack plugs into Core and how a Pro feature is called.
//!
//! Core and Free hold only the catalog of gated features (`features.rs`), this host ("is there a provider for this feature, and what does it offer?") and the calls themselves.
//! The implementation, the licence check and the limits of the demo live in the Pro pack. Without a provider the host answers "not available" and nothing else can change that.
//!
//! The contract between the host and a provider is plain JSON on purpose: a call is a feature id, a method name and a value that can be written as JSON, the answer is such a
//! value, an error is an `ExtensionError`. The host sends every payload through JSON and back, so a provider can never receive or return a live object, and the transport is
//! replaceable: the provider can be a separate program that speaks the same JSON.
//!
//! The Pro pack is a separate program, the sidecar (`lexo-pro-sidecar`), that the application starts and talks to on its standard input and output, one JSON line per request and
//! per answer (see `SidecarProvider`). Without it every gated feature is locked. The Python pack (`lexo_pro`) is not loaded by this host.
use crate::features::Decision;
use crate::messages::Text;
use serde_json::{json, Map, Value};

/// The version of this contract; a provider with another one is refused, not half used.
pub const API: i64 = 1;

#[derive(Debug, PartialEq)]
pub struct ExtensionError(pub Text);

impl std::fmt::Display for ExtensionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.text())
    }
}

pub trait Provider {
    /// The version of the contract it speaks.
    fn api(&self) -> i64;
    /// The version of the pack (shown to the user).
    fn version(&self) -> String;
    /// The interface texts of the messages the pack writes: `{English template: translation}`.
    fn ui_strings(&self, _code: &str) -> Map<String, Value> {
        Map::new()
    }
    /// What it can do right now: `{feature id: {"limit": null | N}}`.
    fn offers(&self) -> Map<String, Value>;
    fn call(&self, feature: &str, method: &str, payload: &Value) -> Result<Value, ExtensionError>;
}

#[derive(Default)]
pub struct ExtensionHost {
    providers: Vec<Box<dyn Provider>>,
    /// Why a pack was not used (shown in the settings, never fatal).
    pub problems: Vec<String>,
}

fn plain(text: String) -> ExtensionError {
    ExtensionError(Text::Plain(text))
}

impl ExtensionHost {
    pub fn new() -> ExtensionHost {
        ExtensionHost::default()
    }

    /// The host of a normal start: uses the Pro pack program (the sidecar) when there is one next to the application or named by `LEXO_PRO_SIDECAR`; `LEXO_NO_PRO` leaves it out
    /// (what a Free installation looks like). A pack that cannot be started is a note in `problems`, never an error.
    pub fn installed() -> ExtensionHost {
        let mut host = ExtensionHost::new();
        if std::env::var_os("LEXO_NO_PRO").is_some() {
            return host;
        }
        if let Some(path) = find_sidecar() {
            match SidecarProvider::start(&path) {
                Ok(p) => {
                    if let Err(e) = host.register(Box::new(p)) {
                        host.problems.push(e.to_string());
                    }
                }
                Err(e) => host.problems.push(format!("The Pro pack could not be started: {e}")),
            }
        }
        host
    }

    pub fn register(&mut self, provider: Box<dyn Provider>) -> Result<(), ExtensionError> {
        if provider.api() != API {
            return Err(plain(format!("The Pro pack speaks interface {}, this application needs {API}.", provider.api())));
        }
        self.providers.push(provider);
        Ok(())
    }

    pub fn present(&self) -> bool {
        !self.providers.is_empty()
    }

    fn offer(&self, feature: &str) -> Option<(&dyn Provider, Value)> {
        self.providers.iter().find_map(|p| p.offers().get(feature).map(|o| (p.as_ref(), o.clone())))
    }

    pub fn decide(&self, feature: &str) -> Decision {
        match self.offer(feature) {
            None => Decision { feature: feature.to_string(), available: false, limit: None },
            Some((_, o)) => Decision { feature: feature.to_string(), available: true, limit: o.get("limit").and_then(Value::as_i64) },
        }
    }

    /// The interface texts (in the language `code`) of the messages that the Pro pack itself writes.
    pub fn ui_strings(&self, code: &str) -> Map<String, Value> {
        let mut out = Map::new();
        for p in &self.providers {
            out.extend(p.ui_strings(code));
        }
        out
    }

    pub fn versions(&self) -> Vec<String> {
        self.providers.iter().map(|p| p.version()).collect()
    }

    /// The licence calls of the pack (`status`, `activate`, `refresh`, `deactivate`): not a gated feature, so they do not need an offer.
    pub fn licence(&self, method: &str, payload: &Value) -> Result<Value, ExtensionError> {
        let Some(p) = self.providers.first() else { return Err(plain("The Pro pack is not installed.".to_string())) };
        let request: Value = serde_json::from_str(&payload.to_string()).map_err(|e| plain(format!("The request is not plain JSON: {e}")))?;
        let out = p.call("licence", method, &request)?;
        serde_json::from_str(&out.to_string()).map_err(|e| plain(format!("The answer is not plain JSON: {e}")))
    }

    pub fn call(&self, feature: &str, method: &str, payload: &Value) -> Result<Value, ExtensionError> {
        let Some((p, _)) = self.offer(feature) else { return Err(plain(format!("{feature} is not available."))) };
        // a provider never gets or returns a live object: everything goes through JSON text
        let request: Value = serde_json::from_str(&payload.to_string()).map_err(|e| plain(format!("The request is not plain JSON: {e}")))?;
        let out = p.call(feature, method, &request)?;
        serde_json::from_str(&out.to_string()).map_err(|e| plain(format!("The answer is not plain JSON: {e}")))
    }
}

// ---------------------------------------------------------------- the Pro pack as a program
/// The name of the pack program.
pub const SIDECAR_NAME: &str = "lexo-pro-sidecar";

/// Where the pack program is: `LEXO_PRO_SIDECAR`, or next to this program (in `pack/` or beside it).
pub fn find_sidecar() -> Option<std::path::PathBuf> {
    if let Some(p) = std::env::var_os("LEXO_PRO_SIDECAR") {
        let p = std::path::PathBuf::from(p);
        return p.is_file().then_some(p);
    }
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let file = if cfg!(windows) { format!("{SIDECAR_NAME}.exe") } else { SIDECAR_NAME.to_string() };
    [dir.join("pack").join(&file), dir.join(&file)].into_iter().find(|p| p.is_file())
}

struct Link {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: std::io::BufReader<std::process::ChildStdout>,
    next_id: u64,
}

/// A provider that is a program: requests and answers are one JSON object per line.
///
/// ```text
///     {"id": 1, "method": "hello"}                                      -> {"id": 1, "ok": true, "result": {"api": 1, "version": "0.1.0"}}
///     {"id": 2, "method": "offers"}                                     -> {"id": 2, "ok": true, "result": {"<feature>": {"limit": null}}}
///     {"id": 3, "method": "ui_strings", "code": "cs"}                   -> {"id": 3, "ok": true, "result": {"<English text>": "<translation>"}}
///     {"id": 4, "method": "call", "feature": "...", "m": "...", "payload": {...}}  -> {"id": 4, "ok": true, "result": {...}}
/// ```
///
/// An error is `{"id": N, "ok": false, "error": <text or {"$m": ...}>}`. One request at a time. The program is ended when this value is dropped.
pub struct SidecarProvider {
    link: std::sync::Mutex<Link>,
    api: i64,
    version: String,
}

impl SidecarProvider {
    pub fn start(program: &std::path::Path) -> Result<SidecarProvider, String> {
        use std::process::{Command, Stdio};
        let mut cmd = Command::new(program);
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000); // no console window
        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        let stdin = child.stdin.take().ok_or("no standard input")?;
        let stdout = std::io::BufReader::new(child.stdout.take().ok_or("no standard output")?);
        let mut provider = SidecarProvider { link: std::sync::Mutex::new(Link { child, stdin, stdout, next_id: 1 }), api: 0, version: String::new() };
        let hello = provider.request(json!({"method": "hello"})).map_err(|e| e.to_string())?;
        provider.api = hello["api"].as_i64().unwrap_or(0);
        provider.version = hello["version"].as_str().unwrap_or("").to_string();
        Ok(provider)
    }

    fn request(&self, mut req: Value) -> Result<Value, ExtensionError> {
        use std::io::{BufRead, Write};
        let mut link = self.link.lock().unwrap();
        let id = link.next_id;
        link.next_id += 1;
        req["id"] = json!(id);
        let gone = || plain("The Pro pack program stopped.".to_string());
        writeln!(link.stdin, "{req}").and_then(|_| link.stdin.flush()).map_err(|_| gone())?;
        let mut line = String::new();
        if link.stdout.read_line(&mut line).map_err(|_| gone())? == 0 {
            return Err(gone());
        }
        let answer: Value = serde_json::from_str(&line).map_err(|e| plain(format!("The Pro pack program answered something that is not JSON: {e}")))?;
        if answer["id"] != json!(id) {
            return Err(plain("The Pro pack program answered another request.".to_string()));
        }
        if answer["ok"] == json!(true) {
            return Ok(answer["result"].clone());
        }
        Err(ExtensionError(match &answer["error"] {
            Value::String(s) => Text::Plain(s.clone()),
            other => Text::Wire(other.clone()),
        }))
    }
}

impl Drop for SidecarProvider {
    fn drop(&mut self) {
        if let Ok(mut link) = self.link.lock() {
            let _ = link.child.kill();
            let _ = link.child.wait();
        }
    }
}

impl Provider for SidecarProvider {
    fn api(&self) -> i64 {
        self.api
    }

    fn version(&self) -> String {
        self.version.clone()
    }

    fn ui_strings(&self, code: &str) -> Map<String, Value> {
        self.request(json!({"method": "ui_strings", "code": code})).ok().and_then(|v| v.as_object().cloned()).unwrap_or_default()
    }

    fn offers(&self) -> Map<String, Value> {
        self.request(json!({"method": "offers"})).ok().and_then(|v| v.as_object().cloned()).unwrap_or_default()
    }

    fn call(&self, feature: &str, method: &str, payload: &Value) -> Result<Value, ExtensionError> {
        self.request(json!({"method": "call", "feature": feature, "m": method, "payload": payload}))
    }
}
