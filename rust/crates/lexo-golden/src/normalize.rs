//! The normalisation of `reference/python/core/tests/golden_tool.py` (`_str`, `normalize`): plain JSON without what differs between runs or systems.
use regex::Regex;
use serde_json::{Map, Value};
use std::sync::OnceLock;

/// Keys whose scalar values are replaced by `<volatile>` (a key counts when it contains one of these, lower-cased).
const VOLATILE_KEYS: [&str; 15] = [
    "time",
    "date",
    "updated",
    "elapsed",
    "token",
    "started",
    "mtime",
    "created",
    "stamp",
    "loaded_at",
    "seconds",
    "author",
    "days_left",
    "week_start",
    "needed_per_day",
];

struct Patterns {
    project_id: Regex,
    snapshot: Regex,
    backup: Regex,
    rel_rpy: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        project_id: Regex::new(r"projects/[0-9a-f]{6,}").unwrap(),
        snapshot: Regex::new(r"snap-\d{8}-\d{6}").unwrap(),
        backup: Regex::new(r"\.\d{8}-\d{6}-\d{6}\.bak").unwrap(),
        // beach\a.rpy (a file in a folder, as Windows writes it); Python's `$` also matches before a final newline
        rel_rpy: Regex::new(r"^[\w .\-]+(?:\\[\w .\-]+)+\.rpy\n?\z").unwrap(),
    })
}

/// `_str`: the temporary folders become `<TMP>` (with `/` as separator), random ids and stamps are replaced, a path to a translation file gets `/`.
pub fn normalize_str(value: &str, roots: &[String]) -> String {
    let p = patterns();
    let mut v = value.to_string();
    for r in roots {
        v = v.replace(r.as_str(), "<TMP>");
    }
    if v.contains("<TMP>") {
        v = v.replace('\\', "/");
    }
    v = p.project_id.replace_all(&v, "projects/<id>").into_owned();
    v = p.snapshot.replace_all(&v, "snap-<stamp>").into_owned();
    v = p.backup.replace_all(&v, ".<stamp>.bak").into_owned();
    if p.rel_rpy.is_match(&v) {
        v = v.split('\\').collect::<Vec<_>>().join("/");
    }
    v
}

fn is_volatile(key: &str) -> bool {
    let k = key.to_lowercase();
    VOLATILE_KEYS.iter().any(|w| k.contains(w))
}

/// `normalize`: the whole tree.
pub fn normalize(value: &Value, roots: &[String]) -> Value {
    match value {
        Value::String(s) => Value::String(normalize_str(s, roots)),
        Value::Array(a) => Value::Array(a.iter().map(|v| normalize(v, roots)).collect()),
        Value::Object(o) => {
            let mut out = Map::new();
            for (k, v) in o {
                let scalar = !matches!(v, Value::Object(_) | Value::Array(_));
                let nv = if is_volatile(k) && scalar { Value::String("<volatile>".into()) } else { normalize(v, roots) };
                out.insert(normalize_str(k, roots), nv);
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn roots() -> Vec<String> {
        vec!["C:\\Temp\\lexo-golden-x\\p".to_string(), "C:/Temp/lexo-golden-x/p".to_string()]
    }

    #[test]
    fn temporary_folders_and_ids_are_replaced() {
        assert_eq!(normalize_str("C:\\Temp\\lexo-golden-x\\p\\game\\tl", &roots()), "<TMP>/game/tl");
        assert_eq!(normalize_str("data/projects/0a1b2c3d4e5f/x", &roots()), "data/projects/<id>/x");
        assert_eq!(normalize_str("snap-20261017-123456.zip", &roots()), "snap-<stamp>.zip");
        assert_eq!(normalize_str("a.rpy.20261017-123456-000001.bak", &roots()), "a.rpy.<stamp>.bak");
    }

    #[test]
    fn a_path_to_a_translation_file_gets_a_slash_only_when_it_is_one() {
        assert_eq!(normalize_str("beach\\a.rpy", &[]), "beach/a.rpy");
        assert_eq!(normalize_str("beach\\a.txt", &[]), "beach\\a.txt");
        assert_eq!(normalize_str("a.rpy", &[]), "a.rpy");
    }

    #[test]
    fn volatile_keys_lose_scalars_but_not_structures() {
        let got = normalize(&json!({"time": 12.5, "Updated": "x", "dates": ["a"], "n": 3, "created": {"k": 1}}), &[]);
        assert_eq!(got["time"], "<volatile>");
        assert_eq!(got["Updated"], "<volatile>");
        assert_eq!(got["dates"], json!(["a"]));
        assert_eq!(got["n"], 3);
        assert_eq!(got["created"], json!({"k": 1}));
    }
}
