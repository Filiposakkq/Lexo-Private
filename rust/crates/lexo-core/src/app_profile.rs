//! Profiles of characters (the Pro feature `mt.profiles`): who is a woman or a man, how a character speaks, and who addresses whom formally. The profile is kept in the note of the
//! glossary entry of the speaker (`profile.rs`); the batch machine translation sends it with every line of that speaker, and the style check uses it. These calls are not in the
//! contract of Core: they belong to the application around it.
use crate::app::App;
use crate::app_base::{app_err, Res};
use crate::messages::{msg, py_str};
use crate::profile::{self, Profile};
use crate::store::{Gloss, Kind, Label};
use serde_json::{json, Map, Value};

pub const FEATURE: &str = "mt.profiles";

fn profile_json(p: &Profile) -> Value {
    json!({"gender": p.gender, "style": p.style, "to": p.to})
}

impl App {
    fn profiles_gate(&self) -> Res<()> {
        if self.ext.decide(FEATURE).available {
            Ok(())
        } else {
            Err(app_err(msg("“{title}” is a Pro feature.", &[("title", json!("Profiles of characters"))])))
        }
    }

    /// The speakers of the project with their profiles: `{items: [{code, name, count, profile}]}` (every speaker of the glossary page, with an empty profile when there is none).
    pub fn profiles(&self) -> Res<Value> {
        self.need_project()?;
        self.profiles_gate()?;
        let payload = self.glossary_payload()?;
        let o = self.o()?;
        let items: Vec<Value> = payload["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|i| i["type"] == "speaker")
            .map(|i| {
                let code = py_str(&i["code"]);
                let p = o.store.glossary.get(&code).map(|g| profile::parse(&g.c)).unwrap_or_default();
                json!({"code": code, "name": i["name"], "count": i["count"], "in_project": i["in_project"], "profile": profile_json(&p)})
            })
            .collect();
        Ok(json!({"ok": true, "items": items}))
    }

    /// Writes the profile of one speaker (an empty profile removes it). One undo step. The free text of the note and the name stay as they are.
    pub fn profile_set(&mut self, code: &str, gender: &str, style: &str, to: &Value) -> Res<Value> {
        self.need_project()?;
        self.profiles_gate()?;
        if code.is_empty() || code.chars().count() > 60 || code.contains(['\r', '\n']) {
            return Err(app_err(msg("Invalid code: {code}", &[("code", json!(code))])));
        }
        if !gender.is_empty() && !["m", "f", "n"].contains(&gender) {
            return Err(app_err(msg("Invalid gender of a character.", &[])));
        }
        if style.chars().count() > 200 {
            return Err(app_err(msg("The name or note is too long.", &[])));
        }
        let mut forms: Map<String, Value> = Map::new();
        for (k, v) in to.as_object().into_iter().flatten() {
            let form = py_str(v);
            if form == "informal" || form == "formal" {
                forms.insert(k.clone(), json!(form));
            }
        }
        let prof = Profile { gender: gender.to_string(), style: style.trim().to_string(), to: forms.iter().map(|(k, v)| (k.clone(), py_str(v))).collect() };
        let line = profile::line_of(&prof);
        let (code, clock) = (code.to_string(), &*self.clock);
        let o = self.open.as_mut().unwrap();
        o.store.change(
            Label::Msg(msg("Profile of a character", &[])),
            &[Kind::Glossary],
            |st| {
                let (name, typ, free) = match st.glossary.get(&code) {
                    Some(g) => (g.n.clone(), g.t.clone(), profile::split(&g.c).0),
                    None => (String::new(), "speaker".to_string(), String::new()),
                };
                let note = profile::with_line(&free, &line);
                if name.is_empty() && note.is_empty() {
                    st.glossary.shift_remove(&code);
                } else {
                    st.glossary.insert(code.clone(), Gloss { n: name, t: typ, c: note });
                }
            },
            clock,
        )?;
        let mut out = self.changed("profile")?;
        out.insert("profile".into(), profile_json(&prof));
        Ok(Value::Object(out))
    }

    /// For the batch machine translation: the profile of the speaker of a line as plain data for the pack (`None` without a profile or without the feature):
    /// `{speaker: {code, name}, gender, style, to: {name or code of the other: form}}`.
    pub fn profile_for(&self, who: &str) -> Option<Value> {
        if !self.ext.decide(FEATURE).available {
            return None;
        }
        let code = App::speaker_code(who);
        let o = self.open.as_ref()?;
        let g = o.store.glossary.get(&code)?;
        let p = profile::parse(&g.c);
        if p.is_empty() {
            return None;
        }
        let name_of = |c: &str| o.store.glossary.get(c).filter(|g| !g.n.is_empty()).map_or_else(|| c.to_string(), |g| g.n.clone());
        let to: Map<String, Value> = p.to.iter().map(|(c, f)| (name_of(c), json!(f))).collect();
        Some(json!({"speaker": {"code": code, "name": name_of(&code)}, "gender": p.gender, "style": p.style, "to": to}))
    }
}
