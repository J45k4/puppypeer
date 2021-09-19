use std::net::ToSocketAddrs;

use client::ServerClient;
use handle_commands::handle_commands;
use log::LevelFilter;
use url::Url;
use anyhow::anyhow;

use crate::connection::connect;

mod client;
mod handle_commands;
mod file_metadata_cache;
mod file_watcher;
mod scan;
mod types;
mod connection;

#[tokio::main]
async fn main() {
    env_logger::Builder::new()
        .filter_level(LevelFilter::Debug)
        .format_timestamp(None)
        .init();

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    log::info!("version {}", VERSION);

	// let url = Url::parse("localhost").unwrap();

	let host = "127.0.0.1:4433";

	connect(host.parse().unwrap()).await;

	// let remote = (host)
	// 	.to_socket_addrs().unwrap()
	// 	.next()
	// 	.ok_or_else(|| anyhow!("couldn't resolve to an address")).unwrap();

	// let mut endpoint = quinn::Endpoint::builder();
	// // let mut client_config = quinn::ClientConfigBuilder::default();

	// // endpoint.default_client_config(client_config.build());

	// let (endpoint, _) = endpoint.bind(&"[::]:0".parse().unwrap()).unwrap();

	// let new_conn = endpoint
	// 	.connect(&remote, "mikko").unwrap()
	// 	.await
	// 	.map_err(|e| anyhow!("failed to connect: {}", e)).unwrap();
}


    // let client = ServerClient::connect("http://[::1]:45000").await.unwrap();

    // handle_commands(client).await;