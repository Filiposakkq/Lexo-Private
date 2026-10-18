//! The Lexo Pro pack in Rust (private, delivered only to licensed users; never part of the public repository).
//!
//! `LexoProProvider` is the pack as the extension host of Core sees it (`lexo_core::extensions::Provider`); `serve` is the sidecar loop, the same provider behind one JSON line per request
//! on standard input and one per answer on standard output (`SidecarProvider` of Core is the other end). Everything here depends on Core, never the other way round.
//! The pack decides on its own what it offers and with which limits (`licence`): the place for the licence check and for the demo mode of an unlicensed pack.
pub mod categorize;
pub mod fix_safe;
pub mod licence;
pub mod licence_client;
pub mod mt_batch;
pub mod publish;
pub mod regex_replace;

use lexo_core::extensions::{ExtensionError, Provider, API};
use lexo_core::layout;
use lexo_core::messages::Text;
use lexo_core::mt::{Http, MtError};
use serde_json::{json, Map, Value};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The Czech texts of the messages of this pack (`pro/lexo_pro/ui/cs.json`, the same file the Python pack reads).
const UI_CS: &str = include_str!("../../../lexo_pro/ui/cs.json");

const FEATURES: [&str; 5] = ["publish.web", "bulk.categorize", "replace.regex", "mt.batch", "fixes.safe"];

fn plain(text: String) -> ExtensionError {
    ExtensionError(Text::Plain(text))
}

/// The function that sends a request of the machine translation.
type HttpFn = Box<dyn Fn(&str, &[(String, String)], &[u8]) -> Result<(u16, String), MtError> + Send + Sync>;

#[derive(Default)]
pub struct LexoProProvider {
    http: Option<HttpFn>,
}

impl LexoProProvider {
    pub fn new() -> LexoProProvider {
        LexoProProvider::default()
    }

    /// A provider whose machine translation uses `http` (tests put a fake network here).
    pub fn with_http(http: impl Fn(&str, &[(String, String)], &[u8]) -> Result<(u16, String), MtError> + Send + Sync + 'static) -> LexoProProvider {
        LexoProProvider { http: Some(Box::new(http)) }
    }

    /// The function for the network: the one of the tests, or the real one.
    fn http(&self) -> Http<'_> {
        match &self.http {
            Some(h) => &**h,
            None => &lexo_core::net::post,
        }
    }

    fn categorize(&self, p: &Value) -> Result<Value, ExtensionError> {
        let names: Vec<String> = p["names"].as_array().into_iter().flatten().map(lexo_core::messages::py_str).collect();
        let (names, cut) = licence::cap("bulk.categorize", names);
        let lay = layout::parse(&p["layout"]).map_err(|e| plain(e.to_string()))?;
        let empty = Map::new();
        let items = categorize::suggest(&names, p["current"].as_object().unwrap_or(&empty), p["known"].as_object().unwrap_or(&empty), &lay);
        let mut out = match cut {
            Some(Value::Object(o)) => o,
            _ => Map::new(),
        };
        out.insert("items".into(), Value::Array(items));
        Ok(Value::Object(out))
    }
}

impl Provider for LexoProProvider {
    fn api(&self) -> i64 {
        API
    }

    fn version(&self) -> String {
        VERSION.to_string()
    }

    fn ui_strings(&self, code: &str) -> Map<String, Value> {
        if code != "cs" {
            return Map::new();
        }
        serde_json::from_str::<Value>(UI_CS).ok().and_then(|v| v.get("strings").and_then(Value::as_object).cloned()).unwrap_or_default()
    }

    fn offers(&self) -> Map<String, Value> {
        FEATURES.iter().filter_map(|f| licence::offer(f).map(|o| (f.to_string(), o))).collect()
    }

    fn call(&self, feature: &str, method: &str, payload: &Value) -> Result<Value, ExtensionError> {
        match (feature, method) {
            ("bulk.categorize", "suggest") => self.categorize(payload),
            ("fixes.safe", "plan") => Ok(fix_safe::plan(payload)),
            ("fixes.safe", "chain") => Ok(fix_safe::chain(payload)),
            ("mt.batch", "targets") => Ok(mt_batch::targets(payload)),
            ("mt.batch", "translate") => mt_batch::translate(payload, self.http()),
            ("replace.regex", "substitute") => regex_replace::substitute(payload),
            ("publish.web", "plan") => publish::plan(&payload["web"], &payload["data"]).map(Value::Object).map_err(publish::Failure::into_error),
            ("publish.web", "publish") => publish::publish(
                &payload["web"],
                &payload["data"],
                payload["text"].as_str().unwrap_or(""),
                payload["token"].as_str().unwrap_or(""),
                payload["branch"].as_str().unwrap_or(""),
            )
            .map_err(publish::Failure::into_error),
            ("licence", "status") => Ok(licence::status().to_json()),
            ("licence", "activate") => licence_client::activate_named(payload["key"].as_str().unwrap_or(""), payload["name"].as_str(), self.http()),
            ("licence", "refresh") => licence_client::refresh(self.http()),
            ("licence", "deactivate") => licence_client::deactivate(self.http()),
            ("licence", "devices") => licence_client::devices(self.http()),
            ("licence", "release") => licence_client::release(payload["target"].as_str().unwrap_or(""), self.http()),
            ("licence", "rename") => licence_client::rename(payload["name"].as_str().unwrap_or(""), self.http()),
            ("licence", "check") => {
                licence_client::refresh_if_stale(self.http());
                Ok(licence::status().to_json())
            }
            ("publish.web", other) => Err(plain(format!("publish.web has no method {other}."))),
            _ => Err(plain(format!("The Pro pack has no feature {feature}."))),
        }
    }
}

/// The sidecar loop: one JSON request per line in, one answer per line out (`SidecarProvider` of Core speaks the other side). Returns when the input ends.
pub fn serve(provider: &dyn Provider, input: impl std::io::BufRead, mut output: impl std::io::Write) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                writeln!(output, "{}", json!({"id": null, "ok": false, "error": format!("The request is not JSON: {e}")}))?;
                output.flush()?;
                continue;
            }
        };
        let id = req["id"].clone();
        let result: Result<Value, ExtensionError> = match req["method"].as_str().unwrap_or("") {
            "hello" => Ok(json!({"api": provider.api(), "version": provider.version()})),
            "offers" => Ok(Value::Object(provider.offers())),
            "ui_strings" => Ok(Value::Object(provider.ui_strings(req["code"].as_str().unwrap_or("")))),
            "call" => provider.call(req["feature"].as_str().unwrap_or(""), req["m"].as_str().unwrap_or(""), &req["payload"]),
            other => Err(plain(format!("The Pro pack program has no method {other}."))),
        };
        let answer = match result {
            Ok(v) => json!({"id": id, "ok": true, "result": v}),
            Err(e) => json!({"id": id, "ok": false, "error": e.0.wire()}),
        };
        writeln!(output, "{answer}")?;
        output.flush()?;
    }
    Ok(())
}
