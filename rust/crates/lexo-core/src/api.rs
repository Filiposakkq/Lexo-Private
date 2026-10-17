//! Port of `core/lexo/api.py`: the interface of Core, every operation the application offers, as a JSON call.
//!
//! This table is THE interface between a client and Core. The browser UI reaches it through a local server (one GET or POST per operation, the parameters in the query string or in the
//! JSON body, the answer as JSON), a script calls `call` directly. Nothing in this file knows about HTTP.
//!
//! An operation has a name (`"entries"`, `"entry/save"` ...: the address after `/api/`), a method (`GET` or `POST`, only a convention of the transport), an effect (`read`, `write`,
//! `external`), the parameters it takes (name, type, default, required), a one-line doc, and an example for the contract tests. `describe()` gives the whole table as plain data;
//! `core/contract/api.json` is generated from the Python table and a test compares it with this one.
//!
//! The answer is a JSON object with `"ok": true`; an error is an `ApiError`. Binary answers (`kind == "binary"`: a font, a picture) are `Answer::Binary(bytes, mime)`.
use crate::app::App;
use crate::app_base::{app_err, ApiError, Res};
use crate::messages::{msg, py_str};
use serde_json::{json, Map, Value};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ty {
    Str,
    Int,
    Flag,
    List,
    Any,
}

impl Ty {
    fn name(self) -> &'static str {
        match self {
            Ty::Str => "str",
            Ty::Int => "int",
            Ty::Flag => "flag",
            Ty::List => "list",
            Ty::Any => "any",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: &'static str,
    pub ty: Ty,
    pub default: Value,
    pub required: bool,
}

fn type_default(ty: Ty) -> Value {
    match ty {
        Ty::Str => json!(""),
        Ty::Int => json!(0),
        Ty::Flag => json!(false),
        Ty::List => json!([]),
        Ty::Any => Value::Null,
    }
}

/// `int(v)` of Python for the values a client can send.
fn py_int(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().filter(|f| f.is_finite()).map(|f| f.trunc() as i64)),
        Value::Bool(b) => Some(*b as i64),
        Value::String(s) => crate::util::py_strip(s).replace('_', "").parse::<i64>().ok(),
        _ => None,
    }
}

impl Param {
    fn new(name: &'static str, ty: Ty, default: Option<Value>, required: bool) -> Param {
        Param { name, ty, default: default.unwrap_or_else(|| type_default(ty)), required }
    }

    pub fn coerce(&self, params: &Map<String, Value>) -> Res<Value> {
        let Some(v) = params.get(self.name) else {
            return if self.required { Err(ApiError::Missing(self.name.to_string())) } else { Ok(self.default.clone()) };
        };
        let empty = matches!(v, Value::String(s) if s.is_empty());
        if v.is_null() || (empty && matches!(self.ty, Ty::Int | Ty::List | Ty::Any)) {
            return Ok(self.default.clone());
        }
        let bad = || ApiError::App(crate::messages::Text::Plain(format!("Invalid value: {}", self.name)));
        Ok(match self.ty {
            Ty::Str => Value::String(py_str(v)),
            Ty::Int => json!(py_int(v).ok_or_else(bad)?),
            Ty::Flag => json!(v == &json!(true) || v.as_f64() == Some(1.0) || matches!(py_str(v).to_lowercase().as_str(), "1" | "true")),
            Ty::List => match v {
                Value::Array(a) => Value::Array(a.clone()),
                Value::String(s) => Value::Array(s.chars().map(|c| json!(c.to_string())).collect()),
                Value::Object(o) => Value::Array(o.keys().map(|k| json!(k)).collect()),
                Value::Null => json!([]),
                Value::Bool(false) => json!([]),
                _ => return Err(bad()),
            },
            Ty::Any => v.clone(),
        })
    }

    pub fn describe(&self) -> Value {
        json!({"name": self.name, "type": self.ty.name(), "default": self.default, "required": self.required})
    }
}

