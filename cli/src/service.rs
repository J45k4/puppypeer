use std::sync::Arc;

use crate::types::Context;
use puppyagent_core::{p2p, wait_group::WaitGroup};

pub async fn start(ctx: Arc<Context>, binds: Vec<String>, peers: Vec<String>, wg: WaitGroup) {
	// {
	// 	let wg = wg.register();
	// 	tokio::spawn(async move {
	// 		p2p::start(binds, peers, wg).await.unwrap();
	// 	});
	// }

	// // HTTP server disabled (module not present)
	// log::info!("waiting for tasks to finish");
	// wg.wait().await;
	// log::info!("all tasks finished");
}
