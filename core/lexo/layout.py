"""Project layout: how a project's files are grouped (sections > groups), defined per project.

A *group* is a numbered unit the translator wants statistics for (an episode, a chapter, a route) and a
*section* is a set of groups (a season, a part). Both are optional: a project without a layout file has
no groups, and every file counts as "unassigned" in the overview.

The layout is read from `<project data folder>/aside.json`:

    {"version": 1,
     "language": "cs",
     "labels": {"section": "Season", "group": "Episode"},
     "sections": [{"n": 1, "status": "active",  "groups": [1, 2, 3, 4]},
                  {"n": 2, "status": "active",  "groups": [5, 6, 7, 8]},
                  {"n": 3, "status": "planned"}]}

`status: "planned"` means the section is shown but never counted. Group numbers must be unique across the
project. `language` is the code of a language pack (lang.py) for the language-dependent checks; unknown or missing = neutral rules.
A broken file is reported and ignored (empty layout), it is never silently "repaired".

Everything that needs the layout reads it from here (`layout.ACTIVE_EPISODES`, `layout.SEASONS`, ...). The
names keep the vocabulary of the first game (episodes, seasons) until the UI is switched to the labels.
The layout of the open project is process-wide state set by `use()`; the application serves one project at a time.
"""
import json
import re
from pathlib import Path

from . import lang
from .util import atomic_write_text

FILE_NAME = "aside.json"
DEFAULT_LABELS = {"section": "Season", "group": "Episode"}


class Layout:
    def __init__(self, sections=(), labels=None, language="", series="", folders=()):
        self.language = language
        self.folders = tuple(folders)     # one group per folder: group N is the N-th folder (a path prefix such as "scripts/A_home")
        self.series = series              # files called <series>, <series>2, <series>3 ... are groups 1, 2, 3 ... (e.g. "script")
        self.sections = tuple(sections)
        self.labels = dict(DEFAULT_LABELS, **(labels or {}))
        self.active_groups = tuple(g for s in self.sections if s["status"] == "active" for g in s["episodes"])

    def section_of(self, group):
        for s in self.sections:
            if group in s["episodes"]:
                return s["n"]
        return None

    def groups_of(self, section):
        for s in self.sections:
            if s["n"] == section:
                return s["episodes"]
        return ()

    def to_json(self):
        return {"version": 1, **({"language": self.language} if self.language else {}), **({"series": self.series} if self.series else {}), **({"folders": list(self.folders)} if self.folders else {}), "labels": self.labels,
                "sections": [{"n": s["n"], "status": s["status"], **({"groups": list(s["episodes"])} if s["status"] == "active" else {})}
                             for s in self.sections]}


class LayoutError(ValueError):
    pass


def parse(raw):
    """Build a Layout from the decoded JSON; raises LayoutError with a readable reason."""
    if not isinstance(raw, dict):
        raise LayoutError("the file must contain a JSON object")
    secs, seen_n, seen_g = [], set(), set()
    for s in raw.get("sections", []) or []:
        if not isinstance(s, dict) or not isinstance(s.get("n"), int) or isinstance(s.get("n"), bool):
            raise LayoutError("every section needs an integer \"n\"")
        if s["n"] in seen_n:
            raise LayoutError(f"section {s['n']} is listed twice")
        seen_n.add(s["n"])
        status = s.get("status", "active")
        if status not in ("active", "planned"):
            raise LayoutError(f"section {s['n']}: status must be \"active\" or \"planned\"")
        groups = s.get("groups", []) if status == "active" else []
        if not isinstance(groups, list) or not all(isinstance(g, int) and not isinstance(g, bool) and g > 0 for g in groups):
            raise LayoutError(f"section {s['n']}: \"groups\" must be a list of positive integers")
        for g in groups:
            if g in seen_g:
                raise LayoutError(f"group {g} belongs to more than one section")
            seen_g.add(g)
        secs.append({"n": s["n"], "status": status, "episodes": tuple(groups)})
    labels = raw.get("labels", {})
    if not isinstance(labels, dict) or not all(isinstance(v, str) for v in labels.values()):
        raise LayoutError("\"labels\" must map names to text")
    language = raw.get("language", "")
    if not isinstance(language, str):
        raise LayoutError("\"language\" must be a language code such as \"cs\"")
    series = raw.get("series", "")
    if not isinstance(series, str) or (series and not re.fullmatch(r"[A-Za-z_][\w-]*", series)):
        raise LayoutError("\"series\" must be a file name stem such as \"script\"")
    folders = raw.get("folders", [])
    if not isinstance(folders, list) or not all(isinstance(f, str) and f.strip("/") and ".." not in f.split("/") for f in folders) or len(folders) > 200:
        raise LayoutError("\"folders\" must be a list of folder paths such as \"scripts/A_home\"")
    return Layout(secs, {k: v for k, v in labels.items() if k in DEFAULT_LABELS}, language.strip().lower(), series, [f.strip("/") for f in folders])


