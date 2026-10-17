"""The game check (Ren'Py lint) run in the background."""
from . import lint
from .messages import msg
from .messages import text_of
from .app_base import AppError


class LintPart:
    # ------------------------------------------------------------ Ren'Py lint of the whole game
    def lint_start(self):
        self._need_project()
        sdk = str(self.config.get("lint.sdk", "")).strip()
        if not sdk:
            raise AppError(msg("First set the Ren'Py SDK folder (the one with renpy.py)."))
        base = self._game_root()
        if base is None:
            raise AppError(msg("The game folder (game) above the translation was not found – lint needs the source files of the game."))
        try:
            self.linter.start(sdk, base)
        except lint.LintError as e:
            raise AppError(text_of(e)) from e
        return {"ok": True}

    def lint_status(self):
        self._need_project()
        st = self.linter.status()
        base = self._game_root()
        for it in st.get("issues", []):
            it["rel"] = ""
            if base is not None:
                try:
                    p = (base / it["file"]).resolve()
                    it["rel"] = p.relative_to(self.project.root.resolve()).as_posix()
                    it["rel"] = next((r for r in self.project.files if r.replace("\\", "/") == it["rel"]), "")
                except (ValueError, OSError):
                    it["rel"] = ""
        return st
