use std::borrow::Cow;
use std::sync::Arc;
use std::u64;

use failure::bail;
use tokio::sync::RwLock;
use tokio::sync::mpsc;

use crate::agent_types::Command;
use crate::types::AgentState;

#[derive(Default)]
struct InternalState {
	agent_id: String,
	cmd_sender: Option<mpsc::Sender<Command>>,
	connected: bool,
	total_bytes_send: u64,
	total_bytes_received: u64,
	current_send_speed: u64,
	current_recv_speed: u64,
	computer_name: String
}

pub struct Agent {
	internal: Arc<RwLock<InternalState>>

}

impl Clone for Agent {
	fn clone(&self) -> Self {
		Agent{
			internal: self.internal.clone()
		}
	}
}

impl Agent {
	pub fn new(agent_id: &str) -> Agent {
		Agent{
			internal: Arc::new(RwLock::new(InternalState{
				agent_id: agent_id.to_string(),
				..Default::default()
			}))
		}
	}

	pub async fn get_state(&self) -> AgentState {
		let internal = self.internal.read().await;

		AgentState {
			agent_id: internal.agent_id.clone(),
			computer_name: internal.computer_name.clone(),
			connected: internal.connected,
			total_bytes_send: internal.total_bytes_send,
			total_bytes_received: internal.total_bytes_received,
			current_recv_speed: internal.current_recv_speed,
			current_send_speed: internal.current_send_speed
		}
	}

	pub async fn get_id(&self) -> String {
		let internal = self.internal.read().await;

		internal.agent_id.clone()
	}
 
	pub async fn get_computer_name(&self) -> Cow<'static, String> {
		Cow::Owned("".to_string())
	}

	pub async fn is_connected(&self) -> bool {
		let internal = self.internal.read().await;
		
		internal.connected
	}

	pub async fn set_connected(&self, connected: bool) {
		let mut internal = self.internal.write().await;

		internal.connected = connected;
	}

	async fn get_total_send_bytes(&self) -> u64 {
		let internal = self.internal.read().await;

		internal.total_bytes_send
	}

	async fn get_total_received_bytes(&self) -> u64 {
		let internal = self.internal.read().await;

		internal.total_bytes_received
	}

	async fn get_current_send_speed(&self) -> u64 {
		let internal = self.internal.read().await;

		internal.current_send_speed
	}

	async fn get_current_recv_speed(&self) -> u64 {
		let internal = self.internal.read().await;

		internal.current_recv_speed
	}

	pub async fn send_command(&self, cmd: Command) -> Result<(), failure::Error> {
		let internal = self.internal.write().await;

		let cmd_sender = match &internal.cmd_sender {
			Some(r) => r,
			None => {
				bail!("Now command sender")
			}
		};

		match cmd_sender.send(cmd).await {
			Ok(_) => {},
			Err(e) => {
				bail!("Failed to send command forward");
			}
		}

		Ok(())
	}

	pub async fn set_cmd_sender(&self, new_cmd_sender: mpsc::Sender<Command>) {
		let mut internal = self.internal.write().await;

		internal.cmd_sender = Some(new_cmd_sender)
	}

	pub async fn remove_cmd_sender(&self) {
		let mut internal = self.internal.write().await;

		internal.cmd_sender = None
	}
}