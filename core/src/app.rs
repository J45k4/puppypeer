use crate::p2p::{
	AuthMethod, CpuInfo, DirEntry, FileWriteAck, InterfaceInfo, PeerReq, PeerRes,
};
use crate::types::FileChunk;
use crate::{
	p2p::{AgentBehaviour, AgentEvent, build_swarm, load_or_generate_keypair},
	state::{Connection, State},
};
use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use futures::executor::block_on;
use libp2p::{PeerId, Swarm, mdns, swarm::SwarmEvent};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::{env, path::Path};
use sysinfo::{Networks, System};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::{
	sync::{
		mpsc::{UnboundedReceiver, UnboundedSender},
		oneshot,
	},
	task::{self, JoinHandle},
};

use libp2p::request_response::OutboundRequestId;

pub struct ReadFileCmd {
	peer_id: libp2p::PeerId,
	path: String,
	offset: u64,
	length: Option<u64>,
	tx: oneshot::Sender<Result<FileChunk>>,
}

pub enum Command {
	Connect {
		peer_id: libp2p::PeerId,
		addr: libp2p::Multiaddr,
	},
	ListDir {
		peer: libp2p::PeerId,
		path: String,
		tx: oneshot::Sender<Result<Vec<DirEntry>>>,
	},
	ListCpus {
		tx: oneshot::Sender<Result<Vec<CpuInfo>>>,
		peer_id: PeerId,
	},
	ReadFile(ReadFileCmd)
}

async fn read_file(path: &str, offset: u64, length: Option<u64>) -> Result<FileChunk> {
	let file = fs::File::open(&path).await?;
	let metadata = file.metadata().await?;
	if metadata.is_dir() {
		bail!("path is a directory")
	}
	let file_len = metadata.len();
	if offset >= file_len {
		return Ok(FileChunk {
			offset,
			data: Vec::new(),
			eof: true,
		});
	}
	let remaining = file_len - offset;
	let to_read = match length {
		Some(l) => l.min(remaining),
		None => remaining,
	};
	let mut reader = tokio::io::BufReader::new(file);
	reader.seek(std::io::SeekFrom::Start(offset)).await?;
	let mut buffer = vec![0u8; to_read as usize];
	let n = reader.read(&mut buffer).await?;
	buffer.truncate(n);
	let eof = offset + n as u64 >= file_len;
	Ok(FileChunk {
		offset,
		data: buffer,
		eof,
	})
}

async fn write_file(path: &str, offset: u64, data: &[u8]) -> Result<FileWriteAck> {
	// Open (or create) file with write capability
	let mut file = match fs::OpenOptions::new()
		.create(true)
		.write(true)
		.read(true)
		.open(&path)
		.await
	{
		Ok(f) => f,
		Err(e) => return Err(anyhow!("open failed: {}", e)),
	};
	// Ensure we don't overflow length when extending
	let current_len = match file.metadata().await {
		Ok(m) => m.len(),
		Err(e) => return Err(anyhow!("metadata failed: {}", e)),
	};
	let required_len = match offset.checked_add(data.len() as u64) {
		Some(v) => v,
		None => return Err(anyhow!("length overflow")),
	};
	if required_len > current_len {
		if let Err(e) = file.set_len(required_len).await {
			return Err(anyhow!("set_len failed: {}", e));
		}
	}
	if let Err(e) = file.seek(std::io::SeekFrom::Start(offset)).await {
		return Err(anyhow!("seek failed: {}", e));
	}
	if let Err(e) = file.write_all(data).await {
		return Err(anyhow!("write failed: {}", e));
	}
	Ok(FileWriteAck {
		bytes_written: data.len() as u64,
	})
}

pub struct App {
	state: Arc<Mutex<State>>,
	swarm: Swarm<AgentBehaviour>,
	rx: UnboundedReceiver<Command>,
	pending_requests: HashMap<OutboundRequestId, PendingRequest>,
	system: System,
}

