//! Port of `core/lexo/mt.py`: optional machine translation, a SUGGESTION for one line that is never written anywhere by itself.
//!
//! Tags `{i}`, variables `[name]` and escapes (`\n`) never go to the service as text: they are replaced by `<x id="N"/>` placeholders and put back afterwards.
//! The three providers (DeepL, OpenAI compatible, LibreTranslate) are plain REST calls; the request is built here and sent by the function that is passed in (`Http`),
//! so the tests use a fake service. The real HTTP client belongs to the application (milestone M8).
//!
//! Known differences from Python: an answer whose text is not a string (a number, `null`) is reported as an answer of the wrong shape, where Python stops with a `TypeError`;
//! `json.loads` also accepts `NaN` and `Infinity`, `serde_json` does not.
use crate::htmlentities::{INVALID_CHARREFS, INVALID_CODEPOINTS, NAMED};
use crate::messages::{msg, py_str, Message};
use crate::pyfmt::json_dumps;
use crate::textedit::token_re;
use crate::util::{py_strip, truthy};
use regex::Regex;
use serde_json::{json, Map, Value};
use std::sync::OnceLock;

pub const PROVIDERS: [&str; 3] = ["deepl", "openai", "libretranslate"];
pub const TIMEOUT: u64 = 30;
const SYSTEM_PROMPT: &str = "You translate dialogue of a visual novel (Ren'Py game) from {src} to {dst}. Keep every <x id=\"N\"/> element exactly as it is, once, in a natural place of the \
sentence. Keep the tone of spoken dialogue. Return only the translated text, no explanations and no quotes.";

#[derive(Debug, PartialEq)]
pub struct MtError(pub Message);

impl std::fmt::Display for MtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.text())
    }
}

/// Sends a POST request: (url, headers, body) -> (status, body text). A failed connection is an `MtError`.
pub type Http<'a> = &'a dyn Fn(&str, &[(String, String)], &[u8]) -> Result<(u16, String), MtError>;

/// The one failure that must not stop a batch.
pub fn no_text() -> Message {
    msg("The line has no text to translate.", &[])
}

#[derive(Debug)]
pub struct Translation {
    pub text: String,
    pub warnings: Vec<Message>,
    pub provider: String,
}

impl Translation {
    pub fn to_json(&self) -> Value {
        json!({"text": self.text, "warnings": self.warnings.iter().map(Message::wire).collect::<Vec<_>>(), "provider": self.provider})
    }
}

// ---------------------------------------------------------------- html.escape / html.unescape
/// `html.escape(text)` (quotes too).
pub fn html_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            c => out.push(c),
        }
    }
    out
}

fn named(key: &str) -> Option<&'static str> {
    NAMED.binary_search_by(|(k, _)| (*k).cmp(key)).ok().map(|i| NAMED[i].1)
}

fn charref_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"&(#[0-9]+;?|#[xX][0-9a-fA-F]+;?|[^\t\n\x0c <&#;]{1,32};?)").unwrap())
}

fn replace_charref(s: &str) -> String {
    if let Some(rest) = s.strip_prefix('#') {
        let (digits, radix) = match rest.strip_prefix(['x', 'X']) {
            Some(h) => (h, 16),
            None => (rest, 10),
        };
        let num = digits.trim_end_matches(';').chars().fold(0u64, |acc, c| (acc * radix + c.to_digit(radix as u32).unwrap_or(0) as u64).min(0x7fff_ffff));
        let num = num as u32;
        if let Ok(i) = INVALID_CHARREFS.binary_search_by_key(&num, |(k, _)| *k) {
            return INVALID_CHARREFS[i].1.to_string();
        }
        if (0xD800..=0xDFFF).contains(&num) || num > 0x10FFFF {
            return "\u{FFFD}".to_string();
        }
        if INVALID_CODEPOINTS.binary_search(&num).is_ok() {
            return String::new();
        }
        return char::from_u32(num).map(String::from).unwrap_or_default();
    }
    if let Some(v) = named(s) {
        return v.to_string();
    }
    let chars: Vec<char> = s.chars().collect();
    for x in (2..chars.len()).rev() {
        let head: String = chars[..x].iter().collect();
        if let Some(v) = named(&head) {
            return v.to_string() + &chars[x..].iter().collect::<String>();
        }
    }
    format!("&{s}")
}

