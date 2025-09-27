use std::{env, path::Path};

use crate::{
	p2p::{AgentBehaviour, AgentEvent, build_swarm, load_or_generate_keypair},
	state::{Connection, State},
};
use futures::StreamExt;
use libp2p::{PeerId, Swarm, mdns, swarm::SwarmEvent};
use tokio::sync::mpsc::UnboundedReceiver;

pub enum Command {
	Connect {
		peer_id: libp2p::PeerId,
		addr: libp2p::Multiaddr,
	},
}

pub struct App {
	state: State,
	swarm: Swarm<AgentBehaviour>,
	rx: UnboundedReceiver<Command>,
}

impl App {
	pub fn new() -> Self {
		let key_path = env::var("KEYPAIR").unwrap_or_else(|_| String::from("peer_keypair.bin"));
		let key_path = Path::new(&key_path);
		let id_keys = load_or_generate_keypair(key_path).unwrap();
		let peer_id = PeerId::from(id_keys.public());

		let swarm = build_swarm(id_keys, peer_id).unwrap();
		let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

		App {
			state: State::default(),
			swarm,
			rx,
		}
	}

	async fn handle_agent_event(&mut self, event: AgentEvent) {
		match event {
			AgentEvent::Ping(event) => todo!(),
			AgentEvent::FileMeta(event) => todo!(),
			AgentEvent::Control(event) => todo!(),
			AgentEvent::Mdns(event) => match event {
				mdns::Event::Discovered(items) => {
					for (peer_id, multiaddr) in items {
						log::info!("mDNS discovered peer {} at {}", peer_id, multiaddr);
						self.state.peer_discovered(peer_id, multiaddr);
					}
				}
				mdns::Event::Expired(items) => {
					for (peer_id, multiaddr) in items {
						log::info!("mDNS expired peer {} at {}", peer_id, multiaddr);
						self.state.peer_expired(peer_id, multiaddr);
					}
				}
			},
		}
	}

	async fn handle_swarm_event(&mut self, event: SwarmEvent<AgentEvent>) {
		match event {
			SwarmEvent::Behaviour(b) => self.handle_agent_event(b).await,
			SwarmEvent::ConnectionEstablished {
				peer_id,
				connection_id,
				endpoint,
				num_established,
				concurrent_dial_errors,
				established_in,
			} => {
				self.state.connections.push(Connection {
					peer_id,
					connection_id,
				});
			}
			SwarmEvent::ConnectionClosed {
				peer_id,
				connection_id,
				endpoint,
				num_established,
				cause,
			} => {
				self.state
					.connections
					.retain(|c| c.connection_id != connection_id);
			}
			SwarmEvent::IncomingConnection {
				connection_id,
				local_addr,
				send_back_addr,
			} => todo!(),
			SwarmEvent::IncomingConnectionError {
				connection_id,
				local_addr,
				send_back_addr,
				error,
				peer_id,
			} => todo!(),
			SwarmEvent::OutgoingConnectionError {
				connection_id,
				peer_id,
				error,
			} => todo!(),
			SwarmEvent::NewListenAddr {
				listener_id,
				address,
			} => todo!(),
			SwarmEvent::ExpiredListenAddr {
				listener_id,
				address,
			} => todo!(),
			SwarmEvent::ListenerClosed {
				listener_id,
				addresses,
				reason,
			} => todo!(),
			SwarmEvent::ListenerError { listener_id, error } => todo!(),
			SwarmEvent::Dialing {
				peer_id,
				connection_id,
			} => todo!(),
			SwarmEvent::NewExternalAddrCandidate { address } => todo!(),
			SwarmEvent::ExternalAddrConfirmed { address } => todo!(),
			SwarmEvent::ExternalAddrExpired { address } => todo!(),
			SwarmEvent::NewExternalAddrOfPeer { peer_id, address } => todo!(),
			_ => todo!(),
		}
	}

	async fn handle_cmd(&mut self, cmd: Command) {
		match cmd {
			Command::Connect { peer_id, addr } => {
				self.swarm.dial(addr).unwrap();
			}
		}
	}

	pub async fn run(&mut self) {
		tokio::select! {
			event = self.swarm.select_next_some() => {
				self.handle_swarm_event(event).await;
			}
			cmd = self.rx.recv() => {
				if let Some(cmd) = cmd {
					match cmd {
						Command::Connect { peer_id, addr } => {
							self.swarm.dial(addr).unwrap();
						}
					}
				}
			}
		}
	}
}

pub struct PuppyPeer {

}

impl PuppyPeer {
	pub fn new() -> Self {
		PuppyPeer {

		}
	}
}