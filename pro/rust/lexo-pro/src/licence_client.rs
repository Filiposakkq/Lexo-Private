//! The activation client: asks the licence server (`server/`) for the signed licence file and keeps it where `licence::status` reads it.
//!
//! `activate` takes the key the user typed, `refresh` is the regular check (the file says how often), `deactivate` frees the place of this computer. Everything the server sends is
//! checked here again with the built-in public key before it is stored, so a wrong or old answer never replaces a good file. No name, no path and no project data is ever sent: only the
//! key (once), the licence file and the device id (a hash of a random id).
use crate::licence::{self, Status};
use lexo_core::extensions::ExtensionError;
use lexo_core::messages::{msg, Message, Text};
use lexo_core::mt::Http;
use serde_json::{json, Value};

/// The address of the licence server; `LEXO_LICENCE_SERVER` replaces it (tests, a staging server). It cannot make a file valid: the signature is checked here.
pub const DEFAULT_SERVER: &str = "https://api.aside.filiposakkq.dev";

fn server() -> String {
    std::env::var("LEXO_LICENCE_SERVER").ok().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| DEFAULT_SERVER.to_string()).trim_end_matches('/').to_string()
}

fn error(m: Message) -> ExtensionError {
    ExtensionError(Text::new_msg(m))
}

/// What the user reads for an error code of the server.
fn refusal(code: &str, answer: &Value) -> ExtensionError {
    error(match code {
        "invalid_key" => msg("That licence key is not valid. Check it for typing mistakes.", &[]),
        "revoked" => msg("This licence was cancelled. Write to the seller if you think that is a mistake.", &[]),
        "device_limit" => msg(
            "This licence is already used on {max} computers. Free a place in Aside on one of them (Settings, Licence), or write to the seller.",
            &[("max", answer["max"].clone())],
        ),
        "release_limit" => msg(
            "You have already freed computers {limit} times in the last {days} days. Write to the seller to free another place.",
            &[("limit", answer["limit"].clone()), ("days", answer["days"].clone())],
        ),
        "not_found" => msg("That computer is not on this licence any more.", &[]),
        "bad_licence" | "not_activated" => msg("The licence file does not fit this computer. Enter your key again.", &[]),
        "bad_request" => msg("The licence server did not understand the request.", &[]),
        other => msg("The licence server refused the request ({code}).", &[("code", json!(other))]),
    })
}

/// One request to the server: the status and the JSON answer (`Null` when it was not JSON).
fn raw(http: Http, path: &str, body: &Value) -> Result<(u16, Value), ExtensionError> {
    let headers = [("Content-Type".to_string(), "application/json".to_string()), ("User-Agent".to_string(), "Aside".to_string())];
    let (status, text) = http(&format!("{}{path}", server()), &headers, body.to_string().as_bytes()).map_err(|e| error(e.0))?;
    Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
}

/// One call of the server: the answer when it said `ok`, otherwise an error for the user.
fn call(http: Http, path: &str, body: &Value) -> Result<Value, ExtensionError> {
    let (status, answer) = raw(http, path, body)?;
    check(status, answer)
}

/// The answer when the server said `ok`, otherwise the error for the user.
fn check(status: u16, answer: Value) -> Result<Value, ExtensionError> {
    if status == 200 && answer["ok"] == json!(true) {
        return Ok(answer);
    }
    match answer["error"].as_str() {
        Some(code) => Err(refusal(code, &answer)),
        None => Err(error(msg("The licence server answered something unexpected ({status}).", &[("status", json!(status))]))),
    }
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Stores a file from the server if it is valid for this computer; the old file stays when it is not.
fn store(file: &Value) -> Result<Status, ExtensionError> {
    let text = file.to_string();
    let status = licence::evaluate_file(&text, now(), &licence::device_id());
    if !matches!(status, Status::Licensed { .. } | Status::Grace { .. }) {
        return Err(error(msg("The licence server sent a file that this version of Aside cannot use. Check for an update.", &[])));
    }
    let path = licence::licence_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| error(msg("The licence file could not be saved: {reason}", &[("reason", json!(e.to_string()))])))?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, &text)
        .and_then(|_| std::fs::rename(&tmp, &path))
        .map_err(|e| error(msg("The licence file could not be saved: {reason}", &[("reason", json!(e.to_string()))])))?;
    licence::clear_revoked(); // a file the server has just made means that the licence is valid (again)
    Ok(status)
}

