//! The release builder (the Pro feature `release.build`): from a translation to a zip the players unpack into their game.
//!
//! Core collects the files and the numbers, writes the zip and keeps the history of the releases; this module is the judgement: whether the release can be made (the checks), what changed
//! since the last one (the changelog), and the texts that go into the zip (the README for the players, the release notes for a page, the script that makes the language the default one).
//! Plain data in, plain data out.
use lexo_core::messages::{msg, py_str};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

/// The most the files you attach may weigh together.
pub const MAX_EXTRAS_BYTES: i64 = 300 * 1024 * 1024;

/// `1`, `1.2`, `1.2.3`, with an optional suffix such as `-beta1`.
pub fn version_ok(v: &str) -> bool {
    let (nums, suffix) = match v.find(['-', '+']) {
        Some(i) => (&v[..i], &v[i + 1..]),
        None => (v, "x"),
    };
    let parts: Vec<&str> = nums.split('.').collect();
    (1..=3).contains(&parts.len())
        && parts.iter().all(|p| !p.is_empty() && p.len() <= 6 && p.chars().all(|c| c.is_ascii_digit()))
        && !suffix.is_empty()
        && suffix.len() <= 20
        && suffix.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')
}

fn nums(v: &str) -> Vec<u64> {
    v.split(['-', '+']).next().unwrap_or("").split('.').filter_map(|p| p.parse().ok()).collect()
}

/// The version after `last`: the last number plus one (`1.2` -> `1.3`, `2` -> `3`); `1.0` when there is no release yet.
pub fn next_version(last: &str) -> String {
    let mut n = nums(last);
    match n.last_mut() {
        None => "1.0".to_string(),
        Some(x) => {
            *x += 1;
            n.iter().map(u64::to_string).collect::<Vec<_>>().join(".")
        }
    }
}

fn check(level: &str, code: &str, m: Value) -> Value {
    json!({"level": level, "code": code, "message": m})
}

fn count(v: &Value) -> i64 {
    v.as_i64().or_else(|| py_str(v).parse().ok()).unwrap_or(0)
}

fn pct(part: i64, whole: i64) -> i64 {
    if whole <= 0 {
        0
    } else {
        (part * 100 + whole / 2) / whole
    }
}

fn files_of(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().map(py_str).collect()
}

/// What changed since the previous release: `{files_added, files_changed, files_removed, lines_new, lines_changed, lines_removed, by_line}`.
fn changes(p: &Value) -> Value {
    let prev = &p["previous"];
    let (mut added, mut changed, mut removed): (Vec<String>, Vec<String>, Vec<String>) = (vec![], vec![], vec![]);
    let cur: Vec<(String, String)> = p["files"].as_array().into_iter().flatten().map(|f| (py_str(&f["path"]), py_str(&f["sha1"]))).collect();
    let before: Map<String, Value> = prev["files"].as_object().cloned().unwrap_or_default();
    for (path, sha) in &cur {
        match before.get(path) {
            None => added.push(path.clone()),
            Some(old) if py_str(old) != *sha => changed.push(path.clone()),
            _ => {}
        }
    }
    let now: BTreeSet<&String> = cur.iter().map(|(p, _)| p).collect();
    for path in before.keys() {
        if !now.contains(path) {
            removed.push(path.clone());
        }
    }
    // line by line, when both releases have the digests of their lines
    let (cl, pl) = (p["lines"].as_object(), prev["lines"].as_object());
    let by_line = cl.is_some() && pl.is_some();
    let (mut lnew, mut lchanged, mut lremoved) = (0, 0, 0);
    if let (Some(cl), Some(pl)) = (cl, pl) {
        for (k, v) in cl {
            match pl.get(k) {
                None => lnew += 1,
                Some(o) if o != v => lchanged += 1,
                _ => {}
            }
        }
        lremoved = pl.keys().filter(|k| !cl.contains_key(*k)).count();
    }
    json!({"files_added": added, "files_changed": changed, "files_removed": removed, "lines_new": lnew, "lines_changed": lchanged, "lines_removed": lremoved, "by_line": by_line})
}

fn is_czech(language: &str) -> bool {
    matches!(language.to_lowercase().as_str(), "czech" | "cs" | "cestina" | "čeština")
}

fn list(items: &[String], max: usize) -> String {
    let shown: Vec<&str> = items.iter().take(max).map(String::as_str).collect();
    if items.len() > max {
        format!("{}, … (+{})", shown.join(", "), items.len() - max)
    } else {
        shown.join(", ")
    }
}