/// `html.unescape(text)`.
pub fn html_unescape(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    charref_re().replace_all(text, |c: &regex::Captures| replace_charref(&c[1])).into_owned()
}

// ---------------------------------------------------------------- placeholders
/// Text with tags / variables / escapes replaced by `<x id="N"/>`; returns (protected, the original tokens).
pub fn protect(text: &str, escape: bool) -> (String, Vec<String>) {
    let esc = |t: &str| if escape { html_escape(t) } else { t.to_string() };
    let (mut tokens, mut out, mut last) = (Vec::new(), String::new(), 0);
    for m in token_re().find_iter(text) {
        out.push_str(&esc(&text[last..m.start()]));
        tokens.push(m.as_str().to_string());
        out.push_str(&format!("<x id=\"{}\"/>", tokens.len()));
        last = m.end();
    }
    out.push_str(&esc(&text[last..]));
    (out, tokens)
}

fn placeholder_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    // Python's \s also holds \x1c-\x1f
    R.get_or_init(|| Regex::new(r#"(?i)<[\s\x1c-\x1f]*x[\s\x1c-\x1f]+id[\s\x1c-\x1f]*=[\s\x1c-\x1f]*["']?(\d+)["']?[\s\x1c-\x1f]*/?[\s\x1c-\x1f]*>"#).unwrap())
}

/// `int()` of a run of decimal digits of any script (Python accepts them all); very long numbers stop growing.
fn int_of_digits(s: &str) -> u64 {
    static ND: OnceLock<Regex> = OnceLock::new();
    let nd = ND.get_or_init(|| Regex::new(r"^\d$").unwrap());
    let is_nd = |u: u32| char::from_u32(u).is_some_and(|c| nd.is_match(c.encode_utf8(&mut [0; 4])));
    s.chars().fold(0u64, |acc, c| {
        let d = if c.is_ascii_digit() {
            c as u32 - '0' as u32
        } else {
            let mut start = c as u32; // the blocks of digits hold ten each and start at 0
            while start > 0 && is_nd(start - 1) {
                start -= 1;
            }
            (c as u32 - start) % 10
        };
        (acc * 10 + d as u64).min(1 << 40)
    })
}

/// Put the original tokens back. Returns (text, warnings).
pub fn restore(text: &str, tokens: &[String], unescape: bool) -> (String, Vec<Message>) {
    let mut seen: Vec<u64> = Vec::new();
    let out = placeholder_re()
        .replace_all(text, |c: &regex::Captures| {
            let i = int_of_digits(&c[1]);
            seen.push(i);
            if i >= 1 && i <= tokens.len() as u64 {
                tokens[i as usize - 1].clone()
            } else {
                String::new()
            }
        })
        .into_owned();
    let out = if unescape { html_unescape(&out) } else { out };
    let mut warnings = Vec::new();
    let lost: Vec<&str> = (1..=tokens.len()).filter(|i| !seen.contains(&(*i as u64))).map(|i| tokens[i - 1].as_str()).collect();
    let mut dup: Vec<u64> = seen.iter().copied().filter(|i| seen.iter().filter(|j| *j == i).count() > 1).collect();
    dup.sort_unstable();
    dup.dedup();
    if !lost.is_empty() {
        warnings.push(msg("The service did not keep: {tokens} – add them by hand.", &[("tokens", json!(lost.join(", ")))]));
    }
    if !dup.is_empty() {
        // `tokens[i - 1]` in Python: a repeated 0 counts from the end, a number beyond the tokens would stop Python with an IndexError (it is left out here)
        let list: Vec<&str> = dup.iter().filter_map(|i| if *i == 0 { tokens.last() } else { tokens.get(*i as usize - 1) }).map(String::as_str).collect();
        if !list.is_empty() {
            warnings.push(msg("These tags are repeated in the suggestion: {tokens}.", &[("tokens", json!(list.join(", ")))]));
        }
    }
    (py_strip(&out).to_string(), warnings)
}

// ---------------------------------------------------------------- services
fn hint(status: u16) -> Message {
    match status {
        400 => msg("the service rejected the request (check the languages and the settings)", &[]),
        401 => msg("the key was rejected", &[]),
        403 => msg("the key was rejected or lacks permission", &[]),
        404 => msg("the service address does not exist (check the address / model)", &[]),
        429 => msg("the request limit was exceeded, try again in a moment", &[]),
        456 => msg("the service quota is used up", &[]),
        500 => msg("an error on the service side", &[]),
        503 => msg("the service is temporarily unavailable", &[]),
        _ => msg("unknown error", &[]),
    }
}

/// `urllib.parse.urlparse` as far as the check needs it: (scheme, netloc).
fn scheme_and_netloc(url: &str) -> (String, String) {
    let url: String = url.trim_start_matches(|c: char| c <= ' ').chars().filter(|c| !matches!(c, '\t' | '\r' | '\n')).collect();
    let mut scheme = String::new();
    let mut rest = url.as_str();
    if let Some(i) = url.find(':') {
        let head = &url[..i];
        let mut cs = head.chars();
        if cs.next().is_some_and(|c| c.is_ascii_alphabetic()) && cs.all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)) {
            scheme = head.to_lowercase();
            rest = &url[i + 1..];
        }
    }
    let netloc = match rest.strip_prefix("//") {
        Some(r) => r[..r.find(['/', '?', '#']).unwrap_or(r.len())].to_string(),
        None => String::new(),
    };
    (scheme, netloc)
}

