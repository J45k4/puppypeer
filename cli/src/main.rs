use args::Command;
use clap::Parser;
use puppypeer_core::{PuppyPeer};

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
	let init_logging = match &args.command {
		Some(Command::Tui) | Some(Command::Gui) => false,
		_ => true,
	};
	if init_logging {
		simple_logger::init_with_level(log::Level::Info).unwrap();
	}

	let version_label = utility::get_version_label().unwrap_or("dev");
	log::info!("puppyagent version {}", version_label);

	#[cfg(feature = "rayon")]
	log::info!("rayon enabled");
	#[cfg(feature = "ring")]
	log::info!("ring enabled");

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
				let app_title = format!("PuppyPeer v{}", version_label);
				if let Err(err) = gui::run(app_title) {
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
			let peer = PuppyPeer::new();
			peer.wait().await;
			return;
		}
	}
}
