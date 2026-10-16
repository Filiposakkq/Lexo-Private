"""Application core: the open project, refresh, persistence of derived files, settings, features, links."""
import json
import shutil
import threading
import webbrowser
from datetime import datetime
from pathlib import Path

from . import datadir, editor, export, features, history as hist, layout, lint, migrate, mt, publish as pub, texthistory
from .config import Config
from .config import migrate_web_json
from .lock import ProjectLock
from .lock import ProjectLocked
from .messages import msg
from .messages import text_of
from .project import DATA_DIR_NAME
from .project import Project
from .project import ProjectUnavailable
from .project import check_project_dir
from .project import totals_of
from .store import Store
from .util import WriteFailed
from .util import write_csv
from .app_base import AppError, FeatureLocked, LEGACY_FILES, LINKS, ProjectBusy, UI_PREF_MAX


class CorePart:
    def __init__(self, config=None):
        self.config = config or Config().load()
        self.entitlements = features.default()
        self.lock = threading.RLock()
        self.project = None
        self.store = None
        self.history = None
        self.data_dir = None       # shared data: <translation folder>/.lexo
        self.local_dir = None      # local data: backups, snapshots, lock, derived files (datadir.py)
        self.notices = []          # one-off messages for the UI (migration, load problems...)
        self._spell_cache = None
        self._memory_cache = None
        self.texthist = None       # TextHistory of the open project
        self.linter = lint.Linter()
        self._mt_http = None       # replaced in tests
        self._gi = None            # (game folder, GameImages)
        self._val_cache = {}       # rel -> (digest, cfg signature, [(line, Issue, entry)])
        self.last_save_error = ""
        self.plock = None          # ProjectLock of the open project

    # ------------------------------------------------------------ project lifecycle
    def close(self):
        """Release the project lock (called on exit)."""
        if self.plock:
            self.plock.release()
            self.plock = None

    def open_project(self, path, force=False):
        ok, problem = check_project_dir(path)
        if not ok:
            raise AppError(problem)
        root = Path(path).resolve()
        data_dir = root / DATA_DIR_NAME
        try:
            data_dir.mkdir(exist_ok=True)
        except OSError as e:
            raise AppError(msg("Cannot create {folder} in the project folder ({reason}).", folder=DATA_DIR_NAME, reason=e.strerror or e)) from e
        local_dir = datadir.local_dir(data_dir, root)
        for name in LEGACY_FILES:                       # data of the oldest versions lying next to the .rpy files
            src = root / name
            dst = (local_dir if name.startswith("progress.") else data_dir) / name       # progress.* is derived data
            if src.is_file() and not dst.exists():
                try:
                    shutil.move(str(src), str(dst))
                    self.notices.append(msg("The file {name} was moved to {folder}.", name=name, folder=dst.parent.name))
                except OSError:
                    pass
        data_notes = datadir.migrate_folder(root, data_dir, local_dir)
        with self.lock:
            new_lock = ProjectLock(local_dir)
            try:
                new_lock.acquire(force=force)
            except ProjectLocked as e:
                raise ProjectBusy(str(e), root) from e
            if self.plock:
                self.plock.release()
            self.plock = new_lock
            self.notices = [n for n in self.notices if n]
            note = migrate_web_json(self.config, data_dir)
            if note:
                self.notices.append(note)
            self.data_dir = data_dir
            self.local_dir = local_dir
            self.notices += data_notes
            lay, lay_note = layout.load(data_dir)
            layout.use(lay)
            if lay_note:
                self.notices.append(lay_note)
            self.notices += migrate.run(data_dir)
            self.store = Store(data_dir, local_dir).load()
            self.notices += self.store.warnings
            self.project = Project(root, self.store, skip=self.config.get("skip_files", []),
                                   symbols_as_done=self.config.get("count_symbol_only_as_done"))
            self.history = hist.History(data_dir / "history.csv", local_dir / "backups")
            self.texthist = texthistory.TextHistory(local_dir / texthistory.FILE_NAME).load()
            self._val_cache.clear()
            report = self.project.refresh(force=True)
            self.store.ensure_files([r.name for r in self.project.rows()])
            self.config.set("project_dir", str(root))
            try:
                self.config.save()
            except WriteFailed as e:
                self.notices.append(msg("The settings could not be saved: {reason}", reason=e.reason))
            self.persist()
            return report

    @property
    def ready(self):
        return self.project is not None

    def _need_project(self):
        if not self.project:
            raise AppError(msg("No project is open."))

    # ------------------------------------------------------------ persistence of derived files
    def persist(self):
        """Write progress.csv, the history row and the local progress.json. Returns a list of problems."""
        self._need_project()
        problems = []
        rows = self.project.rows()
        try:
            write_csv(self.local_dir / "progress.csv",
                      [["file", "translated", "remaining", "total", "proofread", "translation_%", "proofreading_%"]]
                      + [[r.name, r.done, r.todo, r.total, r.corr, f"{r.done / r.total * 100:.1f}",
                          f"{r.corr / r.total * 100:.1f}"] for r in rows])
        except WriteFailed as e:
            problems.append(str(e))
        t_done, total, c_done, tpct, cpct = totals_of(rows)
        try:
            self.history.record(datetime.now().strftime(hist.STAMP_FORMAT), t_done, total, tpct, c_done, cpct)
        except WriteFailed as e:
            problems.append(str(e))
        try:
            export.write_local(self.local_dir / "progress.json", self.build_export())
        except (WriteFailed, ValueError) as e:
            problems.append(str(e))
        self.last_save_error = "; ".join(problems)
        return problems

    def build_export(self):
        return export.build_progress(self.project.rows(), self.store, self.history,
                                     title=self.config.get("web.title", ""))

    # ------------------------------------------------------------ refresh
    def refresh(self, force=False, client_dirty=False, discard=False):
        """Rescan the project folder without restarting. Returns a dict for the UI.

        Nothing the translator typed is dropped by a refresh, but if there is something not yet
        saved (a failed save, or an edit still open in the UI) the caller must confirm first."""
        self._need_project()
        with self.lock:
            retried = []
            for kind in sorted(self.store.unsaved):
                try:
                    self.store.save(kind)
                    retried.append(kind)
                except WriteFailed:
                    pass
            if (self.store.unsaved or client_dirty) and not discard:
                return {"needs_confirm": True,
                        "message": msg("You have unsaved changes{what}. Refreshing the files does not discard them from memory, but fields being edited will be overwritten. Continue?",
                                       what=(msg(" ({files} could not be written)", files=", ".join(sorted(self.store.unsaved)))
                                             if self.store.unsaved else msg(" (a field being edited)")))}
            try:
                rep = self.project.refresh(force=force)
            except ProjectUnavailable as e:
                raise AppError(text_of(e)) from e
            self._log_external(rep.prev)
            moved = self.project.migrate_metadata(rep.renamed) if rep.renamed else 0
            self.store.ensure_files([r.name for r in self.project.rows()])
            problems = self.persist() if rep.changed else []
            out = rep.to_dict()
            out.update(moved_metadata=moved, save_problems=problems, retried=retried, needs_confirm=False)
            return out

    # ------------------------------------------------------------ gated (Pro) features
    def features_status(self):
        return {"ok": True, "tier": self.entitlements.source, "features": self.entitlements.status()}

    def _gate(self, feature):
        d = self.entitlements.decide(feature)
        if not d.available:
            raise FeatureLocked(feature)
        return d

    def _cap(self, feature, seq):
        """Gate `feature` and cut `seq` to the free limit. Returns (items, info); info says what was cut, for the UI."""
        d = self._gate(feature)
        seq = list(seq)
        if d.limit is None or len(seq) <= d.limit:
            return seq, {}
        return seq[:d.limit], {"limited": {"feature": feature, "limit": d.limit, "total": len(seq)}}

    def undo(self):
        self._need_project()
        with self.lock:
            label = self.store.undo()
            if label is None:
                raise AppError(msg("Nothing to undo."))
            self._val_cache.clear()
            out = self._changed("undo")
            out["undone"] = label
            return out

    def open_link(self, link_id, opener=None):
        """Open one of the fixed project links in the default browser."""
        url = LINKS.get(link_id)
        if url is None:
            raise AppError(msg("Unknown link."))
        (opener or webbrowser.open)(url)
        return {"ok": True, "url": url}

    # ------------------------------------------------------------ editor
    def open_editor(self, rel, line):
        self._need_project()
        try:
            path = self.project.path_of(rel)
        except ValueError as e:
            return {"ok": False, "message": str(e)}
        if rel not in self.project.files:
            return {"ok": False, "message": msg("Unknown file.")}
        return editor.open_in_editor(path, line, self.config.get("editor", {}), root=self.project.root)

    # ------------------------------------------------------------ settings / prefs
    def settings(self):
        d = self.config.public()
        d["ui"] = {}
        d["game_gui"] = self.game_gui()
        return d

    def update_settings(self, patch):
        """Validate and store a settings patch. Only known keys are accepted."""
        clean = {}
        ed = patch.get("editor")
        if isinstance(ed, dict):
            pref = str(ed.get("preferred", "auto"))
            if pref not in ("auto", "vscode", "notepadpp", "custom", "system"):
                raise AppError(msg("Invalid editor choice."))
            clean["editor"] = {"preferred": pref, "custom": str(ed.get("custom", ""))[:500]}
        va = patch.get("validation")
        if isinstance(va, dict):
            out = {}
            if "enabled" in va:
                out["enabled"] = bool(va["enabled"])
            if "show_info" in va:
                out["show_info"] = bool(va["show_info"])
            if "spelling" in va:
                out["spelling"] = bool(va["spelling"])
            tbx = va.get("textbox")
            if isinstance(tbx, dict):
                to = {}
                if "enabled" in tbx:
                    to["enabled"] = bool(tbx["enabled"])
                for k, lo_, hi in (("chars_per_line", 20, 400), ("max_lines", 1, 30)):
                    if k in tbx:
                        try:
                            val = int(float(tbx[k]))
                        except (TypeError, ValueError):
                            raise AppError(msg("Invalid value: {k}", k=k)) from None
                        if not lo_ <= val <= hi:
                            raise AppError(msg("{k} must be between {lo} and {hi}.", k=k, lo=lo_, hi=hi))
                        to[k] = val
                out["textbox"] = to
            ln = va.get("length")
            if isinstance(ln, dict):
                lo = {}
                for k, lo_, hi in (("min_source_chars", 0, 500), ("max_ratio", 1.0, 20.0), ("min_ratio", 0.0, 1.0),
                                   ("min_extra_chars", 0, 1000), ("min_missing_chars", 0, 1000)):
                    if k in ln:
                        try:
                            val = float(ln[k])
                        except (TypeError, ValueError):
                            raise AppError(msg("Invalid value: {k}", k=k)) from None
                        if not lo_ <= val <= hi:
                            raise AppError(msg("{k} must be between {lo} and {hi}.", k=k, lo=lo_, hi=hi))
                        lo[k] = int(val) if k in ("min_source_chars", "min_extra_chars", "min_missing_chars") else val
                if "enabled" in ln:
                    lo["enabled"] = bool(ln["enabled"])
                out["length"] = lo
            clean["validation"] = out
        sn = patch.get("snapshots")
        if isinstance(sn, dict):
            o = {}
            if "enabled" in sn:
                o["enabled"] = bool(sn["enabled"])
            for k, lo, hi in (("interval_hours", 1, 168), ("keep", 3, 100)):
                if k in sn:
                    try:
                        val = int(float(sn[k]))
                    except (TypeError, ValueError):
                        raise AppError(msg("Invalid value: {k}", k=k)) from None
                    if not lo <= val <= hi:
                        raise AppError(msg("{k} must be between {lo} and {hi}.", k=k, lo=lo, hi=hi))
                    o[k] = val
            clean["snapshots"] = o
        m = patch.get("mt")
        if isinstance(m, dict):
            cur = dict(self.config.get("mt", {}))
            o = {}
            if "provider" in m:
                if m["provider"] not in ("",) + mt.PROVIDERS:
                    raise AppError(msg("Unknown machine translation provider."))
                o["provider"] = m["provider"]
            for k, n in (("base_url", 300), ("model", 100), ("source_lang", 10), ("target_lang", 10)):
                if k in m:
                    o[k] = str(m[k]).strip()[:n]
            if o.get("base_url"):
                try:
                    mt._check_url(o["base_url"])
                except mt.MTError as e:
                    raise AppError(text_of(e)) from e
            if "consent" in m:
                o["consent"] = bool(m["consent"])
            if m.get("clear_key"):
                o["api_key"] = ""
            elif str(m.get("api_key", "")).strip():
                o["api_key"] = str(m["api_key"]).strip()[:300]          # an empty field keeps the stored key
            clean["mt"] = {**cur, **o}
        li = patch.get("lint")
        if isinstance(li, dict) and "sdk" in li:
            clean["lint"] = {"sdk": str(li["sdk"]).strip().strip('"')[:500]}
        gl = patch.get("goals")
        if isinstance(gl, dict):
            g = {}
            for k in ("translate_per_week", "proofread_per_week"):
                if k in gl:
                    try:
                        val = int(float(gl[k] or 0))
                    except (TypeError, ValueError):
                        raise AppError(msg("Invalid goal: {k}", k=k)) from None
                    if not 0 <= val <= 100000:
                        raise AppError(msg("The goal must be between 0 and 100 000 lines per week (0 = automatic)."))
                    g[k] = val
            clean["goals"] = g
        web = patch.get("web")
        if isinstance(web, dict):
            w = {}
            for k in ("repo", "path", "title"):
                if k in web:
                    w[k] = str(web[k]).strip()[:300]
            if "path" in w:
                try:
                    w["path"] = pub.clean_target(w["path"])
                except pub.PublishError as e:
                    raise AppError(text_of(e)) from e
            clean["web"] = w
        if "skip_files" in patch:
            clean["skip_files"] = [str(x).strip() for x in patch["skip_files"] if str(x).strip()][:200]
        with self.lock:
            try:
                self.config.update(clean)
            except WriteFailed as e:
                raise AppError(msg("The settings cannot be saved: {reason}", reason=e.reason)) from e
            self._val_cache.clear()
            if "skip_files" in clean and self.project:
                self.project.skip = {s.lower() for s in clean["skip_files"]}
        return self.settings()

    def ui_prefs(self):
        return self.config.get("ui", {})

    def set_ui_prefs(self, prefs):
        if not isinstance(prefs, dict) or len(json.dumps(prefs)) > UI_PREF_MAX:
            raise AppError(msg("Invalid preferences."))
        with self.lock:
            self.config.set("ui", prefs)
            try:
                self.config.save()
            except WriteFailed:
                pass
        return {"ok": True}
