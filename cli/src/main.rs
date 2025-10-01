use args::Command;
use clap::Parser;
use puppyagent_core::{PuppyPeer, wait_group::WaitGroup};
use uuid::Uuid;

mod args;
mod gui;
mod installer;
mod service;
mod shell;
mod types;
mod updater;
mod utility;

#[tokio::main]
async fn main() {
	let args = args::Args::parse();
	let version_label = utility::get_version_label().unwrap_or("dev");
	log::info!("puppyagent version {}", version_label);

	#[cfg(feature = "rayon")]
	log::info!("rayon enabled");
	#[cfg(feature = "ring")]
	log::info!("ring enabled");

	// Placeholder: node_id generated; database disabled
	let _node_id = *Uuid::new_v4().as_bytes();

	let _wg = WaitGroup::new();

	match args.command {
		Some(command) => match command {
			Command::Copy { src, dest } => {
				log::info!("copying {} to {}", src, dest);
			}
			Command::Scan { path } => {
				log::info!("scanning {} (database disabled)", path);
				return;
			}
			Command::Install => {
				installer::install();
				return;
			}
			Command::Uninstall => {
				installer::uninstall();
				return;
			}
			Command::Update { version } => {
				if let Err(err) = updater::update(version.as_deref()).await {
					log::error!("failed to update: {err:?}");
					std::process::exit(1);
				}
				log::info!("update completed successfully");
				return;
			}
			Command::Tui => {
				if let Err(err) = shell::run() {
					log::error!("shell error: {err:?}");
					std::process::exit(1);
				}
				return;
			}
			Command::Gui => {
				if let Err(err) = gui::run() {
					log::error!("gui error: {err:?}");
					std::process::exit(1);
				}
				return;
			}
			Command::Daemon => {
				log::warn!("Daemon mode: disabled modules");
				return;
			}
		},
		None => {
			simple_logger::init_with_level(log::Level::Info).unwrap();
			let peer = PuppyPeer::new();
			peer.wait().await;
			return;
		}
	}
}
