//! The Lexo Pro pack in Rust (private, delivered only to licensed users; never part of the public repository).
//!
//! `LexoProProvider` is the pack as the extension host of Core sees it (`lexo_core::extensions::Provider`); `serve` is the sidecar loop, the same provider behind one JSON line per request
//! on standard input and one per answer on standard output (`SidecarProvider` of Core is the other end). Everything here depends on Core, never the other way round.
//! The pack decides on its own what it offers and with which limits (`licence`): the place for the licence check and for the demo mode of an unlicensed pack.
pub mod carry_over;
pub mod categorize;
pub mod fix_safe;
pub mod hunspell;
pub mod licence;
pub mod licence_client;
pub mod mt_batch;
pub mod profiles;
pub mod publish;
pub mod regex_replace;
pub mod release_build;
pub mod style_check;
pub mod team;

use lexo_core::extensions::{ExtensionError, Provider, API};
use lexo_core::layout;
use lexo_core::messages::Text;
use lexo_core::mt::{Http, MtError};
use serde_json::{json, Map, Value};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The Czech texts of the messages of this pack (`pro/rust/lexo-pro/ui/cs.json`, the same file the Python pack reads).
const UI_CS: &str = include_str!("../ui/cs.json");

const FEATURES: [&str; 12] = [
    "publish.web",
    "bulk.categorize",
    "replace.regex",
    "mt.batch",
    "fixes.safe",
    "carry.over",
    "memory.fuzzy",
    "mt.profiles",
    "style.check",
    "release.build",
    "spell.hunspell",
    "team.manage",
];

fn plain(text: String) -> ExtensionError {
    ExtensionError(Text::Plain(text))
}

/// The function that sends a request of the machine translation.
type HttpFn = Box<dyn Fn(&str, &[(String, String)], &[u8]) -> Result<(u16, String), MtError> + Send + Sync>;

#[derive(Default)]
pub struct LexoProProvider {
    http: Option<HttpFn>,
    /// The dictionary that was read last (a file is read again only when it changed), kept for the next call.
    dict: std::sync::Mutex<Option<(String, std::sync::Arc<hunspell::Dict>)>>,
}

impl LexoProProvider {
    pub fn new() -> LexoProProvider {
        LexoProProvider::default()
    }

    /// A provider whose machine translation uses `http` (tests put a fake network here).
    pub fn with_http(http: impl Fn(&str, &[(String, String)], &[u8]) -> Result<(u16, String), MtError> + Send + Sync + 'static) -> LexoProProvider {
        LexoProProvider { http: Some(Box::new(http)), ..Default::default() }
    }

    /// The function for the network: the one of the tests, or the real one.
    fn http(&self) -> Http<'_> {
        match &self.http {
            Some(h) => &**h,
            None => &lexo_core::net::post,
        }
    }

    /// The words of `words` that the Hunspell dictionary (`dic` and `aff`, paths) does not accept, each with suggestions.
    fn spell_check(&self, p: &Value) -> Result<Value, ExtensionError> {
        let fail = |reason: String| {
            ExtensionError(Text::new_msg(lexo_core::messages::msg("The dictionary could not be read: {reason}", &[("reason", json!(reason))])))
        };
        let (dic, aff) = (lexo_core::messages::py_str(&p["dic"]), lexo_core::messages::py_str(&p["aff"]));
        let stamp = |f: &str| {
            std::fs::metadata(f).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs())
        };
        let key = format!("{dic}|{aff}|{}|{}", stamp(&dic), stamp(&aff));
        let dict = {
            let mut cache = self.dict.lock().unwrap_or_else(|e| e.into_inner());
            match &*cache {
                Some((k, d)) if *k == key => d.clone(),
                _ => {
                    let (dic_b, aff_b) = (std::fs::read(&dic).map_err(|e| fail(e.to_string()))?, std::fs::read(&aff).map_err(|e| fail(e.to_string()))?);
                    let d = std::sync::Arc::new(hunspell::Dict::load(&dic_b, &aff_b).map_err(fail)?);
                    *cache = Some((key, d.clone()));
                    d
                }
            }
        };
        let unknown: Vec<Value> = p["words"]
            .as_array()
            .into_iter()
            .flatten()
            .map(lexo_core::messages::py_str)
            .take(20000)
            .filter(|w| !dict.check(w))
            .map(|w| json!({"suggestions": dict.suggest(&w, 5), "word": w}))
            .collect();
        Ok(json!({"unknown": unknown, "roots": dict.len()}))
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
            // the same matcher: the translations of a previous version (`carry.over`) or of the project itself (`memory.fuzzy`)
            (f @ ("carry.over" | "memory.fuzzy"), "match") => {
                let mut out = carry_over::run(payload);
                let items = out["items"].as_array().cloned().unwrap_or_default();
                let (items, cut) = licence::cap(f, items); // without a licence only the first lines are written; the counts still show the value
                out["items"] = Value::Array(items);
                if let Some(Value::Object(info)) = cut {
                    if let Some(o) = out.as_object_mut() {
                        o.extend(info);
                    }
                }
                Ok(out)
            }
            ("spell.hunspell", "check") => self.spell_check(payload),
            ("team.manage", "normalize") => Ok(team::normalize(payload)),
            ("team.manage", "stats") => Ok(team::stats(payload)),
            ("release.build", "plan") => Ok(release_build::plan(payload)),
            ("style.check", "targets") => Ok(style_check::targets(payload)),
            ("style.check", "check") => style_check::check(payload, self.http()),
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
            ("licence", "recover") => licence_client::recover_request(payload["email"].as_str().unwrap_or(""), self.http()),
            ("licence", "recover-confirm") => {
                licence_client::recover_confirm(payload["email"].as_str().unwrap_or(""), payload["code"].as_str().unwrap_or(""), self.http())
            }
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
