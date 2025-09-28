use std::{env, path::Path};
use std::sync::{Arc, Mutex};

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
	state: Arc<Mutex<State>>,
	swarm: Swarm<AgentBehaviour>,
	rx: UnboundedReceiver<Command>,
}

impl App {
	pub fn new(state: Arc<Mutex<State>>) -> Self {
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
		let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();

		swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap()).unwrap();
		{
			if let Ok(mut s) = state.lock() { s.me = peer_id; }
		}
		App { state, swarm, rx }
	}

	async fn handle_agent_event(&mut self, event: AgentEvent) {
		match event {
			AgentEvent::Ping(event) => {
				log::info!("Ping event: {:?}", event);
			},
			AgentEvent::FileMeta(_event) => {},
			AgentEvent::Control(_event) => {},
			AgentEvent::Mdns(event) => match event {
				mdns::Event::Discovered(items) => {
					for (peer_id, multiaddr) in items {
						log::info!("mDNS discovered peer {} at {}", peer_id, multiaddr);
						if let Ok(mut state) = self.state.lock() { state.peer_discovered(peer_id, multiaddr.clone()); }
						self.swarm.dial(multiaddr).unwrap();
					}
				}
				mdns::Event::Expired(items) => {
					for (peer_id, multiaddr) in items {
						log::info!("mDNS expired peer {} at {}", peer_id, multiaddr);
						if let Ok(mut state) = self.state.lock() { state.peer_expired(peer_id, multiaddr); }
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
				endpoint: _,
				num_established: _,
				concurrent_dial_errors: _,
				established_in: _,
			} => {
				log::info!("Connected to peer {}", peer_id);
				if let Ok(mut state) = self.state.lock() { state.connections.push(Connection { peer_id, connection_id }); }
			}
			SwarmEvent::ConnectionClosed {
				peer_id,
				connection_id,
				endpoint: _,
				num_established: _,
				cause: _,
			} => {
				log::info!("Disconnected from peer {}", peer_id);
				if let Ok(mut state) = self.state.lock() { state.connections.retain(|c| c.connection_id != connection_id); }
			}
			SwarmEvent::IncomingConnection { connection_id: _, local_addr: _, send_back_addr: _ } => {}
			SwarmEvent::IncomingConnectionError {
				connection_id: _,
				local_addr: _,
				send_back_addr: _,
				error: _,
				peer_id: _,
			} => {}
			SwarmEvent::OutgoingConnectionError {
				connection_id: _,
				peer_id: _,
				error: _,
			} => {}
			SwarmEvent::Dialing { peer_id: _, connection_id: _ } => {}
			SwarmEvent::NewExternalAddrCandidate { address: _ } => {}
			SwarmEvent::ExternalAddrConfirmed { address: _ } => {}
			SwarmEvent::ExternalAddrExpired { address: _ } => {}
			SwarmEvent::NewExternalAddrOfPeer { peer_id: _, address: _ } => {}
			SwarmEvent::NewListenAddr { listener_id: _, address } => {
				log::info!("listener address added: {:?}", address);
			}
			SwarmEvent::ExpiredListenAddr { listener_id: _, address: _ } => {}
			SwarmEvent::ListenerClosed { listener_id: _, addresses: _, reason: _ } => {}
            SwarmEvent::ListenerError { listener_id: _, error: _ } => {}
            _ => {}
        }
	}

	async fn handle_cmd(&mut self, cmd: Command) {
		match cmd {
			Command::Connect { peer_id: _, addr } => {
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
						Command::Connect { peer_id: _, addr } => {
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
	state: Arc<Mutex<State>>,
}

impl PuppyPeer {
	pub fn new() -> Self {
		let state = Arc::new(Mutex::new(State::default()));
		// channel to request shutdown
		let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
		let state_clone = state.clone();
		let handle: JoinHandle<()> = tokio::spawn(async move {
			let mut app = App::new(state_clone);
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

		PuppyPeer { shutdown_tx: Some(shutdown_tx), handle, state }
	}

	pub fn state(&self) -> Arc<Mutex<State>> { self.state.clone() }

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
