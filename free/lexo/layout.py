"""Project layout: how a project's files are grouped (sections > groups), defined per project.

A *group* is a numbered unit the translator wants statistics for (an episode, a chapter, a route) and a
*section* is a set of groups (a season, a part). Both are optional: a project without a layout file has
no groups, and every file counts as "unassigned" in the overview.

The layout is read from `<project data folder>/project.json`:

    {"version": 1,
     "labels": {"section": "Season", "group": "Episode"},
     "sections": [{"n": 1, "status": "active",  "groups": [1, 2, 3, 4]},
                  {"n": 2, "status": "active",  "groups": [5, 6, 7, 8]},
                  {"n": 3, "status": "planned"}]}

`status: "planned"` means the section is shown but never counted. Group numbers must be unique across the
project. A broken file is reported and ignored (empty layout), it is never silently "repaired".

Everything that needs the layout reads it from here (`layout.ACTIVE_EPISODES`, `layout.SEASONS`, ...). The
names keep the vocabulary of the first game (episodes, seasons) until the UI is switched to the labels.
The layout of the open project is process-wide state set by `use()`; the application serves one project at a time.
"""
import json
from pathlib import Path

FILE_NAME = "project.json"
LEGACY_MARKERS = ("prirazeni.csv", "epizody.csv")      # data written by the BaDIK tool, before layouts existed

BADIK_SECTIONS = (
    {"n": 1, "status": "active", "episodes": (1, 2, 3, 4)},
    {"n": 2, "status": "active", "episodes": (5, 6, 7, 8)},
    {"n": 3, "status": "planned", "episodes": ()},
)
DEFAULT_LABELS = {"section": "Season", "group": "Episode"}


class Layout:
    def __init__(self, sections=(), labels=None):
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
        return {"version": 1, "labels": self.labels,
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
    return Layout(secs, {k: v for k, v in labels.items() if k in DEFAULT_LABELS})


def load(data_dir):
    """(Layout, note). note is "" or a message for the user (legacy data, broken file)."""
    path = Path(data_dir) / FILE_NAME
    if path.is_file():
        try:
            return parse(json.loads(path.read_text(encoding="utf-8-sig"))), ""
        except (OSError, ValueError) as e:             # LayoutError and JSONDecodeError are ValueErrors
            return Layout(), f"{FILE_NAME} was ignored: {e}"
    if any((Path(data_dir) / m).is_file() for m in LEGACY_MARKERS):
        return Layout(BADIK_SECTIONS), ""
    return Layout(), ""


_current = Layout()


def use(layout):
    global _current
    _current = layout


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
