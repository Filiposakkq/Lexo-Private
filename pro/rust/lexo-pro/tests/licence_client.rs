//! The activation client against a fake licence server (no network): activate, refuse, refresh, deactivate, and what is kept when something goes wrong.
use ed25519_dalek::SigningKey;
use lexo_core::extensions::Provider;
use lexo_core::messages::msg;
use lexo_core::mt::MtError;
use lexo_pro::{licence, LexoProProvider};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}

#[test]
fn the_client_talks_to_the_server_and_keeps_a_good_file() {
    let tmp = std::env::temp_dir().join(format!("lexo-pro-client-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let file = tmp.join("licence.json");
    let (server_key, thief_key) = (SigningKey::from_bytes(&[3u8; 32]), SigningKey::from_bytes(&[4u8; 32]));
    std::env::set_var("LEXO_LICENCE_FILE", &file);
    std::env::set_var("LEXO_LICENCE_PUBKEY", hex(server_key.verifying_key().as_bytes()));
    std::env::remove_var("LEXO_PRO_DEMO");

    let down = Arc::new(AtomicBool::new(false));
    let issued = Arc::new(AtomicI64::new(now()));
    let revoked = Arc::new(AtomicBool::new(false));
    let removed = Arc::new(AtomicBool::new(false));
    let refreshes = Arc::new(AtomicUsize::new(0));
    let seen: Arc<Mutex<Vec<(String, Value)>>> = Arc::new(Mutex::new(Vec::new()));
    let fake = {
        let (down, issued, revoked, removed, refreshes, seen) =
            (down.clone(), issued.clone(), revoked.clone(), removed.clone(), refreshes.clone(), seen.clone());
        move |url: &str, _headers: &[(String, String)], body: &[u8]| -> Result<(u16, String), MtError> {
            if down.load(Ordering::SeqCst) {
                return Err(MtError(msg("The connection to the service failed: {reason}", &[("reason", json!("offline"))])));
            }
            let b: Value = serde_json::from_slice(body).unwrap();
            let path = url.rsplit_once("/v1/").map(|(_, p)| p).unwrap_or("");
            seen.lock().unwrap().push((path.to_string(), b.clone()));
            let signed = |key: &SigningKey| {
                let payload = json!({"v": 1, "id": "L-TEST", "features": ["all"], "major": 1, "device": b["device"],
                                     "issued": issued.fetch_add(1, Ordering::SeqCst), "check_days": 14});
                json!({"ok": true, "licence": licence::sign(key, &payload)}).to_string()
            };
            Ok(match (path, b["key"].as_str()) {
                ("activate", Some("ASIDE-GOOD")) => (200, signed(&server_key)),
                ("activate", Some("ASIDE-FORGED")) => (200, signed(&thief_key)),
                ("activate", Some("ASIDE-FULL")) => (409, json!({"ok": false, "error": "device_limit", "devices": 2, "max": 2}).to_string()),
                ("activate", _) => (404, json!({"ok": false, "error": "invalid_key"}).to_string()),
                ("refresh", _) => {
                    refreshes.fetch_add(1, Ordering::SeqCst);
                    if revoked.load(Ordering::SeqCst) {
                        (403, json!({"ok": false, "error": "revoked"}).to_string())
                    } else if removed.load(Ordering::SeqCst) {
                        (403, json!({"ok": false, "error": "not_activated"}).to_string())
                    } else {
                        (200, signed(&server_key))
                    }
                }
                ("deactivate", _) => (200, json!({"ok": true}).to_string()),
                ("devices", _) => (
                    200,
                    json!({"ok": true, "devices": [{"id": b["device"], "name": "Notebook", "current": true}, {"id": "bbbb", "name": "", "current": false}], "max": 2, "releases_left": 1}).to_string(),
                ),
                ("release", _) => match b["target"].as_str() {
                    Some("dead") => (429, json!({"ok": false, "error": "release_limit", "limit": 2, "days": 30}).to_string()),
                    Some("gone") => (404, json!({"ok": false, "error": "not_found"}).to_string()),
                    _ => (200, json!({"ok": true, "releases_left": 0}).to_string()),
                },
                _ => (404, "{}".to_string()),
            })
        }
    };
    let p = LexoProProvider::with_http(fake);
    let call = |m: &str, payload: Value| p.call("licence", m, &payload);
    let text = |r: Result<Value, lexo_core::extensions::ExtensionError>| r.unwrap_err().0.text().to_string();

    // nothing yet
    assert_eq!(call("status", json!({})).unwrap()["state"], "demo");
    assert!(text(call("refresh", json!({}))).contains("no licence"));
    assert!(text(call("activate", json!({"key": "  "}))).contains("Enter your licence key"));

    // refusals say what to do, and store nothing
    assert!(text(call("activate", json!({"key": "ASIDE-WRONG"}))).contains("not valid"));
    assert!(text(call("activate", json!({"key": "ASIDE-FULL"}))).contains("already used on 2 computers"));
    assert!(text(call("activate", json!({"key": "ASIDE-FORGED"}))).contains("cannot use"));
    assert!(!file.exists());

    // a good key: the file is stored and the pack is licensed
    let state = call("activate", json!({"key": " ASIDE-GOOD "})).unwrap();
    assert_eq!(state["state"], "licensed");
    assert!((14..=15).contains(&state["days_to_check"].as_i64().unwrap()), "{state}"); // the whole interval, rounded up
    let stored = std::fs::read_to_string(&file).unwrap();
    assert_eq!(call("status", json!({})).unwrap()["state"], "licensed");
    assert!(p.offers().values().all(|v| v["limit"].is_null()));

    // a forged answer does not replace the good file
    assert!(text(call("activate", json!({"key": "ASIDE-FORGED"}))).contains("cannot use"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), stored);

    // the regular check renews the file; without a connection the old one stays
    call("refresh", json!({})).unwrap();
    let renewed = std::fs::read_to_string(&file).unwrap();
    assert_ne!(renewed, stored);
    down.store(true, Ordering::SeqCst);
    assert!(text(call("refresh", json!({}))).contains("connection"));
    assert!(text(call("deactivate", json!({}))).contains("connection"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), renewed);
    down.store(false, Ordering::SeqCst);

    // deactivating frees the place and removes the file
    assert_eq!(call("deactivate", json!({})).unwrap()["state"], "demo");
    assert!(!file.exists());
    assert!(text(call("deactivate", json!({}))).contains("no licence"));

    // a cancelled licence stops at once (not at the end of the check interval) and a restored one works again
    call("activate", json!({"key": "ASIDE-GOOD"})).unwrap();
    revoked.store(true, Ordering::SeqCst);
    assert!(text(call("refresh", json!({}))).contains("cancelled"));
    let state = call("status", json!({})).unwrap();
    assert_eq!((state["state"].as_str(), state["reason"].as_str()), (Some("refused"), Some(licence::CANCELLED)));
    let offers = p.offers();
    assert!(!offers.contains_key("replace.regex") && offers["bulk.categorize"]["limit"] == 5, "{offers:?}");
    assert!(file.with_extension("revoked").exists());
    revoked.store(false, Ordering::SeqCst);
    assert_eq!(call("refresh", json!({})).unwrap()["state"], "licensed");
    assert!(!file.with_extension("revoked").exists());
    assert!(p.offers().contains_key("replace.regex"));

    // the check at the start and every hour: only a file older than six hours is sent to the server
    let sent = || refreshes.load(Ordering::SeqCst);
    let before = sent();
    assert_eq!(call("check", json!({})).unwrap()["state"], "licensed");
    assert_eq!(sent(), before, "a fresh file is not sent");
    issued.store(now() - 7 * 3600, Ordering::SeqCst);
    call("activate", json!({"key": "ASIDE-GOOD"})).unwrap(); // a file made seven hours ago
    let old = std::fs::read_to_string(&file).unwrap();
    call("check", json!({})).unwrap();
    assert_eq!(sent(), before + 1, "an old file is checked");
    assert_ne!(std::fs::read_to_string(&file).unwrap(), old);
    revoked.store(true, Ordering::SeqCst);
    issued.store(now() - 7 * 3600, Ordering::SeqCst);
    call("activate", json!({"key": "ASIDE-GOOD"})).unwrap();
    call("check", json!({})).unwrap();
    assert_eq!(call("status", json!({})).unwrap()["state"], "refused", "the check at the start finds out that the licence was cancelled");

    // the name of the computer and the version go to the server; the list of computers and the freeing of another one
    revoked.store(false, Ordering::SeqCst);
    call("activate", json!({"key": "ASIDE-GOOD", "name": "  My laptop "})).unwrap();
    let last = |what: &str| seen.lock().unwrap().iter().rev().find(|(p, _)| p == what).map(|(_, b)| b.clone()).unwrap();
    assert_eq!(last("activate")["name"], "My laptop");
    assert_eq!(last("activate")["version"], env!("CARGO_PKG_VERSION"));
    call("rename", json!({"name": "Work PC"})).unwrap();
    assert_eq!(last("refresh")["name"], "Work PC", "the new name is sent at once");
    let d = call("devices", json!({})).unwrap();
    assert_eq!((d["devices"].as_array().unwrap().len(), d["releases_left"].as_i64()), (2, Some(1)));
    assert_eq!(call("release", json!({"target": "bbbb"})).unwrap()["releases_left"], 0);
    assert!(text(call("release", json!({"target": "dead"}))).contains("freed computers 2 times in the last 30 days"));
    assert!(text(call("release", json!({"target": "gone"}))).contains("not on this licence"));

    // a computer whose place was freed stops like a cancelled licence, with its own reason; a new activation brings it back
    revoked.store(false, Ordering::SeqCst);
    call("activate", json!({"key": "ASIDE-GOOD"})).unwrap();
    assert_eq!(call("status", json!({})).unwrap()["state"], "licensed");
    removed.store(true, Ordering::SeqCst);
    issued.store(now() - 7 * 3600, Ordering::SeqCst);
    call("activate", json!({"key": "ASIDE-GOOD"})).unwrap(); // a file made seven hours ago, so that the check at the start asks the server
    removed.store(true, Ordering::SeqCst);
    call("check", json!({})).unwrap();
    let state = call("status", json!({})).unwrap();
    assert_eq!((state["state"].as_str(), state["reason"].as_str()), (Some("refused"), Some(licence::REMOVED)));
    assert!(!p.offers().contains_key("replace.regex"));
    removed.store(false, Ordering::SeqCst);
    assert_eq!(call("activate", json!({"key": "ASIDE-GOOD"})).unwrap()["state"], "licensed");
    let _ = std::fs::remove_dir_all(&tmp);
}
