use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum MessageFromClient {
	#[serde(rename = "subscribeToAgents")]
    SubscribeToAgents
}

#[derive(Serialize, Deserialize)]
pub struct AgentState {
	pub agent_id: String,
    pub computer_name: String,
    pub connected: bool,
    pub total_bytes_send: u64,
    pub total_bytes_received: u64,
    pub current_send_speed: u64,
    pub current_recv_speed: u64
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MessageToClient {
	#[serde(rename = "agentState")]
    AgentState(AgentState)
}