"""Translator-maintained metadata, stored in small CSV files in the data folder (Excel-friendly).

  proofread.csv       file;proofread;translated_manual            proofread lines / manual translated override
  assignments.csv     file;group                                  file -> 'general' | 'episode N'
  approved.csv        file;block;original                         manually approved pending lines
  episode_states.csv  episode;state                               hand-set episode status (none, wip, tr, proof, done)
  line_marks.csv      file;block;original;state;hash;note;date    per-line proofreading marks
  ignored.csv         file;block;original;code;hash;note;date     validator findings the translator chose to ignore
  glossary.csv        code;name;type;note                         speaker code -> full name (type "term" for later entries)
  history.csv         see history.py

Data written by the BaDIK tool (Czech file names) is copied to these names once by migrate.py.
Every write is atomic and backed up (<local data dir>/backups). Mutations go through `change()`,
which keeps an undo stack of the most recent changes.
"""
import copy
import hashlib
from datetime import date
from pathlib import Path

from . import layout
from .util import WriteFailed, fold, norm_ws, read_rows, write_csv
from .messages import msg

EP_STATE_KEYS = ("none", "wip", "tr", "proof", "done")


def ep_states():
    """[(key, label)] of the episode states; labels are messages, so call this at run time."""
    return list(zip(EP_STATE_KEYS, (msg("Not started"), msg("In progress"), msg("Translated"), msg("In proofreading"), msg("Done"))))

YES_WORDS = {"yes", "y", "x", "done", "ok", "1x", "ano", "a", "hotovo"}      # the Czech words stay accepted for people who type them
UNDO_LIMIT = 30

KINDS = ("corr", "assign", "approved", "episodes", "proof", "glossary", "ignored")
GLOSSARY_TYPES = ("speaker", "term")


def text_hash(text):
    """Short fingerprint of a translation; a proofread mark is only valid while it still matches."""
    return hashlib.sha1((text or "").encode("utf-8")).hexdigest()[:12]


def parse_value(value, total):
    """Number / percent / yes-word -> number of lines. Empty or invalid -> None."""
    v = (value or "").strip().lower().replace(",", ".")
    if not v:
        return None
    if v in YES_WORDS:
        return total
    try:
        if v.endswith("%"):
            return max(0, min(total, round(float(v[:-1]) * total / 100)))
        return max(0, min(total, int(float(v))))
    except ValueError:
        return None


def parse_corr(value, total):
    n = parse_value(value, total)
    return 0 if n is None else n


def valid_assign(v):
    if v in ("", "gen"):
        return True
    if isinstance(v, str) and v.startswith("ep:") and v[3:].isdigit():
        return int(v[3:]) in layout.ACTIVE_EPISODES
    return False


def assign_to_text(v):
    if v == "gen":
        return "general"
    if v.startswith("ep:"):
        return "episode " + v[3:]
    return ""


def assign_from_text(t):
    f = fold(t)
    if f in ("general", "gen", "obecne"):
        return "gen"
    digits = "".join(ch for ch in f if ch.isdigit())
    return f"ep:{int(digits)}" if digits else ""


def assign_label(v):
    return {"": msg("Unassigned"), "gen": msg("General (the whole game)")}.get(v) or msg("Episode {n}", n=v[3:])


def ep_of(v):
    """Episode number of an 'ep:N' assignment, else None."""
    return int(v[3:]) if v.startswith("ep:") and v[3:].isdigit() else None