fn changelog(p: &Value, ch: &Value) -> String {
    let (version, date) = (py_str(&p["version"]), py_str(&p["today"]));
    let mut out = format!("Version {version} ({date})\n");
    let gv = py_str(&p["game"]["version"]);
    if !gv.is_empty() {
        out += &format!("For the game version {gv}\n");
    }
    out += "\n";
    let notes = py_str(&p["notes"]);
    if !notes.trim().is_empty() {
        out += &format!("What's new\n{}\n\n", notes.trim());
    }
    let prev = &p["previous"];
    if prev.is_object() {
        out += &format!("Changes since {}\n", py_str(&prev["version"]));
        if ch["by_line"] == true {
            out += &format!(
                "- {} lines translated that were not before\n- {} lines changed\n- {} lines removed\n",
                count(&ch["lines_new"]),
                count(&ch["lines_changed"]),
                count(&ch["lines_removed"])
            );
        }
        let (a, c, r) = (files_of(&ch["files_added"]), files_of(&ch["files_changed"]), files_of(&ch["files_removed"]));
        if !a.is_empty() {
            out += &format!("- files added ({}): {}\n", a.len(), list(&a, 12));
        }
        if !c.is_empty() {
            out += &format!("- files changed ({}): {}\n", c.len(), list(&c, 12));
        }
        if !r.is_empty() {
            out += &format!("- files removed ({}): {}\n", r.len(), list(&r, 12));
        }
        if a.is_empty() && c.is_empty() && r.is_empty() {
            out += "- no file changed\n";
        }
        out += "\n";
    } else {
        out += "First release.\n\n";
    }
    let s = &p["stats"];
    let (total, translated) = (count(&s["total"]), count(&s["translated"]));
    out += &format!("Progress: {} % translated ({translated} of {total} lines), {} % proofread\n", pct(translated, total), pct(count(&s["proofread"]), total));
    out
}

fn readme(p: &Value, language_script: &Option<String>) -> String {
    let (version, date, language) = (py_str(&p["version"]), py_str(&p["today"]), py_str(&p["language"]));
    let game = py_str(&p["game"]["name"]);
    let gv = py_str(&p["game"]["version"]);
    let opts = &p["options"];
    let cz = is_czech(&language);
    let mut out = String::new();
    if cz {
        out += &format!("{} – překlad do češtiny, verze {version} ({date})\n", if game.is_empty() { "Překlad hry" } else { &game });
        if !gv.is_empty() {
            out += &format!("Pro hru ve verzi {gv}.\n");
        }
        out += "\nJak to nainstalovat\n1. Zavři hru.\n2. Rozbal celý tento archiv do složky hry (do složky, ve které je složka „game“). Soubory se přidají do game/tl/";
        out += &format!("{language}/ a případné přepsání potvrď.\n3. Spusť hru. První start může být o něco pomalejší, Ren'Py překlad načítá.\n");
        out += if language_script.is_some() {
            "4. Čeština se zapne sama. Kdyby ne, vyber ji v nastavení hry (Language / Jazyk).\n"
        } else {
            "4. Vyber češtinu v nastavení hry (Language / Jazyk).\n"
        };
        out += "\nCo se změnilo, je v souboru CHANGELOG.txt.\n";
    } else {
        out += &format!("{} – {language} translation, version {version} ({date})\n", if game.is_empty() { "Game" } else { &game });
        if !gv.is_empty() {
            out += &format!("For the game version {gv}.\n");
        }
        out += "\nHow to install\n1. Close the game.\n2. Extract this whole archive into the folder of the game (the folder that contains the \"game\" folder). The files go to ";
        out += &format!("game/tl/{language}/; confirm overwriting if you are asked.\n3. Start the game. The first start can be a little slower while Ren'Py loads the translation.\n");
        out += if language_script.is_some() {
            "4. The language is switched on by itself. If it is not, choose it in the preferences of the game.\n"
        } else {
            "4. Choose the language in the preferences of the game.\n"
        };
        out += "\nWhat changed is in CHANGELOG.txt.\n";
    }
    let credits = py_str(&opts["credits"]);
    if !credits.trim().is_empty() {
        out += &format!("\n{}\n{}\n", if cz { "Autoři" } else { "Credits" }, credits.trim());
    }
    let extra = py_str(&opts["readme_extra"]);
    if !extra.trim().is_empty() {
        out += &format!("\n{}\n", extra.trim());
    }
    out
}

