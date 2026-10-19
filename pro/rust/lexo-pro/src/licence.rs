//! What the pack offers, and with which limits: the licence check (docs/PRO-DESIGN.md, "Entering a licence").
//!
//! Two kinds of Pro features: the "taste" features work in demo mode with a limit, everything else is offered only with a licence.
//!
//! **The licence file** (`licence.json` in the settings folder of the application, `LEXO_LICENCE_FILE` overrides) is `{"payload": <base64url JSON>, "sig": <base64url Ed25519>}`. The
//! signature covers the bytes of the payload and is checked offline with the public key built into the pack. The payload:
//! `{"v": 1, "id": "...", "features": ["all"] or a list, "major": 1, "device": "<device id>" or "any", "issued": <unix seconds>, "check_days": 14}`.
//! `issued` is when the server made the file; a renewed file (the regular online check) has a later `issued`.
//!
//! The states (`Status`): a valid file is `Licensed` until `issued + check_days`, then `Grace` for the same number of days (a missed check never blocks work), then `Expired`: the demo
//! limits with a clear message. No file is `Demo`. A broken signature, another major version or another device is refused (`Refused`), also the demo.
//!
//! **Until the owner has made the key pair and the server exists** the pack is built with the feature `dev-unlock` (a default feature): without any file it behaves as licensed, so the
//! tests and the development builds work. The release build turns it off (`--no-default-features`). `LEXO_PRO_DEMO=1` always shows the demo behaviour.
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// The taste features: items per action in demo mode.
pub const DEMO_LIMITS: [(&str, i64); 6] =
    [("bulk.categorize", 5), ("mt.batch", 20), ("fixes.safe", 20), ("carry.over", 50), ("memory.fuzzy", 20), ("style.check", 20)];

/// The major version of the product that this pack belongs to; a licence covers one major version.
pub const PRODUCT_MAJOR: u64 = 1;

/// The public key of the licence server (64 hex digits; the private key stays with the owner, made with `lexo-pro-licence keygen`).
const PUBLIC_KEY_HEX: &str = "1317d76f9189e042509235a6d05e0b8664f4cfffad8f34638472feccddb94a91";

/// The keys this build trusts, by the key id (`kid`) a licence file names. A file without `kid` was signed by the first key. To rotate the key: add the new one here and release;
/// when the server signs with it, files signed by the old one are still accepted until it is moved to `DISTRUSTED_KEYS` (after a leak) or removed.
const TRUSTED_KEYS: [(&str, &str); 2] = [("k1", PUBLIC_KEY_HEX), ("k2", "fadc26d4f240120f8deb03598a145121a2dd2f2b226e02073102699f3c57c9f4")];

/// Key ids that are known to be compromised: a file signed by one of them is refused even though the key is listed above (the pack asks the server for a new file).
const DISTRUSTED_KEYS: [&str; 0] = [];

/// The key id a licence file names (the first key when it names none); not checked, only used to choose the key to check with.
pub fn kid_of(file: &str) -> String {
    payload_of(file).and_then(|p| p["kid"].as_str().map(String::from)).unwrap_or_else(|| "k1".to_string())
}

const DAY: i64 = 86_400;

/// The interval of the online check when the file does not say.
const DEFAULT_CHECK_DAYS: i64 = 14;

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// No licence file.
    Demo,
    /// A valid file. `features` is `None` for "all". `days_to_check`: days until the online check is due.
    Licensed { id: String, features: Option<Vec<String>>, days_to_check: i64 },
    /// The check is overdue; the pack still works for `days_left` more days.
    Grace { id: String, features: Option<Vec<String>>, days_left: i64 },
    /// The grace period is over.
    Expired { id: String },
    /// The file cannot be used (the reason is for the user).
    Refused(&'static str),
}

impl Status {
    /// The features this status unlocks completely: `Some(None)` all, `Some(Some(list))` those, `None` nothing.
    fn unlocked(&self) -> Option<Option<&[String]>> {
        match self {
            Status::Licensed { features, .. } | Status::Grace { features, .. } => Some(features.as_deref()),
            _ => None,
        }
    }

    /// For the UI: `{state, id?, days?, reason?}`.
    pub fn to_json(&self) -> Value {
        match self {
            Status::Demo => json!({"state": "demo"}),
            Status::Licensed { id, days_to_check, .. } => json!({"state": "licensed", "id": id, "days_to_check": days_to_check}),
            Status::Grace { id, days_left, .. } => json!({"state": "grace", "id": id, "days_left": days_left}),
            Status::Expired { id } => json!({"state": "expired", "id": id}),
            Status::Refused(why) => json!({"state": "refused", "reason": why}),
        }
    }
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) || !s.is_ascii() {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok()).collect()
}

