//! Team work, version 1 (no server): a team is the people who share the translation folder (git, Dropbox, Syncthing). Nothing is registered anywhere. Every file here is written by one person only,
//! so sharing never makes a conflict:
//!
//! * `.aside/team/members/<author>.json` – each person writes their own name (`{author, name}`); anybody can join (this is Free);
//! * `.aside/team/lead.json` – written by the lead only: `{lead, roles: {author: role}, assign: {file: [author]}}`. The file exists = the project has a team.
//!
//! Reading all of it is Free for every member: who is in the team, which files are theirs. Creating the team and changing roles and assignments is the Pro feature `team.manage`; the pack checks and
//! cleans what is written (roles, who may be assigned, limits). It is a team of trust, not of rights: whoever has the folder can write in it. These calls are not in the contract of Core.
use crate::app::App;
use crate::app_base::{app_err, Res};
use crate::fsutil;
use crate::messages::{msg, py_str};
use crate::project::DATA_DIR_NAME;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const FEATURE: &str = "team.manage";

const MAX_NAME: usize = 40;

fn read_json(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

impl App {
    fn team_dir(&self) -> Res<PathBuf> {
        Ok(self.o()?.project.root.join(DATA_DIR_NAME).join("team"))
    }

    fn team_write(&self, path: &Path, v: &Value) -> Res<()> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        fsutil::atomic_write_text(path, &serde_json::to_string_pretty(v).unwrap_or_default(), Some("\n"), None, &*self.clock)
            .map_err(|e| app_err(msg("The team data could not be saved: {reason}", &[("reason", json!(e.to_string()))])))
    }

    /// `{author: name}` of everybody who joined.
    fn team_members(&self) -> Res<BTreeMap<String, String>> {
        let dir = self.team_dir()?.join("members");
        let mut out = BTreeMap::new();
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            if let Some(v) = read_json(&e.path()) {
                let (author, name) = (py_str(&v["author"]), py_str(&v["name"]));
                if !author.is_empty() {
                    out.insert(author, name);
                }
            }
        }
        Ok(out)
    }

    fn team_lead_file(&self) -> Res<Option<Value>> {
        Ok(read_json(&self.team_dir()?.join("lead.json")).filter(Value::is_object))
    }

    /// What the Team dialog and the Files page show: `{exists, me, members, assign, my_files, can_manage}`.
    pub fn team_state(&mut self) -> Res<Value> {
        self.need_project()?;
        let me = self.author_id();
        let members = self.team_members()?;
        let lead = self.team_lead_file()?;
        let lead_author = lead.as_ref().map(|l| py_str(&l["lead"])).unwrap_or_default();
        let roles = lead.as_ref().and_then(|l| l["roles"].as_object().cloned()).unwrap_or_default();
        let assign = lead.as_ref().and_then(|l| l["assign"].as_object().cloned()).unwrap_or_default();
        let mut my_files: Vec<&String> =
            assign.iter().filter(|(_, v)| v.as_array().is_some_and(|a| a.iter().any(|x| py_str(x) == me))).map(|(k, _)| k).collect();
        my_files.sort();
        let list: Vec<Value> = members
            .iter()
            .map(|(a, n)| {
                let role = if *a == lead_author { "lead".to_string() } else { roles.get(a).map(py_str).filter(|r| !r.is_empty()).unwrap_or_else(|| "translator".into()) };
                json!({"author": a, "name": n, "role": role, "me": *a == me, "files": assign.values().filter(|v| v.as_array().is_some_and(|x| x.iter().any(|y| py_str(y) == *a))).count()})
            })
            .collect();
        Ok(json!({
            "ok": true,
            "exists": lead.is_some(),
            "me": {"author": me, "name": members.get(&me).cloned().unwrap_or_default(), "joined": members.contains_key(&me), "lead": !lead_author.is_empty() && lead_author == me},
            "members": list,
            "assign": Value::Object(assign.clone()),
            "my_files": my_files,
            "can_manage": !lead_author.is_empty() && lead_author == me && self.ext.decide(FEATURE).available,
        }))
    }

    /// Say who you are in this project (Free): writes your own file, nobody else's.
    pub fn team_join(&mut self, name: &str) -> Res<Value> {
        self.need_project()?;
        let name: String = crate::util::py_strip(name).chars().filter(|c| !c.is_control()).take(MAX_NAME).collect();
        if name.is_empty() {
            return Err(app_err(msg("Enter your name.", &[])));
        }
        let me = self.author_id();
        let path = self.team_dir()?.join("members").join(format!("{}.json", crate::events::clean_author(&me)));
        self.team_write(&path, &json!({"author": me, "name": name}))?;
        self.team_state()
    }

    /// Make this project a team with you as the lead (Pro).
    pub fn team_create(&mut self) -> Res<Value> {
        self.need_project()?;
        self.gate(FEATURE)?;
        let me = self.author_id();
        if !self.team_members()?.contains_key(&me) {
            return Err(app_err(msg("Enter your name first.", &[])));
        }
        if self.team_lead_file()?.is_some() {
            return Err(app_err(msg("This project already has a team.", &[])));
        }
        let path = self.team_dir()?.join("lead.json");
        self.team_write(&path, &json!({"v": 1, "lead": me, "roles": {}, "assign": {}}))?;
        self.team_state()
    }

    /// Roles and assignments (Pro, the lead only). The pack checks them; what it dropped is counted.
    pub fn team_set(&mut self, roles: &Value, assign: &Value) -> Res<Value> {
        self.need_project()?;
        self.gate(FEATURE)?;
        let me = self.author_id();
        let Some(lead) = self.team_lead_file()? else {
            return Err(app_err(msg("This project has no team yet.", &[])));
        };
        if py_str(&lead["lead"]) != me {
            return Err(app_err(msg("Only the lead of the team can change roles and assignments.", &[])));
        }
        let members: BTreeSet<String> = self.team_members()?.keys().cloned().collect();
        let files: Vec<String> = self.o()?.project.files.keys().cloned().collect();
        let res = self.ext_call(FEATURE, "normalize", &json!({"members": members, "lead": me, "roles": roles, "assign": assign, "files": files}))?;
        let mut out = Map::new();
        out.insert("v".into(), json!(1));
        out.insert("lead".into(), json!(me));
        out.insert("roles".into(), res["roles"].clone());
        out.insert("assign".into(), res["assign"].clone());
        let path = self.team_dir()?.join("lead.json");
        self.team_write(&path, &Value::Object(out))?;
        let mut state = self.team_state()?;
        state["dropped"] = res["dropped"].clone();
        Ok(state)
    }
}
