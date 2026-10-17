"""Findings of the checks (validator, spelling, ignored findings), the overview and the estimates."""
import json
from datetime import date

from . import export, history as hist, layout, spelling, validator
from .messages import msg
from .project import STATUS_ORDER
from .project import file_status
from .project import totals_of
from .store import ep_states
from .store import text_hash
from .util import norm_ws, text_matcher


class ValidationPart:
    # ------------------------------------------------------------ validation
    def _length_cfg(self):
        return self.config.get("validation.length", {})

    def _terms(self):
        """Glossary terms {English: Czech} used by the term-consistency check."""
        return {c: g["n"] for c, g in self.store.glossary.items() if g["t"] == "term" and g["n"]}

    def _spell(self):
        """Spelling hints built from the whole project's translated text; rebuilt when any file or the glossary changes."""
        if not self.config.get("validation.spelling", True):
            return None
        key = (tuple(sorted((rel, id(fi.parsed)) for rel, fi in self.project.files.items())),
               tuple(sorted((c, g["n"]) for c, g in self.store.glossary.items())))
        if self._spell_cache and self._spell_cache[0] == key:
            return self._spell_cache[1]
        known = set()
        for c, g in self.store.glossary.items():
            known.update(spelling.words_of(c))
            known.update(spelling.words_of(g["n"]))
        targets = (e.target for fi in self.project.files.values() for e in fi.parsed.entries if not e.pending)
        obj = spelling.Spelling.build(targets, known)
        self._spell_cache = (key, obj)
        return obj

    def _raw_issues(self, rel):
        fi = self.project.files[rel]
        terms = self._terms()
        spell = self._spell()
        tb = self.config.get("validation.textbox", {})
        sig = json.dumps([self._length_cfg(), terms, tb, spell.sig if spell else ""], sort_keys=True)
        hit = self._val_cache.get(rel)
        if hit and hit[0] is fi.parsed and hit[1] == sig:
            return hit[2]
        res = validator.check_parsed(fi.parsed, self._length_cfg(), terms, spell, tb)
        self._val_cache[rel] = (fi.parsed, sig, res)
        return res

    # ------------------------------------------------------------ ignored findings
    def _ignored_mark(self, rel, e, iss):
        """The stored ignore of this finding if it still applies (same code, same translation text), else None."""
        if e is None:
            return None
        marks = self.store.ignored.get(rel)
        if not marks:
            return None
        m = marks.get((e.block, norm_ws(e.source if e.source is not None else e.target), iss.code))
        return m if m and m["h"] == text_hash(e.target) else None

    def _issues(self, rel, ignored=False):
        """Findings of a file; by default without the ones the translator ignored (ignored=True: only those)."""
        raw = self._raw_issues(rel)
        if not self.store.ignored.get(rel):
            return [] if ignored else raw
        return [r for r in raw if bool(self._ignored_mark(rel, r[2], r[1])) == ignored]

    def ignore_set(self, items, on=True, note=""):
        """Ignore (on=True) or stop ignoring findings. Items: [{file, line, code}] (or {file, key, code} to stop ignoring
        a finding whose line no longer exists). An ignore is bound to the current translation text: when the text
        changes the finding shows up again. One undo step. Returns {changed, skipped}."""
        self._need_project()
        with self.lock:
            plan, skipped = [], []
            for it in items:
                rel, code = it.get("file"), str(it.get("code", ""))
                if rel not in self.project.files:
                    skipped.append((rel, msg("unknown file")))
                    continue
                ents = self.project.entries(rel)
                e = None
                if it.get("line"):
                    e = next((x for x in ents if x.line == int(it["line"])), None)
                    key = None if e is None else (e.block, norm_ws(e.source if e.source is not None else e.target), code)
                else:
                    k = it.get("key") or ["", ""]
                    key = (str(k[0]), norm_ws(str(k[1])), code)
                    e = next((x for x in ents if (x.block, norm_ws(x.source if x.source is not None else x.target)) == key[:2]), None)
                if key is None:
                    skipped.append((rel, msg("the line changed in the file – refresh the files")))
                elif on and not any(r[2] is e and r[1].code == code for r in self._raw_issues(rel)):
                    skipped.append((rel, msg("this finding no longer exists")))
                else:
                    plan.append((rel, key, e))
            changed = 0
            clean_note = str(note or "").strip()[:300]

            def go():
                nonlocal changed
                today = date.today().isoformat()
                for rel, key, e in plan:
                    marks = self.store.ignored.setdefault(rel, {})
                    if on:
                        new = {"h": text_hash(e.target), "n": clean_note, "d": today}
                        if marks.get(key) != new:
                            marks[key] = new
                            changed += 1
                    elif marks.pop(key, None) is not None:
                        changed += 1
                    if not marks:
                        self.store.ignored.pop(rel, None)
            self.store.change(msg("Ignoring findings") if on else msg("Un-ignoring findings"), ("ignored",), go)
            out = self._changed("ignore")
            out.update(changed=changed, skipped=skipped)
            return out

    def ignored_list(self):
        """Everything the translator chose to ignore, with its current state:
        active (still hidden), expired (the translation changed, the finding is shown again),
        resolved (the finding no longer occurs) or gone (the line no longer exists)."""
        self._need_project()
        out = []
        for rel in sorted(self.store.ignored, key=str.lower):
            if rel not in self.project.files:
                for (blk, src, code), m in sorted(self.store.ignored[rel].items()):
                    out.append({"file": rel, "line": 0, "key": [blk, src], "code": code, "note": m["n"], "date": m["d"],
                                "state": "gone", "message": "", "severity": "", "source": src, "target": ""})
                continue
            ents = self.project.entries(rel)
            raw = self._raw_issues(rel)
            for (blk, src, code), m in sorted(self.store.ignored[rel].items()):
                e = next((x for x in ents if (x.block, norm_ws(x.source if x.source is not None else x.target)) == (blk, src)), None)
                rec = {"file": rel, "line": e.line if e else 0, "key": [blk, src], "code": code, "note": m["n"], "date": m["d"],
                       "message": "", "severity": "", "source": (e.source if e and e.source is not None else src),
                       "target": e.target if e else ""}
                if e is None:
                    rec["state"] = "gone"
                elif m["h"] != text_hash(e.target):
                    rec["state"] = "expired"
                else:
                    hit = next((r[1] for r in raw if r[2] is e and r[1].code == code), None)
                    rec["state"] = "active" if hit else "resolved"
                    if hit:
                        rec["message"], rec["severity"] = hit.message, hit.severity
                out.append(rec)
        return {"items": out, "counts": {s: sum(1 for r in out if r["state"] == s)
                                         for s in ("active", "expired", "resolved", "gone")}}

    def _all_issues(self):
        if not self.config.get("validation.enabled", True):
            return {}
        return {rel: self._issues(rel) for rel in self.project.files}

    def _duplicate_ids(self):
        """Duplicate `translate <lang> <id>` blocks across the project (Ren'Py rejects them)."""
        seen, dup = {}, []
        for rel in sorted(self.project.files):
            for lang, ident, line in self.project.files[rel].parsed.block_ids:
                k = (lang, ident)
                if k in seen:
                    dup.append((rel, line, seen[k]))
                else:
                    seen[k] = f"{rel}:{line}"
        return dup

    def validation_summary(self):
        counts = {"error": 0, "warning": 0, "info": 0, "length": 0}
        per_file = {}
        for rel, items in self._all_issues().items():
            c = {"error": 0, "warning": 0, "info": 0}
            for _line, iss, _e in items:
                if iss.category == "length":
                    counts["length"] += 1
                counts[iss.severity] += 1
                c[iss.severity] += 1
            per_file[rel] = c
        show_info = self.config.get("validation.show_info", False)
        counts["ignored"] = sum(1 for rel in self.project.files for _l, i, _e in self._issues(rel, True)
                               if i.severity != "info" or show_info) if self.config.get("validation.enabled", True) else 0
        dups = self._duplicate_ids() if self.config.get("validation.enabled", True) else []
        counts["error"] += len(dups)
        for rel, _l, _w in dups:
            per_file.setdefault(rel, {"error": 0, "warning": 0, "info": 0})["error"] += 1
        return counts, per_file

    def validation_list(self, severity="", category="", file="", q="", offset=0, limit=200, ignored="hide", group="", word=False):
        """Findings. ignored: "hide" (default) | "only" (just the ignored ones) | "all".
        group="line": one item per line with all its (matching) findings in `issues`; total counts lines."""
        self._need_project()
        out = []
        match = text_matcher(q, word)
        show_info = self.config.get("validation.show_info", False)
        enabled = self.config.get("validation.enabled", True)
        sev_counts = {"error": 0, "warning": 0, "info": 0}     # for the filter pills: every filter except the severity
        for rel in sorted(self.project.files) if enabled else []:
            if file and rel != file:
                continue
            rows = []
            if ignored in ("hide", "all"):
                rows += [(r, False) for r in self._issues(rel)]
            if ignored in ("only", "all"):
                rows += [(r, True) for r in self._issues(rel, True)]
            for (line, iss, e), is_ign in rows:
                if category and iss.category != category:
                    continue
                if match and not match(iss.message + " " + rel + " " + (e.target if e else "")):
                    continue
                sev_counts[iss.severity] += 1
                if severity and iss.severity != severity:
                    continue
                if not severity and iss.severity == "info" and not show_info:
                    continue
                out.append({"file": rel, "line": line, "severity": iss.severity, "code": iss.code,
                            "category": iss.category, "message": iss.message,
                            "source": e.source if e else "", "target": e.target if e else "",
                            "who": e.who if e else "", "extra": iss.extra,
                            "src_who": e.src_who if e else "", "ignored": is_ign,
                            "ignore_note": (self._ignored_mark(rel, e, iss) or {}).get("n", "") if is_ign else ""})
        if not category and not q and ignored != "only":
            for rel, line, first in self._duplicate_ids():
                if not file or rel == file:
                    sev_counts["error"] += 1
                    if severity and severity != "error":
                        continue
                    out.append({"file": rel, "line": line, "severity": "error", "code": "duplicate-id",
                                "category": "structure", "message": msg("A translate block with the same identifier already exists ({first}); Ren'Py will reject it.", first=first),
                                "source": "", "target": "", "who": "", "extra": {}})
        order = {"error": 0, "warning": 1, "info": 2}
        out.sort(key=lambda r: (order[r["severity"]], r["file"].lower(), r["line"]))
        if group == "line":
            lines = {}
            for r in out:                                   # `out` is sorted by severity, so the first finding is the worst
                g = lines.get((r["file"], r["line"]))
                if g is None:
                    g = lines[(r["file"], r["line"])] = {"file": r["file"], "line": r["line"], "severity": r["severity"],
                                                          "source": r["source"], "target": r["target"], "who": r["who"],
                                                          "src_who": r.get("src_who", ""), "issues": []}
                g["issues"].append({k: r[k] for k in ("severity", "code", "category", "message", "extra", "ignored", "ignore_note")
                                    if k in r})
            grouped = sorted(lines.values(), key=lambda g: (order[g["severity"]], g["file"].lower(), g["line"]))
            return {"total": len(grouped), "findings": len(out), "sev_counts": sev_counts, "items": grouped[offset:offset + limit]}
        return {"total": len(out), "sev_counts": sev_counts, "items": out[offset:offset + limit]}

    # ------------------------------------------------------------ overview / files
    def estimate(self):
        rows = self.project.rows()
        t_done, total, *_ = totals_of(rows)
        est = hist.estimate(self.history.points(), t_done, total)
        for k in ("eta", "eta_early", "eta_late"):
            est[k] = est[k].isoformat() if est[k] else None
        return est

    def goals(self):
        """Weekly goals and this week's progress (see history.goals)."""
        rows = self.project.rows()
        t_done, total, c_done, *_ = totals_of(rows)
        return hist.goals(self.history.points(), max(0, total - t_done), max(0, total - c_done), self.config.get("goals", {}))

    def overview(self):
        self._need_project()
        rows = self.project.rows()
        t_done, total, c_done, tpct, cpct = totals_of(rows)
        counts = {k: 0 for k in STATUS_ORDER}
        for r in rows:
            counts[file_status(r.done, r.todo, r.total, r.corr)] += 1
        approved = sum(len(v) for v in self.store.approved.values())
        marked = flagged = stale = 0
        for rel in self.store.proof:
            if rel in self.project.files:
                ok, fix = self.project.proof_counts(rel)
                marked, flagged = marked + ok, flagged + fix
                stale += self.project.proof_stale(rel)
        stats = export.stats_for(rows, self.store)
        est_now = self.estimate()
        pace = est_now.get("pace_per_day")
        for e in stats["episodes"]:                       # how long an episode would take at the current pace
            left = e["translation"]["total"] - e["translation"]["done"]
            e["eta_days"] = int(-(-left // pace)) if pace and left > 0 else None
        vcounts, per_file = self.validation_summary()
        pts = self.history.points()
        recent = []
        prev = None
        for ts, done, tot, corr in pts[-40:]:
            if prev is not None and (done != prev[1] or corr != prev[3]):
                recent.append({"at": ts.strftime(hist.STAMP_FORMAT), "translated": done - prev[1],
                               "proofread": corr - prev[3], "total_translated": done})
            prev = (ts, done, tot, corr)
        return {
            "project": {"path": str(self.project.root), "data_dir": str(self.data_dir), "local_dir": str(self.local_dir),
                        "files": len(rows), "skipped": sorted(self.project.skip)},
            "totals": {"done": t_done, "total": total, "remaining": total - t_done, "percent": round(tpct, 1),
                       "proofread": c_done, "proofread_percent": round(cpct, 1), "approved": approved,
                       "proof_marked": marked, "proof_fix": flagged, "proof_stale": stale},
            "file_counts": counts,
            "seasons": stats["seasons"], "episodes": stats["episodes"], "labels": dict(layout.current().labels),
            "groups_defined": bool(layout.ACTIVE_EPISODES),
            "group_names": {str(i): f.rsplit("/", 1)[-1] for i, f in enumerate(layout.current().folders, 1)},
            "general": stats["general"], "unassigned": stats["unassigned"],
            "validation": vcounts, "worst_files": self._worst_files(per_file), "goals": self.goals(),
            "estimate": est_now,
            "recent": recent[-8:][::-1],
            "undo": [s["label"] for s in self.store.undo_stack][-5:][::-1],
            "unsaved": sorted(self.store.unsaved),
            "last_save_error": self.last_save_error,
            "last_refresh": self.project.last_report.to_dict() if self.project.last_report else None,
            "lock_lost": bool(self.plock and not self.plock.check()),
            "loaded_at": self.project.loaded_at,
            "episode_states": [{"key": k, "label": l} for k, l in ep_states()],
        }