def load(data_dir):
    """(Layout, note). note is "" or a message for the user (broken file)."""
    path = Path(data_dir) / FILE_NAME
    if path.is_file():
        try:
            lay = parse(json.loads(path.read_text(encoding="utf-8-sig")))
            return lay, (f"{FILE_NAME}: no language pack for \"{lay.language}\", neutral rules are used."
                         if lay.language and lang.get(lay.language) is None else "")
        except (OSError, ValueError) as e:             # LayoutError and JSONDecodeError are ValueErrors
            return Layout(), f"{FILE_NAME} was ignored: {e}"
    return Layout(), ""


_current = Layout()


def use(layout):
    global _current
    _current = layout
    lang.use(lang.get(layout.language))


def current():
    return _current


def __getattr__(name):                                  # layout.SEASONS / layout.ACTIVE_EPISODES follow the open project
    if name == "SEASONS":
        return _current.sections
    if name == "ACTIVE_EPISODES":
        return _current.active_groups
    raise AttributeError(name)


def season_of(episode):
    return _current.section_of(episode)


def episodes_of(season):
    return _current.groups_of(season)


def is_active_episode(episode):
    return episode in _current.active_groups


# ---------------------------------------------------------------- groups found from file names
# The words a project may call its numbered units, with the pattern that recognises them in a file or folder name.
GROUP_WORDS = {"Folder": r"a^", "Episode": r"episode|ep", "Chapter": r"chapter|chap|ch", "Part": r"part", "Day": r"day", "Update": r"update|upd", "Act": r"act"}
_SEASON_RE = re.compile(r"(?<![a-z0-9])season[ _-]?0*(\d{1,2})(?!\d)")


def group_regex(word=None):
    """Pattern that finds the number of a group in a lower-case path; `word` defaults to the label of the open project."""
    word = word or _current.labels.get("group", "Episode")
    return re.compile(rf"(?<![a-z0-9])(?:{GROUP_WORDS.get(word, GROUP_WORDS['Episode'])})[ _-]?0*(\d{{1,3}})(?!\d)")


def folder_number(path, folders):
    """The group number of a file that lies in one of the project's folders (the longest matching folder wins); None otherwise."""
    p = str(path).replace("\\", "/").lower()
    best = None
    for i, f in enumerate(folders, 1):
        low = f.lower().strip("/")
        if p.startswith(low + "/") and (best is None or len(low) > len(folders[best - 1].strip("/"))):
            best = i
    return best


def _best_folders(names):
    """Folders that split the files into a handful of groups: the shallowest level (1 to 3 folders deep) with 3 to 60 folders, none of
    them holding more than 60 % of the files. Returns the folder paths (original spelling), sorted; [] when no level fits."""
    parts = [str(n).replace("\\", "/").split("/") for n in names]
    for depth in (1, 2, 3):
        counts = {}
        for p in parts:
            if len(p) > depth:
                key = "/".join(p[:depth])
                counts[key] = counts.get(key, 0) + 1
        total = sum(counts.values())
        if 3 <= len(counts) <= 60 and total and max(counts.values()) / total <= 0.6:
            return sorted(counts, key=str.lower)
    return []


def series_number(path, series):
    """The group number of a file named <series>, <series>2, <series>3 ... (the bare name is 1); None for any other file."""
    if not series:
        return None
    stem = str(path).replace("\\", "/").rsplit("/", 1)[-1].rsplit(".", 1)[0].lower()
    s = series.lower()
    if stem == s:
        return 1
    m = re.fullmatch(rf"{re.escape(s)}[ _-]?0*(\d{{1,3}})", stem)
    return int(m.group(1)) if m else None


_STEM_RE = re.compile(r"^([a-z][a-z_]*?)[ _-]?0*(\d{1,3})?$")


