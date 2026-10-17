//! `cargo run -p lexo-golden [-- --list] [-- --strict]`: runs every golden case and prints "passed of total" per group.
//! `--list` also prints every case that does not pass, with the reason; `--strict` exits with 1 unless everything passes.
use lexo_golden::{golden_dir, inventory, run_case, summarize, Outcome};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
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
