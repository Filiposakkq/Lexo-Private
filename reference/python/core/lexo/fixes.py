"""The only place where the manager changes a translation .rpy file.

Operations: editing the translated text of one line in the app (edit_translation), writing many edits as one undoable step (apply_edits), and writing
planned fixes of the speaker / attributes of dialogue lines (apply). What to fix is planned elsewhere (the safe fixes are a Pro feature); this module only writes.

Safety: a plan is built first (nothing written); applying re-reads the file and refuses when it changed
on disk since it was parsed; a backup of the file is always made; the write is atomic; the file is
parsed again afterwards and the result is verified.
"""
import hashlib
import json
import re
from datetime import datetime
from pathlib import Path

from . import rpy
from .util import WriteFailed, atomic_write_bytes, make_backup
from .messages import msg

SPLIT_RE = re.compile(r"(\r\n|\r|\n)")
BOM = b"\xef\xbb\xbf"


LAST_FILE = "last_fix.json"


class FixError(Exception):
    pass


def apply(project, data_dir, planned):
    """Write the planned fixes. Returns {fixed, skipped: [(file, line, reason)], backups}."""
    by_file = {}
    skipped = []
    for p in planned:
        if p["skip"]:
            skipped.append((p["file"], p["line"], p["skip"]))
        else:
            by_file.setdefault(p["file"], []).append(p)
    fixed, backups, record = 0, [], []
    for rel, items in by_file.items():
        full = project.path_of(rel)
        fi = project.files.get(rel)
        try:
            data = full.read_bytes()
        except OSError as e:
            raise FixError(msg("{rel}: the file cannot be read ({reason}).", rel=rel, reason=e.strerror or e)) from e
        if fi is None or rpy.parse_bytes(data).digest != fi.parsed.digest:
            raise FixError(msg("{rel}: the file changed on the disk since it was loaded. Refresh the files and try again.", rel=rel))
        had_bom = data.startswith(BOM)
        text = data[len(BOM):].decode("utf-8", errors="strict") if had_bom else data.decode("utf-8", errors="strict")
        parts = SPLIT_RE.split(text)       # text, eol, text, eol ... — line N is parts[2 * (N - 1)]
        for p in items:
            idx = 2 * (p["line"] - 1)
            if idx >= len(parts) or parts[idx].strip() != p["before"]:
                raise FixError(msg("{rel}:{line}: the line no longer matches the loaded state. Refresh the files.", rel=rel, line=p['line']))
            lead = parts[idx][:len(parts[idx]) - len(parts[idx].lstrip())]
            parts[idx] = lead + p["after"].lstrip() + parts[idx][len(parts[idx].rstrip()):]
        new_bytes = ("".join(parts)).encode("utf-8")
        if had_bom:
            new_bytes = BOM + new_bytes
        backup = make_backup(full, data_dir / "backups", min_interval=0, keep=50)
        if backup:
            backups.append(str(backup))
        try:
            atomic_write_bytes(full, new_bytes)
        except WriteFailed as e:
            raise FixError(msg("Cannot write: {reason} ({name})", reason=e.reason, name=full.name)) from e
        check = rpy.parse_bytes(full.read_bytes())
        old_entries = fi.parsed.entries
        if len(check.entries) != len(old_entries) or any(
                a.target != b.target for a, b in zip(old_entries, check.entries)):
            atomic_write_bytes(full, data)
            raise FixError(msg("{rel}: the check after writing failed, the file was restored to its previous state.", rel=rel))
        fixed += len(items)
        if backup:
            record.append({"file": rel, "backup": str(backup), "after": hashlib.sha1(new_bytes).hexdigest(),
                           "lines": len(items)})
    if record:
        _push(data_dir, {"time": datetime.now().isoformat(timespec="seconds"), "label": msg("Fix of speakers from the original"),
                         "files": record})
    return {"fixed": fixed, "skipped": skipped, "backups": backups}


STACK_MAX = 30