/// The public key of the first key id (kept for the tools and the tests); a licence file is checked with `public_key_for`.
pub fn public_key() -> Option<VerifyingKey> {
    public_key_for("k1")
}

/// The public key for a key id: a built-in one that is not distrusted; a build with `dev-unlock` (or a debug build) also takes `LEXO_LICENCE_PUBKEY` (hex) for every id, which the tests use.
pub fn public_key_for(kid: &str) -> Option<VerifyingKey> {
    if DISTRUSTED_KEYS.contains(&kid) {
        return None;
    }
    let mut hex = TRUSTED_KEYS.iter().find(|(k, _)| *k == kid)?.1.to_string();
    if cfg!(any(feature = "dev-unlock", debug_assertions)) {
        if let Ok(v) = std::env::var("LEXO_LICENCE_PUBKEY") {
            hex = v;
        }
    }
    let bytes: [u8; 32] = from_hex(hex.trim())?.try_into().ok()?;
    VerifyingKey::from_bytes(&bytes).ok()
}

/// Checks a licence file with the key its `kid` names.
pub fn evaluate_file(file: &str, now: i64, device: &str) -> Status {
    let kid = kid_of(file);
    if TRUSTED_KEYS.iter().all(|(k, _)| *k != kid) {
        return Status::Refused("the licence was signed with a key this version does not know");
    }
    if DISTRUSTED_KEYS.contains(&kid.as_str()) {
        return Status::Refused("the licence was signed with a key that is no longer trusted");
    }
    evaluate(file, now, device, public_key_for(&kid).as_ref())
}

/// Checks a licence file text: `now` in unix seconds, `device` this machine's id.
pub fn evaluate(file: &str, now: i64, device: &str, key: Option<&VerifyingKey>) -> Status {
    const UNREADABLE: Status = Status::Refused("the licence file is not readable");
    const BAD_SIGNATURE: Status = Status::Refused("the signature of the licence is wrong");
    let Some(key) = key else { return Status::Refused("this build cannot verify licences") };
    let Ok(outer) = serde_json::from_str::<Value>(file) else { return UNREADABLE };
    let (Some(payload), Some(sig)) = (outer["payload"].as_str(), outer["sig"].as_str()) else { return UNREADABLE };
    let (Ok(bytes), Ok(sig)) = (URL_SAFE_NO_PAD.decode(payload), URL_SAFE_NO_PAD.decode(sig)) else { return UNREADABLE };
    let Ok(sig) = Signature::from_slice(&sig) else { return BAD_SIGNATURE };
    if key.verify_strict(&bytes, &sig).is_err() {
        return BAD_SIGNATURE;
    }
    let Ok(p) = serde_json::from_slice::<Value>(&bytes) else { return UNREADABLE };
    if p["v"].as_i64() != Some(1) {
        return Status::Refused("the licence file comes from a newer version of the licence system");
    }
    if p["major"].as_u64() != Some(PRODUCT_MAJOR) {
        return Status::Refused("the licence covers another major version of the application");
    }
    match p["device"].as_str() {
        Some("any") => {}
        Some(d) if d == device => {}
        _ => return Status::Refused("the licence belongs to another computer"),
    }
    let id = p["id"].as_str().unwrap_or("").to_string();
    let features = match p["features"].as_array() {
        Some(list) if list.iter().any(|f| f == "all") => None,
        Some(list) => Some(list.iter().filter_map(|f| f.as_str().map(String::from)).collect()),
        None => Some(Vec::new()),
    };
    let Some(issued) = p["issued"].as_i64() else { return UNREADABLE };
    let check = p["check_days"].as_i64().unwrap_or(DEFAULT_CHECK_DAYS).clamp(1, 365);
    let due = issued + check * DAY;
    if now <= due {
        Status::Licensed { id, features, days_to_check: (due - now + DAY - 1) / DAY }
    } else if now <= due + check * DAY {
        Status::Grace { id, features, days_left: (due + check * DAY - now + DAY - 1) / DAY }
    } else {
        Status::Expired { id }
    }
}

/// Where the licence file is.
pub fn licence_path() -> PathBuf {
    match std::env::var_os("LEXO_LICENCE_FILE").filter(|p| !p.is_empty()) {
        Some(p) => PathBuf::from(p),
        None => lexo_core::config::config_dir().join("licence.json"),
    }
}

