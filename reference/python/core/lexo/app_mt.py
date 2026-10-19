"""Machine translation of lines."""
from dataclasses import replace

from . import layout, fixes, mt, validator
from .messages import msg
from .messages import text_of
from .app_base import AppError


class MtPart:
    def _mt_config(self):
        """The machine translation settings; an empty target language means "the language of the project" (its pack, or the name of its tl folder)."""
        cfg = dict(self.config.get("mt", {}))
        if not cfg.get("target_lang"):
            cfg["target_lang"] = mt.target_language(layout.current().language, self.project.root.name if self.project else "")
        return cfg

    def _mt_issues(self, ent, text):
        """What the validator says about `text` as the translation of `ent` (information is left out)."""
        probe = replace(ent, target=text, category="")
        return [i.to_dict() for i in validator.check_entry(probe, self._length_cfg(), self._terms(), None, self.config.get("validation.textbox", {})) if i.severity != "info"]

    def _mt_one(self, rel, ents, idx, cfg, terms):
        """Machine translation suggestion for ents[idx] (with the two lines before it as context) + what the validator says."""
        ent = ents[idx]
        context = [e.source for e in ents[max(0, idx - 2):idx] if e.source]
        local = {en: cz for en, cz in terms.items() if validator.term_in_text(ent.source, en)}
        try:
            res = mt.translate(cfg, ent.source, local, context, http=self._mt_http)
        except mt.MTError as e:
            raise AppError(text_of(e)) from e
        return {"suggestion": res["text"], "warnings": res["warnings"], "issues": self._mt_issues(ent, res["text"]), "provider": res["provider"]}

    def mt_suggest(self, rel, line):
        """A machine translation SUGGESTION for one line (only after consent and a key in the settings; never saved)."""
        self._need_project()
        ents = self.project.entries(rel) if rel in self.project.files else []
        idx = next((i for i, e in enumerate(ents) if e.line == line and e.source is not None), None)
        if idx is None:
            raise AppError(msg("The line is no longer in the file – refresh the files."))
        return self._mt_one(rel, ents, idx, self._mt_config(), self._terms())

    MT_BATCH = 10            # lines per request: the UI loops, so it can show progress and stop

    def mt_targets(self, file="", lines=None, only_todo=True):
        """Which lines a batch would translate: the given lines, or every line of a file (all files when empty).
        Only not yet translated lines by default. The Pro pack cuts the list to its limit; returns the count and the characters that would be sent."""
        self._need_project()
        cands = []
        if lines:
            want = {(str(x["file"]), int(x["line"])) for x in lines}
            files = sorted({f for f, _l in want})
        else:
            want = None
            files = [file] if file else list(self.project.files)
        for rel in files:
            if rel not in self.project.files:
                raise AppError(msg("Unknown file."))
            for e in self.project.entries(rel):
                if e.source is None or not e.source.strip() or (want is not None and (rel, e.line) not in want):
                    continue
                if only_todo and self._entry_row(rel, e) != "todo":
                    continue
                cands.append({"file": rel, "line": e.line, "chars": len(e.source)})
        return self._ext_call("mt.batch", "targets", {"items": cands})

    def mt_batch(self, items):
        """Suggestions for up to MT_BATCH lines (the Pro pack translates). Nothing is written. A failed line carries its own error; a rejected key,
        missing consent or a broken connection stops the batch (the same message for every line would be useless)."""
        self._need_project()
        self._gate("mt.batch")
        items = list(items or [])[: self.MT_BATCH]
        cfg, terms = self._mt_config(), self._terms()
        plan, reqs = [], []
        for it in items:
            rel, line = str(it["file"]), int(it["line"])
            ents = self.project.entries(rel) if rel in self.project.files else []
            idx = next((i for i, e in enumerate(ents) if e.line == line and e.source is not None), None)
            if idx is None:
                plan.append((rel, line, None, None))
                continue
            ent = ents[idx]
            reqs.append({"id": len(reqs), "source": ent.source, "context": [e.source for e in ents[max(0, idx - 2):idx] if e.source],
                         "terms": {en: cz for en, cz in terms.items() if validator.term_in_text(ent.source, en)}})
            plan.append((rel, line, ents, idx))
        res = self._ext_call("mt.batch", "translate", {"cfg": cfg, "requests": reqs}) if reqs else {"results": [], "stopped": ""}
        got, results, k = {r["id"]: r for r in res["results"]}, [], 0
        for rel, line, ents, idx in plan:
            if res["stopped"] and k >= len(res["results"]):
                break                                           # the batch stopped before this line
            if ents is None:
                results.append({"file": rel, "line": line, "error": msg("The line is no longer in the file.")})
                continue
            r, k = got[k], k + 1
            if "error" in r:
                results.append({"file": rel, "line": line, "error": r["error"]})
                continue
            ent = ents[idx]
            results.append({"file": rel, "line": line, "key": ent.key, "who": ent.who, "source": ent.source, "current": ent.target, "suggestion": r["suggestion"],
                            "warnings": r["warnings"], "issues": self._mt_issues(ent, r["suggestion"]), "provider": r["provider"]})
        return {"results": results, "stopped": res["stopped"]}

    def mt_apply(self, selections):
        """Write the ticked suggestions as one undoable step. Approved lines are never overwritten."""
        self._need_project()
        selections = selections or []
        with self.lock:
            edits, skipped = [], 0
            for x in selections or []:
                rel, line, text = str(x["file"]), int(x["line"]), str(x.get("text", ""))
                ent = next((e for e in self.project.entries(rel) if e.line == line), None) if rel in self.project.files else None
                if ent is None or not text.strip() or self._entry_row(rel, ent) == "approved":
                    skipped += 1
                    continue
                edits.append((rel, line, text))
            if not edits:
                raise AppError(msg("Nothing to write (the lines changed or are approved)."))
            self._snapshot_for_bulk(len(edits), len({e[0] for e in edits}))
            try:
                res = fixes.apply_edits(self.project, self.local_dir, edits, msg("Machine translation ({n} lines)", n=len(edits)))
                self._log_text(res["details"], "mt")
            except fixes.FixError as e:
                raise AppError(text_of(e)) from e
            finally:
                self.project.refresh(force=True)
                self._val_cache.clear()
            self.store.ensure_files([r.name for r in self.project.rows()])
            out = self._changed("mt")
            out.update(changed=res["changed"], files=res["files"], skipped=skipped, backups=res["backups"])
            return out
