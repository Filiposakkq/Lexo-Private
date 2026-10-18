//! Port of `pro/lexo_pro/licence.py`: what the pack offers, and with which limits. The licence check will live here (roadmap step 6).
//!
//! Two kinds of Pro features (docs/PRO-DESIGN.md): "taste" features work in demo mode with a limit, everything else is offered only with a licence. Until the licence file exists the
//! pack is "licensed" in a development build; `LEXO_PRO_DEMO=1` shows the demo (unlicensed) behaviour.
use serde_json::{json, Value};

/// The taste features: items per action in demo mode.
pub const DEMO_LIMITS: [(&str, i64); 3] = [("bulk.categorize", 5), ("mt.batch", 20), ("fixes.safe", 20)];

pub fn licensed() -> bool {
    std::env::var("LEXO_PRO_DEMO").map_or(true, |v| v != "1")
}

/// `{"limit": null | N}` when the pack offers `feature` now, otherwise `None`.
pub fn offer(feature: &str) -> Option<Value> {
    if licensed() {
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
