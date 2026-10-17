//! A readable difference between two JSON values: the first place where they differ, as a path.
use serde_json::Value;

/// `None` when equal, otherwise e.g. `entries[3].target: expected "a", got "b"`.
pub fn first_difference(got: &Value, want: &Value) -> Option<String> {
    diff_at("$", got, want)
}

fn short(v: &Value) -> String {
    let s = v.to_string();
    if s.chars().count() > 80 {
        format!("{}…", s.chars().take(80).collect::<String>())
    } else {
        s
    }
}

fn diff_at(path: &str, got: &Value, want: &Value) -> Option<String> {
    match (got, want) {
        (Value::Object(g), Value::Object(w)) => {
            for (k, wv) in w {
                match g.get(k) {
                    None => return Some(format!("{path}.{k}: missing in the answer, expected {}", short(wv))),
                    Some(gv) => {
                        if let Some(d) = diff_at(&format!("{path}.{k}"), gv, wv) {
                            return Some(d);
                        }
                    }
                }
            }
            g.keys().find(|k| !w.contains_key(*k)).map(|k| format!("{path}.{k}: not expected, got {}", short(&g[k])))
        }
        (Value::Array(g), Value::Array(w)) => {
            for (i, (gv, wv)) in g.iter().zip(w).enumerate() {
                if let Some(d) = diff_at(&format!("{path}[{i}]"), gv, wv) {
                    return Some(d);
                }
            }
            (g.len() != w.len()).then(|| format!("{path}: {} items, expected {}", g.len(), w.len()))
        }
        _ => (got != want).then(|| format!("{path}: expected {}, got {}", short(want), short(got))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn equal_values_have_no_difference() {
        assert_eq!(first_difference(&json!({"a": [1, {"b": "x"}]}), &json!({"a": [1, {"b": "x"}]})), None);
    }

    #[test]
    fn the_path_of_the_first_difference_is_named() {
        let d = first_difference(&json!({"entries": [{"t": "a"}, {"t": "b"}]}), &json!({"entries": [{"t": "a"}, {"t": "c"}]}));
        assert_eq!(d.unwrap(), "$.entries[1].t: expected \"c\", got \"b\"");
        assert!(first_difference(&json!([1, 2]), &json!([1, 2, 3])).unwrap().contains("2 items, expected 3"));
        assert!(first_difference(&json!({}), &json!({"k": 1})).unwrap().contains("missing"));
    }
}
