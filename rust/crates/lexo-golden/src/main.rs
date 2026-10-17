//! `cargo run -p lexo-golden [-- --list] [-- --strict]`: runs every golden case and prints "passed of total" per group.
//! `--list` also prints every case that does not pass, with the reason; `--strict` exits with 1 unless everything passes.
use lexo_golden::{golden_dir, inventory, run_case, summarize, Outcome};

/// `--dump <cs|neutral> <file>...`: one line per file, `path<TAB>json` with the parse of the file in the shape of the golden data (for comparing Rust Core with
/// Python Core on real files, see docs/RUST-PORT.md M9).
fn dump(args: &[String]) {
    let root = golden_dir().join("../..");
    let pack = lexo_golden::run::pack_named(&args[0], &root).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    for path in &args[1..] {
        match std::fs::read(path) {
            Ok(data) => println!("{path}	{}", lexo_golden::run::parsed_json(&lexo_core::rpy::parse_bytes(&data, &pack))),
            Err(e) => println!("{path}	ERROR {e}"),
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--dump") && args.len() > 2 {
        return dump(&args[1..]);
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
