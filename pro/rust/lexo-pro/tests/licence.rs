//! The licence end to end: the tool of the author makes a key pair and licence files, the sidecar program reads them, and the offers of the pack follow.
use lexo_core::extensions::{Provider, SidecarProvider};
use std::path::{Path, PathBuf};
use std::process::Command;

const SIDECAR: &str = env!("CARGO_BIN_EXE_lexo-pro-sidecar");
const TOOL: &str = env!("CARGO_BIN_EXE_lexo-pro-licence");

fn tool(args: &[&str]) -> String {
    let out = Command::new(TOOL).args(args).output().unwrap();
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// The offers of a sidecar that is started with this licence file and public key.
fn offers(file: &Path, pubkey: &str) -> serde_json::Map<String, serde_json::Value> {
    std::env::set_var("LEXO_LICENCE_FILE", file);
    std::env::set_var("LEXO_LICENCE_PUBKEY", pubkey);
    let p = SidecarProvider::start(Path::new(SIDECAR)).unwrap();
    p.offers()
}

#[test]
fn the_pack_follows_the_licence_file() {
    std::env::remove_var("LEXO_PRO_DEMO");
    let tmp: PathBuf = std::env::temp_dir().join(format!("lexo-pro-licence-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let (mine, other) = (tmp.join("mine"), tmp.join("other"));
    let public = |dir: &Path| tool(&["keygen", dir.to_str().unwrap()]).lines().find_map(|l| l.strip_prefix("public key:  ").map(str::to_string)).unwrap();
    let my_public = public(&mine);
    public(&other);
    let issue = |dir: &Path, extra: &[&str], out: &str| -> PathBuf {
        let file = tmp.join(out);
        let key = dir.join("licence-private.key");
        let mut args = vec!["issue", "--key", key.to_str().unwrap(), "--id", "L-test", "--out", file.to_str().unwrap()];
        args.extend_from_slice(extra);
        tool(&args);
        file
    };

    // all features: everything without a limit
    let all = issue(&mine, &[], "all.json");
    let o = offers(&all, &my_public);
    assert_eq!(o.len(), 8);
    assert!(o.values().all(|v| v["limit"].is_null()), "{o:?}");

    // a licence for one feature: that one is free, the taste features keep their demo limits, the other premium feature is not offered
    let one = issue(&mine, &["--features", "replace.regex"], "one.json");
    let o = offers(&one, &my_public);
    assert!(o["replace.regex"]["limit"].is_null());
    assert_eq!(o["bulk.categorize"]["limit"], 5);
    assert!(!o.contains_key("publish.web"), "{o:?}");

    // a file signed with another key, a file for another computer or another major version: the demo (expiry is covered by the unit tests)
    let forged = issue(&other, &[], "forged.json");
    assert_eq!(offers(&forged, &my_public).get("bulk.categorize").map(|v| v["limit"].clone()), Some(serde_json::json!(5)));
    assert!(!offers(&forged, &my_public).contains_key("replace.regex"));
    let device = issue(&mine, &["--device", "somebody-else"], "device.json");
    assert!(!offers(&device, &my_public).contains_key("replace.regex"));
    let major = issue(&mine, &["--major", "2"], "major.json");
    assert!(!offers(&major, &my_public).contains_key("replace.regex"));

    // the tool checks a file the same way
    assert!(tool(&["show", forged.to_str().unwrap(), &my_public]).contains("signature"));

    // no file at all: a development build behaves as licensed (the feature `dev-unlock`)
    let o = offers(&tmp.join("no-such-file.json"), &my_public);
    assert!(o.values().all(|v| v["limit"].is_null()), "{o:?}");
    let _ = std::fs::remove_dir_all(&tmp);
}
