"""Optional machine translation: a SUGGESTION for one line, never written anywhere by itself.

Off by default. It sends the English text of the line you ask about to the service you chose, so it needs your explicit
consent in the settings (`mt.consent`) and your own API key, which is stored in plain text in the app's config file.
Nothing is sent until you click the button for a line.

Tags `{i}`, variables `[name]` and escapes (`\\n`) never go to the service as text: they are replaced by `<x id="N"/>`
placeholders and put back afterwards. If a placeholder comes back lost or duplicated, the suggestion carries a warning.

Providers (all REST, no extra packages): "deepl", "openai" (any OpenAI compatible chat endpoint, also local ones),
"libretranslate". Experimental: written from the services' documentation and covered by tests with a fake HTTP layer;
it has not been run against the real services.
"""
import html
import json
import re
import urllib.error
import urllib.parse
import urllib.request

from .textedit import TOKEN_RE
from .messages import msg

PROVIDERS = ("deepl", "openai", "libretranslate")
TIMEOUT = 30
PLACEHOLDER_RE = re.compile(r"<\s*x\s+id\s*=\s*[\"']?(\d+)[\"']?\s*/?\s*>", re.IGNORECASE)
SYSTEM_PROMPT = ("You translate dialogue of a visual novel (Ren'Py game) from {src} to {dst}. Keep every <x id=\"N\"/> element "
                 "exactly as it is, once, in a natural place of the sentence. Keep the tone of spoken dialogue. "
                 "Return only the translated text, no explanations and no quotes.")


NO_TEXT = msg("The line has no text to translate.")      # the one failure that must not stop a batch


class MTError(Exception):
    pass


def protect(text, escape=False):
    """Text with tags / variables / escapes replaced by <x id="N"/>; returns (protected, [original tokens])."""
    tokens, out, last = [], [], 0
    esc = html.escape if escape else (lambda t: t)
    for m in TOKEN_RE.finditer(text):
        out.append(esc(text[last:m.start()]))
        tokens.append(m.group(0))
        out.append(f'<x id="{len(tokens)}"/>')
        last = m.end()
    out.append(esc(text[last:]))
    return "".join(out), tokens


def restore(text, tokens, unescape=False):
    """Put the original tokens back. Returns (text, warnings)."""
    seen = []

    def put(m):
        i = int(m.group(1))
        seen.append(i)
        return tokens[i - 1] if 1 <= i <= len(tokens) else ""
    out = PLACEHOLDER_RE.sub(put, text)
    if unescape:
        out = html.unescape(out)
    warnings = []
    lost = [i for i in range(1, len(tokens) + 1) if i not in seen]
    dup = sorted({i for i in seen if seen.count(i) > 1})
    if lost:
        warnings.append(msg("The service did not keep: {tokens} – add them by hand.", tokens=", ".join(tokens[i - 1] for i in lost)))
    if dup:
        warnings.append(msg("These tags are repeated in the suggestion: {tokens}.", tokens=", ".join(tokens[i - 1] for i in dup)))
    return out.strip(), warnings