pub fn check_url(url: &str) -> Result<String, MtError> {
    let (scheme, netloc) = scheme_and_netloc(url);
    if (scheme != "http" && scheme != "https") || netloc.is_empty() {
        return Err(MtError(msg("The service address must start with http:// or https://.", &[])));
    }
    Ok(url.trim_end_matches('/').to_string())
}

fn call(url: &str, headers: &[(String, String)], data: &[u8], http: Http) -> Result<Value, MtError> {
    let (status, body) = http(url, headers, data)?;
    if status >= 400 {
        return Err(MtError(msg("The service answered with the error {status}: {hint}.", &[("status", json!(status)), ("hint", hint(status).wire())])));
    }
    serde_json::from_str(&body).map_err(|_| MtError(msg("The service returned an answer I do not understand.", &[])))
}

/// `quote_plus` of `urllib.parse.urlencode`.
fn quote_plus(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            b => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn urlencode(pairs: &[(&str, &str)]) -> String {
    pairs.iter().map(|(k, v)| format!("{}={}", quote_plus(k), quote_plus(v))).collect::<Vec<_>>().join("&")
}

/// Ren'Py language folder names (game/tl/<name>) -> the code the services want.
pub const LANGUAGE_CODES: [(&str, &str); 31] = [
    ("czech", "CS"),
    ("german", "DE"),
    ("french", "FR"),
    ("spanish", "ES"),
    ("italian", "IT"),
    ("portuguese", "PT"),
    ("russian", "RU"),
    ("polish", "PL"),
    ("slovak", "SK"),
    ("english", "EN"),
    ("dutch", "NL"),
    ("swedish", "SV"),
    ("danish", "DA"),
    ("finnish", "FI"),
    ("norwegian", "NB"),
    ("hungarian", "HU"),
    ("romanian", "RO"),
    ("bulgarian", "BG"),
    ("ukrainian", "UK"),
    ("turkish", "TR"),
    ("greek", "EL"),
    ("japanese", "JA"),
    ("chinese", "ZH"),
    ("korean", "KO"),
    ("indonesian", "ID"),
    ("arabic", "AR"),
    ("latvian", "LV"),
    ("lithuanian", "LT"),
    ("estonian", "ET"),
    ("slovenian", "SL"),
    ("croatian", "HR"),
];

/// The target language code for the open project: the language of its pack (`cs`) or, failing that, the name of its tl folder (`czech`). "" when neither says.
pub fn target_language(project_language: &str, folder_name: &str) -> String {
    let code = py_strip(project_language);
    let n = code.chars().count();
    if (2..=3).contains(&n) && code.chars().all(char::is_alphabetic) {
        return code.to_uppercase();
    }
    let name = py_strip(folder_name).to_lowercase();
    LANGUAGE_CODES.iter().find(|(k, _)| *k == name).map(|(_, v)| v.to_string()).unwrap_or_default()
}

/// `value[key]` of a JSON object; `None` where Python would raise `KeyError` or `TypeError`.
fn key<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    v.as_object().and_then(|o| o.get(k))
}