def _best_series(paths):
    """The file name stem whose files form a numbered series (script, script2, script3 ...): (stem, numbers, files).
    A series needs the numbers 1, 2, 3 ... without a gap (at least three), and in a big project (more than 30 files) at least 15 % of
    all files: scene families such as `bedroom1`, `bedroom4`, `bedroom5` are not episodes."""
    fam = {}
    for p in paths:
        m = _STEM_RE.match(p.rsplit("/", 1)[-1].rsplit(".", 1)[0])
        if m:
            fam.setdefault(m.group(1), {})[int(m.group(2)) if m.group(2) else 1] = p
    best = ("", [], 0)
    for stem, nums in fam.items():
        ordered = sorted(nums)
        if len(ordered) < 3 or ordered != list(range(1, len(ordered) + 1)):
            continue
        if len(paths) > 30 and len(ordered) / len(paths) < 0.15:
            continue
        if (len(ordered), stem) > (len(best[1]), best[0]):
            best = (stem, ordered, len(ordered))
    return best


def propose(names):
    """Guess the groups of a project from its file names: {label, groups, sections, matched, total}.
    The word (episode, chapter, part, day, update, act) that names the most different numbers wins; `season N` in a name makes sections.
    Nothing is written. Empty groups = nothing recognisable."""
    paths = [str(n).replace("\\", "/").lower() for n in names]
    best = ("", [], 0)
    for word in GROUP_WORDS:
        rx = group_regex(word)
        nums, files = set(), 0
        for p in paths:
            found = {int(x) for x in rx.findall(p)}
            if found:
                nums |= found
                files += 1
        if len(nums) >= 2 and (len(nums), files) > (len(best[1]), best[2]):
            best = (word, sorted(nums)[:200], files)
    word, groups, matched = best
    stem, snums, sfiles = _best_series(paths)
    if len(snums) > len(groups):                        # script, script2, script3 ... beat a weak word match
        return {"label": "Episode", "groups": snums[:200], "sections": [{"n": 1, "groups": snums[:200]}], "matched": sfiles, "total": len(paths), "series": stem,
                "folders": _best_folders(names)}
    if not groups:
        return {"label": "Episode", "groups": [], "sections": [], "matched": 0, "total": len(paths), "series": "", "folders": _best_folders(names)}
    rx = group_regex(word)
    season_of_group = {}
    for p in paths:
        seasons = {int(x) for x in _SEASON_RE.findall(p)}
        found = {int(x) for x in rx.findall(p)}
        if len(seasons) == 1:
            for g in found:
                season_of_group.setdefault(g, next(iter(seasons)))
    if len(set(season_of_group.values())) >= 2:
        first = min(season_of_group.values())
        by_season = {}
        for g in groups:
            by_season.setdefault(season_of_group.get(g, first), []).append(g)
        sections = [{"n": n, "groups": gs} for n, gs in sorted(by_season.items())]
    else:
        sections = [{"n": 1, "groups": groups}]
    return {"label": word, "groups": groups, "sections": sections, "matched": matched, "total": len(paths), "series": "", "folders": _best_folders(names)}


def save_groups(data_dir, label, sections, series="", folders=()):
    """Write the groups into aside.json (other keys, such as `language`, stay). `sections`: [{"n": 1, "groups": [1, 2, 3]}].
    Returns the new Layout; raises LayoutError for numbers that cannot be used."""
    path = Path(data_dir) / FILE_NAME
    raw = {}
    if path.is_file():
        try:
            raw = json.loads(path.read_text(encoding="utf-8-sig"))
        except (OSError, ValueError):
            raw = {}
        if not isinstance(raw, dict):
            raw = {}
    labels = dict(raw.get("labels", {}) if isinstance(raw.get("labels"), dict) else {})
    labels["group"] = label
    raw.update(version=1, labels=labels, sections=[{"n": s["n"], "status": "active", "groups": list(s["groups"])} for s in sections])
    if series:
        raw["series"] = series
    else:
        raw.pop("series", None)
    if folders:
        raw["folders"] = list(folders)
    else:
        raw.pop("folders", None)
    lay = parse(raw)                                    # validates: raises LayoutError before anything is written
    atomic_write_text(path, json.dumps(raw, ensure_ascii=False, indent=1) + "\n")
    return lay
