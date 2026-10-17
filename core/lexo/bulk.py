"""Bulk operations: applying reviewed group assignments and global approval of identical lines.

The suggestions of groups for files are a Pro feature (the Pro pack, bulk.categorize); applying them, and the approval of identical lines, are here.
Both only *propose* what they show; nothing changes until the caller applies the reviewed result.
"""
import posixpath
import re
from collections import Counter, defaultdict

from . import rpy
from . import layout
from .store import assign_label, valid_assign
from .util import norm_ws
from .messages import join, msg

def episode_hint(n, rows):
    """Rough % for an episode from file names alone (kept from the original tool; display only)."""
    pat = re.compile(rf"(?<![a-z])(?:episode|ep)[ _-]?0*{n}(?!\d)")
    d = t = 0
    for r in rows:
        if pat.search(_posix(r.name)):
            d += r.done
            t += r.total
    return f"≈ {d / t * 100:.0f} %" if t else ""


def apply_assignments(store, items):
    """Apply reviewed assignments in one undoable step.

    items: [{name, value, overwrite}] — a manual value is only replaced when overwrite is True.
    Returns {applied, skipped:[(name, reason)]}.
    """
    applied, skipped = [], []

    def go():
        for it in items:
            name, v = it["name"], it.get("value", "")
            if not valid_assign(v):
                skipped.append((name, msg("invalid assignment")))
                continue
            cur = store.assign.get(name, "")
            if cur == v:
                continue
            if cur and not it.get("overwrite"):
                skipped.append((name, msg("has an assignment set by hand (the overwrite was not confirmed)")))
                continue
            if v:
                store.assign[name] = v
            else:
                store.assign.pop(name, None)
            applied.append(name)
    store.change(msg("Assignment of files ({n})", n=len(items)), ("assign",), go)
    return {"applied": applied, "skipped": skipped}


# ---------------------------------------------------------------- global approval

SCOPES = ("occurrence", "file", "project")


def approval_candidates(project, rel, line, scope="project", same_speaker=True):
    """Find lines that are identical to the chosen pending line and could be approved together.

    A match needs the same normalized source *and* translation. Everything else about the
    context is shown and decides the default checkbox:
      - a different speaker is unchecked (the same words can mean something else),
      - empty translations can never be approved,
      - lines that look like untranslated English are unchecked,
      - already approved lines are listed but not selectable.
    Returns {reference, scope, items:[...], counts}.
    """
    if scope not in SCOPES:
        raise ValueError(msg("unknown scope"))
    ref = next((e for e in project.entries(rel) if e.line == line), None)
    if ref is None or not ref.pending:
        raise ValueError(msg("The line is not among the untranslated ones."))
    src, tgt = norm_ws(ref.source), norm_ws(ref.target)
    approved = project.store.approved
    files = [rel] if scope in ("occurrence", "file") else sorted(project.files, key=str.lower)
    items = []
    for f in files:
        done = approved.get(f, ())
        for e in project.entries(f):
            if not e.pending or norm_ws(e.source) != src or norm_ws(e.target) != tgt:
                continue
            if scope == "occurrence" and e.line != line:
                continue
            already = e.key in done
            same_who = norm_ws(e.who) == norm_ws(ref.who)
            reasons = []
            selectable, default = True, True
            if already:
                selectable = default = False
                reasons.append(msg("already approved"))
            if e.category == rpy.K_EMPTY:
                selectable = default = False
                reasons.append(msg("an empty translation cannot be approved"))
            if not same_who and same_speaker:
                default = False
                reasons.append(msg("another speaker (“{a}” instead of “{b}”)", a=e.who or '—', b=ref.who or '—'))
            if e.category == rpy.K_ENGLISH and default:
                default = False
                reasons.append(msg("looks like untranslated English"))
            if e is ref:
                default = selectable
            items.append({"file": f, "line": e.line, "block": e.block, "who": e.who,
                          "text": e.target, "kind": e.category,
                          "key": [e.block, norm_ws(e.source)], "approved": already,
                          "selectable": selectable, "selected": default, "notes": reasons,
                          "is_reference": e is ref})
    return {"reference": {"file": rel, "line": line, "text": ref.target, "who": ref.who},
            "scope": scope, "items": items,
            "counts": {"total": len(items), "selected": sum(1 for i in items if i["selected"]),
                       "files": len({i["file"] for i in items})}}


