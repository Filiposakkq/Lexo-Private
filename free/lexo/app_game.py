"""Read-only facts about the game folder: gui, fonts, images, characters, scene preview data."""
import re
import urllib.parse
from pathlib import Path

from . import fontmetrics, gameimages
from .messages import msg
from .messages import text_of
from .app_base import AppError


class GamePart:
    def _game_dir(self):
        """The game's own `game` folder: the nearest folder called `game` above the translation folder (None when there is none, e.g. a
        translation kept somewhere else). Searches never go above it, so they cannot wander into other games next to this one."""
        for parent in self.project.root.parents:
            if parent.name.lower() == "game":
                return parent
        return None

    def _game_root(self):
        """Folder that contains `game/` (references look like game/script.rpy:123), found above the translation folder."""
        for parent in self.project.root.parents:
            if parent.name.lower() == "game":
                return parent.parent
        return None

    GUI_DEFAULTS = {"text_size": 33, "dialogue_width": 1116, "textbox_height": 278, "dialogue_ypos": 125, "dialogue_xpos": 402,
                    "name_xpos": 360, "name_ypos": 50, "name_text_size": 56, "screen_width": 1920, "screen_height": 1080}

    def game_gui(self):
        """Dialogue box metrics of the game (gui.rpy: screen size, text size and width, box height, name position), for the scene
        preview. Falls back to the values of Being a DIK when the game sources are not next to the translation."""
        out = dict(self.GUI_DEFAULTS, source=msg("default values"))
        base = self._game_root() if self.project else None
        if base is None:
            return out
        try:
            text = (base / "game" / "gui.rpy").read_text(encoding="utf-8", errors="replace")
        except OSError:
            return out
        found, fractions = {}, {}
        # numbers may be negative (name_ypos = -66) and a number with a decimal point is a FRACTION of the container in Ren'Py (dialogue_ypos = 0.33)
        for m in re.finditer(r"^\s*define\s+gui\.(\w+)\s*=\s*(-?\d+(?:\.\d+)?)\b", text, re.MULTILINE):
            if m.group(1) in found:
                continue
            if "." in m.group(2):
                fractions[m.group(1)] = float(m.group(2))
                found[m.group(1)] = 0
            else:
                found[m.group(1)] = int(m.group(2))
        m = re.search(r"gui\.init\(\s*(\d+)\s*,\s*(\d+)", text)
        if m:
            found["screen_width"], found["screen_height"] = int(m.group(1)), int(m.group(2))
        for k in self.GUI_DEFAULTS:
            if k in found:
                out[k] = found[k]
        width, height = out["screen_width"], out["screen_height"]
        tb_height = out["textbox_height"] if "textbox_height" not in fractions else round(fractions["textbox_height"] * height)
        out["textbox_height"] = tb_height
        for k, whole in (("name_ypos", tb_height), ("dialogue_ypos", tb_height), ("name_xpos", width), ("dialogue_xpos", width), ("dialogue_width", width)):
            if k in fractions:
                out[k] = round(fractions[k] * whole)
        out["source"] = msg("gui.rpy of the game") if any(k in found for k in self.GUI_DEFAULTS) else msg("default values")
        return out

    # ------------------------------------------------------------ fonts of the game (with the translation's overrides)
    _TL_FONT_PY = re.compile(r"^\s*gui\.(text_font|name_text_font)\s*=\s*[\"']([^\"']+)[\"']", re.MULTILINE)
    _TL_FONT_STYLE = re.compile(r"^translate\s+\w+\s+style\s+(\w+)\s*:\s*\n(?:[ \t]+[^\n]*\n)*?[ \t]+font\s+[\"']([^\"']+)[\"']", re.MULTILINE)

    def game_fonts(self):
        """Font files for the dialogue text and the speaker name: gui.rpy, overridden by the translation folder (`translate <lang> python:
        gui.text_font = ...`, `translate <lang> style say_dialogue: font ...`), which is how Czech diacritics are usually added."""
        out = {"text": "", "name": "", "source": ""}
        base = self._game_root()
        if base is None:
            return out
        try:
            gui = (base / "game" / "gui.rpy").read_text(encoding="utf-8", errors="replace")
        except OSError:
            gui = ""
        for m in re.finditer(r"^\s*define\s+gui\.(text_font|name_text_font)\s*=\s*[\"']([^\"']+)[\"']", gui, re.MULTILINE):
            out["text" if m.group(1) == "text_font" else "name"] = m.group(2)
        found = bool(out["text"] or out["name"])
        styles = {}
        for f in sorted(self.project.root.rglob("*.rpy")):
            try:
                txt = f.read_text(encoding="utf-8", errors="replace")
            except OSError:
                continue
            if "font" not in txt:
                continue
            for m in self._TL_FONT_PY.finditer(txt):
                out["text" if m.group(1) == "text_font" else "name"] = m.group(2)
                found = True
            for m in self._TL_FONT_STYLE.finditer(txt):
                styles.setdefault(m.group(1), m.group(2))
        if styles.get("say_dialogue"):
            out["text"] = styles["say_dialogue"]
        elif styles.get("default") and not out["text"]:
            out["text"] = styles["default"]
        if styles.get("say_label"):
            out["name"] = styles["say_label"]
        out["source"] = msg("gui.rpy and the translation") if (found or styles) else ""
        return out

    # ------------------------------------------------------------ the picture a line is shown on
    def _game_images(self):
        """Index of the game's pictures (archives + loose files + `image` statements); built once per game folder."""
        base = self._game_root()
        if base is None or not (base / "game").is_dir():
            return None
        if self._gi is None or self._gi[0] != base:
            self._gi = (base, gameimages.GameImages(base / "game").build())
        return self._gi[1]

    def scene_info(self, rel, line):
        """Which picture is behind a line (the last `scene` / `show` before its place in the game script), plus the textbox metrics
        needed to draw the line over it. Read-only; an approximation that follows the script text upwards."""
        self._need_project()
        ent = next((e for e in self.project.entries(rel) if e.line == line), None) if rel in self.project.files else None
        if ent is None:
            raise AppError(msg("The line is no longer in the file – refresh the files."))
        gui = self.game_gui()
        out = {"available": False, "gui": gui, "who": ent.who, "reason": ""}
        gi = self._game_images()
        if gi is None:
            out["reason"] = msg("The game folder (game) above the translation was not found, so the scene pictures cannot be shown.")
            return out
        m = re.match(r"^(game/.+?):(\d+)$", ent.ref or "")
        if not m:
            out["reason"] = msg("The line has no recorded position in the game script.")
            return out
        script = (self._game_root() / m.group(1)).resolve()
        if self._game_root().resolve() not in script.parents or script.suffix.lower() not in (".rpy", ".rpym"):
            out["reason"] = msg("Invalid script path.")
            return out
        try:
            sc = gi.scene_for(script, int(m.group(2)))
        except OSError:
            out["reason"] = f"Skript {m.group(1)} nebyl nalezen (jsou po ruce jen .rpyc?)."
            return out
        fonts = self.game_fonts()
        for kind in ("text", "name"):
            fn = Path(fonts[kind]).name.lower() if fonts[kind] else ""
            out["fonts_" + kind] = {"file": fn, "url": "/api/game/font?k=" + urllib.parse.quote(fn, safe="")} if fn in gi.fonts else None
        ch = gi.characters.get(ent.who or "")
        out["speaker"] = ch
        out.update(available=True, found=sc["found"], image=sc["image"], missing=sc["missing"], script=f"{m.group(1)}:{sc['script_line']}",
                   flat=sc.get("flat", ""), index_errors=gi.errors[:3], images=len(gi.stems))
        if sc["found"] and sc["key"]:
            out["url"] = "/api/game/image?k=" + urllib.parse.quote(sc["key"], safe="")
        elif not sc["found"]:
            out["reason"] = msg("I found no picture (scene / show) before this line that can be traced in the game archives.")
        return out

    def game_font(self, name):
        """(bytes, mime) of a font file of the game, by file name (only fonts the index knows)."""
        self._need_project()
        gi = self._game_images()
        if gi is None:
            raise AppError(msg("The game fonts are not available."))
        try:
            return gi.font_bytes(name)
        except (gameimages.rpa.RpaError, OSError) as e:
            raise AppError(text_of(e)) from e

    def game_characters(self):
        """{speaker code: {name, color}} from the game's `define x = Character(...)` lines (empty without the game sources)."""
        self._need_project()
        gi = self._game_images()
        return {"characters": dict(gi.characters) if gi is not None else {}}

    def game_font_metrics(self, name):
        """Advance widths (from hmtx) of a game font, so the scene preview can place characters like Ren'Py does."""
        data, _mime = self.game_font(name)
        m = fontmetrics.advances(data)
        if m is None:
            raise AppError(msg("The character widths cannot be read from the font."))
        return {"upm": m["upm"], "adv": {str(k): v for k, v in m["adv"].items()}}

    def game_image(self, key):
        """(bytes, mime) of a picture the index knows by its stem key (never an arbitrary path)."""
        self._need_project()
        gi = self._game_images()
        if gi is None:
            raise AppError(msg("The game pictures are not available."))
        try:
            return gi.image_bytes(key)
        except (gameimages.rpa.RpaError, OSError) as e:
            raise AppError(text_of(e)) from e