/// `value[0]`: the first item of a list (or the first character of a text); `None` where Python would raise.
fn first(v: &Value) -> Option<Value> {
    match v {
        Value::Array(a) => a.first().cloned(),
        Value::String(s) => s.chars().next().map(|c| Value::String(c.to_string())),
        _ => None,
    }
}

fn shape(text: &str) -> MtError {
    MtError(msg(text, &[]))
}

fn text_of(v: Option<Value>, error: &str) -> Result<String, MtError> {
    match v {
        Some(Value::String(s)) => Ok(s),
        _ => Err(shape(error)),
    }
}

fn json_body(value: Value) -> Vec<u8> {
    json_dumps(&value, true).into_bytes()
}

/// Suggest a translation of `text` into the target language. `cfg` = {provider, api_key, base_url, model, source_lang, target_lang, consent}; `glossary` = (English term, translation)
/// pairs; `context` = the previous lines (used by the openai provider only).
pub fn translate(cfg: &Map<String, Value>, text: &str, glossary: &[(String, String)], context: &[String], http: Http) -> Result<Translation, MtError> {
    let provider = cfg.get("provider").and_then(Value::as_str).unwrap_or("");
    if !PROVIDERS.contains(&provider) {
        return Err(shape("Machine translation is not set up."));
    }
    if !cfg.get("consent").is_some_and(truthy) {
        return Err(shape("Missing consent to send text to an external service (Settings → Machine translation)."));
    }
    let get = |k: &str| cfg.get(k).map(py_str).unwrap_or_default();
    let (key_text, base) = (py_strip(&get("api_key")).to_string(), py_strip(&get("base_url")).to_string());
    let key_text = key_text.as_str();
    if key_text.is_empty() && !((provider == "openai" || provider == "libretranslate") && !base.is_empty()) {
        return Err(shape("The API key is missing."));
    }
    if py_strip(text).is_empty() {
        return Err(MtError(no_text()));
    }
    let src = match cfg.get("source_lang") {
        None => "EN".to_string(),
        Some(v) if !truthy(v) => "EN".to_string(),
        Some(v) => py_str(v),
    };
    let dst = cfg.get("target_lang").filter(|v| truthy(v)).map(py_str).unwrap_or_default();
    if dst.is_empty() {
        return Err(shape("The target language is not known. Set it in Settings → Machine translation."));
    }
    let markup = provider == "deepl" || provider == "libretranslate"; // these two parse the text as XML / HTML
    let (protected, tokens) = protect(text, markup);
    let (out, unescape) = match provider {
        "deepl" => {
            let host = if !base.is_empty() {
                check_url(&base)?
            } else if key_text.ends_with(":fx") {
                "https://api-free.deepl.com".to_string()
            } else {
                "https://api.deepl.com".to_string()
            };
            let data = urlencode(&[
                ("text", &protected),
                ("source_lang", &src.to_uppercase()),
                ("target_lang", &dst.to_uppercase()),
                ("tag_handling", "xml"),
                ("non_splitting_tags", "x"),
                ("ignore_tags", "x"),
            ]);
            let headers = [
                ("Authorization".to_string(), format!("DeepL-Auth-Key {key_text}")),
                ("Content-Type".to_string(), "application/x-www-form-urlencoded".to_string()),
            ];
            let res = call(&format!("{host}/v2/translate"), &headers, data.as_bytes(), http)?;
            let out = key(&res, "translations").and_then(first).and_then(|t| key(&t, "text").cloned());
            (text_of(out, "The DeepL answer does not have the expected shape.")?, true)
        }
        "libretranslate" => {
            let host = check_url(if base.is_empty() { "https://libretranslate.com" } else { &base })?;
            let mut body = Map::new();
            body.insert("q".into(), json!(protected));
            body.insert("source".into(), json!(src.to_lowercase()));
            body.insert("target".into(), json!(dst.to_lowercase()));
            body.insert("format".into(), json!("html"));
            if !key_text.is_empty() {
                body.insert("api_key".into(), json!(key_text));
            }
            let res =
                call(&format!("{host}/translate"), &[("Content-Type".to_string(), "application/json".to_string())], &json_body(Value::Object(body)), http)?;
            (text_of(key(&res, "translatedText").cloned(), "The LibreTranslate answer does not have the expected shape.")?, true)
        }
        _ => {
            let host = check_url(if base.is_empty() { "https://api.openai.com/v1" } else { &base })?;
            let mut system = SYSTEM_PROMPT.replace("{src}", &src).replace("{dst}", &dst);
            if !glossary.is_empty() {
                system += &format!(" Preferred translations of terms: {}.", glossary.iter().map(|(k, v)| format!("{k} = {v}")).collect::<Vec<_>>().join("; "));
            }
            let user = if context.is_empty() { String::new() } else { format!("Context (previous lines, do not translate):\n{}\n\n", context.join("\n")) }
                + "Translate:\n"
                + &protected;
            let model = cfg.get("model").filter(|v| truthy(v)).map(py_str).unwrap_or_else(|| "gpt-4o-mini".to_string());
            let body = json!({"model": model, "temperature": 0.2, "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}]});
            let mut headers = vec![("Content-Type".to_string(), "application/json".to_string())];
            if !key_text.is_empty() {
                headers.push(("Authorization".to_string(), format!("Bearer {key_text}")));
            }
            let res = call(&format!("{host}/chat/completions"), &headers, &json_body(body), http)?;
            let out = key(&res, "choices").and_then(first).and_then(|c| key(&c, "message").and_then(|m| key(m, "content")).cloned());
            let out = text_of(out, "The answer of the service does not have the expected shape.")?;
            (py_strip(&out).trim_matches('"').to_string(), false)
        }
    };
    let (result, warnings) = restore(&out, &tokens, unescape);
    Ok(Translation { text: result, warnings, provider: provider.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unescape_follows_python() {
        assert_eq!(html_unescape("a &amp; b &lt;x&gt; &quot;q&quot; &#39;s&#x27; &nbsp;|"), "a & b <x> \"q\" 's' \u{a0}|");
        assert_eq!(html_unescape("&ampfoo &unknown; &#0; &#x80; &#1; &#xD800; &#99999999999;"), "&foo &unknown; \u{fffd} \u{20ac}  \u{fffd} \u{fffd}");
        assert_eq!(html_unescape("no entity"), "no entity");
    }

    #[test]
    fn tokens_are_protected_and_restored() {
        let (p, t) = protect("Hi {i}[name]\\n <b>", true);
        assert_eq!(p, "Hi <x id=\"1\"/><x id=\"2\"/><x id=\"3\"/> &lt;b&gt;");
        assert_eq!(t, ["{i}", "[name]", "\\n"]);
        let (back, warn) = restore("Ahoj <X id='1'/> <x id=3 > &lt;b&gt;", &t, true);
        assert_eq!(back, "Ahoj {i} \\n <b>");
        assert_eq!(warn.len(), 1);
        assert_eq!(warn[0].text(), "The service did not keep: [name] – add them by hand.");
    }

    #[test]
    fn language_codes() {
        assert_eq!(target_language("cs", ""), "CS");
        assert_eq!(target_language("", "Czech"), "CS");
        assert_eq!(target_language("czech", "x"), "");
    }
}
