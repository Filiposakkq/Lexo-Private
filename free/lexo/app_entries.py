"""Files and lines: listing, detail, saving, context, scenes, text history of a line."""
import bisect
import re
from dataclasses import replace
from pathlib import Path

from . import fixes, rpy, texthistory, validator
from .messages import msg
from .messages import text_of
from .project import STATUS_ORDER
from .project import file_status
from .store import parse_value
from .util import norm_ws
from .app_base import AppError, PAGE_LIMIT


class EntriesPart:
    def files_payload(self):
        self._need_project()
        _, per_file = self.validation_summary()
        out = []
        for r in self.project.rows():
            appr = self.project.counts(r.name)[2]
            st = file_status(r.done, r.todo, r.total, r.corr)
            v = per_file.get(r.name, {"error": 0, "warning": 0, "info": 0})
            p_ok, p_fix = self.project.proof_counts(r.name)
            out.append({"name": r.name, "proof_marked": p_ok, "proof_fix": p_fix, "assign": self.store.assign.get(r.name, ""), "total": r.total,
                        "done": r.done, "auto": r.auto, "todo": r.todo, "corr": r.corr, "approved": appr,
                        "status": st, "order": STATUS_ORDER[st], "errors": v["error"], "warnings": v["warning"],
                        "proof_stale": self.project.proof_stale(r.name)})
        return out

    # ------------------------------------------------------------ entries
    def _entry_row(self, rel, e):
        approved = self.store.approved.get(rel, ())
        if not e.pending:
            status = "translated"
        elif e.key in approved:
            status = "approved"
        else:
            status = "todo"
        return status

    def entries(self, file="", status="all", kind="", q="", issues="", sort="line", offset=0, limit=200, around=0,
                unit=""):
        """Paged, filtered list of translation units (server side, so 45k lines stay fast)."""
        self._need_project()
        limit = max(1, min(int(limit), PAGE_LIMIT))
        ql = q.strip().lower()
        files = [file] if file else sorted(self.project.files, key=str.lower)
        if file and file not in self.project.files:
            raise AppError(msg("Unknown file."))
        iss_by = {}
        if self.config.get("validation.enabled", True):
            for rel in files:
                for line, iss, _e in self._issues(rel):
                    iss_by.setdefault((rel, line), []).append(iss)
        rows = []
        counts = {k: 0 for k in ("all", "todo", "approved", "translated", "review", "proofed", "fix", "stale")}
        for rel in files:
            for e in self.project.entries(rel):
                st = self._entry_row(rel, e)
                pf = self.project.proof_state(rel, e) if st != "todo" else ""
                member = {"all": True, "todo": st == "todo", "approved": st == "approved", "translated": st == "translated",
                          "review": st != "todo" and pf not in ("ok", "fix"), "proofed": pf == "ok", "fix": pf == "fix",
                          "stale": pf == "stale"}
                kinds = e.category
                if kind and kinds != kind:
                    continue
                if unit and e.unit != unit:
                    continue
                iss = iss_by.get((rel, e.line), [])
                shown = [i for i in iss if i.severity != "info" or self.config.get("validation.show_info", False)]
                if issues == "any" and not shown:
                    continue
                if issues == "error" and not any(i.severity == "error" for i in shown):
                    continue
                if issues == "warning" and not any(i.severity == "warning" for i in shown):
                    continue
                if issues == "overflow" and not any(i.code == "textbox-overflow" for i in iss):
                    continue
                if ql and ql not in (e.target or "").lower() and ql not in (e.source or "").lower() \
                        and ql not in e.who.lower() and ql not in rel.lower() \
                        and ql not in self.store.glossary.get(self.speaker_code(e.who), {}).get("n", "").lower():
                    continue
                for k, v in member.items():            # counts for the status pills: every filter except the status itself
                    if v:
                        counts[k] += 1
                if status in member and not member[status]:
                    continue
                rows.append((rel, e, st, shown, pf))
        if sort == "kind":
            rows.sort(key=lambda r: (rpy.KIND_RANK.get(r[1].category, 9), r[0].lower(), r[1].line))
        elif sort == "issues":
            rows.sort(key=lambda r: (-sum(3 if i.severity == "error" else 1 for i in r[3]), r[0].lower(), r[1].line))
        elif sort == "file":
            rows.sort(key=lambda r: (r[0].lower(), r[1].line))
        total = len(rows)
        if around:
            idx = next((i for i, r in enumerate(rows) if r[1].line == around and (not file or r[0] == file)), None)
            if idx is not None:
                offset = max(0, idx - 30)
        page = rows[offset:offset + limit]
        return {"total": total, "offset": offset, "counts": counts, "items": [{
            "file": rel, "line": e.line, "src_line": e.src_line, "block": e.block, "who": e.who,
            "source": e.source, "target": e.target, "status": st,
            "kind": e.category, "proof": pf, "unit": e.unit, "ref": e.ref,
            "issues": [{"severity": i.severity, "code": i.code} for i in shown],
        } for rel, e, st, shown, pf in page]}

    def entry_detail(self, rel, line):
        self._need_project()
        if rel not in self.project.files:
            raise AppError(msg("Unknown file."))
        ents = self.project.entries(rel)
        idx = next((i for i, e in enumerate(ents) if e.line == line), None)
        if idx is None:
            raise AppError(msg("The line is no longer in the file – refresh the files."))
        e = ents[idx]
        on = self.config.get("validation.enabled", True)
        iss = [i for ln, i, _e in self._issues(rel) if ln == line] if on else []
        ign = [(i, self._ignored_mark(rel, e, i)) for ln, i, _e in self._issues(rel, True) if ln == line] if on else []

        def brief(x):
            return {"line": x.line, "who": x.who, "source": x.source, "target": x.target,
                    "status": self._entry_row(rel, x),
                    "proof": self.project.proof_state(rel, x) if self.project.is_translated(rel, x) else ""}
        return {"file": rel, "path": str(self.project.root / rel), "line": e.line, "src_line": e.src_line,
                "block": e.block, "who": e.who, "unit": e.unit, "ref": e.ref, "source": e.source, "target": e.target,
                "src_raw": e.src_raw, "cur_raw": e.cur_raw,
                "status": self._entry_row(rel, e), "kind": e.category,
                "proof": self.project.proof_state(rel, e) if self.project.is_translated(rel, e) else "",
                "proof_note": self.store.proof.get(rel, {}).get(e.key, {}).get("n", ""),
                "proof_date": self.store.proof.get(rel, {}).get(e.key, {}).get("d", ""),
                "key": [e.block, norm_ws(e.source if e.source is not None else e.target)],
                "terms": [{"en": en, "cz": cz, "used": validator.term_used(e.target, cz)} for en, cz in self._terms().items()
                          if e.source is not None and validator.term_in_text(e.source, en)],
                "issues": [dict(i.to_dict(), ignored=False) for i in iss]
                          + [dict(i.to_dict(), ignored=True, ignore_note=(m or {}).get("n", "")) for i, m in ign],
                "before": [brief(x) for x in ents[max(0, idx - 2):idx]],
                "after": [brief(x) for x in ents[idx + 1:idx + 3]]}

    # ------------------------------------------------------------ mutations
    def _changed(self, label):
        """After a store change: derived files + a result the UI can show."""
        problems = self.persist()
        return {"ok": True, "problems": problems, "label": label}

    def save_file(self, name, c, t):
        self._need_project()
        with self.lock:
            if name not in self.project.files:
                raise AppError(msg("Unknown file."))
            row = self.project.row(name)
            total, auto = row.total, row.auto
            c_n, t_n = parse_value(c, total), parse_value(t, total)
            if t_n is not None and t_n == auto:
                t_n = None
            # the number shown = hand-typed baseline + lines marked one by one; keep the marks, store the rest
            c_n = max(0, c_n - self.project.proof_counts(name)[0]) if c_n else c_n

            def go():
                self.store.corr[name] = {"c": "" if not c_n else str(c_n), "t": "" if t_n is None else str(t_n)}
            self.store.change(msg("Editing the counts in {name}", name=Path(name).name), ("corr",), go)
            return self._changed("save")

    def entry_context(self, rel, line, radius=6):
        """The original game script around the line this translation unit comes from (read-only)."""
        self._need_project()
        if rel not in self.project.files:
            raise AppError(msg("Unknown file."))
        e = next((x for x in self.project.entries(rel) if x.line == line), None)
        if e is None:
            raise AppError(msg("The line is no longer in the file – refresh the files."))
        m = re.match(r"^(game/.+?):(\d+)$", e.ref or "")
        if not m:
            return {"available": False, "reason": msg("The line has no recorded position in the game script.")}
        base = self._game_root()
        if base is None:
            return {"available": False, "ref": e.ref, "reason": msg("The game folder (game) above the translation was not found.")}
        path = (base / m.group(1)).resolve()
        if base.resolve() not in path.parents or path.suffix.lower() not in (".rpy", ".rpym"):
            return {"available": False, "ref": e.ref, "reason": msg("Invalid script path.")}
        try:
            if path.stat().st_size > 8 * 1024 * 1024:
                return {"available": False, "ref": e.ref, "reason": msg("The script is too large.")}
            lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        except OSError:
            return {"available": False, "ref": e.ref,
                    "reason": msg("The script {name} was not found (the original .rpy files of the game are not at hand, maybe only .rpyc).", name=m.group(1))}
        n = int(m.group(2))
        a, b = max(1, n - radius), min(len(lines), n + radius)
        return {"available": True, "ref": e.ref, "path": str(path),
                "lines": [{"n": i, "text": lines[i - 1], "hit": i == n} for i in range(a, b + 1)]}

    def _overflows(self, e):
        """True when the translation needs more room than the game's dialogue box has (and the original did not)."""
        if not e.source or not e.target or e.unit == "string":
            return False
        return any(i.code == "textbox-overflow" for i in validator.check_textbox(e.source, e.target, self.config.get("validation.textbox", {})))

    def entry_next(self, rel, line, kind):
        """The next line (after `line`, wrapping round the file) that is untranslated (todo), still to be checked (review), marked
        for a fix (fix) or does not fit the dialogue box (over). {line: 0} when there is none."""
        self._need_project()
        if rel not in self.project.files:
            raise AppError(msg("Unknown file."))
        if kind not in ("todo", "review", "fix", "over"):
            raise AppError(msg("Unknown kind of line."))
        ents = self.project.entries(rel)

        def match(e):
            st = self._entry_row(rel, e)
            if kind == "todo":
                return st == "todo"
            if st == "todo":
                return False
            if kind == "over":
                return self._overflows(e)
            pf = self.project.proof_state(rel, e)
            return pf not in ("ok", "fix") if kind == "review" else pf == "fix"
        idx = next((i for i, e in enumerate(ents) if e.line == line), -1)
        for i in list(range(idx + 1, len(ents))) + list(range(0, idx + 1)):
            if match(ents[i]):
                return {"line": ents[i].line, "wrapped": i <= idx}
        return {"line": 0, "wrapped": False}

    def scene_list(self, rel, line=0):
        """The scenes of a file (runs of lines that follow the same `scene` statement of the game script) with their progress, and
        the one that holds `line`. Needs the game sources; without them {available: False}."""
        self._need_project()
        if rel not in self.project.files:
            raise AppError(msg("Unknown file."))
        gi, base = self._game_images(), self._game_root()
        if gi is None or base is None:
            return {"available": False, "items": [], "cur": -1, "reason": msg("The game folder (game) above the translation was not found.")}
        root = base.resolve()
        items, key, cur, starts = [], None, -1, {}
        for e in self.project.entries(rel):
            m = re.match(r"^(game/.+?):(\d+)$", e.ref or "")
            name = None
            if m:
                script = (base / m.group(1)).resolve()
                if root in script.parents and script.suffix.lower() in (".rpy", ".rpym"):
                    if script not in starts:
                        st = gi.scene_starts(script)
                        starts[script] = (st, [s[0] for s in st])
                    st, nums = starts[script]
                    pos = bisect.bisect_right(nums, int(m.group(2))) - 1
                    k = (m.group(1), st[pos][0] if pos >= 0 else 0)
                    name = st[pos][1] if pos >= 0 else ""
                else:
                    k = key
            else:
                k = key
            if not items or k != key:
                items.append({"line": e.line, "count": 0, "done": 0, "proofed": 0, "name": name or "", "script": f"{k[0]}:{k[1]}" if k else ""})
                key = k
            it = items[-1]
            it["count"] += 1
            if self._entry_row(rel, e) != "todo":
                it["done"] += 1
                if self.project.proof_state(rel, e) == "ok":
                    it["proofed"] += 1
            if e.line <= line:
                cur = len(items) - 1
        return {"available": True, "items": items, "cur": cur}

    def entry_scene(self, rel, line, radius=6):
        """The lines around one line (original and translation), i.e. the scene it belongs to."""
        self._need_project()
        if rel not in self.project.files:
            raise AppError(msg("Unknown file."))
        ents = self.project.entries(rel)
        idx = next((i for i, e in enumerate(ents) if e.line == line), None)
        if idx is None:
            raise AppError(msg("The line is no longer in the file – refresh the files."))
        r = max(1, min(int(radius), 30))
        a, b = max(0, idx - r), min(len(ents), idx + r + 1)
        return {"file": rel, "items": [{"line": e.line, "who": e.who, "source": e.source, "target": e.target,
                                       "status": self._entry_row(rel, e), "current": e.line == line, "unit": e.unit,
                                       "over": self._overflows(e),
                                       "proof": self.project.proof_state(rel, e) if self.project.is_translated(rel, e) else ""}
                                      for e in ents[a:b]]}

    def entry_check(self, rel, line, text):
        """Validator result for a text the translator is typing (nothing is written)."""
        self._need_project()
        with self.lock:
            ent = next((e for e in self.project.entries(rel) if e.line == line), None) if rel in self.project.files else None
            if ent is None:
                raise AppError(msg("The line is no longer in the file – refresh the files."))
            try:
                fixes.check_literal(text)
                literal = ""
            except fixes.FixError as e:
                literal = str(e)
            probe = replace(ent, target=text, category="")
            issues = [i.to_dict() for i in validator.check_entry(
                probe, self._length_cfg(), self._terms(), self._spell(), self.config.get("validation.textbox", {}))
                if i.severity != "info" or self.config.get("validation.show_info", False)]
            return {"literal_error": literal, "issues": issues}

    def entry_save(self, rel, line, text):
        """Write a new translation of one line into the .rpy file (backup, atomic, verified, undoable)."""
        self._need_project()
        with self.lock:
            try:
                res = fixes.edit_translation(self.project, self.local_dir, rel, int(line), str(text))
            except fixes.FixError as e:
                raise AppError(text_of(e)) from e
            finally:
                self.project.refresh(force=True)
                self._val_cache.clear()
            self.store.ensure_files([r.name for r in self.project.rows()])
            self._log_text(res.get("details", []), "edit")
            out = self._changed("edit")
            out.update({k: v for k, v in res.items() if k != "details"})
            return out

    # ------------------------------------------------------------ history of the text of a line
    def _log_text(self, details, via):
        """Record changed texts (from apply_edits) in the line history. Never lets a logging problem break an edit."""
        try:
            self.texthist.record([{"file": d["file"], "key": d["key"], "old": d["old"], "new": d["new"]} for d in details], via)
        except Exception:                                  # noqa: BLE001 - history is a convenience, not part of the edit
            pass

    def _log_external(self, prev):
        """Changes noticed at a refresh that this app did not make itself (VS Code, another program)."""
        for rel, old in (prev or {}).items():
            fi = self.project.files.get(rel)
            if fi is None:
                continue
            ch = texthistory.diff_files(old.parsed.entries, fi.parsed.entries)
            try:
                self.texthist.record([{"file": rel, "key": c["key"], "old": c["old"], "new": c["new"]} for c in ch], "external")
            except Exception:                              # noqa: BLE001
                pass

    def entry_history(self, rel, line):
        """All texts this line has had, newest first (see texthistory)."""
        self._need_project()
        ent = next((e for e in self.project.entries(rel) if e.line == line), None) if rel in self.project.files else None
        if ent is None:
            raise AppError(msg("The line is no longer in the file – refresh the files."))
        versions = self.texthist.versions(rel, ent.key, ent.target)
        return {"versions": versions, "count": max(0, len(versions) - 1)}
