//! What the demo of the pack (no licence) offers; its own file, because it sets an environment variable of the process.
use lexo_pro::licence;
use serde_json::json;

#[test]
fn the_demo_offers_only_the_taste_features_with_their_limits() {
    std::env::set_var("LEXO_PRO_DEMO", "1");
    for (feature, limit) in [("mt.batch", 20), ("fixes.safe", 20), ("bulk.categorize", 5), ("carry.over", 10)] {
        assert_eq!(licence::offer(feature), Some(json!({"limit": limit})), "{feature}");
    }
    // the rest needs a licence: the style check, filling from similar translations, profiles, regex, publishing, the release builder
    for feature in ["style.check", "memory.fuzzy", "mt.profiles", "replace.regex", "publish.web", "release.build"] {
        assert_eq!(licence::offer(feature), None, "{feature}");
    }
}