pub fn s(name: &'static str) -> Param {
    Param::new(name, Ty::Str, None, false)
}
pub fn s_def(name: &'static str, default: &str) -> Param {
    Param::new(name, Ty::Str, Some(json!(default)), false)
}
pub fn s_req(name: &'static str) -> Param {
    Param::new(name, Ty::Str, None, true)
}
pub fn i(name: &'static str) -> Param {
    Param::new(name, Ty::Int, None, false)
}
pub fn i_def(name: &'static str, default: i64) -> Param {
    Param::new(name, Ty::Int, Some(json!(default)), false)
}
pub fn i_req(name: &'static str) -> Param {
    Param::new(name, Ty::Int, None, true)
}
pub fn f(name: &'static str) -> Param {
    Param::new(name, Ty::Flag, None, false)
}
pub fn f_def(name: &'static str, default: bool) -> Param {
    Param::new(name, Ty::Flag, Some(json!(default)), false)
}
pub fn l(name: &'static str) -> Param {
    Param::new(name, Ty::List, None, false)
}
pub fn a(name: &'static str) -> Param {
    Param::new(name, Ty::Any, None, false)
}
pub fn a_def(name: &'static str, default: Value) -> Param {
    Param::new(name, Ty::Any, Some(default), false)
}

/// The parameters of one call, coerced by their types; `raw()` is the whole object as it came.
pub struct Args {
    values: Map<String, Value>,
    raw: Map<String, Value>,
}

impl Args {
    pub fn s(&self, name: &str) -> &str {
        self.values.get(name).and_then(Value::as_str).unwrap_or("")
    }
    pub fn i(&self, name: &str) -> i64 {
        self.values.get(name).and_then(Value::as_i64).unwrap_or(0)
    }
    pub fn f(&self, name: &str) -> bool {
        self.values.get(name).and_then(Value::as_bool).unwrap_or(false)
    }
    pub fn l(&self, name: &str) -> &[Value] {
        self.values.get(name).and_then(Value::as_array).map_or(&[], Vec::as_slice)
    }
    pub fn a(&self, name: &str) -> &Value {
        self.values.get(name).unwrap_or(&Value::Null)
    }
    pub fn raw(&self) -> &Map<String, Value> {
        &self.raw
    }
}

/// What an operation returns: a JSON object, or the bytes of a file with its type.
#[derive(Debug)]
pub enum Answer {
    Json(Value),
    Binary(Vec<u8>, String),
}

impl Answer {
    pub fn json(&self) -> Option<&Value> {
        match self {
            Answer::Json(v) => Some(v),
            Answer::Binary(..) => None,
        }
    }
}

pub type Run = fn(&mut App, &Args) -> Res<Answer>;

pub struct Operation {
    pub name: &'static str,
    pub method: &'static str,
    pub effect: &'static str,
    pub params: Vec<Param>,
    pub doc: &'static str,
    pub needs_project: bool,
    pub kind: &'static str,
    pub example: Option<Value>,
    pub path: &'static str,
    pub run: Run,
}

impl Operation {
    pub fn bind(&self, raw: &Value) -> Res<Args> {
        let raw = raw.as_object().cloned().unwrap_or_default();
        let mut values = Map::new();
        for p in &self.params {
            values.insert(p.name.to_string(), p.coerce(&raw)?);
        }
        Ok(Args { values, raw })
    }

    pub fn describe(&self) -> Value {
        json!({
            "name": self.name, "path": self.path, "method": self.method, "effect": self.effect, "kind": self.kind, "needs_project": self.needs_project, "doc": self.doc,
            "params": self.params.iter().map(Param::describe).collect::<Vec<_>>(), "example": self.example,
        })
    }
}

/// `{"ok": true, ...d}`.
pub fn ok(d: Value) -> Res<Answer> {
    let mut out = Map::new();
    out.insert("ok".into(), json!(true));
    if let Value::Object(o) = d {
        out.extend(o);
    }
    Ok(Answer::Json(Value::Object(out)))
}

/// A bare object answer (the operation made the `ok` itself).
pub fn plain(d: Value) -> Res<Answer> {
    Ok(Answer::Json(d))
}

