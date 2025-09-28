use std::{env, path::Path};

use crate::{
	p2p::{AgentBehaviour, AgentEvent, build_swarm, load_or_generate_keypair},
	state::{Connection, State},
};
use futures::StreamExt;
use libp2p::{PeerId, Swarm, mdns, swarm::SwarmEvent};
use tokio::{
	sync::{mpsc::UnboundedReceiver, oneshot},
	task::JoinHandle,
};

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
		if !key_path.exists() {
			log::warn!(
				"keypair file {} does not exist, generating new keypair",
				key_path.display()
			);
		}
		let id_keys = load_or_generate_keypair(key_path).unwrap();
		let peer_id = PeerId::from(id_keys.public());

		let mut swarm = build_swarm(id_keys, peer_id).unwrap();
		let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

		swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap()).unwrap();

		App {
			state: State::default(),
			swarm,
			rx,
		}
	}

	async fn handle_agent_event(&mut self, event: AgentEvent) {
		match event {
			AgentEvent::Ping(event) => {
				log::info!("Ping event: {:?}", event);
			},
			AgentEvent::FileMeta(event) => {},
			AgentEvent::Control(event) => {},
			AgentEvent::Mdns(event) => match event {
				mdns::Event::Discovered(items) => {
					for (peer_id, multiaddr) in items {
						log::info!("mDNS discovered peer {} at {}", peer_id, multiaddr);
						self.state.peer_discovered(peer_id, multiaddr.clone());
						self.swarm.dial(multiaddr).unwrap();
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
		log::info!("SwarmEvent: {:?}", event);
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
				log::info!("Connected to peer {}", peer_id);
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
				log::info!("Disconnected from peer {}", peer_id);
				self.state
					.connections
					.retain(|c| c.connection_id != connection_id);
			}
			SwarmEvent::IncomingConnection {
				connection_id,
				local_addr,
				send_back_addr,
			} => {}
			SwarmEvent::IncomingConnectionError {
				connection_id,
				local_addr,
				send_back_addr,
				error,
				peer_id,
			} => {}
			SwarmEvent::OutgoingConnectionError {
				connection_id,
				peer_id,
				error,
			} => {}
			SwarmEvent::NewListenAddr {
				listener_id,
				address,
			} => {
				log::info!("listener {:?} listening on {:?}", listener_id, address);
			}
			SwarmEvent::ExpiredListenAddr {
				listener_id,
				address,
			} => {}
			SwarmEvent::ListenerClosed {
				listener_id,
				addresses,
				reason,
			} => {}
			SwarmEvent::ListenerError { listener_id, error } => {}
			SwarmEvent::Dialing {
				peer_id,
				connection_id,
			} => {}
			SwarmEvent::NewExternalAddrCandidate { address } => {}
			SwarmEvent::ExternalAddrConfirmed { address } => {}
			SwarmEvent::ExternalAddrExpired { address } => {}
			SwarmEvent::NewExternalAddrOfPeer { peer_id, address } => {}
			_ => {}
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
		//log::info!("run");
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
	shutdown_tx: Option<oneshot::Sender<()>>,
	handle: JoinHandle<()>,
}

impl PuppyPeer {
	pub fn new() -> Self {
		// channel to request shutdown
		let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
		let handle: JoinHandle<()> = tokio::spawn(async move {
			let mut app = App::new();
			loop {
				tokio::select! {
					_ = &mut shutdown_rx => {
						log::info!("PuppyPeer shutting down");
						break;
					}
					_ = app.run() => {}
				}
			}
		});

		PuppyPeer {
			shutdown_tx: Some(shutdown_tx),
			handle,
		}
	}

	pub fn get_state() -> State {
		State::default()
	}

	/// Wait for the peer until Ctrl+C (SIGINT) then perform a graceful shutdown.
	pub async fn wait(mut self) {
		// Wait for Ctrl+C
		if let Err(e) = tokio::signal::ctrl_c().await {
			log::error!("failed to listen for ctrl_c: {e}");
		}
		log::info!("interrupt received, shutting down");
		if let Some(tx) = self.shutdown_tx.take() {
			let _ = tx.send(());
		}
		// Await the background task
		if let Err(e) = self.handle.await {
			log::error!("task join error: {e}");
		}
	}
}
