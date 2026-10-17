"""Safe fixes, replace in files, undo of the last file change."""

from . import fixes, textedit, texthistory
from .messages import msg
from .messages import text_of
from .app_base import AppError


class FixesPart:
    FIX_KINDS = ("speaker", "spaces", "typo")
    FIX_LABELS = {"speaker": msg("speaker / attributes"), "spaces": msg("double spaces"), "typo": msg("typos")}

    def fix_preview(self, items=None, file="", kinds=None):
        """Every safe fix that could be applied. Nothing is written.

        kinds: "speaker" (copy speaker / attributes / trailing code from the original), "spaces" (collapse double spaces),
        "typo" (apply the spelling hints). Variables and tags are never touched – their order may differ legitimately.
        Each item has `default` (the checkbox state a UI should start with; doubtful ones are off) and `skip` (reason when
        it cannot be applied)."""
        self._need_project()
        kinds = [k for k in (kinds or ["speaker"]) if k in self.FIX_KINDS]
        wanted = {(str(i["file"]), int(i["line"])) for i in items} if items else None
        with self.lock:
            out = []
            if "speaker" in kinds:
                for p in fixes.plan(self.project, items, file):
                    out.append({"file": p["file"], "line": p["line"], "kind": "speaker", "before": p["before"],
                                "after": p["after"], "skip": p["skip"], "default": not p["skip"], "code": True,
                                "note": msg("speaker / attributes from the original")})
            for p in textedit.plan_text_fixes(self.project, self._spell(), kinds, wanted, file):
                out.append(dict(p, skip="", code=False))
            out.sort(key=lambda r: (r["file"].lower(), r["line"], r["kind"]))
            counts = {k: sum(1 for r in out if r["kind"] == k) for k in kinds}
            full = len(out)
            out, cut = self._cap("fixes.safe", out)
            return {**cut, "items": out, "total": full, "fixable": sum(1 for r in out if not r["skip"]), "counts": counts}

    def fix_apply(self, items=None, file="", kinds=None, selections=None):
        """Apply fixes. `selections` = [{file, line, kind}] picks exactly what the user ticked in the preview;
        without it every applicable fix of `kinds` (default: speaker) is applied (older callers)."""
        self._need_project()
        with self.lock:
            if selections is not None:
                selections, _cut = self._cap("fixes.safe", selections)
                chosen = {(str(x["file"]), int(x["line"]), str(x["kind"])) for x in selections}
                kinds = sorted({k for _f, _l, k in chosen})
                items = [{"file": f, "line": l} for f, l, _k in chosen] or None
                if not chosen:
                    raise AppError(msg("Nothing is selected to fix."))
            else:
                chosen = None
            plan = self.fix_preview(items, file, kinds)["items"]
            todo = [p for p in plan if not p["skip"] and (chosen is None or (p["file"], p["line"], p["kind"]) in chosen)]
            skipped = [(p["file"], p["line"], p["skip"]) for p in plan if p["skip"]]
            if not todo:
                raise AppError(msg("Nothing to fix."))
            fixed, backups = 0, []
            self._snapshot_for_bulk(len(todo), len({p["file"] for p in todo}))
            try:
                speaker = {(p["file"], p["line"]) for p in todo if p["kind"] == "speaker"}
                if speaker:
                    res = fixes.apply(self.project, self.local_dir, [p for p in fixes.plan(self.project, None, "")
                                                                    if (p["file"], p["line"]) in speaker and not p["skip"]])
                    fixed += res["fixed"]
                    backups += res["backups"]
                    skipped += res["skipped"]
                    self.project.refresh(force=True)
                    self._val_cache.clear()
                texts = {(p["file"], p["line"], p["kind"]) for p in todo if p["kind"] != "speaker"}
                if texts:
                    by_line = {}
                    for f, l, k in texts:
                        by_line.setdefault((f, l), set()).add(k)
                    pick = {}
                    for (f, l), ks in by_line.items():                       # one line can carry several kinds: chain them
                        e = next((x for x in self.project.entries(f) if x.line == l), None)
                        if e is not None and e.source is not None:
                            pick[(f, l)] = textedit.chain_fixes(self._spell(), e.source, e.target, ks)
                    kinds_txt = ", ".join(self.FIX_LABELS[k] for k in sorted({k for _f, _l, k in texts}))
                    res = fixes.apply_edits(self.project, self.local_dir, [(f, l, t) for (f, l), t in pick.items()],
                                            f"Opravy textu ({kinds_txt})")
                    self._log_text(res["details"], "fix")
                    fixed += res["changed"]
                    backups += res["backups"]
            except fixes.FixError as e:
                raise AppError(text_of(e)) from e
            finally:
                self.project.refresh(force=True)
                self._val_cache.clear()
            self.store.ensure_files([r.name for r in self.project.rows()])
            out = self._changed("fix")
            out.update(fixed=fixed, skipped=skipped, backups=backups)
            return out

    # ------------------------------------------------------------ find and replace in translations
    def _replace_plan(self, find, replace, mode, case, files, protect, limit):
        rep = None
        if mode != "regex":
            try:
                rep = textedit.make_replacer(str(find), str(replace), mode, bool(case))
            except ValueError as e:
                raise AppError(text_of(e)) from e
        files = [str(f) for f in (files or [])]
        unknown = [f for f in files if f not in self.project.files]
        if unknown:
            raise AppError(msg("Unknown file: {name}", name=unknown[0]))
        candidates = [(rel, e) for rel in sorted(files or self.project.files, key=str.lower) for e in self.project.entries(rel)
                      if e.target and not (e.pending and e.category and not self.project.is_translated(rel, e))]
        if rep is None:                                             # regex: the Pro pack does the substitution on the plain texts
            res = self._ext_call("replace.regex", "substitute", {"pattern": str(find), "replace": str(replace), "case": bool(case), "protect": bool(protect),
                                                                   "texts": [e.target for _r, e in candidates]})["results"]
            done = iter((r["new"], r["n"]) for r in res)
        else:
            done = None
        items, matches, truncated = [], 0, False
        for rel, e in candidates:
            if done is not None:
                new, n = next(done)
            else:
                try:
                    new, n = rep(e.target, bool(protect))
                except ValueError as err:
                    raise AppError(str(err)) from err
            if not n or new == e.target:
                continue
            skip = ""
            try:
                fixes.check_literal(new)
            except fixes.FixError as err:
                skip = str(err)
            if len(items) >= limit:
                truncated = True
                break
            matches += n
            items.append({"file": rel, "line": e.line, "who": e.who, "source": e.source, "before": e.target,
                          "after": new, "count": n, "skip": skip})
        return items, matches, truncated

    def replace_preview(self, find, replace, mode="plain", case=False, files=None, protect=True, limit=2000):
        """Lines of already translated text that the search would change, with the new text. Nothing is written."""
        self._need_project()
        if mode == "regex":
            self._gate("replace.regex")
        with self.lock:
            items, matches, truncated = self._replace_plan(find, replace, mode, case, files, protect, max(1, min(int(limit), 20000)))
            return {"items": items, "total": len(items), "matches": matches, "truncated": truncated,
                    "applicable": sum(1 for i in items if not i["skip"])}

    def replace_apply(self, selections, find, replace, mode="plain", case=False, files=None, protect=True):
        """Write the replacement into the ticked lines (one undoable step, backup of every touched file)."""
        self._need_project()
        if mode == "regex":
            self._gate("replace.regex")
        with self.lock:
            chosen = {(str(x["file"]), int(x["line"])) for x in (selections or [])}
            if not chosen:
                raise AppError(msg("No line is selected."))
            items, _m, _t = self._replace_plan(find, replace, mode, case, files, protect, 20000)
            edits = [(i["file"], i["line"], i["after"]) for i in items if not i["skip"] and (i["file"], i["line"]) in chosen]
            if not edits:
                raise AppError(msg("The selected lines changed in the meantime – show the preview again."))
            label = msg("Replacing “{a}” → “{b}”", a=str(find)[:30], b=str(replace)[:30])
            self._snapshot_for_bulk(len(edits), len({e[0] for e in edits}))
            try:
                res = fixes.apply_edits(self.project, self.local_dir, edits, label)
                self._log_text(res["details"], "replace")
            except fixes.FixError as e:
                raise AppError(text_of(e)) from e
            finally:
                self.project.refresh(force=True)
                self._val_cache.clear()
            self.store.ensure_files([r.name for r in self.project.rows()])
            out = self._changed("replace")
            out.update(changed=res["changed"], files=res["files"], backups=res["backups"])
            return out

    def fix_last(self):
        self._need_project()
        return {"last": fixes.last(self.local_dir)}

    def fix_undo(self):
        """Restore the files changed by the last Opravit from their backups."""
        self._need_project()
        with self.lock:
            before = {rel: list(self.project.entries(rel)) for rel in self.project.files}
            try:
                res = fixes.undo(self.project, self.local_dir)
            except fixes.FixError as e:
                raise AppError(text_of(e)) from e
            finally:
                self.project.refresh(force=True)
                self._val_cache.clear()
            for rel in res.get("restored", []):
                if rel in before and rel in self.project.files:
                    ch = texthistory.diff_files(before[rel], self.project.entries(rel))
                    self._log_text([{"file": rel, "key": c["key"], "old": c["old"], "new": c["new"]} for c in ch], "undo")
            out = self._changed("fix-undo")
            out.update(res)
            return out
