"""Glossary of names: speakers and terms, import from the game's character definitions."""
import os
from pathlib import Path

from . import validator
from .messages import msg
from .store import GLOSSARY_TYPES
from .app_base import AppError, CHARACTERS_MAX_BYTES, CHARACTER_RE, SCREEN_KEYWORDS


class GlossaryPart:
    # ------------------------------------------------------------ glossary
    @staticmethod
    def speaker_code(who):
        """'mc happy' -> 'mc': the character code is the first word, the rest are image attributes."""
        parts = (who or "").split()
        return parts[0] if parts else ""

    def glossary_payload(self):
        """Glossary rows: every speaker code found in the project (with its line count and an example)
        merged with the saved names, plus saved entries that are not speakers (terms)."""
        self._need_project()
        found = {}
        for rel in sorted(self.project.files, key=str.lower):
            for e in self.project.entries(rel):
                code = self.speaker_code(e.who)
                if e.unit == "dialogue" and code and code not in SCREEN_KEYWORDS:
                    f = found.setdefault(code, {"count": 0, "example": "", "file": rel, "line": e.line})
                    f["count"] += 1
                    if not f["example"] and e.source:
                        f["example"] = e.source[:90]
        items = []
        for code, f in found.items():
            g = self.store.glossary.get(code, {})
            items.append({"code": code, "name": g.get("n", ""), "type": g.get("t", "speaker"), "note": g.get("c", ""),
                          "count": f["count"], "example": f["example"], "file": f["file"], "line": f["line"],
                          "in_project": True})
        for code, g in self.store.glossary.items():
            if code not in found:
                items.append({"code": code, "name": g["n"], "type": g["t"], "note": g["c"], "count": 0,
                              "example": "", "file": "", "line": 0, "in_project": False})
        items.sort(key=lambda i: (-i["count"], i["code"].lower()))
        names = {c: g["n"] for c, g in self.store.glossary.items() if g["n"] and g["t"] == "speaker"}
        return {"items": items, "names": names, "types": list(GLOSSARY_TYPES),
                "unnamed": sum(1 for i in items if i["type"] == "speaker" and i["in_project"] and not i["name"])}

    def glossary_set(self, items):
        """Create / update entries. An entry with neither name nor note is removed. One undo step."""
        self._need_project()
        clean = []
        for it in items:
            code = str(it.get("code", "")).strip()
            if not code or len(code) > 60 or any(ch in code for ch in "\r\n"):
                raise AppError(msg("Invalid code: {code}", code=code))
            typ = it.get("type", "speaker")
            if typ not in GLOSSARY_TYPES:
                raise AppError(msg("Invalid type of a glossary item."))
            name, note = str(it.get("name", "")).strip(), str(it.get("note", "")).strip()
            if len(name) > 200 or len(note) > 500:
                raise AppError(msg("The name or note is too long."))
            clean.append((code, name, typ, note))
        with self.lock:
            def go():
                for code, name, typ, note in clean:
                    if name or note:
                        self.store.glossary[code] = {"n": name, "t": typ, "c": note}
                    else:
                        self.store.glossary.pop(code, None)
            self.store.change(msg("Glossary ({n})", n=len(clean)), ("glossary",), go)
            return {"ok": True, **self.glossary_payload()}

    def find_characters_files(self):
        """All files of the game that define characters: the files named characters*.rpy (BaDIK keeps them there) and, when there are none,
        every other script that has a `define x = Character(...)` line (most games put them in script.rpy or a definitions file).
        The translation folder is usually <game>/game/tl/<lang>, so the game folder is searched upwards."""
        skip = ("tl", "cache", "saves", "_progress_data", ".lexo", ".git")
        base = self._game_dir()
        for base in ([base] if base is not None and base.is_dir() else []):
            named, scripts = [], []
            for dirpath, dirs, files in os.walk(base):
                dirs[:] = [d for d in dirs if d.lower() not in skip]
                for f in files:
                    if f.lower().endswith(".rpy"):
                        (named if f.lower().startswith("characters") else scripts).append(Path(dirpath) / f)
            if named:
                return sorted(named, key=lambda x: str(x).lower())      # Ren'Py loads files in path order
            defining = [f for f in scripts if self._defines_characters(f)]
            if defining:
                return sorted(defining, key=lambda x: str(x).lower())
        return []

    @staticmethod
    def _defines_characters(path):
        """Does the script have at least one `define x = Character(...)` line? (a quick text test, no parsing)"""
        try:
            if path.stat().st_size > CHARACTERS_MAX_BYTES:
                return False
            return any(CHARACTER_RE.match(line) for line in path.read_text(encoding="utf-8-sig", errors="replace").splitlines())
        except OSError:
            return False

    def glossary_import_preview(self, path=""):
        """Read `define xx = Character("Name", ...)` lines of the game's character files and compare with the
        glossary. Nothing is written. `path` may be a file, a folder (all characters*.rpy below it) or empty
        (found automatically). Ren'Py rule: a later definition of the same code replaces an earlier one, so the
        last one wins; codes defined several times with different names are reported in `duplicates`.
        states: new | same | conflict (another name is already stored) | dynamic (name comes from a variable)."""
        self._need_project()
        given = str(path).strip()
        if given:
            target = Path(given).expanduser()
            if target.is_dir():
                files = sorted((p for p in target.rglob("characters*.rpy") if "tl" not in p.relative_to(target).parts),
                               key=lambda x: str(x).lower())
                if not files:
                    files = sorted((p for p in target.rglob("*.rpy") if not {"tl", "cache", "saves"} & set(p.relative_to(target).parts)
                                    and self._defines_characters(p)), key=lambda x: str(x).lower())
            else:
                files = [target]
        else:
            files = self.find_characters_files()
        if not files:
            return {"path": given, "files": [], "items": [], "missing": [], "duplicates": [],
                    "message": msg("No character definitions were found – give the path to a file or to the game folder.")}
        defs = {}          # code -> list of (name, dynamic, file)
        for f in files:
            if f.suffix.lower() != ".rpy":
                raise AppError(msg("Only .rpy files (characters.rpy) can be imported."))
            try:
                if not f.is_file():
                    raise AppError(msg("The file does not exist: {path}", path=f))
                if f.stat().st_size > CHARACTERS_MAX_BYTES:
                    raise AppError(msg("The file is too large for a list of characters: {name}", name=f.name))
                text = f.read_text(encoding="utf-8-sig", errors="replace")
            except OSError as e:
                raise AppError(msg("The file cannot be read: {reason}", reason=e.strerror or e)) from e
            for line in text.splitlines():
                m = CHARACTER_RE.match(line)
                if m:
                    name = (m.group(3) if m.group(3) is not None else m.group(4) or "").replace('\\"', '"').strip()
                    dynamic = bool(m.group(2)) or name.startswith("[")            # Character("[mc.name]"): the name comes from a variable
                    defs.setdefault(m.group(1), []).append((name, dynamic, f))
        in_project = {self.speaker_code(e.who) for rel in self.project.files for e in self.project.entries(rel)
                      if e.unit == "dialogue"} - SCREEN_KEYWORDS
        game_dir = None
        try:
            game_dir = Path(os.path.commonpath([str(f.parent) for f in files]))
        except ValueError:
            pass
        items, duplicates = [], []
        for code, versions in defs.items():
            name, dynamic, src = versions[-1]
            names = []
            for n, _d, _f in versions:
                if n and n not in names:
                    names.append(n)
            if len(names) > 1:
                duplicates.append({"code": code, "names": names, "used": name})
            cur = self.store.glossary.get(code, {}).get("n", "")
            if dynamic:
                state, name = "dynamic", ""
            elif not name:
                continue
            elif not cur:
                state = "new"
            elif cur == name:
                state = "same"
            else:
                state = "conflict"
            try:
                shown = str(src.relative_to(game_dir)) if game_dir else src.name
            except ValueError:
                shown = src.name
            note = msg("the name is entered by the player (DynamicCharacter)") if dynamic else (
                msg("defined several times ({names}), the last one counts", names=' / '.join(names)) if len(names) > 1 else "")
            items.append({"code": code, "name": name, "current": cur, "state": state, "in_project": code in in_project,
                          "note": note, "source": shown})
        missing = sorted(c for c in in_project - set(defs) if c)
        return {"path": given or str(game_dir or files[0].parent), "files": [str(f) for f in files], "items": items,
                "missing": missing, "duplicates": duplicates, "message": ""}

    def glossary_import_apply(self, items):
        """Store the reviewed names (a stored note is kept). One undo step."""
        self._need_project()
        out = []
        for it in items:
            code = str(it.get("code", "")).strip()
            cur = self.store.glossary.get(code, {})
            out.append({"code": code, "name": str(it.get("name", "")).strip(), "type": "speaker",
                        "note": cur.get("c", "") or str(it.get("note", ""))})
        return self.glossary_set(out)

    def glossary_delete(self, codes):
        self._need_project()
        with self.lock:
            self.store.change(msg("Glossary: deleted ({n})", n=len(codes)), ("glossary",),
                              lambda: [self.store.glossary.pop(str(c), None) for c in codes])
            return {"ok": True, **self.glossary_payload()}

    def glossary_occurrences(self, code, only_missing=False, limit=400):
        """Every line whose original contains the glossary term `code`, with whether the Czech form is used."""
        self._need_project()
        g = self.store.glossary.get(code)
        if g is None:
            raise AppError(msg("The glossary entry does not exist."))
        czech = g.get("n", "")
        rows, found, missing, untranslated = [], 0, 0, 0
        for rel in sorted(self.project.files, key=str.lower):
            for e in self.project.entries(rel):
                if e.source is None or not validator.term_in_text(e.source, code):
                    continue
                translated = self.project.is_translated(rel, e)
                used = (validator.term_used(e.target, czech) if czech and translated else None)
                if used:
                    found += 1
                elif used is False:
                    missing += 1
                else:
                    untranslated += 1
                if only_missing and used is not False:
                    continue
                rows.append({"file": rel, "line": e.line, "who": e.who, "source": e.source, "target": e.target,
                             "translated": translated, "used": used})
        return {"code": code, "czech": czech, "found": found, "missing": missing, "untranslated": untranslated,
                "items": rows[:limit], "truncated": len(rows) > limit}
