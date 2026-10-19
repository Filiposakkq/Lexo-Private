//! The synthetic project of the UI self-test (`reference/python/free/tests/ui/serve_selftest.py`), shared by `lexo-server --selftest` and `lexo-app` (LEXO_SELFTEST=1).
use std::path::PathBuf;

const SCRIPT: &str = "# game/script.rpy:10
translate czech a1:

    # mc happy \"Hello there, [name].\"
    mc happy \"Ahoj tam, [name].\"

# game/script.rpy:20
translate czech a2:

    # mc \"Take the Dik Coins.\"
    mc \"Vezmi si peníze.\"

# game/script.rpy:30
translate czech a3:

    # de \"Hello [name].\"
    de \"Ahoj [jmeno].\"

# game/script.rpy:40
translate czech a4:

    # de \"Hmm...\"
    de \"Hmm...\"

# game/script.rpy:50
translate czech a5:

    # mc \"Where are you going today?\"
    mc \"Where are you going today?\"

# game/script.rpy:55
translate czech a6:

    # mc \"Yes.\"
    mc \"Ano.\"

# game/script.rpy:56
translate czech a7:

    # de \"Yes.\"
    de \"Jo.\"

# game/script.rpy:57
translate czech a8:

    # mc \"Where are you going tonight?\"
    mc \"Kam jdeš dnes večer?\"
";

const STRINGS: &str = "translate czech strings:

    # game/script.rpy:60
    old \"Start\"
    new \"Začít\"

    # game/script.rpy:61
    old \"Quit\"
    new \"Quit\"
";

/// The synthetic project of the UI self-test (`reference/python/free/tests/ui/serve_selftest.py`): the folder to open (`.../.lexo-selftest/game/tl/czech`).
pub fn make_project() -> std::io::Result<PathBuf> {
    let base = std::env::temp_dir().join(format!("lexo-{}-{}", std::process::id(), lexo_core::fsutil::token_hex(3))).join(".lexo-selftest");
    let proj = base.join("game/tl/czech");
    std::fs::create_dir_all(&proj)?;
    let script: String = (1..80).map(|i| format!("label line_{i}:")).collect::<Vec<_>>().join("\n");
    std::fs::write(base.join("game/script.rpy"), script)?;
    std::fs::write(proj.join("script.rpy"), SCRIPT)?;
    std::fs::write(proj.join("options.rpy"), STRINGS)?;
    Ok(proj)
}
