//! Port of `reference/python/core/lexo/features.py`: the catalog of gated (Pro) features and the decision for each one.
//!
//! The application never decides "is this Pro?" on its own: at the few places where a Pro feature is used it asks the extension host (`extensions.rs`) and gets an answer
//! (available or not, and the limit per action). Everything not in the catalog is free.
use crate::extensions::ExtensionHost;
use serde_json::{json, Map, Value};

pub struct Feature {
    pub id: &'static str,
    pub title: &'static str,
}

/// The single list of gated features (docs/product/FEATURES.md); every one of them lives in the Pro pack.
pub const CATALOG: [Feature; 11] = [
    Feature { id: "mt.batch", title: "Batch machine translation" },
    Feature { id: "fixes.safe", title: "Safe bulk fixes with preview" },
    Feature { id: "replace.regex", title: "Regex find & replace" },
    Feature { id: "bulk.categorize", title: "Bulk categorisation suggestions" },
    Feature { id: "publish.web", title: "Publishing progress to a website" },
    Feature { id: "carry.over", title: "Carry a translation over to a new version" },
    Feature { id: "memory.fuzzy", title: "Fill from similar translations" },
    Feature { id: "mt.profiles", title: "Profiles of characters" },
    Feature { id: "style.check", title: "Style check" },
    Feature { id: "release.build", title: "Release builder" },
    Feature { id: "spell.hunspell", title: "Spelling with a dictionary" },
];

pub fn feature(id: &str) -> Option<&'static Feature> {
    CATALOG.iter().find(|f| f.id == id)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    pub feature: String,
    pub available: bool,
    /// `None` = unlimited.
    pub limit: Option<i64>,
}

/// A feature outside the catalog is free, a gated one is what the Pro pack offers (nothing without the pack).
pub fn decide(feature_id: &str, host: &ExtensionHost) -> Decision {
    if feature(feature_id).is_none() {
        return Decision { feature: feature_id.to_string(), available: true, limit: None };
    }
    host.decide(feature_id)
}

/// For the UI: every gated feature, whether it is available and with which limit, and whether a Pro pack is there at all.
pub fn status_of(host: &ExtensionHost) -> Value {
    let mut out = Map::new();
    for f in &CATALOG {
        let d = decide(f.id, host);
        out.insert(f.id.to_string(), json!({"title": f.title, "available": d.available, "limit": d.limit, "pro": host.present()}));
    }
    Value::Object(out)
}
