"""The menu a choice belongs to: question, the other choices, and what follows each of them (read from the game script)."""
import re

from . import menus
from .app_base import AppError
from .messages import msg
from .util import norm_ws

REF_RE = re.compile(r"^(game/.+?):(\d+)$")


class MenuPart:
    def entry_menu(self, rel, line):
        """Menu around the choice at `line` of the translation file `rel`. {available: False, reason} when the line is no choice of a menu we can read."""
        self._need_project()
        if rel not in self.project.files:
            raise AppError(msg("Unknown file."))
        ent = next((e for e in self.project.entries(rel) if e.line == line), None)
        if ent is None:
            raise AppError(msg("The line is no longer in the file – refresh the files."))
        m = REF_RE.match(ent.ref or "")
        base = self._game_root()
        if not m or base is None:
            return {"available": False, "reason": msg("The line has no recorded position in the game script.")}
        path = (base / m.group(1)).resolve()
        if base.resolve() not in path.parents or path.suffix.lower() not in (".rpy", ".rpym"):
            return {"available": False, "reason": msg("Invalid script path.")}
        try:
            if path.stat().st_size > 8 * 1024 * 1024:
                return {"available": False, "reason": msg("The script is too large.")}
            lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        except OSError:
            return {"available": False, "reason": msg("The script {name} was not found (the original .rpy files of the game are not at hand, maybe only .rpyc).", name=m.group(1))}
        found = menus.find_menu(lines, int(m.group(2)), ent.source)
        if found is None:
            return {"available": False, "reason": msg("This line is not a choice of a menu in the game script.")}
        by_ref, by_text = {}, {}
        for r, fi in self.project.files.items():
            for e in fi.parsed.entries:
                if e.unit == "string":
                    if e.source is not None:
                        by_text.setdefault(norm_ws(e.source), (r, e))
                elif e.ref:
                    by_ref[e.ref] = (r, e)

        def row(r, e):
            return {"file": r, "line": e.line, "who": e.who, "source": e.source, "target": e.target, "status": self._entry_row(r, e)}

        def say(n):
            hit = by_ref.get(f"{m.group(1)}:{n}")
            return {"n": n, "entry": row(*hit) if hit else None, "script": "" if hit else lines[n - 1].strip()}

        out_opts = []
        for k, o in enumerate(found["options"]):
            hit = by_text.get(norm_ws(o["text"]))
            out_opts.append({"n": o["n"], "text": o["text"], "cond": o["cond"], "current": k == found["current"],
                             "entry": row(*hit) if hit else None, "after": [say(n) for n in o["after"]]})
        return {"available": True, "script": f"{m.group(1)}:{found['menu_line']}", "question": [say(n) for n in found["question"]],
                "before": [say(n) for n in found["before"]], "options": out_opts}
