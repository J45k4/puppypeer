use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::broadcast;
use crate::agent::Agent;

pub struct AgentRegister {
	agents: Arc<RwLock<HashMap<String, Agent>>>,
	chan: broadcast::Sender<Agent>
}

impl Clone for AgentRegister {
	fn clone(&self) -> Self {
		AgentRegister{
			agents: self.agents.clone(),
			chan: self.chan.clone()
		}
	}
}

impl AgentRegister {
	pub fn new() -> AgentRegister {
		let (chan, _) = broadcast::channel(10);

		AgentRegister{
			agents: Arc::new(RwLock::new(HashMap::new())),
			chan: chan
		}
	}

	pub async fn add_agent(&self, agent: Agent) {
		let mut agents = self.agents.write().await;

		agents.insert(agent.get_id().await, agent);
	}

	pub async fn get_agent(&self, agent_id: &str) -> Option<Agent> {
		let agents = self.agents.read().await;

		match agents.get(agent_id) {
			Some(r) => Some(r.clone()),
			None => None
		}
	}

	pub async fn get_agents(&self) -> Vec<Agent> {
		let agents = self.agents.read().await;

		agents.values().map(|a| a.clone()).collect()
	}

	pub async fn emit_agent_change(&self, agent: Agent) {
		let mut agents = self.agents.write().await;

		let agent_id = agent.get_id().await;

		if agents.contains_key(&agent_id) == false {
			log::info!("Inserting new agent to register {}", agent_id);

			agents.insert(agent_id, agent.clone());
		}

		let chan = self.chan.clone();

		chan.send(agent).unwrap_or_default();
	}

	pub fn sub_to_changes(&self) -> broadcast::Receiver<Agent> {
		self.chan.subscribe()
	}
}