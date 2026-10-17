"""Public progress data: preview, plan, publishing."""
from . import export
from .extensions import ExtensionError
from .messages import msg
from .messages import text_of
from .app_base import AppError


class PublishPart:
    # ------------------------------------------------------------ export / publish
    def export_preview(self):
        """Generate the public data and its checks; writes the local progress.json only."""
        self._need_project()
        with self.lock:
            data = self.build_export()
            problems = export.validate_progress(data)
            return {"data": data, "problems": problems, "path": str(self.local_dir / "progress.json")}

    def publish_plan(self):
        self._need_project()
        self._gate("publish.web")
        with self.lock:
            data = self.build_export()
            problems = export.validate_progress(data)
            if problems:
                raise AppError(msg("The export did not pass the check: {problems}", problems="; ".join(problems)))
            try:
                info = self.ext.call("publish.web", "plan", {"web": self.config.get("web", {}), "data": data, "text": export.render(data)})
            except ExtensionError as e:
                raise AppError(text_of(e)) from e
            info["summary"] = {"episodes": len(data["episodes"]), "files": data["files"]["total"],
                               "estimate": data["estimate"]["status"]}
            return info

    def publish_confirm(self, token, branch):
        self._need_project()
        self._gate("publish.web")
        with self.lock:
            data = self.build_export()
            problems = export.validate_progress(data)
            if problems:
                raise AppError(msg("The export did not pass the check: {problems}", problems="; ".join(problems)))
            try:
                return self.ext.call("publish.web", "publish", {"web": self.config.get("web", {}), "data": data, "text": export.render(data), "token": token, "branch": branch})
            except ExtensionError as e:
                raise AppError(text_of(e)) from e
