fn main() {
    let provider = lexo_pro::LexoProProvider::new();
    // renew the licence file quietly when its check is due (a thread: the application never waits for the network)
    std::thread::spawn(|| lexo_pro::licence_client::refresh_when_due(&lexo_core::net::post));
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    if let Err(e) = lexo_pro::serve(&provider, stdin.lock(), stdout.lock()) {
        eprintln!("lexo-pro-sidecar: {e}");
        std::process::exit(1);
    }
}