struct Def(Operation);

fn op(name: &'static str, method: &'static str, effect: &'static str, doc: &'static str, params: Vec<Param>, run: Run) -> Def {
    Def(Operation { name, method, effect, params, doc, needs_project: true, kind: "json", example: None, path: name, run })
}

impl Def {
    fn no_project(mut self) -> Def {
        self.0.needs_project = false;
        self
    }
    fn binary(mut self) -> Def {
        self.0.kind = "binary";
        self
    }
    fn ex(mut self, example: Value) -> Def {
        self.0.example = Some(example);
        self
    }
    fn path(mut self, path: &'static str) -> Def {
        self.0.path = path;
        self
    }
}

use crate::app_core as core;

fn table() -> &'static Vec<Operation> {
    static T: OnceLock<Vec<Operation>> = OnceLock::new();
    T.get_or_init(|| {
        let g = "GET";
        let p = "POST";
        let defs: Vec<Def> = vec![
            // ---------------------------------------------------------------- reads (GET)
            op(
                "state",
                g,
                "read",
                "What the client needs at start: is a project open, notices, config error, UI preferences, version.",
                vec![],
                core::op_state,
            )
            .no_project()
            .ex(json!({})),
            op("ui-langs", g, "read", "The interface languages (language packs that have UI texts).", vec![], core::op_ui_langs).no_project().ex(json!({})),
            op(
                "ui-lang",
                g,
                "read",
                "The interface texts of one language: {code, name, plural, strings}; English is an empty table.",
                vec![s("code")],
                core::op_ui_lang,
            )
            .no_project()
            .ex(json!({"code": "cs"})),
            op("overview", g, "read", "Totals, groups, estimates and the data of the Overview tab.", vec![], crate::app_validation::op_overview).ex(json!({})),
            op("files", g, "read", "The translation files with their counts and states.", vec![], crate::app_entries::op_files).ex(json!({})),
            op(
                "entries",
                g,
                "read",
                "A page of translation lines, filtered and sorted on the server.",
                vec![
                    s("file"),
                    s_def("status", "all"),
                    s("kind"),
                    s("q"),
                    s("issues"),
                    s_def("sort", "line"),
                    i("offset"),
                    i_def("limit", 200),
                    i("around"),
                    s("unit"),
                    f("word"),
                ],
                crate::app_entries::op_entries,
            )
            .ex(json!({"status": "all", "limit": 5})),
            op(
                "entry",
                g,
                "read",
                "The detail of one line (texts, state, findings, proofreading).",
                vec![s_req("file"), i_req("line")],
                crate::app_entries::op_entry,
            )
            .ex(json!({"file": "script.rpy", "line": 5})),
            op(
                "validation",
                g,
                "read",
                "The findings of the translation check, one item per line.",
                vec![s("severity"), s("category"), s("file"), s("q"), i("offset"), i_def("limit", 200), s_def("ignored", "hide"), s("group"), f("word")],
                crate::app_validation::op_validation,
            )
            .ex(json!({"group": "line", "limit": 5})),
            op("ignore/list", g, "read", "The findings the translator told the app to ignore.", vec![], crate::app_validation::op_ignore_list).ex(json!({})),
            op("settings", g, "read", "The settings (validation, text box, machine translation, publishing ...).", vec![], core::op_settings).ex(json!({})),
            op("fix/last", g, "read", "What the last bulk fix changed (for undo).", vec![], crate::app_fixes::op_fix_last).ex(json!({})),
            op("game/fontmetrics", g, "read", "Advance widths of a font of the game (key = file name).", vec![s("k")], crate::app_game::op_game_fontmetrics),
            op("game/characters", g, "read", "The characters defined in the game scripts: code, name, colour.", vec![], crate::app_game::op_game_characters)
                .ex(json!({})),
            op(
                "entry/next",
                g,
                "read",
                "The next line of a kind (todo, review, fix, over) after this one, wrapping round the file.",
                vec![s("file"), i("line"), s("kind")],
                crate::app_entries::op_entry_next,
            )
            .ex(json!({"file": "script.rpy", "line": 5, "kind": "todo"})),
            op(
                "scenes",
                g,
                "read",
                "The scenes of a file (runs of lines after the same `scene` statement) with progress.",
                vec![s("file"), i("line")],
                crate::app_entries::op_scenes,
            )
            .ex(json!({"file": "script.rpy", "line": 5})),
            op(
                "game/scene",
                g,
                "read",
                "The picture(s) behind a line and the text box metrics for the scene preview.",
                vec![s("file"), i("line")],
                crate::app_game::op_game_scene,
            )
            .ex(json!({"file": "script.rpy", "line": 5})),
            op(
                "entry/menu",
                g,
                "read",
                "The menu a line belongs to: question, choices with translations, what follows each.",
                vec![s("file"), i("line")],
                crate::app_menu::op_entry_menu,
            )
            .ex(json!({"file": "script.rpy", "line": 23})),
            op(
                "entry/scene",
                g,
                "read",
                "The lines around one line (original and translation).",
                vec![s("file"), i("line"), i_def("radius", 6)],
                crate::app_entries::op_entry_scene,
            )
            .ex(json!({"file": "script.rpy", "line": 5, "radius": 3})),
            op("snapshots", g, "read", "The snapshots (zips of the .rpy files) of the project.", vec![], crate::app_snapshots::op_snapshots).ex(json!({})),
            op("snapshots/files", g, "read", "The files inside one snapshot.", vec![s("name")], crate::app_snapshots::op_snapshots_files),
            op("lint/status", g, "read", "State and result of the Ren'Py lint run.", vec![], crate::app_lint::op_lint_status).ex(json!({})),
            op("entry/history", g, "read", "The versions of a line's text over time.", vec![s("file"), i("line")], crate::app_entries::op_entry_history)
                .ex(json!({"file": "script.rpy", "line": 5})),
            op(
                "entry/memory",
                g,
                "read",
                "Other translations of the same original (translation memory).",
                vec![s("file"), i("line")],
                crate::app_memory::op_entry_memory,
            )
            .ex(json!({"file": "script.rpy", "line": 5})),
            op(
                "consistency",
                g,
                "read",
                "Originals translated in several different ways.",
                vec![f("loose"), f("by_speaker")],
                crate::app_memory::op_consistency,
            )
            .ex(json!({})),
            op(
                "entry/context",
                g,
                "read",
                "The original game script around the line this translation comes from.",
                vec![s("file"), i("line")],
                crate::app_entries::op_entry_context,
            )
            .ex(json!({"file": "script.rpy", "line": 5})),
            op(
                "glossary/occurrences",
                g,
                "read",
                "Where a speaker code (or a missing name) occurs.",
                vec![s("code"), f("missing")],
                crate::app_glossary::op_glossary_occurrences,
            )
            .ex(json!({"code": "friend"})),
            op("glossary", g, "read", "The glossary: speakers and terms with counts and examples.", vec![], crate::app_glossary::op_glossary).ex(json!({})),
            op("proof/next", g, "read", "The next line still to proofread after a line number.", vec![s("file"), i("after")], crate::app_proof::op_proof_next)
                .ex(json!({"file": "script.rpy", "after": 0})),
            op(
                "export/preview",
                g,
                "read",
                "The public progress data and its checks (writes only the local progress.json).",
                vec![],
                crate::app_publish::op_export_preview,
            )
            .ex(json!({})),
            op(
                "missed",
                g,
                "read",
                "Texts the Ren'Py generator missed, found in the game scripts.",
                vec![s_def("level", "likely")],
                crate::app_missed::op_missed,
            )
            .ex(json!({})),
            op(
                "missed/functions",
                g,
                "read",
                "Functions of the game that are called again and again with a text.",
                vec![],
                crate::app_missed::op_missed_functions,
            )
            .ex(json!({})),
            op(
                "groups/suggest",
                g,
                "read",
                "The groups the project has now and the groups its file names suggest.",
                vec![],
                crate::app_assign::op_groups_suggest,
            )
            .ex(json!({})),
            op("game/font", g, "read", "A font of the game by file name (never a path): (bytes, mime).", vec![s("k")], crate::app_game::op_game_font).binary(),
            op(
                "game/image",
                g,
                "read",
                "A picture of the game by the key an answer gave (never a path): (bytes, mime).",
                vec![s("g"), s("k")],
                crate::app_game::op_game_image,
            )
            .binary(),
            // ---------------------------------------------------------------- calls with a body (POST): previews and writes
            op("project/set", p, "write", "Open a translation folder (and remember it).", vec![s("path"), f("force")], core::op_project_set).no_project(),
            op("refresh", p, "write", "Read the files again (merging what changed on disk).", vec![f("force"), f("dirty"), f("discard")], core::op_refresh)
                .ex(json!({})),
            op(
                "file/save",
                p,
                "write",
                "Save the counts of a file (the manual correction columns).",
                vec![s_req("name"), s("c"), s("t")],
                crate::app_entries::op_file_save,
            )
            .ex(json!({"name": "script.rpy", "c": "", "t": ""})),
            op("assign", p, "write", "Assign a file to a group.", vec![s_req("name"), s("v")], crate::app_assign::op_assign)
                .ex(json!({"name": "script.rpy", "v": ""})),
            op(
                "assign/suggest",
                p,
                "read",
                "Suggested group for files (a Pro feature: the Pro pack suggests, the app shows the groups to choose from).",
                vec![l("names")],
                crate::app_assign::op_assign_suggest,
            ),
            op("assign/apply", p, "write", "Apply assignments.", vec![l("items")], crate::app_assign::op_assign_apply).ex(json!({"items": []})),
            op(
                "features",
                p,
                "read",
                "Which gated (Pro) features are available, with which limit, and which Pro pack is installed.",
                vec![],
                core::op_features,
            )
            .ex(json!({})),
            op("episode", p, "write", "Set the state of a group (episode).", vec![i_req("ep"), s_req("state")], crate::app_assign::op_episode),
            op(
                "approve/preview",
                p,
                "read",
                "Preview approving identical lines.",
                vec![s_req("file"), i_req("line"), s_def("scope", "project"), f_def("same_speaker", true)],
                crate::app_assign::op_approve_preview,
            )
            .ex(json!({"file": "script.rpy", "line": 11})),
            op("approve/apply", p, "write", "Approve (or un-approve) lines.", vec![l("selections"), f_def("on", true)], crate::app_assign::op_approve_apply)
                .ex(json!({"selections": [], "on": true})),
            op(
                "approve/rules",
                p,
                "read",
                "Preview approving by rules (names, interjections).",
                vec![a("rules"), l("files")],
                crate::app_assign::op_approve_rules,
            )
            .ex(json!({"rules": [], "files": []})),
            op("glossary/set", p, "write", "Set glossary items.", vec![l("items")], crate::app_glossary::op_glossary_set)
                .ex(json!({"items": [{"code": "anna", "name": "Anna", "type": "speaker"}]})),
            op(
                "glossary/import-preview",
                p,
                "read",
                "Preview importing character names from a game file.",
                vec![s("path")],
                crate::app_glossary::op_glossary_import_preview,
            ),
            op("glossary/import-apply", p, "write", "Import character names.", vec![l("items")], crate::app_glossary::op_glossary_import_apply),
            op("glossary/delete", p, "write", "Delete glossary items.", vec![l("codes")], crate::app_glossary::op_glossary_delete)
                .ex(json!({"codes": ["nobody"]})),
            op(
                "proof/set",
                p,
                "write",
                "Mark lines as proofread / to fix, with a note.",
                vec![l("items"), s_def("state", "ok"), a("note")],
                crate::app_proof::op_proof_set,
            )
            .ex(json!({"items": [{"file": "script.rpy", "line": 5}], "state": "ok", "note": ""})),
            op(
                "proof/upto",
                p,
                "write",
                "Mark everything up to a line as proofread.",
                vec![s_req("file"), i_req("line"), s_def("state", "ok")],
                crate::app_proof::op_proof_upto,
            )
            .ex(json!({"file": "script.rpy", "line": 5, "state": "ok"})),
            op("proof/clear-file", p, "write", "Clear the proofreading marks of a file.", vec![s_req("file")], crate::app_proof::op_proof_clear_file)
                .ex(json!({"file": "script.rpy"})),
            op(
                "fix/preview",
                p,
                "read",
                "Preview the bulk fixes (speaker, spaces, typos ...): a Pro feature, the Pro pack plans them.",
                vec![l("items"), s("file"), l("kinds")],
                crate::app_fixes::op_fix_preview,
            ),
            op("fix/apply", p, "write", "Apply the chosen fixes.", vec![l("items"), s("file"), l("kinds"), a("selections")], crate::app_fixes::op_fix_apply),
            op("fix/undo", p, "write", "Undo the last bulk fix.", vec![], crate::app_fixes::op_fix_undo),
            op(
                "replace/preview",
                p,
                "read",
                "Preview a find and replace over the translations.",
                vec![a_def("find", json!("")), a_def("replace", json!("")), s_def("mode", "plain"), f("case"), l("files"), f_def("protect", true)],
                crate::app_fixes::op_replace_preview,
            )
            .ex(json!({"find": "Hello", "replace": "Hi", "mode": "plain"})),
            op(
                "replace/apply",
                p,
                "write",
                "Apply a find and replace to the chosen places.",
                vec![
                    l("selections"),
                    a_def("find", json!("")),
                    a_def("replace", json!("")),
                    s_def("mode", "plain"),
                    f("case"),
                    l("files"),
                    f_def("protect", true),
                ],
                crate::app_fixes::op_replace_apply,
            )
            .ex(json!({"selections": [{"file": "script.rpy", "line": 5}], "find": "Hi", "replace": "Hey", "mode": "plain"})),
            op(
                "entry/mt",
                p,
                "external",
                "A machine translation suggestion for one line (uses the network).",
                vec![s_req("file"), i_req("line")],
                crate::app_mt::op_entry_mt,
            ),
            op(
                "mt/targets",
                p,
                "read",
                "The lines a batch machine translation would take (a Pro feature: the Pro pack cuts the list to its limit).",
                vec![s("file"), a("lines"), f_def("only_todo", true)],
                crate::app_mt::op_mt_targets,
            ),
            op(
                "mt/batch",
                p,
                "external",
                "Machine translate many lines (a Pro feature; the pack translates, uses the network).",
                vec![a("items")],
                crate::app_mt::op_mt_batch,
            ),
            op("mt/apply", p, "write", "Write the chosen machine translations.", vec![a("selections")], crate::app_mt::op_mt_apply)
                .ex(json!({"selections": [{"file": "script.rpy", "line": 11, "text": "Kavu?"}]})),
            op(
                "missed/add",
                p,
                "write",
                "Add texts the generator missed to the translation files.",
                vec![a("items"), f("into_files")],
                crate::app_missed::op_missed_add,
            )
            .ex(json!({"items": [{"text": "A text the generator missed", "file": "", "line": 0}]})),
            op("missed/settings", p, "write", "Set the project's own functions that show text.", vec![a("functions")], crate::app_missed::op_missed_settings)
                .ex(json!({"functions": ["notify"]})),
            op(
                "missed/ignore",
                p,
                "write",
                "Leave texts out of the missed-texts list for good.",
                vec![a("texts"), f("clear")],
                crate::app_missed::op_missed_ignore,
            )
            .ex(json!({"texts": ["not a text of the game"]})),
            op(
                "groups/apply",
                p,
                "write",
                "Define the groups of the project (written to lexo.json).",
                vec![a("label"), a("groups"), s("series"), a("folders")],
                crate::app_assign::op_groups_apply,
            )
            .ex(json!({"label": "Episode", "groups": [1, 2]})),
            op("snapshots/create", p, "write", "Make a snapshot of the .rpy files now.", vec![], crate::app_snapshots::op_snapshots_create).ex(json!({})),
            op("snapshots/restore", p, "write", "Restore files from a snapshot.", vec![s("name"), a("files")], crate::app_snapshots::op_snapshots_restore),
            op("snapshots/delete", p, "write", "Delete a snapshot.", vec![s("name")], crate::app_snapshots::op_snapshots_delete),
            op("lint/start", p, "external", "Start the Ren'Py lint of the whole game (needs the SDK).", vec![], crate::app_lint::op_lint_start),
            op(
                "consistency/unify",
                p,
                "write",
                "Make all translations of one original the same.",
                vec![s("key"), s("target"), a("lines"), f("loose"), f("by_speaker")],
                crate::app_memory::op_consistency_unify,
            ),
            op(
                "ignore/set",
                p,
                "write",
                "Ignore (or stop ignoring) findings on lines.",
                vec![l("items"), f_def("on", true), s("note")],
                crate::app_validation::op_ignore_set,
            )
            .ex(json!({"items": [{"file": "script.rpy", "line": 5, "code": "typo-suspect"}], "on": true, "note": "example"})),
            op(
                "entry/check",
                p,
                "read",
                "The check of a text the translator is typing (nothing is written).",
                vec![s_req("file"), i_req("line"), s("text")],
                crate::app_entries::op_entry_check,
            )
            .ex(json!({"file": "script.rpy", "line": 5, "text": "Ahoj."})),
            op(
                "entry/save",
                p,
                "write",
                "Write a new translation of one line into the .rpy file (backup, atomic, undoable).",
                vec![s_req("file"), i_req("line"), s("text")],
                crate::app_entries::op_entry_save,
            )
            .ex(json!({"file": "script.rpy", "line": 5, "text": "Hi, world."})),
            op("undo", p, "write", "Undo the last change (approval, proofreading, glossary).", vec![], core::op_undo).ex(json!({})),
            op("open-editor", p, "external", "Open a file at a line in the editor.", vec![s_req("file"), i_def("line", 1)], core::op_open_editor),
            op("open-url", p, "external", "Open one of the known addresses in the browser (by id).", vec![s("id")], core::op_open_url).no_project(),
            op("settings/update", p, "write", "Change settings (the body is the patch).", vec![], core::op_settings_update)
                .no_project()
                .ex(json!({"web": {"title": "Example"}}))
                .path("settings"),
            op("prefs", p, "write", "Remember UI preferences (the body is the object of preferences).", vec![], core::op_prefs)
                .no_project()
                .ex(json!({"density": "compact"})),
            op("publish/plan", p, "read", "Preview publishing the progress data (a Pro feature).", vec![], crate::app_publish::op_publish_plan),
            op(
                "publish/confirm",
                p,
                "external",
                "Publish the progress data after the typed confirmation (a Pro feature).",
                vec![s("token"), s("branch")],
                crate::app_publish::op_publish_confirm,
            ),
        ];
        defs.into_iter().map(|d| d.0).collect()
    })
}

/// The operation `name`, if there is one.
pub fn find(name: &str) -> Option<&'static Operation> {
    table().iter().find(|o| o.name == name)
}

/// The operation behind a (method, path) pair, for a transport that works with addresses.
pub fn route(method: &str, path: &str) -> Option<&'static Operation> {
    table().iter().find(|o| o.method == method && o.path == path)
}

/// Run the operation `name` with `params`.
pub fn call(app: &mut App, name: &str, params: &Value) -> Res<Answer> {
    let op = find(name).ok_or_else(|| ApiError::Unknown(name.to_string()))?;
    if op.needs_project && !app.ready() {
        return Err(app_err(msg("No project is open.", &[])));
    }
    let args = op.bind(params)?;
    (op.run)(app, &args)
}

/// The whole interface as plain data, sorted by name.
pub fn describe() -> Value {
    let mut ops: Vec<&Operation> = table().iter().collect();
    ops.sort_by_key(|o| o.name);
    Value::Array(ops.iter().map(|o| o.describe()).collect())
}
