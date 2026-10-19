//! Port of `reference/python/core/lexo/app_assign.py`: assigning files to groups (episodes), episode states, approving pending lines.
use crate::api::{ok, plain, Answer, Args};
use crate::app::App;
use crate::app_base::{app_err, ApiError, Res};
use crate::bulk::{self, Assignment, Selection};
use crate::layout::{self, GROUP_WORDS};
use crate::messages::{msg, py_str, Text};
use crate::store::{assign_label, valid_assign, Kind, Label, EP_STATE_KEYS};
use crate::util::{py_strip, truthy};
use serde_json::{json, Map, Value};
use std::path::Path;

fn base_name(name: &str) -> String {
    Path::new(name).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// `int(g)` for what a client can send as a group number.
fn py_int(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().filter(|f| f.is_finite()).map(|f| f.trunc() as i64)),
        Value::Bool(b) => Some(*b as i64),
        Value::String(s) => py_strip(s).replace('_', "").parse().ok(),
        _ => None,
    }
}

impl App {
    pub fn set_assign(&mut self, name: &str, v: &str) -> Res<Value> {
        self.need_project()?;
        let clock = &*self.clock;
        let o = self.open.as_mut().unwrap();
        if !o.project.files.contains_key(name) || !valid_assign(v, &o.store.layout) {
            return Err(app_err(msg("Invalid value.", &[])));
        }
        let label = Label::Msg(msg("Assignment of {name}", &[("name", json!(base_name(name)))]));
        o.store.change(
            label,
            &[Kind::Assign],
            |st| {
                if v.is_empty() {
                    st.assign.shift_remove(name);
                } else {
                    st.assign.insert(name.to_string(), v.to_string());
                }
            },
            clock,
        )?;
        Ok(Value::Object(self.changed("assign")?))
    }

    pub fn set_episode_state(&mut self, n: i64, state: &str) -> Res<Value> {
        self.need_project()?;
        let clock = &*self.clock;
        let o = self.open.as_mut().unwrap();
        if !o.store.layout.active_groups().contains(&n) || !EP_STATE_KEYS.contains(&state) {
            return Err(app_err(msg("Invalid value.", &[])));
        }
        o.store.change(
            Label::Msg(msg("State of group {n}", &[("n", json!(n))])),
            &[Kind::Episodes],
            |st| {
                st.episodes.insert(n, state.to_string());
            },
            clock,
        )?;
        Ok(Value::Object(self.changed("episode")?))
    }

    /// Suggested group for files (a Pro feature: the Pro pack suggests, the app shows the groups to choose from).
    pub fn assign_suggest(&mut self, names: &[Value]) -> Res<Value> {
        self.need_project()?;
        let o = self.o()?;
        let names: Vec<String> = if names.is_empty() {
            let mut v: Vec<String> = o.project.files.keys().cloned().collect();
            v.sort_by_key(|r| r.to_lowercase());
            v
        } else {
            names.iter().map(py_str).collect()
        };
        let unknown: Vec<&String> = names.iter().filter(|n| !o.project.files.contains_key(*n)).collect();
        if !unknown.is_empty() {
            let shown = unknown.iter().take(3).map(|s| s.as_str()).collect::<Vec<_>>().join(", ");
            return Err(app_err(msg("Unknown files: {names}", &[("names", json!(shown))])));
        }
        let assign: Map<String, Value> = o.store.assign.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
        let payload = json!({"names": names, "current": assign, "known": assign, "layout": o.store.layout.to_json()});
        let res = self.ext_call("bulk.categorize", "suggest", &payload)?;
        let o = self.o()?;
        let layout = &o.store.layout;
        let many = layout.sections.len() > 1;
        let mut out: Map<String, Value> = res.as_object().cloned().unwrap_or_default();
        out.insert("groups_defined".into(), json!(!layout.active_groups().is_empty()));
        let mut options =
            vec![json!({"value": "", "label": msg("Unassigned", &[]).wire()}), json!({"value": "gen", "label": msg("General (the whole game)", &[]).wire()})];
        for n in layout.active_groups() {
            let mut parts = vec![assign_label(&format!("ep:{n}"), layout).wire()];
            parts.push(if many { msg(" (season {season})", &[("season", json!(layout.section_of(n)))]).wire() } else { json!("") });
            options.push(json!({"value": format!("ep:{n}"), "label": {"$j": parts, "sep": "", "end": ""}}));
        }
        out.insert("options".into(), Value::Array(options));
        Ok(Value::Object(out))
    }

