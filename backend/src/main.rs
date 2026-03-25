fn main() {
    let host = std::env::var("EDGEL_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("PORT")
        .ok()
        .or_else(|| std::env::var("EDGEL_PORT").ok())
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(4040);

    if let Err(error) = edgelvm::serve(&host, port) {
        eprintln!("GoldEdge Browser failed: {error}");
        std::process::exit(1);
    }
}
