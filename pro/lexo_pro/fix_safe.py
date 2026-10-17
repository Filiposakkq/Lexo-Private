"""Safe bulk fixes with a preview (the Pro feature fixes.safe): planning the fixes and chaining them on one line.

Three kinds, all of them touch only what they must: "speaker" (the speaker, attributes and trailing code of the line are copied from the original comment, the quoted text is
never touched), "spaces" (collapse a double space between words unless the original has one) and "typo" (apply the spelling hints: missing diacritics, swapped or doubled letters).
Tags, variables and escapes are protected. Plain data in, plain data out: Core finds the candidate lines (it already knows from its own checks which lines need a fix) and writes the
confirmed result with its single writer; this module decides what each fix is. Untranslated lines are never candidates.
"""
import re
from types import SimpleNamespace

from lexo import rpy
from lexo.messages import msg
from lexo.textedit import LETTER, case_like, sub_pattern
from lexo.util import norm_ws

from . import licence

KINDS = ("speaker", "spaces", "typo")


def fixed_line(entry, raw_line):
    """The code line with speaker and trailing code taken from the original, or None when not applicable."""
    m = rpy.STR_RE.search(raw_line)
    if not m:
        return None
    indent = raw_line[:len(raw_line) - len(raw_line.lstrip())]
    who = entry.src_who.strip()
    suffix = entry.src_suffix.strip()
    return indent + (who + " " if who else "") + raw_line[m.start():m.end()] + (" " + suffix if suffix else "")


def fix_spaces(source, target):
    """Collapse runs of spaces between words, unless the original has them too. Returns the new text or None."""
    probe = re.compile(r"\S {2,}\S")
    if not probe.search(target) or probe.search(source):
        return None
    new, n = sub_pattern(re.compile(r"(?<=\S) {2,}(?=\S)"), lambda m: " ", target, True)
    return new if n and new != target else None


def fix_typos(target, issues):
    """Apply the spelling hints [{word, better, typo_kind}] to the text. Returns (new_text, [(word, better, kind)]) or None."""
    new, done = target, []
    for iss in issues or []:
        w, better = iss.get("word"), iss.get("better")
        if not w or not better:
            continue
        pat = re.compile(f"(?<!{LETTER}){re.escape(w)}(?!{LETTER})", re.IGNORECASE | re.UNICODE)
        changed, n = sub_pattern(pat, lambda m: case_like(m.group(0), better), new, True, limit=1)
        if n:
            new = changed
            done.append((w, better, iss.get("typo_kind", "")))
    return (new, done) if done else None


def plan(p):
    """{items, total, fixable, counts, limited?}: every fix of the requested kinds for the candidates, cut to the limit of the licence."""
    kinds = [k for k in p.get("kinds") or ["speaker"] if k in KINDS]
    out = []
    if "speaker" in kinds:
        for c in p.get("speaker", []):
            after = fixed_line(SimpleNamespace(src_who=c["src_who"], src_suffix=c["src_suffix"]), c["cur_raw"])
            skip = ""
            if after is None:
                skip = msg("The line does not contain text in quotes.")
            elif norm_ws(after) == norm_ws(c["src_raw"]):
                skip = msg("After the fix the line would be identical to the original (it would count as untranslated) – fix it by hand.")
            out.append({"file": c["file"], "line": c["line"], "kind": "speaker", "before": c["cur_raw"], "after": after or "", "skip": skip, "default": not skip, "code": True,
                        "note": msg("speaker / attributes from the original")})
    for c in p.get("text", []):
        if "spaces" in kinds:
            new = fix_spaces(c["source"], c["target"])
            if new is not None:
                out.append({"file": c["file"], "line": c["line"], "kind": "spaces", "before": c["target"], "after": new, "default": True, "note": msg("double space"),
                            "skip": "", "code": False})
        if "typo" in kinds:
            res = fix_typos(c["target"], c.get("issues"))
            if res is not None:
                new, done = res
                sure = all(k == "diacritics" and len(w) >= 6 for w, _b, k in done)
                out.append({"file": c["file"], "line": c["line"], "kind": "typo", "before": c["target"], "after": new, "default": sure,
                            "note": "; ".join(f"{w} → {b}" for w, b, _k in done), "skip": "", "code": False})
    out.sort(key=lambda r: (r["file"].lower(), r["line"], r["kind"]))
    counts = {k: sum(1 for r in out if r["kind"] == k) for k in kinds}
    full = len(out)
    out, cut = licence.cap("fixes.safe", out)
    return {**cut, "items": out, "total": full, "fixable": sum(1 for r in out if not r["skip"]), "counts": counts}


def chain(p):
    """The text of each requested line after the chosen text fixes, one after another (spaces first, then typos): [{file, line, text}]."""
    out = []
    for c in p["items"]:
        text = c["target"]
        if "spaces" in c["kinds"]:
            new = fix_spaces(c["source"], text)
            text = new if new is not None else text
        if "typo" in c["kinds"]:
            res = fix_typos(text, c.get("issues"))
            text = res[0] if res else text
        out.append({"file": c["file"], "line": c["line"], "text": text})
    return {"items": out}
