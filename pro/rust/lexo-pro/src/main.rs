//! `lexo-pro-sidecar`: the Pro pack as a program. The application (`SidecarProvider` of Core) starts it and talks to it on standard input and output, one JSON line per request.
fn main() {
    let provider = lexo_pro::LexoProProvider::new();
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    if let Err(e) = lexo_pro::serve(&provider, stdin.lock(), stdout.lock()) {
        eprintln!("lexo-pro-sidecar: {e}");
        std::process::exit(1);
    }
}