/// This machine's id: a hash of a random id that is made once and stored next to the licence file. Nothing readable (no names, no paths) ever leaves the machine.
/// An identifier of this machine (the Windows `MachineGuid`, `/etc/machine-id` elsewhere), empty when there is none. It is mixed into the device id, so that copying the licence file
/// and `device.id` to another computer gives another device (and not a free clone). Read once.
fn machine_id() -> &'static str {
    static ID: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ID.get_or_init(|| {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let out = std::process::Command::new("reg")
                .args(["query", r"HKLM\SOFTWARE\Microsoft\Cryptography", "/v", "MachineGuid"])
                .creation_flags(0x0800_0000)
                .output()
                .ok();
            out.and_then(|o| String::from_utf8(o.stdout).ok())
                .and_then(|s| s.lines().find(|l| l.contains("MachineGuid")).and_then(|l| l.split_whitespace().last().map(String::from)))
                .unwrap_or_default()
        }
        #[cfg(not(windows))]
        {
            std::fs::read_to_string("/etc/machine-id").map(|s| s.trim().to_string()).unwrap_or_default()
        }
    })
}

pub fn device_id() -> String {
    let path = licence_path().with_file_name("device.id");
    let secret = match std::fs::read_to_string(&path) {
        Ok(s) if s.trim().len() >= 32 => s.trim().to_string(),
        _ => {
            let mut raw = [0u8; 24];
            let made = getrandom::getrandom(&mut raw).is_ok();
            let s: String = raw.iter().map(|b| format!("{b:02x}")).collect();
            if made {
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let _ = std::fs::write(&path, &s);
            }
            s
        }
    };
    let digest = Sha256::digest(format!("aside-device:{secret}:{}", machine_id()).as_bytes());
    digest.iter().take(16).map(|b| format!("{b:02x}")).collect()
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// The name the user gave to this computer (optional, kept next to the licence file, sent to the server for the list of computers).
fn name_path() -> PathBuf {
    licence_path().with_file_name("device.name")
}

pub fn device_name() -> String {
    std::fs::read_to_string(name_path()).map(|s| s.trim().chars().take(40).collect()).unwrap_or_default()
}

pub fn set_device_name(name: &str) {
    let name: String = name.trim().chars().filter(|c| !c.is_control()).take(40).collect();
    if name.is_empty() {
        let _ = std::fs::remove_file(name_path());
    } else {
        let _ = std::fs::write(name_path(), name);
    }
}

/// The text of the reason that is shown when the server has cancelled the licence.
pub const CANCELLED: &str = "this licence was cancelled";
/// ... and when the place of this computer was freed (by the user from another computer, or by the owner).
pub const REMOVED: &str = "this computer was removed from the licence";

/// The marker of a licence that the server cancelled: `licence.revoked` next to the licence file holds the id of that licence. It makes the pack stop at once when the server says
/// "cancelled" (the pack asks at its start and every few hours), instead of waiting for the end of the check interval. A new activation removes it.
fn revoked_marker() -> PathBuf {
    licence_path().with_extension("revoked")
}

/// Remembers that the server refused this licence file: `reason` is "cancelled" (the licence was cancelled) or "removed" (this computer was freed). Two lines: the id and the reason.
pub fn mark_revoked(id: &str, reason: &str) {
    let path = revoked_marker();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, format!("{id}\n{reason}\n"));
}

pub fn clear_revoked() {
    let _ = std::fs::remove_file(revoked_marker());
}

/// The payload of a licence file text, not checked (only for reading the id and the time of issue of a file this pack stored itself).
fn payload_of(text: &str) -> Option<Value> {
    let outer: Value = serde_json::from_str(text).ok()?;
    serde_json::from_slice(&URL_SAFE_NO_PAD.decode(outer["payload"].as_str()?).ok()?).ok()
}

pub fn file_id(text: &str) -> Option<String> {
    payload_of(text)?["id"].as_str().map(String::from)
}

pub fn file_issued(text: &str) -> Option<i64> {
    payload_of(text)?["issued"].as_i64()
}

/// The highest time this computer has seen with a licence file (a file next to the licence). A clock set back does not count: the age of the licence is measured with the later of the two.
/// It does not stop someone who deletes the file, but a licence cannot be kept alive for months by turning the clock back, and the time a licence file was issued is signed, so it counts too.
fn seen_path() -> PathBuf {
    licence_path().with_file_name("clock.seen")
}

/// `real` is the clock; `stored` the highest time seen; `issued` the time of issue of the file (signed, so it cannot be lower than the truth).
pub fn trusted_time(real: i64, stored: i64, issued: i64) -> i64 {
    real.max(stored).max(issued)
}

