//! `cargo run -p lexo-golden [-- --list] [-- --strict]`: runs every golden case and prints "passed of total" per group.
//! `--list` also prints every case that does not pass, with the reason; `--strict` exits with 1 unless everything passes.
use lexo_golden::{golden_dir, inventory, run_case, summarize, Outcome};

/// `--dump <cs|neutral> <file>...` (or `--dump-check`, the issues of the checks instead of the parse): one line per file, `path<TAB>json` with the parse of the file in the shape of the golden data (for comparing Rust Core with
/// Python Core on real files, see docs/RUST-PORT.md M9).
fn dump(args: &[String], checks: bool, spell: bool) {
    let root = golden_dir().join("../..");
    let pack = lexo_golden::run::pack_named(&args[0], &root).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    let parsed: Vec<(&String, Result<lexo_core::rpy::ParsedFile, std::io::Error>)> =
        args[1..].iter().map(|path| (path, std::fs::read(path).map(|data| lexo_core::rpy::parse_bytes(&data, &pack)))).collect();
    // the vocabulary of the spelling hints is the translated text of all the files given
    let spelling = spell.then(|| {
        let targets: Vec<&str> =
            parsed.iter().filter_map(|(_, p)| p.as_ref().ok()).flat_map(|p| p.entries.iter()).filter(|e| !e.pending()).map(|e| e.target.as_str()).collect();
        lexo_core::spelling::Spelling::build(&targets, &[])
    });
    for (path, result) in &parsed {
        match result {
            Ok(p) => {
                let json = if checks {
                    lexo_golden::run::checked_json(p, &pack, spelling.as_ref().map(|s| s as &dyn lexo_core::validator::SpellCheck))
                } else {
                    lexo_golden::run::parsed_json(p)
                };
                println!("{path}	{json}")
            }
            Err(e) => println!("{path}	ERROR {e}"),
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if matches!(args.first().map(String::as_str), Some("--dump" | "--dump-check" | "--dump-spell")) && args.len() > 2 {
        return dump(&args[1..], args[0] != "--dump", args[0] == "--dump-spell");
    }
    if args.first().map(String::as_str) == Some("--dump-data") && args.len() > 2 {
        // `--dump-data <today> <data folder>...`: the store, the history and the events read from a data folder, one `folder<TAB>json` line each (nothing is written)
        let today: chrono::NaiveDate = args[1].parse().unwrap_or_else(|e| {
            eprintln!("today must be YYYY-MM-DD: {e}");
            std::process::exit(2)
        });
        for dir in &args[2..] {
            println!("{dir}	{}", lexo_golden::data::folder_json(std::path::Path::new(dir), today));
        }
        return;
    }
    let (list, strict) = (args.iter().any(|a| a == "--list"), args.iter().any(|a| a == "--strict"));
    let golden = golden_dir();
    let cases = match inventory(&golden) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("cannot read the golden data: {e}");
            std::process::exit(2);
        }
    };
    let results: Vec<_> = cases
        .into_iter()
        .map(|c| {
            let o = run_case(&golden, &c);
            (c, o)
        })
        .collect();
    if list {
        for (c, o) in &results {
            match o {
                Outcome::Pass => {}
                Outcome::Fail(why) => println!("FAIL {}/{}: {why}", c.group, c.id),
                Outcome::NotImplemented => println!("todo {}/{}", c.group, c.id),
            }
        }
    }
    let (mut passed, mut total) = (0, 0);
    for (group, p, f, n) in summarize(&results) {
        println!("{group:<10} {p} of {}{}", p + f + n, if f > 0 { format!("  ({f} failed)") } else { String::new() });
        passed += p;
        total += p + f + n;
    }
    println!("{:<10} {passed} of {total}", "all");
    if strict && passed != total {
        std::process::exit(1);
    }
}
