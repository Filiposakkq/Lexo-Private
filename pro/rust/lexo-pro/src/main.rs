fn main() {
    let provider = lexo_pro::LexoProProvider::new();
    // check the licence with the server at the start and then every hour it is looked at (a thread: the application never waits for the network). A file younger than six hours is not sent.
    std::thread::spawn(|| loop {
        lexo_pro::licence_client::refresh_if_stale(&lexo_core::net::post);
        std::thread::sleep(std::time::Duration::from_secs(3600));
    });
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    if let Err(e) = lexo_pro::serve(&provider, stdin.lock(), stdout.lock()) {
        eprintln!("lexo-pro-sidecar: {e}");
        std::process::exit(1);
    }
}