fn now_trusted(file_text: &str) -> i64 {
    let path = seen_path();
    let stored = std::fs::read_to_string(&path).ok().and_then(|s| s.trim().parse::<i64>().ok()).unwrap_or(0);
    let t = trusted_time(now(), stored, file_issued(file_text).unwrap_or(0));
    if t > stored + 3600 {
        let _ = std::fs::write(&path, t.to_string());
    }
    t
}

/// The state of the licence of this machine now.
pub fn status() -> Status {
    match std::fs::read_to_string(licence_path()) {
        Ok(text) => {
            let status = evaluate_file(&text, now_trusted(&text), &device_id());
            if matches!(status, Status::Licensed { .. } | Status::Grace { .. }) {
                if let Ok(marker) = std::fs::read_to_string(revoked_marker()) {
                    let mut lines = marker.lines();
                    let (id, reason) = (lines.next().unwrap_or("").trim(), lines.next().unwrap_or("cancelled").trim());
                    if file_id(&text).is_some_and(|mine| mine == id) {
                        return Status::Refused(if reason == "removed" { REMOVED } else { CANCELLED });
                    }
                }
            }
            status
        }
        Err(_) => Status::Demo,
    }
}

/// Is `feature` unlocked completely?
fn unlocked(feature: &str) -> bool {
    if std::env::var("LEXO_PRO_DEMO").is_ok_and(|v| v == "1") {
        return false;
    }
    let status = status();
    match status.unlocked() {
        Some(None) => true,
        Some(Some(list)) => list.iter().any(|f| f == feature),
        None => cfg!(feature = "dev-unlock") && status == Status::Demo,
    }
}

/// `{"limit": null | N}` when the pack offers `feature` now, otherwise `None`.
pub fn offer(feature: &str) -> Option<Value> {
    if unlocked(feature) {
        return Some(json!({"limit": null}));
    }
    DEMO_LIMITS.iter().find(|(f, _)| *f == feature).map(|(_, n)| json!({"limit": n}))
}

/// Cut `seq` to the limit of `feature`. Returns (items, info); info says what was cut, for the UI (`None` when nothing was).
pub fn cap<T>(feature: &str, mut seq: Vec<T>) -> (Vec<T>, Option<Value>) {
    let limit = offer(feature).and_then(|o| o["limit"].as_i64());
    match limit {
        Some(n) if (seq.len() as i64) > n => {
            let total = seq.len();
            seq.truncate(n as usize);
            (seq, Some(json!({"limited": {"feature": feature, "limit": n, "total": total}})))
        }
        _ => (seq, None),
    }
}

