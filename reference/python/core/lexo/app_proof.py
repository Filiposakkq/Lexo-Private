"""Proofreading marks on lines."""
from datetime import date
from pathlib import Path

from .messages import msg
from .store import text_hash
from .util import norm_ws
from .app_base import AppError


class ProofPart:
    # ------------------------------------------------------------ proofreading
    def _proof_entry(self, rel, key):
        key = (str(key[0]), norm_ws(str(key[1])))
        e = next((x for x in self.project.entries(rel) if x.key == key), None)
        return key, e

    def proof_set(self, items, state, note=None):
        """Mark lines proofread ('ok'), flagged for fixing ('fix', optional note) or clear the mark ('').
        Only translated lines can be proofread. One undo step. Returns {changed, skipped}."""
        self._need_project()
        if state not in ("ok", "fix", ""):
            raise AppError(msg("Invalid proofreading state."))
        with self.lock:
            plan, skipped = [], []
            for it in items:
                rel = it.get("file")
                if rel not in self.project.files:
                    skipped.append((rel, msg("unknown file")))
                    continue
                key, e = self._proof_entry(rel, it.get("key") or ["", ""])
                if e is None:
                    skipped.append((rel, msg("the line changed in the file – refresh the files")))
                elif state and not self.project.is_translated(rel, e):
                    skipped.append((rel, msg("the line is not translated yet")))
                else:
                    plan.append((rel, key, e))
            changed = 0

            def go():
                nonlocal changed
                today = date.today().isoformat()
                for rel, key, e in plan:
                    marks = self.store.proof.setdefault(rel, {})
                    old = marks.get(key)
                    if not state:
                        if marks.pop(key, None) is not None:
                            changed += 1
                    else:
                        keep = (old or {}).get("n", "")
                        new = {"s": state, "h": text_hash(e.target), "d": today,
                               "n": note if note is not None else keep}
                        if old != new:
                            marks[key] = new
                            changed += 1
                    if not marks:
                        self.store.proof.pop(rel, None)
            label = {"ok": msg("Proofreading: checked"), "fix": msg("Proofreading: to fix"), "": msg("Proofreading: cleared")}[state]
            self.store.change(f"{label} ({len(plan)})", ("proof",), go)
            out = self._changed("proof")
            out.update(changed=changed, skipped=skipped)
            return out

    def proof_upto(self, rel, line, state="ok"):
        """Every line of a file up to `line` (inclusive):
        state "ok"  - mark translated lines that are not yet proofread (or whose proofreading went stale) as proofread;
        state ""    - take the "proofread" mark away again (also stale ones). Lines flagged "to fix" are left alone."""
        self._need_project()
        if state not in ("ok", ""):
            raise AppError(msg("Invalid proofreading state."))
        with self.lock:
            if rel not in self.project.files:
                raise AppError(msg("Unknown file."))
            wanted = ("", "stale") if state == "ok" else ("ok", "stale")
            items = [{"file": rel, "key": list(e.key)} for e in self.project.entries(rel)
                     if e.line <= line and self.project.is_translated(rel, e)
                     and self.project.proof_state(rel, e) in wanted]
            if not items:
                return {"ok": True, "changed": 0, "skipped": [], "problems": []}
            return self.proof_set(items, state)

    def proof_clear_file(self, rel):
        """Remove all per-line proofread marks of a file (one undo step)."""
        self._need_project()
        with self.lock:
            if rel not in self.project.files:
                raise AppError(msg("Unknown file."))
            n = len(self.store.proof.get(rel, {}))
            self.store.change(msg("Proofreading: marks removed in {name}", name=Path(rel).name), ("proof",),
                              lambda: self.store.proof.pop(rel, None))
            out = self._changed("proof")
            out["changed"] = n
            return out

    def proof_next(self, file="", after=0):
        """First translated line (after `after`, in file order) that still needs proofreading, or None."""
        self._need_project()
        files = [file] if file else sorted(self.project.files, key=str.lower)
        for rel in files:
            if rel not in self.project.files:
                raise AppError(msg("Unknown file."))
            for e in self.project.entries(rel):
                if (not file or e.line > after) and self.project.is_translated(rel, e) \
                        and self.project.proof_state(rel, e) in ("", "stale"):
                    return {"found": True, "file": rel, "line": e.line}
        return {"found": False}
