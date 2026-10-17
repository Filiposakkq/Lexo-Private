"""Public progress data: preview, plan, publishing."""
from . import export, publish as pub
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
                info = pub.plan(self.config.get("web", {}), data, export.render(data))
            except pub.PublishError as e:
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
                return pub.publish(self.config.get("web", {}), data, export.render(data), token, branch)
            except pub.PublishError as e:
                raise AppError(text_of(e)) from e
