//! Port of `reference/python/core/lexo/app_base.py`: the errors and constants shared by the parts of the application (`app.rs` and the `app_*.rs` files).
use crate::features;
use crate::fsutil::WriteFailed;
use crate::messages::{msg, Message, Text};
use regex::Regex;
use serde_json::{json, Value};
use std::sync::OnceLock;

/// The only addresses the app will open in the browser (the UI sends just the id). Empty until the product has a website to point to.
pub const LINKS: [(&str, &str); 0] = [];
pub const CHARACTERS_MAX_BYTES: u64 = 16 * 1024 * 1024;
/// First words of Ren'Py screen statements that appear in translated UI blocks; they look like speakers but are not.
pub const SCREEN_KEYWORDS: [&str; 26] = [
    "text",
    "textbutton",
    "label",
    "tooltip",
    "hover",
    "idle",
    "insensitive",
    "timer",
    "key",
    "add",
    "bar",
    "vbar",
    "image",
    "imagebutton",
    "use",
    "frame",
    "window",
    "vbox",
    "hbox",
    "fixed",
    "grid",
    "mousearea",
    "input",
    "button",
    "action",
    "style",
];
pub const PAGE_LIMIT: i64 = 500;
pub const UI_PREF_MAX: usize = 20000;

/// `define de = Character("Derek", color="#0066ff")` / `define mc = DynamicCharacter("name", ...)`.
pub fn character_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"^\s*define\s+(?:\w+\.)?(\w+)\s*=\s*(Dynamic)?Character\s*\(\s*(?:"((?:[^"\\]|\\.)*)"|'((?:[^'\\]|\\.)*)')"#).unwrap())
}

/// Everything an operation can fail with.
#[derive(Debug)]
pub enum ApiError {
    /// `AppError`: an operation failed for a reason the user should read.
    App(Text),
    /// `ProjectBusy`: the project is open in another running copy of the app.
    Busy { text: Text, path: String },
    /// `FeatureLocked`: a Pro feature was used without it (the id of the feature).
    Locked(String),
    /// `KeyError`: a required parameter is missing.
    Missing(String),
    /// `WriteFailed`.
    Write(WriteFailed),
    /// `UnknownOperation`.
    Unknown(String),
}

pub type Res<T> = Result<T, ApiError>;

impl ApiError {
    /// The message of the error (what the transport sends as `error`).
    pub fn text(&self) -> Text {
        match self {
            ApiError::App(t) | ApiError::Busy { text: t, .. } => t.clone(),
            ApiError::Locked(f) => Text::new_msg(locked_message(f)),
            ApiError::Missing(name) => Text::Plain(format!("'{name}'")),
            ApiError::Write(w) => Text::Plain(w.to_string()),
            ApiError::Unknown(name) => Text::Plain(format!("'{name}'")),
        }
    }

    /// An `AppError` (or one of its subclasses): the kinds the golden scenarios record as `error`.
    pub fn is_app_error(&self) -> bool {
        matches!(self, ApiError::App(_) | ApiError::Busy { .. } | ApiError::Locked(_))
    }
}

pub fn locked_message(feature: &str) -> Message {
    let title = features::feature(feature).map_or(feature, |f| f.title);
    msg("“{title}” is a Pro feature.", &[("title", msg(title, &[]).wire())]) // the title is a message of its own, so that it is translated too
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text().text())
    }
}

impl From<Message> for ApiError {
    fn from(m: Message) -> ApiError {
        ApiError::App(Text::new_msg(m))
    }
}

impl From<WriteFailed> for ApiError {
    fn from(w: WriteFailed) -> ApiError {
        ApiError::Write(w)
    }
}

impl From<Text> for ApiError {
    fn from(t: Text) -> ApiError {
        ApiError::App(t)
    }
}

/// `AppError(message)`.
pub fn app_err(message: Message) -> ApiError {
    ApiError::App(Text::new_msg(message))
}

/// A message value for an answer (`{"$m": ...}`).
pub fn wire(m: &Message) -> Value {
    m.wire()
}
