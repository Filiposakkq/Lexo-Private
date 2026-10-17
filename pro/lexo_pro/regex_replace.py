"""Regular expression find and replace over a list of texts (the Pro feature replace.regex).

Works on plain data: the request carries the pattern, the replacement and the texts; the answer is the new text and the number of replacements for every text.
Core decides which lines are candidates and writes the confirmed result; tags, variables and escapes are protected exactly as in the plain modes (Core's `textedit`).
"""
import re

from lexo import textedit
from lexo.extensions import ExtensionError
from lexo.messages import msg


def substitute(p):
    find, replace, case, protect, texts = str(p["pattern"]), str(p["replace"]), bool(p.get("case")), bool(p.get("protect", True)), p["texts"]
    if not find:
        raise ExtensionError(msg("Enter what to search for."))
    if len(find) > 300 or len(replace) > 2000:
        raise ExtensionError(msg("The text to find or replace is too long."))
    try:
        pat = re.compile(find, (0 if case else re.IGNORECASE) | re.UNICODE)
        re.compile(pat.pattern).sub(replace, "test")                # reject bad group references early
    except re.error as e:
        raise ExtensionError(msg("Invalid regular expression: {e}", e=e)) from e

    def rx(m):
        try:
            return m.expand(replace)
        except (re.error, IndexError) as e:
            raise ValueError(msg("Invalid replacement: {e}", e=e)) from e
    out = []
    for text in texts:
        try:
            new, n = textedit.sub_pattern(pat, rx, str(text), protect)
        except ValueError as e:
            raise ExtensionError(*e.args) from e
        out.append({"new": new, "n": n})
    return {"results": out}