class Store:
    def __init__(self, data_dir, local_dir=None):
        self.dir = Path(data_dir)
        self.backups = Path(local_dir or data_dir) / "backups"
        self.corr = {}       # name -> {"c": str, "t": str}
        self.assign = {}     # name -> '' | 'gen' | 'ep:N'
        self.approved = {}   # name -> {(block, normalized source)}
        self.episodes = {}   # episode number -> state key
        self.proof = {}      # name -> {(block, normalized source): {"s": "ok"|"fix", "h": hash, "n": note, "d": date}}
        self.glossary = {}   # code -> {"n": full name, "t": "speaker"|"term", "c": note}
        self.ignored = {}    # name -> {(block, normalized source, issue code): {"h": translation hash, "n": note, "d": date}}
        self.undo_stack = []
        self.unsaved = set()  # kinds whose last save failed
        self.warnings = []

    # ------------------------------------------------------------ paths
    def path(self, kind):
        return self.dir / {"corr": "proofread.csv", "assign": "assignments.csv",
                           "approved": "approved.csv", "episodes": "episode_states.csv",
                           "proof": "line_marks.csv", "glossary": "glossary.csv",
                           "ignored": "ignored.csv"}[kind]

    # ------------------------------------------------------------ load
    def load(self):
        self.warnings.clear()
        for kind in KINDS:
            try:
                getattr(self, "_load_" + kind)()
            except OSError as e:
                self.warnings.append(msg("{file} cannot be loaded: {reason}", file=self.path(kind).name, reason=e))
        return self

    def _load_corr(self):
        rows = read_rows(self.path("corr"))
        self.corr = {r[0]: {"c": r[1] if len(r) > 1 else "", "t": r[2] if len(r) > 2 else ""}
                     for r in rows[1:]}

    def _load_assign(self):
        self.assign = {}
        for r in read_rows(self.path("assign"))[1:]:
            if len(r) >= 2:
                v = assign_from_text(r[1])
                if v and valid_assign(v):
                    self.assign[r[0]] = v
                elif v:
                    self.warnings.append(msg("assignments.csv: {a} → {b} is not a valid assignment, ignoring it.", a=r[0], b=r[1]))

    def _load_approved(self):
        self.approved = {}
        for r in read_rows(self.path("approved"))[1:]:
            if len(r) >= 3 and r[0]:
                self.approved.setdefault(r[0], set()).add((r[1], norm_ws(r[2])))

    def _load_glossary(self):
        self.glossary = {}
        for r in read_rows(self.path("glossary"))[1:]:
            if r and r[0].strip():
                kind = r[2] if len(r) > 2 and r[2] in GLOSSARY_TYPES else "speaker"
                self.glossary[r[0].strip()] = {"n": r[1].strip() if len(r) > 1 else "", "t": kind,
                                                "c": r[3] if len(r) > 3 else ""}

    def _load_proof(self):
        self.proof = {}
        for r in read_rows(self.path("proof"))[1:]:
            if len(r) >= 5 and r[0] and r[3] in ("ok", "fix"):
                self.proof.setdefault(r[0], {})[(r[1], norm_ws(r[2]))] = {
                    "s": r[3], "h": r[4], "n": r[5] if len(r) > 5 else "", "d": r[6] if len(r) > 6 else ""}

    def _load_ignored(self):
        self.ignored = {}
        for r in read_rows(self.path("ignored"))[1:]:
            if len(r) >= 5 and r[0] and r[3]:
                self.ignored.setdefault(r[0], {})[(r[1], norm_ws(r[2]), r[3])] = {
                    "h": r[4], "n": r[5] if len(r) > 5 else "", "d": r[6] if len(r) > 6 else ""}

    def _load_episodes(self):
        names = {key: key for key in EP_STATE_KEYS}
        self.episodes = {}
        for r in read_rows(self.path("episodes"))[1:]:
            if len(r) < 2:
                continue
            digits = "".join(ch for ch in r[0] if ch.isdigit())
            st = names.get(fold(r[1]))
            if digits and st:
                self.episodes[int(digits)] = st

    # ------------------------------------------------------------ save
    def save(self, kind):
        try:
            getattr(self, "_save_" + kind)()
        except WriteFailed:
            self.unsaved.add(kind)
            raise
        self.unsaved.discard(kind)

    def _save_corr(self):
        data = sorted(self.corr.items(), key=lambda kv: kv[0].lower())
        rows = [["file", "proofread", "translated_manual"]]
        rows += [[n, v.get("c", ""), v.get("t", "")] for n, v in data]
        write_csv(self.path("corr"), rows, self.backups)

    def _save_assign(self):
        data = sorted(((k, v) for k, v in self.assign.items() if v), key=lambda kv: kv[0].lower())
        rows = [["file", "group"]] + [[n, assign_to_text(v)] for n, v in data]
        write_csv(self.path("assign"), rows, self.backups)

    def _save_approved(self):
        data = sorted((f, b, o) for f, keys in self.approved.items() for b, o in keys)
        write_csv(self.path("approved"), [["file", "block", "original"]] + [list(r) for r in data],
                  self.backups)

    def _save_glossary(self):
        rows = [["code", "name", "type", "note"]]
        for code in sorted(self.glossary, key=str.lower):
            g = self.glossary[code]
            rows.append([code, g["n"], g["t"], g["c"]])
        write_csv(self.path("glossary"), rows, self.backups)

    def _save_proof(self):
        rows = [["file", "block", "original", "state", "hash", "note", "date"]]
        for f in sorted(self.proof, key=str.lower):
            for (b, o), m in sorted(self.proof[f].items()):
                rows.append([f, b, o, m["s"], m.get("h", ""), m.get("n", ""), m.get("d", "")])
        write_csv(self.path("proof"), rows, self.backups)

    def _save_ignored(self):
        rows = [["file", "block", "original", "code", "hash", "note", "date"]]
        for f in sorted(self.ignored, key=str.lower):
            for (b, o, code), m in sorted(self.ignored[f].items()):
                rows.append([f, b, o, code, m.get("h", ""), m.get("n", ""), m.get("d", "")])
        write_csv(self.path("ignored"), rows, self.backups)

    def _save_episodes(self):
        rows = [["episode", "state"]]
        rows += [[n, self.episodes.get(n, "none")] for n in layout.ACTIVE_EPISODES]
        write_csv(self.path("episodes"), rows, self.backups)

    def ensure_files(self, names):
        """Add blank correction rows for new files; existing values (also of missing files) stay."""
        new = [n for n in names if n not in self.corr]
        for n in new:
            self.corr[n] = {"c": "", "t": ""}
        head = read_rows(self.path("corr"))[:1] if self.path("corr").exists() else []
        if new or not head or len(head[0]) < 3:
            try:
                self.save("corr")
            except WriteFailed as e:
                self.warnings.append(str(e))

    # ------------------------------------------------------------ undoable change
    def change(self, label, kinds, fn):
        """Run `fn()` (which mutates in-memory state), save `kinds`, and record an undo step.
        If saving fails, memory is rolled back and WriteFailed propagates."""
        before = {k: copy.deepcopy(getattr(self, k)) for k in kinds}
        try:
            result = fn()
            for k in kinds:
                self.save(k)
        except Exception:
            for k, v in before.items():
                setattr(self, k, v)
            raise
        if before != {k: getattr(self, k) for k in kinds}:
            self.undo_stack.append({"label": label, "kinds": tuple(kinds), "before": before})
            del self.undo_stack[:-UNDO_LIMIT]
        return result

    def undo(self):
        """Revert the most recent change. Returns its label, or None when there is nothing to undo."""
        if not self.undo_stack:
            return None
        step = self.undo_stack[-1]
        current = {k: copy.deepcopy(getattr(self, k)) for k in step["kinds"]}
        for k, v in step["before"].items():
            setattr(self, k, copy.deepcopy(v))
        try:
            for k in step["kinds"]:
                self.save(k)
        except WriteFailed:
            for k, v in current.items():
                setattr(self, k, v)
            raise
        self.undo_stack.pop()
        return step["label"]

    def clear_undo(self):
        self.undo_stack.clear()
