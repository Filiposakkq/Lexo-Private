"""Texts the generator missed (show text, input prompts, titles in variables ...): find them in the game's scripts and add them to the translation."""
from pathlib import Path

from . import missed
from .app_base import AppError
from .messages import msg, msgn
from .util import WriteFailed


class MissedPart:
    def _script_folder(self):
        """The folder with the game's scripts (.rpy outside tl): <game>/ for a translation in <game>/tl/<language>."""
        base = self._game_dir()
        return base if base is not None and missed.has_scripts(base) else None

    def missed_scan(self, level="likely"):
        """Look for texts in the game's scripts that the translation does not have. Nothing is changed.
        level: 'likely' (default) or 'all' (adds the weaker guesses). Returns {items, truncated, folder, counts, message}."""
        self._need_project()
        folder = self._script_folder()
        if folder is None:
            return {"items": [], "truncated": False, "folder": "", "counts": {"likely": 0, "maybe": 0},
                    "message": msg("The game's scripts (.rpy) were not found next to the translation. Unpack and decompile the game first.")}
        strings, dialogue, places = missed.known_texts(self.project.root)
        res = missed.find_missed(folder, strings, dialogue, places, limit=2000 if level == "all" else 100000)
        items = res["items"]
        counts = {"likely": sum(1 for i in items if i["level"] == "likely"), "maybe": sum(1 for i in items if i["level"] != "likely")}
        if level != "all":
            items = [i for i in items if i["level"] == "likely"]
        return {"items": items, "truncated": res["truncated"], "folder": str(folder), "counts": counts, "message": ""}

    def missed_add(self, items):
        """Write the chosen texts to <translation folder>/extra_strings.rpy (untranslated, so they appear as lines to do) and reload the files."""
        self._need_project()
        chosen = [{"text": str(i.get("text", "")), "file": str(i.get("file", "")), "line": int(i.get("line", 0) or 0)}
                  for i in (items or []) if isinstance(i, dict) and str(i.get("text", "")).strip()]
        if not chosen:
            raise AppError(msg("Nothing was chosen."))
        with self.lock:
            self._snapshot("bulk")
            try:
                res = missed.write_strings(self.project.root, Path(self.project.root).name, chosen, self.local_dir / "backups")
            except WriteFailed as e:
                raise AppError(msg("The strings file could not be written: {reason}", reason=e.reason)) from e
            self.refresh(force=True)
            res["message"] = msgn(res["added"], "{n} text was added to extra_strings.rpy.", "{n} texts were added to extra_strings.rpy.")
            return res