/// The file that is stored now, as JSON.
fn current() -> Result<Value, ExtensionError> {
    std::fs::read_to_string(licence::licence_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .ok_or_else(|| error(msg("There is no licence on this computer.", &[])))
}

/// The key the user typed: takes a place for this computer and stores the licence file.
pub fn activate(key: &str, http: Http) -> Result<Value, ExtensionError> {
    activate_named(key, None, http)
}

/// The same with the name of this computer (kept when given).
pub fn activate_named(key: &str, name: Option<&str>, http: Http) -> Result<Value, ExtensionError> {
    if let Some(name) = name {
        licence::set_device_name(name);
    }
    let key = key.trim();
    if key.is_empty() {
        return Err(error(msg("Enter your licence key.", &[])));
    }
    let answer = call(http, "/v1/activate", &json!({"key": key, "device": licence::device_id(), "name": licence::device_name(), "version": crate::VERSION}))?;
    Ok(store(&answer["licence"])?.to_json())
}

/// The regular check: a renewed file (the clock of the interval starts again).
pub fn refresh(http: Http) -> Result<Value, ExtensionError> {
    let file = current()?;
    let (status, answer) =
        raw(http, "/v1/refresh", &json!({"licence": file, "device": licence::device_id(), "name": licence::device_name(), "version": crate::VERSION}))?;
    // the server cancelled the licence ("revoked") or freed the place of this computer ("not_activated"): the pack stops now, not at the end of the check interval
    let reason = match answer["error"].as_str() {
        Some("revoked") => Some("cancelled"),
        Some("not_activated") => Some("removed"),
        _ => None,
    };
    if let (Some(reason), Some(id)) = (reason, licence::file_id(&file.to_string())) {
        licence::mark_revoked(&id, reason);
    }
    let answer = check(status, answer)?;
    Ok(store(&answer["licence"])?.to_json())
}

/// The computers that hold the licence, how many places there are and how many times the user may still free one.
pub fn devices(http: Http) -> Result<Value, ExtensionError> {
    let file = current()?;
    call(http, "/v1/devices", &json!({"licence": file, "device": licence::device_id()}))
}

/// Frees the place of another computer (a lost one). Limited by the server.
pub fn release(target: &str, http: Http) -> Result<Value, ExtensionError> {
    let file = current()?;
    call(http, "/v1/release", &json!({"licence": file, "device": licence::device_id(), "target": target}))
}

/// Gives this computer a name for the list of computers (empty = no name); the server learns it with the next check, which is made now when there is a connection.
pub fn rename(name: &str, http: Http) -> Result<Value, ExtensionError> {
    licence::set_device_name(name);
    let _ = refresh(http);
    Ok(json!({"name": licence::device_name()}))
}

/// Frees the place of this computer and removes the file. A file the server does not know any more is removed too; a missing connection keeps it (nothing is lost).
pub fn deactivate(http: Http) -> Result<Value, ExtensionError> {
    let file = current()?;
    let (status, answer) = raw(http, "/v1/deactivate", &json!({"licence": file, "device": licence::device_id()}))?;
    let known_nothing = answer["error"] == json!("bad_licence"); // the server does not know this file: nothing to free
    if !(status == 200 && answer["ok"] == json!(true)) && !known_nothing {
        return Err(match answer["error"].as_str() {
            Some(code) => refusal(code, &answer),
            None => error(msg("The licence server answered something unexpected ({status}).", &[("status", json!(status))])),
        });
    }
    let _ = std::fs::remove_file(licence::licence_path());
    licence::clear_revoked();
    Ok(licence::status().to_json())
}

/// A file older than this is checked with the server again (at the start of the pack and then every hour it is looked at).
pub const STALE_AFTER: i64 = 6 * 3600;

/// When the file is older than `STALE_AFTER` and there is a connection, ask the server and renew it quietly. That is how a cancelled licence stops within hours (and a restored one starts
/// again) instead of at the end of the check interval. Never blocks the application; errors are not reported. Without a licence file nothing is sent.
pub fn refresh_if_stale(http: Http) {
    let Some(issued) = std::fs::read_to_string(licence::licence_path()).ok().and_then(|t| licence::file_issued(&t)) else { return };
    if now() - issued >= STALE_AFTER {
        let _ = refresh(http);
    }
}