trait ResponseDecoder: Sized + Send + 'static {
	fn decode(response: PeerRes) -> anyhow::Result<Self>;
}

impl ResponseDecoder for Vec<DirEntry> {
	fn decode(response: PeerRes) -> anyhow::Result<Self> {
		match response {
			PeerRes::DirEntries(entries) => Ok(entries),
			other => Err(anyhow!("unexpected response: {:?}", other)),
		}
	}
}

impl ResponseDecoder for Vec<CpuInfo> {
	fn decode(response: PeerRes) -> anyhow::Result<Self> {
		match response {
			PeerRes::Cpus(cpus) => Ok(cpus),
			other => Err(anyhow!("unexpected response: {:?}", other)),
		}
	}
}

impl ResponseDecoder for FileChunk {
	fn decode(response: PeerRes) -> anyhow::Result<Self> {
		match response {
			PeerRes::FileChunk(chunk) => Ok(chunk),
			other => Err(anyhow!("unexpected response: {:?}", other)),
		}
	}
}

trait PendingResponseHandler: Send {
	fn complete(self: Box<Self>, response: PeerRes);
	fn fail(self: Box<Self>, error: anyhow::Error);
}

struct Pending<T: ResponseDecoder> {
	tx: oneshot::Sender<Result<T>>,
}

impl<T: ResponseDecoder> Pending<T> {
	fn new(tx: oneshot::Sender<Result<T>>) -> PendingRequest {
		Box::new(Self { tx })
	}
}

impl<T: ResponseDecoder> PendingResponseHandler for Pending<T> {
	fn complete(self: Box<Self>, response: PeerRes) {
		let result = match response {
			PeerRes::Error(err) => Err(anyhow!(err)),
			other => T::decode(other),
		};
		let _ = self.tx.send(result);
	}

	fn fail(self: Box<Self>, error: anyhow::Error) {
		let _ = self.tx.send(Err(error));
	}
}

type PendingRequest = Box<dyn PendingResponseHandler>;

