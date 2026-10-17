"""Snapshots (zip backups) of the translation folder."""
import threading

from . import __version__, snapshots, texthistory
from .messages import msg
from .messages import text_of
from .app_base import AppError


class SnapshotsPart:
    # ------------------------------------------------------------ snapshots of the translation folder
    def _snapshot(self, reason, force=False):
        """Zip of all .rpy files (see snapshots.py). Never raises; problems become a notice."""
        cfg = self.config.get("snapshots", {})
        if not force and not cfg.get("enabled", True):
            return None
        if reason == "bulk" and snapshots.recent(self.local_dir, 10):
            return None
        try:
            snap = snapshots.create(self.project.root, self.local_dir, reason, __version__)
            if snap:
                snapshots.prune(self.local_dir, cfg.get("keep", 20))
            return snap
        except (snapshots.SnapshotError, OSError) as e:
            self.notices.append(msg("The backup was not created: {reason}", reason=e))
            return None

    def _snapshot_for_bulk(self, lines, files):
        if lines >= 20 or files >= 2:
            self._snapshot("bulk")

    def maybe_auto_snapshot(self):
        """At start: a snapshot in the background when the last one is older than the interval (called by the UI server)."""
        if not self.project or not self.config.get("snapshots.enabled", True):
            return
        interval = self.config.get("snapshots.interval_hours", 12)
        if not snapshots.due(self.local_dir, interval):
            return
        threading.Thread(target=lambda: self._snapshot("auto"), daemon=True).start()

    def snapshot_list(self):
        self._need_project()
        cfg = self.config.get("snapshots", {})
        return {"items": snapshots.list_snapshots(self.local_dir), "enabled": cfg.get("enabled", True),
                "interval_hours": cfg.get("interval_hours", 12), "keep": cfg.get("keep", 20)}

    def snapshot_create(self):
        self._need_project()
        with self.lock:
            try:
                snap = snapshots.create(self.project.root, self.local_dir, "manual", __version__)
            except snapshots.SnapshotError as e:
                raise AppError(text_of(e)) from e
            if snap is None:
                raise AppError(msg("There are no .rpy files in the folder."))
            snapshots.prune(self.local_dir, self.config.get("snapshots.keep", 20))
            return {"ok": True, "snapshot": snap}

    def snapshot_files(self, name):
        self._need_project()
        try:
            return {"files": snapshots.compare(self.project.root, self.local_dir, name)}
        except (snapshots.SnapshotError, OSError) as e:
            raise AppError(text_of(e)) from e

    def snapshot_restore(self, name, files=None):
        """Put files of a snapshot back. The current state is saved first, so the restore can be undone the same way."""
        self._need_project()
        with self.lock:
            before = {rel: list(self.project.entries(rel)) for rel in self.project.files}
            try:
                res = snapshots.restore(self.project.root, self.local_dir, name, files, __version__)
            except snapshots.SnapshotError as e:
                raise AppError(text_of(e)) from e
            finally:
                self.project.refresh(force=True)
                self._val_cache.clear()
            for rel in self.project.files:
                if rel in before:
                    ch = texthistory.diff_files(before[rel], self.project.entries(rel))
                    self._log_text([{"file": rel, "key": c["key"], "old": c["old"], "new": c["new"]} for c in ch], "restore")
            self.store.ensure_files([r.name for r in self.project.rows()])
            out = self._changed("restore")
            out.update(restored=res["restored"], safety=(res["safety"] or {}).get("name", ""))
            return out

    def snapshot_delete(self, name):
        self._need_project()
        try:
            snapshots.delete(self.local_dir, name)
        except snapshots.SnapshotError as e:
            raise AppError(text_of(e)) from e
        return {"ok": True}