/// Makes a licence file: used by the tool `lexo-pro-licence` and by the tests (the server will do the same with the same format).
pub fn sign(signing: &ed25519_dalek::SigningKey, payload: &Value) -> Value {
    use ed25519_dalek::Signer;
    let bytes = serde_json::to_vec(payload).unwrap_or_default();
    json!({"payload": URL_SAFE_NO_PAD.encode(&bytes), "sig": URL_SAFE_NO_PAD.encode(signing.sign(&bytes).to_bytes())})
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    const NOW: i64 = 1_800_000_000;

    fn keys() -> (SigningKey, VerifyingKey) {
        let s = SigningKey::from_bytes(&[7u8; 32]);
        let v = s.verifying_key();
        (s, v)
    }

    fn file(s: &SigningKey, edit: impl FnOnce(&mut Value)) -> String {
        let mut p = json!({"v": 1, "id": "L-1", "features": ["all"], "major": 1, "device": "any", "issued": NOW - 5 * DAY, "check_days": 30});
        edit(&mut p);
        sign(s, &p).to_string()
    }

    #[test]
    fn the_built_in_keys_are_valid_and_different() {
        let keys: Vec<_> = TRUSTED_KEYS.iter().map(|(k, _)| public_key_for(k)).collect();
        assert!(keys.iter().all(|k| k.is_some()), "every listed key must be a valid public key");
        assert_ne!(keys[0].unwrap().to_bytes(), keys[1].unwrap().to_bytes());
    }

    #[test]
    fn a_clock_set_back_does_not_give_back_the_time() {
        assert_eq!(trusted_time(1_000, 5_000, 0), 5_000); // the clock went back below the highest time seen
        assert_eq!(trusted_time(1_000, 0, 4_000), 4_000); // below the time the file was issued (signed)
        assert_eq!(trusted_time(9_000, 5_000, 4_000), 9_000); // the normal case
    }

    #[test]
    fn the_device_id_is_stable_and_does_not_show_the_machine() {
        let (a, b) = (device_id(), device_id());
        assert_eq!(a, b);
        assert_eq!(a.len(), 32);
        assert!(!a.contains(machine_id()) || machine_id().is_empty());
    }

    #[test]
    fn a_file_is_checked_with_the_key_its_kid_names() {
        let (s, v) = keys();
        let named = file(&s, |p| p["kid"] = json!("k1"));
        assert_eq!(kid_of(&named), "k1");
        assert_eq!(kid_of(&file(&s, |_| {})), "k1"); // no kid = the first key
        assert_eq!(kid_of(&file(&s, |p| p["kid"] = json!("k7"))), "k7");
        // an id this build does not list is refused with its own reason, not as a wrong signature
        let unknown = file(&s, |p| p["kid"] = json!("k7"));
        assert_eq!(evaluate_file(&unknown, NOW, "d"), Status::Refused("the licence was signed with a key this version does not know"));
        // a file of a listed id is checked with that key (here the built-in one does not match the throw-away key)
        assert_eq!(evaluate_file(&named, NOW, "d"), Status::Refused("the signature of the licence is wrong"));
        assert!(matches!(evaluate(&named, NOW, "d", Some(&v)), Status::Licensed { .. }));
    }

    #[test]
    fn a_valid_file_is_licensed_then_in_grace_then_expired() {
        let (s, v) = keys();
        let f = file(&s, |_| {});
        assert_eq!(evaluate(&f, NOW, "d", Some(&v)), Status::Licensed { id: "L-1".into(), features: None, days_to_check: 25 });
        assert!(matches!(evaluate(&f, NOW + 30 * DAY, "d", Some(&v)), Status::Grace { days_left: 25, .. }));
        assert!(matches!(evaluate(&f, NOW + 60 * DAY, "d", Some(&v)), Status::Expired { .. }));
    }

    #[test]
    fn the_check_interval_is_inside_the_signed_file() {
        let (s, v) = keys();
        let f = file(&s, |p| p["check_days"] = json!(14));
        assert!(matches!(evaluate(&f, NOW + 10 * DAY, "d", Some(&v)), Status::Grace { .. }));
        assert!(matches!(evaluate(&f, NOW + 30 * DAY, "d", Some(&v)), Status::Expired { .. }));
    }

    #[test]
    fn a_forged_or_foreign_file_is_refused() {
        let (s, v) = keys();
        // changed payload, the old signature
        let good: Value = serde_json::from_str(&file(&s, |_| {})).unwrap();
        let other: Value = serde_json::from_str(&file(&s, |p| p["features"] = json!(["publish.web"]))).unwrap();
        let forged = json!({"payload": other["payload"], "sig": good["sig"]}).to_string();
        assert_eq!(evaluate(&forged, NOW, "d", Some(&v)), Status::Refused("the signature of the licence is wrong"));
        // another key signed it
        let thief = SigningKey::from_bytes(&[9u8; 32]);
        assert_eq!(evaluate(&file(&thief, |_| {}), NOW, "d", Some(&v)), Status::Refused("the signature of the licence is wrong"));
        // garbage and no key
        assert!(matches!(evaluate("not json", NOW, "d", Some(&v)), Status::Refused(_)));
        assert!(matches!(evaluate(&file(&s, |_| {}), NOW, "d", None), Status::Refused(_)));
    }

    #[test]
    fn major_version_and_device_must_match() {
        let (s, v) = keys();
        assert!(matches!(evaluate(&file(&s, |p| p["major"] = json!(2)), NOW, "d", Some(&v)), Status::Refused(_)));
        let bound = file(&s, |p| p["device"] = json!("aaa"));
        assert!(matches!(evaluate(&bound, NOW, "aaa", Some(&v)), Status::Licensed { .. }));
        assert!(matches!(evaluate(&bound, NOW, "bbb", Some(&v)), Status::Refused("the licence belongs to another computer")));
    }

    #[test]
    fn a_licence_can_list_features() {
        let (s, v) = keys();
        let f = file(&s, |p| p["features"] = json!(["replace.regex"]));
        match evaluate(&f, NOW, "d", Some(&v)) {
            Status::Licensed { features: Some(list), .. } => assert_eq!(list, vec!["replace.regex".to_string()]),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn hex_and_the_built_in_key() {
        let bytes: [u8; 32] = from_hex(PUBLIC_KEY_HEX).and_then(|b| b.try_into().ok()).expect("the built-in key is 64 hex digits");
        assert!(VerifyingKey::from_bytes(&bytes).is_ok(), "the built-in key is a valid Ed25519 key");
        assert_eq!(from_hex("zz"), None);
        assert_eq!(from_hex("0aFf"), Some(vec![10, 255]));
    }
}
