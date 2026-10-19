//! Profiles of characters (the Pro feature `mt.profiles`): turns the profile of a speaker into the sentences that go into the request of the language model of the batch machine
//! translation. Plain data in, a text out; Core keeps the profile (in the glossary) and sends it with every line of that speaker.
use lexo_core::messages::py_str;
use serde_json::Value;

/// `{speaker: {code, name}, gender, style, to: {name: "informal" | "formal"}}` -> the sentences for the model; empty for an empty profile.
pub fn describe(p: &Value) -> String {
    let name = {
        let n = py_str(&p["speaker"]["name"]);
        if n.is_empty() {
            py_str(&p["speaker"]["code"])
        } else {
            n
        }
    };
    let mut parts: Vec<String> = Vec::new();
    match py_str(&p["gender"]).as_str() {
        "f" => parts.push(format!("The speaker of this line, {name}, is female: use the feminine forms where {name} speaks about themselves.")),
        "m" => parts.push(format!("The speaker of this line, {name}, is male: use the masculine forms where {name} speaks about themselves.")),
        "n" => parts.push(format!("The speaker of this line, {name}, is neither male nor female: avoid gendered forms where {name} speaks about themselves, if the language allows it.")),
        _ => {}
    }
    let style = py_str(&p["style"]);
    if !style.trim().is_empty() {
        parts.push(format!("How {name} speaks: {}.", style.trim().trim_end_matches('.')));
    }
    let mut informal: Vec<String> = Vec::new();
    let mut formal: Vec<String> = Vec::new();
    for (other, form) in p["to"].as_object().into_iter().flatten() {
        match py_str(form).as_str() {
            "informal" => informal.push(other.clone()),
            "formal" => formal.push(other.clone()),
            _ => {}
        }
    }
    if !informal.is_empty() {
        parts.push(format!("{name} addresses {} in the informal form (the familiar \"you\", for example tykání in Czech).", informal.join(", ")));
    }
    if !formal.is_empty() {
        parts.push(format!("{name} addresses {} in the formal form (the polite \"you\", for example vykání in Czech).", formal.join(", ")));
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_profile_becomes_sentences_and_an_empty_one_nothing() {
        let p = json!({"speaker": {"code": "li", "name": "Lisa"}, "gender": "f", "style": "sarcastic, short sentences.", "to": {"Alex": "informal", "Mr. Smith": "formal"}});
        let text = describe(&p);
        assert!(text.contains("Lisa, is female") && text.contains("feminine forms"), "{text}");
        assert!(text.contains("How Lisa speaks: sarcastic, short sentences.") && !text.contains(".."), "{text}");
        assert!(text.contains("Lisa addresses Alex in the informal form") && text.contains("Lisa addresses Mr. Smith in the formal form"), "{text}");
        assert_eq!(describe(&json!({"speaker": {"code": "x", "name": ""}, "gender": "", "style": "", "to": {}})), "");
        // without a name the code is used
        assert!(describe(&json!({"speaker": {"code": "mc", "name": ""}, "gender": "m", "style": "", "to": {}})).contains("mc, is male"));
    }
}
