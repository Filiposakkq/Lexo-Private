"""Language packs: the rules of the checks that depend on the language of the translation.

Core checks (tags, variables, brackets, length, dialogue box) are language-neutral and live in `validator`.
What depends on the target language is a `Pack`, loaded from DATA FILES (no executable code, so a downloaded pack is safe):

    <pack folder>/pack.json    {"format": 1, "code": "cs", "name": "Čeština", "version": 1, "min_app": "1.7.0"}
    <pack folder>/rules.json   {"check_repeats": true, "repeat_ok": [...], "number_words_ok": true,
                                "stem": {"type": "cut", "drop": 2, "min": 3}}

  repeat_ok          words that are normally doubled in speech, so "the same word twice" must not be reported
  check_repeats      the repeated-word check is only reliable when `repeat_ok` is known; off for the neutral pack
  number_words_ok    a digit written out in words ("14" -> "čtrnáct") is normal, so a missing number is only an info
  stem               how a word is reduced so that glossary terms are found in any inflected form:
                     {"type": "none"} (exact word) or {"type": "cut", "drop": N, "min": M} (drop the last N letters, keep at least M)

Where packs come from: the ones shipped with the app (`lexo/packs/`) and the user's folder (`<config dir>/packs/`, where downloaded
packs go). A pack with the same code in the user's folder wins when its `version` is higher. A broken pack is skipped and reported
in `problems()`; it never stops the app. The neutral pack `und` is built in and used when no pack matches.

The pack of the open project is chosen by the `language` key of project.json (layout.py). Messages shown to the user are still Czech
sentences; they move to message keys with the UI translation.
"""
import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable, FrozenSet

from .util import fold

FORMAT = 1
BUNDLED_DIR = Path(__file__).resolve().parent / "packs"


class PackError(ValueError):
    pass


def _stem_none(word):
    return fold(word)


def _stem_cut(drop, minimum):
    def stem(word):
        folded = fold(word)
        return folded[:max(minimum, len(folded) - drop)]
    return stem


@dataclass(frozen=True)
class Pack:
    code: str
    name: str
    version: int = 0
    repeat_ok: FrozenSet[str] = field(default_factory=frozenset)
    check_repeats: bool = False
    number_words_ok: bool = False
    stem: Callable[[str], str] = _stem_none


NEUTRAL = Pack(code="und", name="Neutral")


def _read(path):
    try:
        return json.loads(Path(path).read_text(encoding="utf-8-sig"))
    except (OSError, ValueError) as e:
        raise PackError(f"{Path(path).name}: {e}") from e


def _stem_rule(raw):
    if raw is None:
        return _stem_none
    if not isinstance(raw, dict) or raw.get("type") not in ("none", "cut"):
        raise PackError("rules.json: \"stem\" must be {\"type\": \"none\"} or {\"type\": \"cut\", \"drop\": N, \"min\": M}")
    if raw["type"] == "none":
        return _stem_none
    drop, minimum = raw.get("drop"), raw.get("min", 3)
    if not all(isinstance(v, int) and not isinstance(v, bool) and v >= 0 for v in (drop, minimum)):
        raise PackError("rules.json: stem \"drop\" and \"min\" must be whole numbers, 0 or more")
    return _stem_cut(drop, minimum)


def load_dir(folder):
    """Read one pack folder; raises PackError with a readable reason."""
    folder = Path(folder)
    meta, rules = _read(folder / "pack.json"), _read(folder / "rules.json")
    if not isinstance(meta, dict) or not isinstance(rules, dict):
        raise PackError("pack.json and rules.json must contain JSON objects")
    if meta.get("format") != FORMAT:
        raise PackError(f"pack.json: unsupported format {meta.get('format')!r} (this app reads {FORMAT})")
    code, name = meta.get("code"), meta.get("name")
    if not isinstance(code, str) or not code.strip() or not isinstance(name, str):
        raise PackError("pack.json needs a text \"code\" and \"name\"")
    version = meta.get("version", 0)
    if not isinstance(version, int) or isinstance(version, bool):
        raise PackError("pack.json: \"version\" must be a whole number")
    repeat_ok = rules.get("repeat_ok", [])
    if not isinstance(repeat_ok, list) or not all(isinstance(w, str) for w in repeat_ok):
        raise PackError("rules.json: \"repeat_ok\" must be a list of words")
    flags = {}
    for key in ("check_repeats", "number_words_ok"):
        flags[key] = rules.get(key, False)
        if not isinstance(flags[key], bool):
            raise PackError(f"rules.json: \"{key}\" must be true or false")
    return Pack(code=code.strip().lower(), name=name, version=version, repeat_ok=frozenset(w.lower() for w in repeat_ok),
                stem=_stem_rule(rules.get("stem")), **flags)


def user_dir():
    from .config import config_dir
    return config_dir() / "packs"


_problems = []


def discover(extra_dirs=None):
    """All usable packs {code: Pack} from the bundled folder and the user's folder; problems go to problems()."""
    _problems.clear()
    found = {}
    for root in [BUNDLED_DIR, *(extra_dirs if extra_dirs is not None else [user_dir()])]:
        if not Path(root).is_dir():
            continue
        for folder in sorted(p for p in Path(root).iterdir() if p.is_dir()):
            try:
                pack = load_dir(folder)
            except PackError as e:
                _problems.append(f"{folder.name}: {e}")
                continue
            if pack.code not in found or pack.version > found[pack.code].version:
                found[pack.code] = pack
    found.setdefault(NEUTRAL.code, NEUTRAL)
    return found


def problems():
    return list(_problems)


def get(code):
    """The pack for a language code, or None when none is installed (looked up on every call, so a new download is seen at once)."""
    code = (code or "").strip().lower()
    return discover().get(code) if code else None


CS = load_dir(BUNDLED_DIR / "cs")           # shipped with the app; used by the tests as the reference pack
PACKS = {CS.code: CS, NEUTRAL.code: NEUTRAL}

_current = NEUTRAL


def use(pack):
    global _current
    _current = pack or NEUTRAL


def current():
    return _current