impl App {
	pub fn new(state: Arc<Mutex<State>>) -> (Self, tokio::sync::mpsc::UnboundedSender<Command>) {
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

		swarm
			.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap())
			.unwrap();
		{
			if let Ok(mut s) = state.lock() {
				s.me = peer_id;
			}
		}
		(
			App {
				state,
				swarm,
				rx,
				pending_requests: HashMap::new(),
				system: System::new(),
			},
			tx,
		)
	}

	async fn handle_puppy_peer_req(&mut self, req: PeerReq) -> anyhow::Result<PeerRes> {
		log::info!("Received PeerReq: {:?}", req);
		let res = match req {
			PeerReq::ListDir { path } => {
				let entries = Self::collect_dir_entries(&path).await?;
				PeerRes::DirEntries(entries)
			}
			PeerReq::StatFile { path } => {
				let path = Path::new(&path);
				let meta = fs::metadata(path).await?;
				let file_type = meta.file_type();
				let ext = path
					.extension()
					.and_then(|s| s.to_str().map(|s| s.to_string()));
				PeerRes::FileStat(DirEntry {
					name: path
						.file_name()
						.and_then(|s| s.to_str().map(|s| s.to_string()))
						.unwrap_or_default(),
					is_dir: file_type.is_dir(),
					extension: ext,
					size: meta.len(),
					created_at: meta
						.created()
						.ok()
						.and_then(|t| DateTime::<Utc>::from(t).into()),
					modified_at: meta
						.modified()
						.ok()
						.and_then(|t| DateTime::<Utc>::from(t).into()),
					accessed_at: meta
						.accessed()
						.ok()
						.and_then(|t| DateTime::<Utc>::from(t).into()),
				})
			}
			PeerReq::ReadFile {path, offset, length } => PeerRes::FileChunk(read_file(&path, offset, length).await?),
			PeerReq::WriteFile { path, offset, data } => PeerRes::WriteAck(write_file(&path, offset, &data).await?),
			PeerReq::ListCpus => {
				let cpus = self.collect_cpu_info();
				PeerRes::Cpus(cpus)
			}
			PeerReq::ListDisks => PeerRes::Error("ListDisks not implemented".into()),
			PeerReq::ListInterfaces => {
				let networks = Networks::new_with_refreshed_list();
				let infos = networks
					.iter()
					.map(|(name, data)| InterfaceInfo {
						name: name.clone(),
						mac: data.mac_address().to_string(),
						ips: data.ip_networks().iter().map(|ip| ip.to_string()).collect(),
						total_received: data.total_received(),
						total_transmitted: data.total_transmitted(),
						packets_received: data.total_packets_received(),
						packets_transmitted: data.total_packets_transmitted(),
						errors_on_received: data.total_errors_on_received(),
						errors_on_transmitted: data.total_errors_on_transmitted(),
						mtu: data.mtu(),
					})
					.collect();
				PeerRes::Interfaces(infos)
			}
			PeerReq::Authenticate { method } => match method {
				AuthMethod::Token { token } => todo!(),
				AuthMethod::Credentials { username, password } => todo!(),
			},
			PeerReq::CreateUser {
				username,
				password,
				roles,
				permissions,
			} => {
				let mut state = self.state.lock().unwrap();
				state.create_user(username.clone(), password)?;
				PeerRes::UserCreated { username }
			}
			PeerReq::CreateToken {
				username,
				label,
				expires_in,
				permissions,
			} => {
				let mut state = self.state.lock().unwrap();
				if !state.users.iter().any(|u| u.name == username) {
					return Ok(PeerRes::Error("User does not exist".into()));
				}
				PeerRes::TokenIssued {
					token: "".into(),
					token_id: "".into(),
					username: username.clone(),
					permissions: Vec::new(),
					expires_at: None,
				}
			}
			PeerReq::GrantAccess { .. } => PeerRes::Error("GrantAccess not implemented".into()),
			PeerReq::ListUsers => PeerRes::Error("ListUsers not implemented".into()),
			PeerReq::ListTokens { .. } => PeerRes::Error("ListTokens not implemented".into()),
			PeerReq::RevokeToken { .. } => PeerRes::Error("RevokeToken not implemented".into()),
			PeerReq::RevokeUser { .. } => PeerRes::Error("RevokeUser not implemented".into()),
		};
		Ok(res)
	}

	fn collect_cpu_info(&mut self) -> Vec<CpuInfo> {
		self.system.refresh_cpu_usage();
		self.system
			.cpus()
			.iter()
			.map(|cpu| CpuInfo {
				name: cpu.name().to_string(),
				usage: cpu.cpu_usage(),
				frequency_hz: cpu.frequency(),
			})
			.collect()
	}

	async fn collect_dir_entries(path: &str) -> Result<Vec<DirEntry>> {
		let mut entries = Vec::new();
		let mut reader = fs::read_dir(path).await?;
		while let Some(entry) = reader.next_entry().await? {
			let file_type = entry.file_type().await?;
			let metadata = match entry.metadata().await {
				Ok(m) => m,
				Err(err) => {
					log::warn!("metadata failed for {:?}: {err}", entry.path());
					continue;
				}
			};
			let extension = entry
				.path()
				.extension()
				.and_then(|s| s.to_str().map(|s| s.to_string()));
			entries.push(DirEntry {
				name: entry.file_name().to_string_lossy().to_string(),
				is_dir: file_type.is_dir(),
				extension,
				size: metadata.len(),
				created_at: metadata
					.created()
					.ok()
					.and_then(|t| DateTime::<Utc>::from(t).into()),
				modified_at: metadata
					.modified()
					.ok()
					.and_then(|t| DateTime::<Utc>::from(t).into()),
				accessed_at: metadata
					.accessed()
					.ok()
					.and_then(|t| DateTime::<Utc>::from(t).into()),
			});
		}
		entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
			(true, false) => std::cmp::Ordering::Less,
			(false, true) => std::cmp::Ordering::Greater,
			_ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
		});
		Ok(entries)
	}

	async fn handle_agent_event(&mut self, event: AgentEvent) {
		match event {
			AgentEvent::Ping(event) => {
				log::info!("Ping event: {:?}", event);
			}
			AgentEvent::PuppyPeer(event) => {
				match event {
					libp2p::request_response::Event::Message {
						peer: _,
						connection_id: _,
						message,
					} => {
						match message {
							libp2p::request_response::Message::Request {
								request_id: _,
								request,
								channel,
							} => {
								if let Ok(res) = self.handle_puppy_peer_req(request).await {
									let _ = self
										.swarm
										.behaviour_mut()
										.puppypeer
										.send_response(channel, res);
								} else {
									let _ = self.swarm.behaviour_mut().puppypeer.send_response(
										channel,
										PeerRes::Error("Internal error".into()),
									);
								}
								// self.swarm.behaviour_mut().puppypeer.send_response(channel, PuppyPeerResponse::)
							}
							libp2p::request_response::Message::Response {
								request_id,
								response,
							} => {
								if let Some(pending) = self.pending_requests.remove(&request_id) {
									pending.complete(response);
								}
							}
						}
					}
					libp2p::request_response::Event::OutboundFailure {
						peer,
						connection_id: _,
						request_id,
						error,
					} => {
						log::warn!("outbound request to {} failed: {error}", peer);
						if let Some(pending) = self.pending_requests.remove(&request_id) {
							pending.fail(anyhow!("request failed: {error}"));
						}
					}
					libp2p::request_response::Event::InboundFailure {
						peer,
						connection_id: _,
						request_id: _,
						error,
					} => {
						log::warn!("inbound failure from {}: {error}", peer);
					}
					libp2p::request_response::Event::ResponseSent {
						peer,
						connection_id: _,
						request_id: _,
					} => {
						log::debug!("response sent to {}", peer);
					}
				}
			}
			AgentEvent::Mdns(event) => match event {
				mdns::Event::Discovered(items) => {
					for (peer_id, multiaddr) in items {
						log::info!("mDNS discovered peer {} at {}", peer_id, multiaddr);
						if let Ok(mut state) = self.state.lock() {
							state.peer_discovered(peer_id, multiaddr.clone());
						}
						self.swarm.dial(multiaddr).unwrap();
					}
				}
				mdns::Event::Expired(items) => {
					for (peer_id, multiaddr) in items {
						log::info!("mDNS expired peer {} at {}", peer_id, multiaddr);
						if let Ok(mut state) = self.state.lock() {
							state.peer_expired(peer_id, multiaddr);
						}
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
				if let Ok(mut state) = self.state.lock() {
					state.connections.push(Connection {
						peer_id,
						connection_id,
					});
				}
			}
			SwarmEvent::ConnectionClosed {
				peer_id,
				connection_id,
				endpoint: _,
				num_established: _,
				cause: _,
			} => {
				log::info!("Disconnected from peer {}", peer_id);
				if let Ok(mut state) = self.state.lock() {
					state
						.connections
						.retain(|c| c.connection_id != connection_id);
				}
			}
			SwarmEvent::IncomingConnection {
				connection_id: _,
				local_addr: _,
				send_back_addr: _,
			} => {}
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
			SwarmEvent::Dialing {
				peer_id: _,
				connection_id: _,
			} => {}
			SwarmEvent::NewExternalAddrCandidate { address: _ } => {}
			SwarmEvent::ExternalAddrConfirmed { address: _ } => {}
			SwarmEvent::ExternalAddrExpired { address: _ } => {}
			SwarmEvent::NewExternalAddrOfPeer {
				peer_id: _,
				address: _,
			} => {}
			SwarmEvent::NewListenAddr {
				listener_id: _,
				address,
			} => {
				log::info!("listener address added: {:?}", address);
			}
			SwarmEvent::ExpiredListenAddr {
				listener_id: _,
				address: _,
			} => {}
			SwarmEvent::ListenerClosed {
				listener_id: _,
				addresses: _,
				reason: _,
			} => {}
			SwarmEvent::ListenerError {
				listener_id: _,
				error: _,
			} => {}
			_ => {}
		}
	}

	async fn handle_cmd(&mut self, cmd: Command) {
		match cmd {
			Command::Connect { peer_id: _, addr } => {
				if let Err(err) = self.swarm.dial(addr) {
					log::error!("dial failed: {err}");
				}
			}
			Command::ListDir { peer, path, tx } => {
				let is_self = {
					self.state.lock().map(|state| state.me == peer).unwrap_or(false)
				};
				if is_self {
					let result = Self::collect_dir_entries(&path).await;
					let _ = tx.send(result);
					return;
				}
				let request_id = self
					.swarm
					.behaviour_mut()
					.puppypeer
					.send_request(&peer, PeerReq::ListDir { path: path.clone() });
				if let Some(prev) = self
					.pending_requests
					.insert(request_id, Pending::<Vec<DirEntry>>::new(tx))
				{
					prev.fail(anyhow!("pending ListDir request was replaced"));
				}
			}
			Command::ListCpus { tx, peer_id } => {
				if self.state.lock().unwrap().me == peer_id {
					let cpus = self.collect_cpu_info();
					let _ = tx.send(Ok(cpus));
					return;
				}
				let request_id = self
					.swarm
					.behaviour_mut()
					.puppypeer
					.send_request(&peer_id, PeerReq::ListCpus);
				self
					.pending_requests
					.insert(request_id, Pending::<Vec<CpuInfo>>::new(tx));
			}
			Command::ReadFile(req) => {
				if self.state.lock().unwrap().me == req.peer_id {
					let chunk = read_file(&req.path, req.offset, req.length).await;
					let _ = req.tx.send(chunk);
					return;
				}
				let request_id = self.swarm.behaviour_mut().puppypeer.send_request(
					&req.peer_id,
					PeerReq::ReadFile { path: req.path.clone(), offset: req.offset, length: req.length },
				);
				self
					.pending_requests
					.insert(request_id, Pending::<FileChunk>::new(req.tx));
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
					self.handle_cmd(cmd).await;
				}
			}
		}
	}
}

