//! Without a licence the style check looks at the first 20 lines only (the demo of the pack); its own file, because it sets an environment variable of the process.
use lexo_pro::style_check;
use serde_json::{json, Value};

#[test]
fn the_targets_are_cut_to_the_limit_of_the_demo() {
    std::env::set_var("LEXO_PRO_DEMO", "1");
    let items: Vec<Value> = (0..30).map(|i| json!({"file": "a.rpy", "line": i, "chars": 10})).collect();
    let out = style_check::targets(&json!({"items": items}));
    assert_eq!((out["items"].as_array().unwrap().len(), out["total"].clone(), out["chars"].clone()), (20, json!(30), json!(200)));
    assert_eq!(out["limited"]["limit"], 20);
}
