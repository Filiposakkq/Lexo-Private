"""Assigning files to groups (episodes), episode states, approving pending lines."""
from pathlib import Path

from . import bulk, layout
from .messages import msg
from .messages import text_of
from .store import EP_STATE_KEYS
from .store import valid_assign
from .app_base import AppError


class AssignPart:
    def set_assign(self, name, v):
        self._need_project()
        with self.lock:
            if name not in self.project.files or not valid_assign(v):
                raise AppError(msg("Invalid value."))

            def go():
                if v:
                    self.store.assign[name] = v
                else:
                    self.store.assign.pop(name, None)
            self.store.change(msg("Assignment of {name}", name=Path(name).name), ("assign",), go)
            return self._changed("assign")

    def set_episode_state(self, n, state):
        self._need_project()
        with self.lock:
            if n not in layout.ACTIVE_EPISODES or state not in EP_STATE_KEYS:
                raise AppError(msg("Invalid value."))
            self.store.change(f"Stav epizody {n}", ("episodes",), lambda: self.store.episodes.__setitem__(n, state))
            return self._changed("episode")

    def assign_suggest(self, names=None):
        self._need_project()
        names = names or sorted(self.project.files, key=str.lower)
        unknown = [n for n in names if n not in self.project.files]
        if unknown:
            raise AppError(msg("Unknown files: {names}", names=', '.join(unknown[:3])))
        names, cut = self._cap("bulk.categorize", names)
        sugg = bulk.suggest(names, self.store.assign, known_assign=self.store.assign)
        return {**cut, "items": sugg, "options": [{"value": "", "label": msg("Unassigned")}, {"value": "gen", "label": msg("General (the whole game)")}]
                + [{"value": f"ep:{n}", "label": f"Epizoda {n} (sezóna {next(s['n'] for s in layout.SEASONS if n in s['episodes'])})"}
                   for n in layout.ACTIVE_EPISODES]}

    def assign_apply(self, items):
        self._need_project()
        items, _cut = self._cap("bulk.categorize", items)
        with self.lock:
            for it in items:
                if it.get("name") not in self.project.files:
                    raise AppError(msg("Unknown file: {name}", name=it.get('name')))
            res = bulk.apply_assignments(self.store, items)
            out = self._changed("bulk-assign")
            out.update(res)
            return out

    def approval_preview(self, file, line, scope="project", same_speaker=True):
        self._need_project()
        try:
            return bulk.approval_candidates(self.project, file, int(line), scope, same_speaker)
        except ValueError as e:
            raise AppError(text_of(e)) from e

    def rule_preview(self, rules, files=None):
        self._need_project()
        files = [str(f) for f in (files or [])]
        unknown = [f for f in files if f not in self.project.files]
        if unknown:
            raise AppError(msg("Unknown file: {name}", name=unknown[0]))
        return bulk.rule_candidates(self.project, self.store.glossary, list(rules or []), files or None)

    def _worst_files(self, counts_by_file, limit=10):
        totals = {r.name: r.total for r in self.project.rows()}
        out = []
        for rel, c in counts_by_file.items():
            problems = c["error"] * 3 + c["warning"]
            if not problems or rel not in totals:
                continue
            out.append({"name": rel, "errors": c["error"], "warnings": c["warning"], "lines": totals[rel],
                        "score": round(problems / max(1, totals[rel]) * 100, 1)})
        out.sort(key=lambda r: (-r["score"], -r["errors"], r["name"].lower()))
        return out[:limit]

    def approval_apply(self, selections, on=True):
        self._need_project()
        with self.lock:
            res = bulk.apply_approvals(self.project, selections, on)
            out = self._changed("approve")
            out.update(res)
            return out
