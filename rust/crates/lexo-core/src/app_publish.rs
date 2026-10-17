//! Port of `core/lexo/app_publish.py`: public progress data: preview, plan, publishing.
use crate::api::{ok, plain, Answer, Args};
use crate::app::App;
use crate::app_base::{app_err, Res};
use crate::export;
use crate::messages::msg;
use serde_json::{json, Value};

impl App {
    /// The public data and its checks as `(data, problems as text)`.
    fn checked_export(&self) -> Res<(Value, Vec<crate::messages::Message>)> {
        let data = self.build_export()?;
        let problems = export::validate_progress(&data, &self.o()?.store.layout);
        Ok((data, problems))
    }

    /// Generate the public data and its checks; writes the local progress.json only.
    pub fn export_preview(&self) -> Res<Value> {
        self.need_project()?;
        let (data, problems) = self.checked_export()?;
        Ok(
            json!({"data": data, "problems": problems.iter().map(|m| m.wire()).collect::<Vec<_>>(), "path": self.o()?.local_dir.join("progress.json").to_string_lossy()}),
        )
    }

    fn publish_call(&self, method: &str, extra: Value) -> Res<Value> {
        self.need_project()?;
        self.gate("publish.web")?;
        let (data, problems) = self.checked_export()?;
        if !problems.is_empty() {
            let joined = problems.iter().map(|m| m.text()).collect::<Vec<_>>().join("; ");
            return Err(app_err(msg("The export did not pass the check: {problems}", &[("problems", json!(joined))])));
        }
        let mut payload = json!({"web": self.config.get("web").cloned().unwrap_or_else(|| json!({})), "data": data, "text": export::render(&data)});
        if let (Some(p), Some(e)) = (payload.as_object_mut(), extra.as_object()) {
            p.extend(e.clone());
        }
        let mut info = self.ext_call("publish.web", method, &payload)?;
        if method == "plan" {
            info["summary"] =
                json!({"episodes": data["episodes"].as_array().map_or(0, Vec::len), "files": data["files"]["total"], "estimate": data["estimate"]["status"]});
        }
        Ok(info)
    }

    pub fn publish_plan(&self) -> Res<Value> {
        self.publish_call("plan", json!({}))
    }

    pub fn publish_confirm(&self, token: &str, branch: &str) -> Res<Value> {
        self.publish_call("publish", json!({"token": token, "branch": branch}))
    }
}

pub fn op_export_preview(app: &mut App, _a: &Args) -> Res<Answer> {
    ok(app.export_preview()?)
}

pub fn op_publish_plan(app: &mut App, _a: &Args) -> Res<Answer> {
    ok(app.publish_plan()?)
}

pub fn op_publish_confirm(app: &mut App, a: &Args) -> Res<Answer> {
    plain(app.publish_confirm(a.s("token"), a.s("branch"))?)
}