def _post(url, headers, data, timeout=TIMEOUT):
    req = urllib.request.Request(url, data=data, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return r.status, r.read().decode("utf-8", errors="replace")
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode("utf-8", errors="replace")
    except (urllib.error.URLError, OSError) as e:
        raise MTError(msg("The connection to the service failed: {reason}", reason=getattr(e, 'reason', e))) from e


HTTP_HINTS = {400: msg("the service rejected the request (check the languages and the settings)"), 401: msg("the key was rejected"), 403: msg("the key was rejected or lacks permission"),
              404: msg("the service address does not exist (check the address / model)"), 429: msg("the request limit was exceeded, try again in a moment"),
              456: msg("the service quota is used up"), 500: msg("an error on the service side"), 503: msg("the service is temporarily unavailable")}


def _check_url(url):
    p = urllib.parse.urlparse(url)
    if p.scheme not in ("http", "https") or not p.netloc:
        raise MTError(msg("The service address must start with http:// or https://."))
    return url.rstrip("/")


def _call(url, headers, data, http):
    status, body = (http or _post)(url, headers, data)
    if status >= 400:
        raise MTError(msg("The service answered with the error {status}: {hint}.", status=status, hint=HTTP_HINTS.get(status, msg("unknown error"))))
    try:
        return json.loads(body)
    except ValueError as e:
        raise MTError(msg("The service returned an answer I do not understand.")) from e


# Ren'Py language folder names (game/tl/<name>) and ISO codes -> the code the services want
LANGUAGE_CODES = {"czech": "CS", "german": "DE", "french": "FR", "spanish": "ES", "italian": "IT", "portuguese": "PT", "russian": "RU", "polish": "PL",
                  "slovak": "SK", "english": "EN", "dutch": "NL", "swedish": "SV", "danish": "DA", "finnish": "FI", "norwegian": "NB", "hungarian": "HU",
                  "romanian": "RO", "bulgarian": "BG", "ukrainian": "UK", "turkish": "TR", "greek": "EL", "japanese": "JA", "chinese": "ZH", "korean": "KO",
                  "indonesian": "ID", "arabic": "AR", "latvian": "LV", "lithuanian": "LT", "estonian": "ET", "slovenian": "SL", "croatian": "HR"}


def target_language(project_language="", folder_name=""):
    """The target language code for the open project: the language of its pack (`cs`) or, failing that, the name of its tl folder (`czech`).
    "" when neither says."""
    code = (project_language or "").strip()
    if 2 <= len(code) <= 3 and code.isalpha():
        return code.upper()
    return LANGUAGE_CODES.get((folder_name or "").strip().lower(), "")


def translate(cfg, text, glossary=None, context=None, http=None):
    """Suggest a translation of `text` into the target language. Returns {text, warnings, provider}.

    cfg = {provider, api_key, base_url, model, source_lang, target_lang, consent}.
    glossary = {english term: czech}; context = previous lines [str] (only used by the openai provider).
    http(url, headers, data_bytes) -> (status, body) can be replaced in tests."""
    provider = cfg.get("provider", "")
    if provider not in PROVIDERS:
        raise MTError(msg("Machine translation is not set up."))
    if not cfg.get("consent"):
        raise MTError(msg("Missing consent to send text to an external service (Settings → Machine translation)."))
    key = str(cfg.get("api_key", "")).strip()
    base = str(cfg.get("base_url", "")).strip()
    if not key and not (provider in ("openai", "libretranslate") and base):
        raise MTError(msg("The API key is missing."))
    if not text.strip():
        raise MTError(NO_TEXT)
    src, dst = cfg.get("source_lang", "EN") or "EN", cfg.get("target_lang", "")
    if not dst:
        raise MTError(msg("The target language is not known. Set it in Settings → Machine translation."))
    markup = provider in ("deepl", "libretranslate")          # these two parse the text as XML / HTML
    protected, tokens = protect(text, escape=markup)
    if provider == "deepl":
        host = _check_url(base) if base else ("https://api-free.deepl.com" if key.endswith(":fx") else "https://api.deepl.com")
        data = urllib.parse.urlencode({"text": protected, "source_lang": src.upper(), "target_lang": dst.upper(), "tag_handling": "xml",
                                       "non_splitting_tags": "x", "ignore_tags": "x"}).encode("utf-8")
        res = _call(host + "/v2/translate", {"Authorization": f"DeepL-Auth-Key {key}", "Content-Type": "application/x-www-form-urlencoded"}, data, http)
        try:
            out = res["translations"][0]["text"]
        except (KeyError, IndexError, TypeError) as e:
            raise MTError(msg("The DeepL answer does not have the expected shape.")) from e
        result, warnings = restore(out, tokens, unescape=True)
    elif provider == "libretranslate":
        host = _check_url(base or "https://libretranslate.com")
        body = {"q": protected, "source": src.lower(), "target": dst.lower(), "format": "html"}
        if key:
            body["api_key"] = key
        res = _call(host + "/translate", {"Content-Type": "application/json"}, json.dumps(body).encode("utf-8"), http)
        try:
            out = res["translatedText"]
        except (KeyError, TypeError) as e:
            raise MTError(msg("The LibreTranslate answer does not have the expected shape.")) from e
        result, warnings = restore(out, tokens, unescape=True)
    else:
        host = _check_url(base or "https://api.openai.com/v1")
        system = SYSTEM_PROMPT.format(src=src, dst=dst)
        if glossary:
            system += " Preferred translations of terms: " + "; ".join(f"{k} = {v}" for k, v in glossary.items()) + "."
        user = ("Context (previous lines, do not translate):\n" + "\n".join(context) + "\n\n" if context else "") + "Translate:\n" + protected
        body = {"model": cfg.get("model") or "gpt-4o-mini", "temperature": 0.2,
                "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}]}
        headers = {"Content-Type": "application/json"}
        if key:
            headers["Authorization"] = f"Bearer {key}"
        res = _call(host + "/chat/completions", headers, json.dumps(body).encode("utf-8"), http)
        try:
            out = res["choices"][0]["message"]["content"]
        except (KeyError, IndexError, TypeError) as e:
            raise MTError(msg("The answer of the service does not have the expected shape.")) from e
        result, warnings = restore(out.strip().strip('"'), tokens)
    return {"text": result, "warnings": warnings, "provider": provider}