fn release_notes(p: &Value, ch: &Value) -> String {
    let (version, date, language) = (py_str(&p["version"]), py_str(&p["today"]), py_str(&p["language"]));
    let game = py_str(&p["game"]["name"]);
    let gv = py_str(&p["game"]["version"]);
    let mut out = format!("## {}{language} translation {version}\n\n", if game.is_empty() { String::new() } else { format!("{game}: ") });
    out += &format!("Released {date}.");
    if !gv.is_empty() {
        out += &format!(" For the game version **{gv}**.");
    }
    out += "\n\n";
    let notes = py_str(&p["notes"]);
    if !notes.trim().is_empty() {
        out += &format!("{}\n\n", notes.trim());
    }
    let s = &p["stats"];
    let (total, translated) = (count(&s["total"]), count(&s["translated"]));
    out += &format!(
        "**Progress:** {} % translated ({translated} / {total} lines), {} % proofread.\n\n",
        pct(translated, total),
        pct(count(&s["proofread"]), total)
    );
    if p["previous"].is_object() && ch["by_line"] == true {
        out += &format!("**Since {}:** {} new lines, {} changed.\n\n", py_str(&p["previous"]["version"]), count(&ch["lines_new"]), count(&ch["lines_changed"]));
    }
    out += "**Install:** extract the archive into the game folder (the one with the `game` folder).\n";
    out
}

/// The script that makes the language the default one of the game.
fn language_script(p: &Value) -> Option<String> {
    let language = py_str(&p["language"]);
    if p["options"]["language_script"] != true || language.is_empty() || !language.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some(format!("# Made by Aside: the translation \"{language}\" is the language the game starts in.\n# Delete this file if you want the game to start in its own language.\ndefine config.default_language = \"{language}\"\n"))
}