pub struct PuppyPeer {
	shutdown_tx: Option<oneshot::Sender<()>>,
	handle: JoinHandle<()>,
	state: Arc<Mutex<State>>,
	cmd_tx: UnboundedSender<Command>,
}

impl PuppyPeer {
	pub fn new() -> Self {
		let state = Arc::new(Mutex::new(State::default()));
		// channel to request shutdown
		let (shutdown_tx, shutdown_rx) = oneshot::channel();
		let state_clone = state.clone();
		let (mut app, cmd_tx) = App::new(state_clone);
		let mut shutdown_rx = shutdown_rx;
		let handle: JoinHandle<()> = tokio::spawn(async move {
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
			state,
			cmd_tx,
		}
	}

	pub fn state(&self) -> Arc<Mutex<State>> {
		self.state.clone()
	}

	pub async fn list_dir(&self, peer: PeerId, path: impl Into<String>) -> Result<Vec<DirEntry>> {
		let path = path.into();
		let (tx, rx) = oneshot::channel();
		self.cmd_tx.send(Command::ListDir { peer, path, tx }).map_err(|e| anyhow!("failed to send ListDir command: {e}"))?;
		rx.await.map_err(|e| anyhow!("ListDir response channel closed: {e}"))?
	}

	pub async fn list_dir_local(&self, path: impl Into<String>) -> Result<Vec<DirEntry>> {
		let peer = {
			let state = self.state.lock().unwrap();
			state.me
		};
		self.list_dir(peer, path).await
	}

	pub fn list_dir_blocking(&self, path: impl Into<String>) -> Result<Vec<DirEntry>> {
		let sender = self.cmd_tx.clone();
		let state = self.state.clone();
		let path = path.into();
		let handle = tokio::runtime::Handle::current();
		task::block_in_place(move || {
			let peer = {
				let state = state.lock().unwrap();
				state.me
			};
			handle.block_on(async move {
				let (tx, rx) = oneshot::channel();
				sender
					.send(Command::ListDir { peer, path, tx })
					.map_err(|e| anyhow!("failed to send ListDir command: {e}"))?;
				rx.await
					.map_err(|e| anyhow!("ListDir response channel closed: {e}"))?
			})
		})
	}

	pub async fn list_cpus(&self, peer_id: PeerId) -> Result<Vec<CpuInfo>> {
		let (tx, rx) = oneshot::channel();
		self.cmd_tx
			.send(Command::ListCpus { tx, peer_id })
			.map_err(|e| anyhow!("failed to send ListCpus command: {e}"))?;
		rx.await
			.map_err(|e| anyhow!("ListCpus response channel closed: {e}"))?
	}

	pub fn list_cpus_blocking(&self, peer_id: PeerId) -> Result<Vec<CpuInfo>> {
		let cpus = block_on(self.list_cpus(peer_id));
		cpus
	}

	pub async fn list_dir_remote(
		&self,
		peer: libp2p::PeerId,
		path: impl Into<String>,
	) -> Result<Vec<DirEntry>> {
		let path = path.into();
		let (tx, rx) = oneshot::channel();
		self.cmd_tx
			.send(Command::ListDir { peer, path, tx })
			.map_err(|e| anyhow!("failed to send FetchDir command: {e}"))?;
		rx.await
			.map_err(|e| anyhow!("FetchDir response channel closed: {e}"))?
	}

	pub fn list_dir_remote_blocking(
		&self,
		peer: libp2p::PeerId,
		path: impl Into<String>,
	) -> Result<Vec<DirEntry>> {
		let sender = self.cmd_tx.clone();
		let path = path.into();
		let handle = tokio::runtime::Handle::current();
		task::block_in_place(move || {
			handle.block_on(async move {
				let (tx, rx) = oneshot::channel();
				sender
					.send(Command::ListDir { peer, path, tx })
					.map_err(|e| anyhow!("failed to send FetchDir command: {e}"))?;
				rx.await
					.map_err(|e| anyhow!("FetchDir response channel closed: {e}"))?
			})
		})
	}

	pub async fn read_file(
		&self,
		peer: libp2p::PeerId,
		path: impl Into<String>,
		offset: u64,
		length: Option<u64>,
	) -> Result<FileChunk> {
		let path = path.into();
		let (tx, rx) = oneshot::channel();
		self
			.cmd_tx
			.send(Command::ReadFile(ReadFileCmd {
				peer_id: peer,
				path,
				offset,
				length,
				tx,
			}))
			.map_err(|e| anyhow!("failed to send ReadFile command: {e}"))?;
		rx.await.map_err(|e| anyhow!("ReadFile response channel closed: {e}"))?
	}

	pub async fn read_file_local(
		&self,
		path: impl Into<String>,
		offset: u64,
		length: Option<u64>,
	) -> Result<FileChunk> {
		let peer = {
			let state = self.state.lock().unwrap();
			state.me
		};
		self.read_file(peer, path, offset, length).await
	}

	pub async fn read_file_remote(
		&self,
		peer: libp2p::PeerId,
		path: impl Into<String>,
		offset: u64,
		length: Option<u64>,
	) -> Result<FileChunk> {
		self.read_file(peer, path, offset, length).await
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
