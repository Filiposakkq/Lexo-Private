"""Suggesting which group (episode, chapter ...) a file belongs to, from its name, its folder and the assignments of its neighbours.

Works on plain data: the request carries the names, the current and known assignments and the layout of the project (`Layout.to_json()`), so it needs
nothing of the open project. It only proposes; the assignments are written by Core when the translator accepts them.
"""
import posixpath
import re
from collections import Counter, defaultdict

from lexo import layout
from lexo.messages import join, msg
from lexo.store import assign_label, valid_assign

EP_RE = re.compile(r"(?<![a-z0-9])(?:episode|ep)[ _-]?0*(\d{1,2})(?!\d)")
UPDATE_RE = re.compile(r"(?<![a-z0-9])update[ _-]?0*(\d{1,2})(?!\d)")
SEASON_RE = re.compile(r"(?<![a-z0-9])season[ _-]?0*(\d)(?!\d)")
# File stems that belong to the whole game, not to one episode.
SHARED_STEMS = {"header", "options", "screens", "gui", "common", "title_screens"}


def _posix(rel):
    return rel.replace("\\", "/").lower()


def _signals(rel, lay):
    p = _posix(rel)
    found = {int(m) for m in layout.group_regex(lay.labels.get("group", "Episode")).findall(p)}
    for n in (layout.series_number(p, lay.series),            # script, script2, script3 ... (a project setting)
              layout.folder_number(p, lay.folders)):          # one group per folder
        if n:
            found.add(n)
    eps = sorted(found)
    updates = sorted({int(m) for m in UPDATE_RE.findall(p)})
    seasons = sorted({int(m) for m in SEASON_RE.findall(p)})
    return eps, updates, seasons


def suggest(names, current, known_assign=None, lay=None):
    """Propose an assignment for every file in `names`.

    current:      {file: assignment} what the translator has set (manual values are protected)
    known_assign: the whole project's assignments, used to learn folder conventions

    Returns one dict per file:
      name, current, suggested ('' = none), confidence (high|medium|low|none),
      state: new | same | conflict | ambiguous | unknown | keep
      reason, candidates, overwrite (True when applying would replace a manual value)
    """
    lay = lay or layout.Layout()
    known = dict(known_assign if known_assign is not None else current)
    by_dir = defaultdict(Counter)
    for f, v in known.items():
        if v:
            by_dir[posixpath.dirname(_posix(f))][v] += 1

    out = []
    for name in names:
        cur = current.get(name, "")
        eps, updates, seasons = _signals(name, lay)
        valid = [e for e in eps if e in lay.active_groups]
        invalid = [e for e in eps if e not in lay.active_groups]
        season_dirs = [s for s in seasons if lay.groups_of(s)]
        allowed = {e for s in season_dirs for e in lay.groups_of(s)}
        sug, conf, reason, cands = "", "none", "", []
        season_conflict = False
        note = ""
        if invalid:
            note = msg(" (the episode number(s) {numbers} in the path are outside the active range 1–{max})", numbers=', '.join(map(str, invalid)), max=max(lay.active_groups, default=0))

        if len(valid) == 1:
            ep = valid[0]
            sug, conf = f"ep:{ep}", "high"
            reason = msg("The name or folder contains “ep{ep}”.", ep=ep)
            if allowed and ep not in allowed:
                conf, season_conflict = "low", True
                reason = (msg("The path contains episode {ep} (season {season}), but it lies in the folder of season {folders}.", ep=ep, season=lay.section_of(ep), folders=', '.join(map(str, season_dirs))))
        elif len(valid) > 1:
            cands = [f"ep:{e}" for e in valid]
            in_season = [e for e in valid if not allowed or e in allowed]
            if len(in_season) == 1 and allowed:
                sug, conf = f"ep:{in_season[0]}", "medium"
                reason = msg("There are several episodes in the path ({eps}); the season folder selects episode {ep}.", eps=', '.join(map(str, valid)), ep=in_season[0])
            else:
                reason = msg("Ambiguous: the path contains several episodes ({eps}).", eps=', '.join(map(str, valid)))
        elif len(updates) == 1 and updates[0] in lay.active_groups:
            ep = updates[0]
            sug, conf = f"ep:{ep}", "medium"
            reason = msg("The file “update{ep}” corresponds to episode {ep} (the convention of this project).", ep=ep)
            if allowed and ep not in allowed:
                conf, season_conflict = "low", True
                reason = join([reason, msg("The season folder does not fit this.")], " ")
        else:
            d = posixpath.dirname(_posix(name))
            sib = by_dir.get(d)
            stem = posixpath.splitext(posixpath.basename(_posix(name)))[0]
            if sib and len(sib) == 1 and next(iter(sib)):
                sug, conf = next(iter(sib)), "medium"
                reason = msg("The other files in the same folder are assigned as “{label}”.", label=assign_label(sug))
            elif stem in SHARED_STEMS:
                sug, conf = "gen", "medium"
                reason = msg("The name matches the files of the whole game (header, options, screens).")
            elif sib and len(sib) > 1:
                cands = [k for k, _ in sib.most_common()]
                reason = msg("The files in the same folder are assigned to different episodes.")
            elif allowed:
                reason = (msg("The folder belongs to season {seasons} (episodes {a}–{b}), the exact episode cannot be told from the path.", seasons=', '.join(map(str, season_dirs)), a=min(allowed), b=max(allowed)))
            else:
                reason = join([msg("Cannot be derived from the name or the folder."), note], "")
        if sug and not valid_assign(sug):
            sug, conf = "", "none"

        if not sug and not cands:
            state = "keep" if cur else "unknown"
        elif not sug:
            state = "keep" if cur else "ambiguous"
        elif season_conflict:
            state = "conflict"
        elif cur and cur == sug:
            state = "same"
        elif cur:
            state = "conflict"
            reason = join([reason, msg("It is set to “{label}” by hand.", label=assign_label(cur))], " ")
        else:
            state = "new"
        out.append({"name": name, "current": cur, "suggested": sug, "confidence": conf,
                    "state": state, "reason": reason, "candidates": cands,
                    "overwrite": bool(cur and sug and cur != sug)})
    return out