def apply_approvals(project, selections, on=True, label=None):
    """Approve / un-approve [{file, key:[block, source]}] in one undoable step.

    Each selection is re-checked against the current parse, so a file that changed since the
    preview cannot get a stale approval. Returns {changed, skipped}.
    """
    store = project.store
    skipped = []
    plan = defaultdict(set)
    for sel in selections:
        f, key = sel["file"], (str(sel["key"][0]), norm_ws(str(sel["key"][1])))
        if f not in project.files:
            skipped.append((f, msg("unknown file")))
            continue
        if on:
            ent = next((e for e in project.entries(f) if e.pending and e.key == key), None)
            if ent is None:
                skipped.append((f, msg("the line changed in the meantime – refresh the files")))
                continue
            if ent.category == rpy.K_EMPTY:
                skipped.append((f, msg("an empty translation cannot be approved")))
                continue
        plan[f].add(key)
    changed = 0

    def go():
        nonlocal changed
        for f, keys in plan.items():
            cur = set(store.approved.get(f, set()))
            new = (cur | keys) if on else (cur - keys)
            changed += len(new ^ cur)
            if new:
                store.approved[f] = new
            else:
                store.approved.pop(f, None)
    n = sum(len(k) for k in plan.values())
    store.change(label or ((msg("Approving {n} lines", n=n) if on else msg("Removing the approval of {n} lines", n=n))), ("approved",), go)
    return {"changed": changed, "skipped": skipped}


# ---------------------------------------------------------------- approval by rule

RULES = {
    "symbols": msg("Symbols only (…, ?!, numbers) – nothing to translate"),
    "interjections": msg("Interjections (Hmm, Ah, Haha, Wow, Ugh…) – the same in the translation"),
    "names": msg("Only names from the Glossary (speakers, terms)"),
}
INTERJECTIONS = {"hm", "hmm", "hmmm", "mm", "mmm", "mhm", "ah", "aha", "ahh", "oh", "ooh", "uh", "uhm", "um", "umm",
                 "ugh", "ha", "haha", "hahaha", "hehe", "heh", "hihi", "wow", "whoa", "yay", "ow", "ouch", "argh",
                 "eh", "huh", "shh", "psst", "ok", "okay", "ahem", "pfft", "tsk", "tss", "boo", "bam", "pow", "zzz"}
_WORDS = re.compile(r"[^\W\d_]+(?:'[^\W\d_]+)?", re.UNICODE)


def _plain(text):
    return re.sub(r"\{[^}]*\}|\[[^\]]*\]", " ", text.replace("\\n", " "))


def name_words(glossary):
    """Lower-case words of every speaker name and glossary term (both the English and the Czech side)."""
    out = set()
    for code, g in glossary.items():
        for part in (code, g.get("n", "")):
            out.update(w.lower() for w in _WORDS.findall(part))
    return out


def rule_candidates(project, glossary, rules, rels=None):
    """Pending lines that a simple rule says need no translation. Nothing is changed.

    Returns {items:[{file, line, key, who, text, rule, selectable, selected, notes}], counts}.
    `rels` limits the search to those files (None / empty = the whole project).
    An already approved line is listed but not selectable; an empty translation never qualifies."""
    rules = [r for r in rules if r in RULES]
    names = name_words(glossary) if "names" in rules else set()
    approved = project.store.approved
    items, per_rule = [], Counter()
    files = [f for f in (rels or []) if f in project.files] if rels else sorted(project.files, key=str.lower)
    for f in sorted(files, key=str.lower):
        done = approved.get(f, ())
        for e in project.entries(f):
            if not e.pending or e.category == rpy.K_EMPTY:
                continue
            text = e.target or ""
            words = [w.lower() for w in _WORDS.findall(_plain(text))]
            hit = None
            if "symbols" in rules and e.category == rpy.K_SYMBOLS:
                hit = "symbols"
            elif words and "interjections" in rules and all(w in INTERJECTIONS for w in words):
                hit = "interjections"
            elif words and "names" in rules and len(words) <= 4 and all(w in names for w in words):
                hit = "names"
            if not hit:
                continue
            already = e.key in done
            per_rule[hit] += 1
            items.append({"file": f, "line": e.line, "block": e.block, "who": e.who, "text": text, "rule": hit,
                          "key": [e.block, norm_ws(e.source)], "approved": already,
                          "selectable": not already, "selected": not already,
                          "notes": [msg("already approved")] if already else []})
    return {"items": items, "rules": RULES, "files": len(files),
            "counts": {"total": len(items), "selectable": sum(1 for i in items if i["selectable"]),
                       "by_rule": dict(per_rule)}}