def _stack(data_dir):
    try:
        data = json.loads((data_dir / LAST_FILE).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return []
    if isinstance(data, dict):                  # older format: a single record
        data = [data]
    return [r for r in data if isinstance(r, dict) and r.get("files")]


def _push(data_dir, rec):
    stack = (_stack(data_dir) + [rec])[-STACK_MAX:]
    atomic_write_bytes(data_dir / LAST_FILE, json.dumps(stack, ensure_ascii=False, indent=1).encode("utf-8"))


def last(data_dir):
    """The newest change that can still be undone: {time, label, files, lines} or None."""
    stack = _stack(data_dir)
    if not stack:
        return None
    rec = dict(stack[-1])
    rec["lines"] = sum(f.get("lines", 0) for f in rec["files"])
    rec["depth"] = len(stack)
    return rec


def undo(project, data_dir):
    """Put the files of the newest change back from their backups. Refuses when a file was edited since
    (the backup would silently throw those edits away). Returns {restored: [rel...], lines, label}."""
    stack = _stack(data_dir)
    if not stack:
        raise FixError(msg("Nothing to undo."))
    rec = stack[-1]
    checked = []
    for f in rec["files"]:
        full = project.path_of(f["file"])
        bak = Path(f["backup"])
        try:
            now = hashlib.sha1(full.read_bytes()).hexdigest()
            before = bak.read_bytes()
        except OSError as e:
            raise FixError(msg("{file}: the file or the backup cannot be read ({reason}).", file=f['file'], reason=e.strerror or e)) from e
        if now != f["after"]:
            raise FixError(msg("{file}: the file changed elsewhere since the last change (e.g. in the editor). Undoing would delete later edits – the backup is in {folder}.", file=f['file'], folder=bak.parent))
        checked.append((full, before, f["file"]))
    for full, before, _rel in checked:
        try:
            atomic_write_bytes(full, before)
        except WriteFailed as e:
            raise FixError(msg("Cannot write: {reason} ({name})", reason=e.reason, name=full.name)) from e
    rest = stack[:-1]
    try:
        if rest:
            atomic_write_bytes(data_dir / LAST_FILE, json.dumps(rest, ensure_ascii=False, indent=1).encode("utf-8"))
        else:
            (data_dir / LAST_FILE).unlink()
    except OSError:
        pass
    return {"restored": [c[2] for c in checked], "lines": sum(f.get("lines", 0) for f in rec["files"]),
            "label": rec.get("label", "")}


# ---------------------------------------------------------------- editing one translated text

def check_literal(text):
    """The raw text that goes between the quotes of a Ren'Py string must stay one valid string literal."""
    if "\n" in text or "\r" in text:
        raise FixError(msg("The text must not contain a real line break. Write a new line as \n."))
    i = 0
    while i < len(text):
        ch = text[i]
        if ch == "\\":
            if i + 1 >= len(text):
                raise FixError(msg("The text must not end with a lone backslash “\\” (write \\ or \n)."))
            i += 2
            continue
        if ch == '"':
            raise FixError(msg("Write a quote in the text as \\\" – a bare quote would end the string."))
        i += 1


def _stage_file(project, rel, changes):
    """Prepare the new bytes of one file for {line: new_text}. Nothing is written. Returns a dict."""
    fi = project.files.get(rel)
    if fi is None:
        raise FixError(msg("{rel}: unknown file.", rel=rel))
    ents = {e.line: e for e in project.entries(rel)}
    full = project.path_of(rel)
    try:
        data = full.read_bytes()
    except OSError as e:
        raise FixError(msg("{rel}: the file cannot be read ({reason}).", rel=rel, reason=e.strerror or e)) from e
    if rpy.parse_bytes(data).digest != fi.parsed.digest:
        raise FixError(msg("{rel}: the file changed on the disk since it was loaded (maybe in the editor). Refresh the files and try again.", rel=rel))
    had_bom = data.startswith(BOM)
    try:
        text = (data[len(BOM):] if had_bom else data).decode("utf-8")
    except UnicodeDecodeError as e:
        raise FixError(msg("{rel}: the file is not valid UTF-8, I rather do not edit it.", rel=rel)) from e
    parts = SPLIT_RE.split(text)
    for line, new_text in changes.items():
        ent = ents.get(line)
        if ent is None:
            raise FixError(msg("{rel}:{line}: the line is no longer in the file – refresh the files.", rel=rel, line=line))
        check_literal(new_text)
        idx = 2 * (line - 1)
        if idx >= len(parts) or parts[idx].strip() != ent.cur_raw:
            raise FixError(msg("{rel}:{line}: the line no longer matches the loaded state. Refresh the files.", rel=rel, line=line))
        raw = parts[idx]
        m = rpy.STR_RE.search(raw)
        if not m:
            raise FixError(msg("{rel}:{line}: there is no text in quotes on the line.", rel=rel, line=line))
        parts[idx] = raw[:m.start()] + '"' + new_text + '"' + raw[m.end():]
    new_bytes = "".join(parts).encode("utf-8")
    if had_bom:
        new_bytes = BOM + new_bytes
    return {"rel": rel, "full": full, "data": data, "new": new_bytes, "changes": changes, "old": fi.parsed.entries}


def _verify(staged, written):
    """The file on disk must differ from the old one in exactly the planned lines and nowhere else."""
    old, changes = staged["old"], staged["changes"]
    if len(written.entries) != len(old):
        return False
    for a, b in zip(old, written.entries):
        want = changes.get(a.line, a.target)
        if b.line != a.line or b.target != want or b.who != a.who or b.suffix != a.suffix:
            return False
    return True


def apply_edits(project, data_dir, edits, label):
    """Replace the translated text of several lines (possibly in several files) as ONE undoable step.

    edits = [(rel, line, new_text)]. Everything else in the files stays byte for byte as it was. Every file is
    backed up, written atomically and verified; if anything fails the files already written are restored.
    Returns {changed, files, backups}."""
    by_file = {}
    for rel, line, new_text in edits:
        ent = next((e for e in project.entries(rel) if e.line == line), None) if rel in project.files else None
        if ent is not None and ent.target == new_text:
            continue                                         # nothing to do for this line
        by_file.setdefault(rel, {})[int(line)] = new_text
    if not by_file:
        return {"changed": 0, "files": 0, "backups": [], "details": []}
    staged = [_stage_file(project, rel, ch) for rel, ch in sorted(by_file.items())]   # validates everything first
    written, record, backups = [], [], []
    try:
        for st in staged:
            backup = make_backup(st["full"], data_dir / "backups", min_interval=0, keep=50)
            atomic_write_bytes(st["full"], st["new"])
            written.append(st)
            if not _verify(st, rpy.parse_bytes(st["full"].read_bytes())):
                raise FixError(msg("{rel}: the check after writing failed, the changes were restored to their previous state.", rel=st['rel']))
            if backup:
                backups.append(str(backup))
                record.append({"file": st["rel"], "backup": str(backup), "after": hashlib.sha1(st["new"]).hexdigest(),
                               "lines": len(st["changes"])})
    except (FixError, WriteFailed, OSError) as e:
        for st in written:                                   # roll back what was already written
            try:
                atomic_write_bytes(st["full"], st["data"])
            except (WriteFailed, OSError):
                pass
        if isinstance(e, WriteFailed):
            raise FixError(msg("Cannot write: {reason} ({name})", reason=e.reason, name=e.path.name)) from e
        if isinstance(e, FixError):
            raise
        raise FixError(msg("The write failed: {reason}", reason=e)) from e
    if record:
        _push(data_dir, {"time": datetime.now().isoformat(timespec="seconds"), "label": label, "files": record})
    details = [{"file": st["rel"], "line": e.line, "key": e.key, "old": e.target, "new": st["changes"][e.line]}
               for st in staged for e in st["old"] if e.line in st["changes"]]
    return {"changed": sum(len(st["changes"]) for st in staged), "files": len(staged), "backups": backups, "details": details}


def edit_translation(project, data_dir, rel, line, new_text):
    """Replace the translated text of one unit. Returns {changed, backup}."""
    ent = next((e for e in project.entries(rel) if e.line == line), None)
    if ent is None:
        raise FixError(msg("The line is no longer in the file – refresh the files."))
    check_literal(new_text)
    if new_text == ent.target:
        return {"changed": False}
    res = apply_edits(project, data_dir, [(rel, line, new_text)], msg("Editing line {line} in {name}", line=line, name=Path(rel).name))
    return {"changed": True, "backup": res["backups"][0] if res["backups"] else "", "details": res["details"]}
