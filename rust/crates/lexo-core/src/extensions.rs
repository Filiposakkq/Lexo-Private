//! Port of `core/lexo/extensions.py`: the extension host, how a separately delivered Pro pack plugs into Core and how a Pro feature is called.
//!
//! Core and Free hold only the catalog of gated features (`features.rs`), this host ("is there a provider for this feature, and what does it offer?") and the calls themselves.
//! The implementation, the licence check and the limits of the demo live in the Pro pack. Without a provider the host answers "not available" and nothing else can change that.
//!
//! The contract between the host and a provider is plain JSON on purpose: a call is a feature id, a method name and a value that can be written as JSON, the answer is such a
//! value, an error is an `ExtensionError`. The host sends every payload through JSON and back, so a provider can never receive or return a live object, and the transport is
//! replaceable: the provider can be a separate program that speaks the same JSON.
//!
//! Known difference from Python: the Pro pack of today is a Python package (`lexo_pro`); this host can only use providers written in Rust (an object that implements `Provider`)
//! or, later, a sidecar program speaking the JSON contract. So `ExtensionHost::installed()` finds nothing yet and every gated feature is locked.
use crate::features::Decision;
use crate::messages::Text;
use serde_json::{Map, Value};

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

    /// The host of a normal start. The installed Python pack cannot be loaded from Rust (see the module note), so it has no provider.
    pub fn installed() -> ExtensionHost {
        ExtensionHost::new()
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

    pub fn call(&self, feature: &str, method: &str, payload: &Value) -> Result<Value, ExtensionError> {
        let Some((p, _)) = self.offer(feature) else { return Err(plain(format!("{feature} is not available."))) };
        // a provider never gets or returns a live object: everything goes through JSON text
        let request: Value = serde_json::from_str(&payload.to_string()).map_err(|e| plain(format!("The request is not plain JSON: {e}")))?;
        let out = p.call(feature, method, &request)?;
        serde_json::from_str(&out.to_string()).map_err(|e| plain(format!("The answer is not plain JSON: {e}")))
    }
}