    /// The groups the project has now and the groups its file names suggest (episode, chapter, part, day, update, act). Nothing is changed.
    pub fn groups_suggest(&self) -> Res<Value> {
        let o = self.o()?;
        let cur = &o.store.layout;
        let mut names: Vec<String> = o.project.files.keys().cloned().collect();
        names.sort_by_key(|r| r.to_lowercase());
        Ok(json!({
            "current": {"label": cur.labels.get("group").map_or("Episode", String::as_str), "groups": cur.active_groups(), "sections": cur.sections.len(), "series": cur.series, "folders": cur.folders},
            "proposal": layout::propose(&names).to_json(),
            "words": GROUP_WORDS.iter().map(|(w, _)| *w).collect::<Vec<_>>(),
        }))
    }

    /// Define the groups of the project (one section) and write them to aside.json. Files keep their assignments; assignments to a group that no longer exists stay in the
    /// data but are not counted until the group is back.
    pub fn groups_apply(&mut self, label: &Value, groups: &Value, series: &str, folders: &Value) -> Res<Value> {
        self.need_project()?;
        let mut label = if truthy(label) { py_strip(&py_str(label)).to_string() } else { String::new() };
        let mut folder_list: Vec<String> = Vec::new();
        if truthy(folders) {
            for f in folders.as_array().into_iter().flatten() {
                let s = py_str(f);
                let s = py_strip(&s).trim_matches('/').to_string();
                if !s.is_empty() {
                    folder_list.push(s);
                }
            }
        }
        let mut groups = groups.clone();
        let mut series = series.to_string();
        if !folder_list.is_empty() {
            // one group per folder: the numbers are 1..N and the label is "Folder"
            label = "Folder".to_string();
            groups = json!((1..=folder_list.len()).collect::<Vec<_>>());
            series = String::new();
        }
        if !GROUP_WORDS.iter().any(|(w, _)| *w == label) {
            return Err(app_err(msg("Choose what the groups are called.", &[])));
        }
        let bad_groups = || app_err(msg("The groups must be positive whole numbers, for example 1–8.", &[]));
        let mut nums: Vec<i64> = match &groups {
            Value::Array(a) => a.iter().map(|g| py_int(g).ok_or_else(bad_groups)).collect::<Result<_, _>>()?,
            _ => return Err(bad_groups()),
        };
        nums.sort_unstable();
        nums.dedup();
        if nums.is_empty() || nums[0] < 1 || nums[nums.len() - 1] > 999 {
            return Err(bad_groups());
        }
        if self.o()?.store.layout.sections.len() > 1 {
            return Err(app_err(msg("This project has several sections (seasons) in aside.json; edit the file by hand to change them.", &[])));
        }
        let clock = &*self.clock;
        let o = self.open.as_mut().unwrap();
        let lay = layout::save_groups(&o.data_dir, &label, &[(1, nums.clone())], &series, &folder_list, clock)
            .map_err(|e| app_err(msg("The groups could not be saved: {reason}", &[("reason", json!(e.0))])))?;
        let out = json!({"groups": nums, "label": label, "series": lay.series});
        o.store.layout = lay;
        self.persist()?;
        Ok(out)
    }