/// `{checks, ok, can_force, suggested_version, changes, changelog, readme, release_notes, language_script}`.
pub fn plan(p: &Value) -> Value {
    let mut checks: Vec<Value> = Vec::new();
    let version = py_str(&p["version"]);
    let prev = &p["previous"];
    let suggested = next_version(&py_str(&prev["version"]));
    if !version_ok(&version) {
        checks.push(check(
            "error",
            "version",
            msg("The version is not valid: use numbers with dots, such as 1.2 or 1.2.3 (a suffix like -beta1 is allowed).", &[]).wire(),
        ));
    } else if prev.is_object() && py_str(&prev["version"]) == version {
        checks.push(check(
            "error",
            "version",
            msg("The version {v} was already released. Choose a new one (for example {next}).", &[("v", json!(version)), ("next", json!(suggested))]).wire(),
        ));
    } else if prev.is_object() && nums(&version) < nums(&py_str(&prev["version"])) {
        checks.push(check(
            "warn",
            "version-lower",
            msg("The version {v} is lower than the last release ({last}).", &[("v", json!(version)), ("last", prev["version"].clone())]).wire(),
        ));
    }
    let files = p["files"].as_array().cloned().unwrap_or_default();
    if files.is_empty() {
        checks.push(check("error", "no-files", msg("The translation has no .rpy file to put into the release.", &[]).wire()));
    }
    let s = &p["stats"];
    let (total, translated, errors, warnings) = (count(&s["total"]), count(&s["translated"]), count(&s["errors"]), count(&s["warnings"]));
    if errors > 0 {
        checks.push(check(
            "error",
            "findings",
            msg("{n} lines have an error that could crash the game. Fix them (the Check tab), or build the release anyway.", &[("n", json!(errors))]).wire(),
        ));
    }
    if warnings > 0 {
        checks.push(check("warn", "warnings", msg("{n} lines have a warning.", &[("n", json!(warnings))]).wire()));
    }
    if total > translated {
        checks.push(check(
            "warn",
            "untranslated",
            msg(
                "{n} of {total} lines are not translated ({pct} % done): the players will see English there.",
                &[("n", json!(total - translated)), ("total", json!(total)), ("pct", json!(pct(translated, total)))],
            )
            .wire(),
        ));
    }
    let partial = files.iter().filter(|f| f["state"] == "partial").count();
    if partial > 0 {
        checks.push(check(
            "warn",
            "partial-files",
            msg("{n} files are translated only partly; the players will see English in the rest.", &[("n", json!(partial))]).wire(),
        ));
    }
    if count(&s["fix"]) > 0 {
        checks.push(check("warn", "to-fix", msg("{n} lines are flagged \"to fix\".", &[("n", json!(count(&s["fix"])))]).wire()));
    }
    if count(&s["stale"]) > 0 {
        checks.push(check("warn", "stale", msg("{n} lines changed after they were proofread.", &[("n", json!(count(&s["stale"])))]).wire()));
    }
    let (gv, pgv) = (py_str(&p["game"]["version"]), py_str(&prev["game_version"]));
    if prev.is_object() && !gv.is_empty() && !pgv.is_empty() && gv != pgv {
        checks.push(check(
            "warn",
            "game-version",
            msg(
                "The game is now version {now}, the last release was made for {then}: check that the translation fits the new version.",
                &[("now", json!(gv)), ("then", json!(pgv))],
            )
            .wire(),
        ));
    }
    // the files you attach
    let mut extras_bytes = 0i64;
    for e in p["extras"].as_array().into_iter().flatten() {
        let to = py_str(&e["to"]);
        if !e["exists"].as_bool().unwrap_or(false) {
            checks.push(check("error", "extra-missing", msg("The attached file is missing: {file}", &[("file", json!(py_str(&e["from"])))]).wire()));
        }
        if !to.starts_with("game/") || to.split('/').any(|x| x == ".." || x.is_empty()) || to.contains('\\') {
            checks.push(check(
                "error",
                "extra-place",
                msg("The place of an attached file must be inside the game folder, like game/tl/czech/fonts/x.ttf: {to}", &[("to", json!(to))]).wire(),
            ));
        }
        extras_bytes += count(&e["size"]);
    }
    if extras_bytes > MAX_EXTRAS_BYTES {
        checks.push(check(
            "error",
            "extras-size",
            msg("The attached files weigh more than {mb} MB together.", &[("mb", json!(MAX_EXTRAS_BYTES / 1024 / 1024))]).wire(),
        ));
    }
    let any_error = checks.iter().any(|c| c["level"] == "error");
    let hard = checks.iter().any(|c| c["level"] == "error" && c["code"] != "findings");
    let ch = changes(p);
    let script = language_script(p);
    let readme_text = if p["options"]["readme"] == true { Some(readme(p, &script)) } else { None };
    let notes_md = if p["options"]["notes_md"] == true { Some(release_notes(p, &ch)) } else { None };
    if checks.is_empty() {
        checks.push(check("ok", "ok", msg("Nothing stands in the way of this release.", &[]).wire()));
    }
    json!({
        "checks": checks,
        "ok": !any_error,
        "can_force": any_error && !hard,
        "suggested_version": suggested,
        "changes": ch,
        "changelog": changelog(p, &ch),
        "readme": readme_text,
        "release_notes": notes_md,
        "language_script": script,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Value {
        json!({
            "language": "czech", "game": {"name": "Midnight Paradise", "version": "1.1"}, "version": "1.2", "notes": "Fixed the names in episode 3.", "today": "2026-10-19",
            "files": [{"path": "game/tl/czech/script.rpy", "sha1": "b", "state": "complete"}, {"path": "game/tl/czech/ep2.rpy", "sha1": "new", "state": "partial"}],
            "lines": {"a": "1", "b": "2", "c": "3"},
            "stats": {"total": 200, "translated": 150, "proofread": 100, "approved": 5, "fix": 0, "stale": 0, "errors": 0, "warnings": 2},
            "previous": {"version": "1.1", "date": "2026-09-01", "game_version": "1.0", "files": {"game/tl/czech/script.rpy": "a", "game/tl/czech/gone.rpy": "z"}, "lines": {"a": "1", "b": "OLD", "d": "9"}},
            "extras": [{"from": "D:/fonts/x.ttf", "to": "game/tl/czech/fonts/x.ttf", "size": 1000, "exists": true}],
            "options": {"language_script": true, "readme": true, "notes_md": true, "credits": "Anna, Tom", "readme_extra": ""}
        })
    }

    #[test]
    fn versions_are_checked_and_the_next_one_is_suggested() {
        for good in ["1", "1.2", "1.2.3", "10.0.1", "1.2-beta1", "2.0+build.5"] {
            assert!(version_ok(good), "{good}");
        }
        for bad in ["", "v1", "1..2", "1.2.3.4", "1.x", "1.2-", "a.b", "1.2 beta"] {
            assert!(!version_ok(bad), "{bad}");
        }
        assert_eq!((next_version("1.2").as_str(), next_version("2").as_str(), next_version("").as_str()), ("1.3", "3", "1.0"));
        assert_eq!(next_version("1.9.9-beta1"), "1.9.10");
    }

    #[test]
    fn a_good_release_has_texts_and_what_changed_since_the_last_one() {
        let r = plan(&base());
        // the untranslated lines and the warnings are only warnings: the release can be made
        assert_eq!((r["ok"].clone(), r["can_force"].clone()), (json!(true), json!(false)), "{r}");
        let codes: Vec<String> = r["checks"].as_array().unwrap().iter().map(|c| py_str(&c["code"])).collect();
        assert!(
            codes.contains(&"untranslated".into())
                && codes.contains(&"warnings".into())
                && codes.contains(&"game-version".into())
                && codes.contains(&"partial-files".into()),
            "{codes:?}"
        );
        assert_eq!(r["suggested_version"], "1.2");
    }

    #[test]
    fn the_changes_are_counted_by_file_and_by_line() {
        let r = plan(&base());
        let ch = &r["changes"];
        assert_eq!(ch["files_added"], json!(["game/tl/czech/ep2.rpy"]));
        assert_eq!(ch["files_changed"], json!(["game/tl/czech/script.rpy"]));
        assert_eq!(ch["files_removed"], json!(["game/tl/czech/gone.rpy"]));
        assert_eq!((count(&ch["lines_new"]), count(&ch["lines_changed"]), count(&ch["lines_removed"])), (1, 1, 1)); // c is new, b changed, d is gone
        let text = r["changelog"].as_str().unwrap();
        assert!(
            text.contains("Version 1.2 (2026-10-19)") && text.contains("For the game version 1.1") && text.contains("Fixed the names in episode 3."),
            "{text}"
        );
        assert!(text.contains("Changes since 1.1") && text.contains("1 lines changed") && text.contains("files added (1): game/tl/czech/ep2.rpy"), "{text}");
        assert!(text.contains("Progress: 75 % translated (150 of 200 lines), 50 % proofread"), "{text}");
    }

    #[test]
    fn the_readme_is_czech_for_a_czech_translation_and_the_script_sets_the_language() {
        let r = plan(&base());
        let readme = r["readme"].as_str().unwrap();
        assert!(
            readme.contains("Jak to nainstalovat") && readme.contains("game/tl/czech/") && readme.contains("Anna, Tom") && readme.contains("verze 1.2"),
            "{readme}"
        );
        assert!(r["language_script"].as_str().unwrap().contains("define config.default_language = \"czech\""));
        let mut p = base();
        p["language"] = json!("german");
        p["options"]["language_script"] = json!(false);
        let r = plan(&p);
        assert!(r["readme"].as_str().unwrap().contains("How to install") && r["language_script"].is_null());
        assert!(r["release_notes"].as_str().unwrap().contains("## Midnight Paradise: german translation 1.2"));
        p["options"]["readme"] = json!(false);
        p["options"]["notes_md"] = json!(false);
        let r = plan(&p);
        assert!(r["readme"].is_null() && r["release_notes"].is_null());
    }

    #[test]
    fn problems_are_errors_or_warnings_and_only_the_findings_can_be_forced() {
        let mut p = base();
        p["version"] = json!("1.1");
        let r = plan(&p);
        assert_eq!((r["ok"].clone(), r["can_force"].clone()), (json!(false), json!(false)), "the old version number is a hard error");
        let mut p = base();
        p["stats"]["errors"] = json!(3);
        let r = plan(&p);
        assert_eq!((r["ok"].clone(), r["can_force"].clone()), (json!(false), json!(true)), "errors of the check can be built anyway");
        let mut p = base();
        p["extras"][0]["to"] = json!("../outside.txt");
        p["extras"][0]["exists"] = json!(false);
        p["files"] = json!([]);
        let codes: Vec<String> = plan(&p)["checks"].as_array().unwrap().iter().map(|c| py_str(&c["code"])).collect();
        for want in ["extra-missing", "extra-place", "no-files"] {
            assert!(codes.contains(&want.to_string()), "{codes:?}");
        }
        let mut p = base();
        p["extras"][0]["size"] = json!(MAX_EXTRAS_BYTES + 1);
        assert!(plan(&p)["checks"].as_array().unwrap().iter().any(|c| c["code"] == "extras-size"));
        let mut p = base();
        p["stats"] = json!({"total": 10, "translated": 10, "proofread": 10, "errors": 0, "warnings": 0, "fix": 1, "stale": 2});
        p["game"]["version"] = json!("1.0");
        let codes: Vec<String> = plan(&p)["checks"].as_array().unwrap().iter().map(|c| py_str(&c["code"])).collect();
        assert!(codes.contains(&"to-fix".into()) && codes.contains(&"stale".into()), "{codes:?}");
        assert!(!codes.contains(&"game-version".into()), "the same game version is fine: {codes:?}");
    }

    #[test]
    fn a_first_release_has_no_comparison() {
        let mut p = base();
        p["previous"] = Value::Null;
        let r = plan(&p);
        assert_eq!(r["suggested_version"], "1.0");
        assert!(r["changelog"].as_str().unwrap().contains("First release."));
        assert_eq!(r["changes"]["by_line"], false);
    }
}
