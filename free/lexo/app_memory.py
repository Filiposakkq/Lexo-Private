"""Translation memory and consistency of repeated originals."""
from . import fixes, memory
from .messages import msg
from .messages import text_of
from .app_base import AppError


class MemoryPart:
    # ------------------------------------------------------------ fixing structure errors in the .rpy files
    # ------------------------------------------------------------ translation memory / consistency / game window
    def _memory(self):
        """Index of everything already translated (looked up by the English original). Rebuilt when a file changes."""
        key = tuple(sorted((rel, id(fi.parsed)) for rel, fi in self.project.files.items()))
        if self._memory_cache and self._memory_cache[0] == key:
            return self._memory_cache[1]
        rows = ((rel, e.line, e.who, e.source, e.target) for rel, fi in self.project.files.items() for e in fi.parsed.entries
                if e.source is not None and self.project.is_translated(rel, e))
        mem = memory.Memory(rows)
        self._memory_cache = (key, mem)
        return mem

    def entry_memory(self, rel, line):
        """Suggestions for one line from the project's own translations: the same original translated before, and
        originals that differ by a few words. Reuse needs a human eye (case, gender, a different variable)."""
        self._need_project()
        ent = next((e for e in self.project.entries(rel) if e.line == line), None) if rel in self.project.files else None
        if ent is None or ent.source is None:
            raise AppError(msg("The line is no longer in the file – refresh the files."))
        mem, skip = self._memory(), (rel, line)
        mine = memory.tags_of(ent.source)

        def mark(item, src):
            item["vars_differ"] = memory.tags_of(src) != mine
            return item
        exact = []
        for g in mem.exact_matches(ent.source, skip):
            row = next((r for r in mem.rows if r["file"] == g["lines"][0]["file"] and r["line"] == g["lines"][0]["line"]), None)
            exact.append(mark(g, row["source"] if row else ent.source))
        similar = [mark(x, x["source"]) for x in mem.similar(ent.source, skip)]
        return {"exact": exact, "similar": similar}

    def consistency(self, loose=False, by_speaker=False):
        """Originals translated in several different ways (exactly the same original text; see Memory.groups)."""
        self._need_project()
        groups = self._memory().groups(loose=bool(loose), by_speaker=bool(by_speaker))
        return {"groups": groups, "total": len(groups), "lines": sum(g["total"] for g in groups)}

    def consistency_unify(self, key, target, lines=None, loose=False, by_speaker=False):
        """Give the chosen lines of one original the same chosen translation (one undoable step).
        `lines` = [{file, line}] picks exactly which lines change; without it every line of the group does."""
        self._need_project()
        with self.lock:
            group = next((g for g in self._memory().groups(10 ** 6, bool(loose), bool(by_speaker)) if g["key"] == str(key)), None)
            if group is None:
                raise AppError(msg("The group has changed in the meantime – refresh the files and try again."))
            target = str(target)
            chosen = {(str(x["file"]), int(x["line"])) for x in lines} if lines else None
            edits = [(l["file"], l["line"], target) for v in group["variants"] for l in v["lines"]
                     if (chosen is None or (l["file"], l["line"]) in chosen)]
            if not edits:
                raise AppError(msg("Nothing to unify."))
            self._snapshot_for_bulk(len(edits), len({e[0] for e in edits}))
            try:
                res = fixes.apply_edits(self.project, self.local_dir, edits, msg("Unifying the translation: {text}", text=group['source'][:40]))
                self._log_text(res["details"], "unify")
            except fixes.FixError as e:
                raise AppError(text_of(e)) from e
            finally:
                self.project.refresh(force=True)
                self._val_cache.clear()
            self.store.ensure_files([r.name for r in self.project.rows()])
            out = self._changed("unify")
            out.update(changed=res["changed"], files=res["files"], backups=res["backups"])
            return out
