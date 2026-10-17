"""Text-level helpers for batch edits of translations: find / replace in the plain modes (the safe fixes and the regex mode are Pro features).

Everything here is pure (no file access): it turns a translated text into a proposed new text. Writing is done by
fixes.apply_edits after the user confirmed a preview. Tags `{i}`, variables `[name]` and escapes (`\\n`, `\\"`) are
protected by default, so a replacement can never break them.

What is deliberately NOT automated: variables and tags that are missing, extra or in a different order. In Czech the
word order often differs from English, so those findings are frequently false positives; they can only be ignored
or fixed by hand.
"""
import re

from . import validator
from .messages import msg

TOKEN_RE = re.compile(r"\{[^}]*\}|\[[^\]]*\]|\\.")      # tag, variable, escape
LETTER = r"[^\W\d_]"
MODES = ("plain", "word", "stem", "regex")


def mask(text):
    """Same length, with protected parts replaced by NUL so no pattern can match inside them."""
    return TOKEN_RE.sub(lambda m: "\x00" * len(m.group()), text)


def case_like(model, new):
    if model[:1].isupper() and new[:1].islower():
        return new[:1].upper() + new[1:]
    return new


def sub_pattern(pattern, repl, text, protect=True, limit=0):
    """Apply a compiled `pattern` outside protected parts. `repl` is a function(match) -> str. Returns (new, count)."""
    hay = mask(text) if protect else text
    out, last, n = [], 0, 0
    for m in pattern.finditer(hay):
        if m.end() == m.start() or "\x00" in m.group():
            continue
        out.append(text[last:m.start()])
        out.append(repl(m))
        last, n = m.end(), n + 1
        if limit and n >= limit:
            break
    out.append(text[last:])
    return "".join(out), n


def stem_of(word):
    return word[:max(3, len(word) - 2)]


def make_replacer(find, replace, mode="plain", case=False):
    """Returns f(text, protect=True) -> (new_text, count). Raises ValueError for an unusable search."""
    if not find:
        raise ValueError(msg("Enter what to search for."))
    if len(find) > 300 or len(replace) > 2000:
        raise ValueError(msg("The text to find or replace is too long."))
    if mode not in MODES:
        raise ValueError(msg("Unknown search mode."))
    flags = 0 if case else re.IGNORECASE
    if mode == "regex":                                        # a Pro feature: the app asks the Pro pack (replace.regex), it is never made here
        raise ValueError(msg("Regex find & replace is a Pro feature."))
    if mode == "stem":
        if not re.fullmatch(LETTER + "{4,}", find) or not re.fullmatch(LETTER + "{2,}", replace):
            raise ValueError(msg("The “by stem” mode is only for one word (at least 4 letters) and one replacement word."))
        stem = stem_of(find)
        pat = re.compile(f"(?<!{LETTER})({re.escape(stem)})({LETTER}*)", flags | re.UNICODE)
        new_stem = stem_of(replace)

        def st(m):
            whole = m.group(0)
            if whole.lower().startswith(find.lower()):
                new = replace + whole[len(find):]               # the whole word as typed, plus any ending after it
            else:
                new = new_stem + whole[len(stem):]              # another form: keep its ending, swap the stem
            return case_like(whole, new)
        return lambda text, protect=True: sub_pattern(pat, st, text, protect)
    body = re.escape(find)
    if mode == "word":
        body = f"(?<!{LETTER}){body}(?!{LETTER})"
    pat = re.compile(body, flags | re.UNICODE)
    return lambda text, protect=True: sub_pattern(pat, lambda m: replace, text, protect)