    pub fn assign_apply(&mut self, items: &[Value]) -> Res<Value> {
        self.need_project()?;
        for it in items {
            let name = it.get("name").map(py_str).unwrap_or_default();
            if !it.get("name").and_then(Value::as_str).is_some_and(|n| self.open.as_ref().is_some_and(|o| o.project.files.contains_key(n))) {
                let shown = it.get("name").map_or("None".to_string(), |_| name);
                return Err(app_err(msg("Unknown file: {name}", &[("name", json!(shown))])));
            }
        }
        let list: Vec<Assignment> = items
            .iter()
            .map(|i| Assignment {
                name: i["name"].as_str().unwrap_or("").to_string(),
                value: i.get("value").map(py_str).unwrap_or_default(),
                overwrite: i.get("overwrite").is_some_and(truthy),
            })
            .collect();
        let clock = &*self.clock;
        let o = self.open.as_mut().unwrap();
        let (applied, skipped) = bulk::apply_assignments(&mut o.store, &list, clock)?;
        let mut out = self.changed("bulk-assign")?;
        out.insert("applied".into(), json!(applied));
        out.insert("skipped".into(), json!(skipped.iter().map(|(n, m)| json!([n, m.wire()])).collect::<Vec<_>>()));
        Ok(Value::Object(out))
    }

    pub fn approval_preview(&self, file: &str, line: i64, scope: &str, same_speaker: bool) -> Res<Value> {
        let o = self.o()?;
        bulk::approval_candidates(&o.project, &o.store, file, line.max(0) as usize, scope, same_speaker).map_err(|m| ApiError::App(Text::new_msg(m)))
    }

    pub fn rule_preview(&self, rules: &Value, files: &[Value]) -> Res<Value> {
        let o = self.o()?;
        let files: Vec<String> = files.iter().map(py_str).collect();
        if let Some(u) = files.iter().find(|f| !o.project.files.contains_key(*f)) {
            return Err(app_err(msg("Unknown file: {name}", &[("name", json!(u))])));
        }
        let rules: Vec<String> = rules.as_array().map(|a| a.iter().map(py_str).collect()).unwrap_or_default();
        let glossary: Vec<(String, String)> = o.store.glossary.iter().map(|(c, g)| (c.clone(), g.n.clone())).collect();
        let rels = if files.is_empty() { None } else { Some(files.as_slice()) };
        Ok(bulk::rule_candidates(&o.project, &o.store, &glossary, &rules, rels))
    }

    pub fn approval_apply(&mut self, selections: &[Value], on: bool) -> Res<Value> {
        self.need_project()?;
        let list: Vec<Selection> = selections
            .iter()
            .map(|s| Selection {
                file: s["file"].as_str().unwrap_or("").to_string(),
                key: (s["key"][0].as_str().unwrap_or("").to_string(), s["key"][1].as_str().unwrap_or("").to_string()),
            })
            .collect();
        let clock = &*self.clock;
        let o = self.open.as_mut().unwrap();
        let (changed, skipped) = bulk::apply_approvals(&o.project, &mut o.store, &list, on, None, clock)?;
        let mut out = self.changed("approve")?;
        out.insert("changed".into(), json!(changed));
        out.insert("skipped".into(), json!(skipped.iter().map(|(n, m)| json!([n, m.wire()])).collect::<Vec<_>>()));
        Ok(Value::Object(out))
    }
}

// ---------------------------------------------------------------- the wrappers of api.rs
pub fn op_assign(app: &mut App, a: &Args) -> Res<Answer> {
    plain(app.set_assign(a.s("name"), a.s("v"))?)
}

pub fn op_assign_suggest(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.assign_suggest(a.l("names"))?)
}

pub fn op_assign_apply(app: &mut App, a: &Args) -> Res<Answer> {
    plain(app.assign_apply(a.l("items"))?)
}

pub fn op_episode(app: &mut App, a: &Args) -> Res<Answer> {
    plain(app.set_episode_state(a.i("ep"), a.s("state"))?)
}

pub fn op_approve_preview(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.approval_preview(a.s("file"), a.i("line"), a.s("scope"), a.f("same_speaker"))?)
}

pub fn op_approve_apply(app: &mut App, a: &Args) -> Res<Answer> {
    plain(app.approval_apply(a.l("selections"), a.f("on"))?)
}

pub fn op_approve_rules(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.rule_preview(a.a("rules"), a.l("files"))?)
}

pub fn op_groups_suggest(app: &mut App, _a: &Args) -> Res<Answer> {
    ok(app.groups_suggest()?)
}

pub fn op_groups_apply(app: &mut App, a: &Args) -> Res<Answer> {
    ok(app.groups_apply(a.a("label"), a.a("groups"), a.s("series"), a.a("folders"))?)
}
