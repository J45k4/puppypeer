use client::ServerClient;
use handle_commands::handle_commands;
use log::LevelFilter;
mod client;
mod handle_commands;
mod file_metadata_cache;
mod file_watcher;
mod scan;
mod types;

#[tokio::main]
async fn main() {
    env_logger::Builder::new()
        .filter_level(LevelFilter::Debug)
        .format_timestamp(None)
        .init();

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    log::info!("version {}", VERSION);

    let client = ServerClient::connect("http://[::1]:45000").await.unwrap();

    handle_commands(client).await;
}
