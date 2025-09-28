use anyhow::Result;
use libp2p::identity::Keypair;
use libp2p::multiaddr::Protocol;
use libp2p::request_response::{
	self, Config as RequestResponseConfig, Event as RequestResponseEvent,
	Message as RequestResponseMessage, ProtocolSupport,
};
use libp2p::{
	Multiaddr, PeerId, StreamProtocol, Swarm, SwarmBuilder, identity, noise,
	swarm::{NetworkBehaviour, SwarmEvent},
	tcp, yamux,
};
use libp2p::{mdns, ping};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{IpAddr, UdpSocket};
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::{Disks, Networks, System};
use tokio::sync::Mutex;
use tokio::time::{Duration, interval};
use uuid::Uuid;

use crate::wait_group::WaitGroupGuard;

const PUPPYPEER_PROTOCOL: &str = "/puppypeer/0.0.1";
const MAX_FILE_CHUNK: u64 = 4 * 1024 * 1024; // 4 MiB per transfer chunk
const OWNER_ROLE: &str = "owner";
const VIEWER_ROLE: &str = "viewer";
const DEFAULT_SESSION_TTL: u64 = 60 * 60; // 1 hour sessions for credential auth

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PuppyPeerRequest {
	ListDir {
		path: String,
	},
	StatFile {
		path: String,
	},
	ReadFile {
		path: String,
		offset: u64,
		length: Option<u64>,
	},
	WriteFile {
		path: String,
		offset: u64,
		data: Vec<u8>,
	},
	ListCpus,
	ListDisks,
	ListInterfaces,
		Authenticate {
		method: AuthMethod,
	},
	CreateUser {
		username: String,
		password: String,
		roles: Vec<String>,
		permissions: Vec<PermissionGrant>,
	},
	CreateToken {
		username: String,
		label: Option<String>,
		expires_in: Option<u64>,
		permissions: Vec<PermissionGrant>,
	},
	GrantAccess {
		username: String,
		permissions: Vec<PermissionGrant>,
		merge: bool,
	},
	ListUsers,
	ListTokens {
		username: Option<String>,
	},
	RevokeToken {
		token_id: String,
	},
	RevokeUser {
		username: String,
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PuppyPeerResponse {
	DirEntries(Vec<FileEntry>),
	FileStat(FileEntry),
	FileChunk(FileChunk),
	WriteAck(FileWriteAck),
	Cpus(Vec<CpuInfo>),
	Disks(Vec<DiskInfo>),
	Interfaces(Vec<InterfaceInfo>),
		AuthSuccess {
		session: SessionInfo,
	},
	AuthFailure {
		reason: String,
	},
	UserCreated {
		username: String,
	},
	UserRemoved {
		username: String,
	},
	TokenIssued {
		token: String,
		token_id: String,
		username: String,
		permissions: Vec<PermissionGrant>,
		expires_at: Option<u64>,
	},
	TokenRevoked {
		token_id: String,
	},
	AccessGranted {
		username: String,
		permissions: Vec<PermissionGrant>,
	},
	Users(Vec<UserSummary>),
	Tokens(Vec<TokenInfo>),
	Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileEntry {
	name: String,
	is_dir: bool,
	extension: Option<String>,
	size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileChunk {
	offset: u64,
	data: Vec<u8>,
	eof: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileWriteAck {
	bytes_written: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CpuInfo {
	name: String,
	usage: f32,
	frequency_hz: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiskInfo {
	name: String,
	mount_path: String,
	filesystem: String,
	total_space: u64,
	available_space: u64,
	usage_percent: f32,
	total_read_bytes: u64,
	total_written_bytes: u64,
	read_only: bool,
	removable: bool,
	kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InterfaceInfo {
	name: String,
	mac: String,
	ips: Vec<String>,
	total_received: u64,
	total_transmitted: u64,
	packets_received: u64,
	packets_transmitted: u64,
	errors_on_received: u64,
	errors_on_transmitted: u64,
	mtu: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlPlaneRequest {

}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlPlaneResponse {

}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
	Token { token: String },
	Credentials { username: String, password: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PermissionGrant {
	Owner,
	Viewer,
	Files { path: String, access: FileAccess },
	SystemInfo,
	DiskInfo,
	NetworkInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FileAccess {
	Read,
	ReadWrite,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
	pub session_id: String,
	pub username: String,
	pub roles: Vec<String>,
	pub permissions: Vec<PermissionGrant>,
	pub expires_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSummary {
	pub username: String,
	pub roles: Vec<String>,
	pub permissions: Vec<PermissionGrant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
	pub id: String,
	pub username: String,
	pub label: Option<String>,
	pub permissions: Vec<PermissionGrant>,
	pub expires_at: Option<u64>,
	pub revoked: bool,
	pub issued_at: u64,
	pub issued_by: String,
}

type PuppyPeerBehaviour = request_response::json::Behaviour<PuppyPeerRequest, PuppyPeerResponse>;

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "AgentEvent", event_process = false)]
pub struct AgentBehaviour {
	ping: ping::Behaviour,
	pub puppypeer: PuppyPeerBehaviour,
	pub mdns: mdns::tokio::Behaviour,
}

#[derive(Debug)]
pub enum AgentEvent {
	Ping(ping::Event),
	PuppyPeer(RequestResponseEvent<PuppyPeerRequest, PuppyPeerResponse>),
	Mdns(mdns::Event),
}

impl From<ping::Event> for AgentEvent {
	fn from(event: ping::Event) -> Self {
		AgentEvent::Ping(event)
	}
}

impl From<RequestResponseEvent<PuppyPeerRequest, PuppyPeerResponse>> for AgentEvent {
	fn from(event: RequestResponseEvent<PuppyPeerRequest, PuppyPeerResponse>) -> Self {
		AgentEvent::PuppyPeer(event)
	}
}

impl From<mdns::Event> for AgentEvent {
	fn from(event: mdns::Event) -> Self {
		AgentEvent::Mdns(event)
	}
}

impl AgentBehaviour {
	fn new(local_peer_id: PeerId) -> Self {
		let puppypeer_protocol = std::iter::once((StreamProtocol::new(PUPPYPEER_PROTOCOL), ProtocolSupport::Full));
		let puppypeer = request_response::json::Behaviour::new(puppypeer_protocol, RequestResponseConfig::default(),);
		let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id).expect("mDNS init failed");
		Self {
			ping: ping::Behaviour::default(),
			puppypeer,
			mdns,
		}
	}
}

fn read_file_chunk(path: &str, offset: u64, length: Option<u64>) -> Result<FileChunk, String> {
	let mut file = File::open(path).map_err(|err| err.to_string())?;
	let metadata = file.metadata().map_err(|err| err.to_string())?;
	if metadata.is_dir() {
		return Err("Cannot read directory".to_string());
	}

	file.seek(SeekFrom::Start(offset))
		.map_err(|err| err.to_string())?;

	let mut buffer = Vec::new();
	let max_len = length.unwrap_or(MAX_FILE_CHUNK).min(MAX_FILE_CHUNK) as usize;
	buffer.resize(max_len, 0);
	let bytes_read = file.read(&mut buffer).map_err(|err| err.to_string())?;
	buffer.truncate(bytes_read);

	let eof = file.stream_position().map_err(|err| err.to_string())? >= metadata.len();

	Ok(FileChunk {
		offset,
		data: buffer,
		eof,
	})
}

fn write_file_range(path: &str, offset: u64, data: &[u8]) -> Result<FileWriteAck, String> {
	let mut file = OpenOptions::new()
		.write(true)
		.create(true)
		.read(true)
		.open(path)
		.map_err(|err| err.to_string())?;

	let current_len = file.metadata().map_err(|err| err.to_string())?.len();
	let required_len = offset
		.checked_add(data.len() as u64)
		.ok_or_else(|| "Write would overflow file length".to_string())?;

	if required_len > current_len {
		file.set_len(required_len).map_err(|err| err.to_string())?;
	}

	file.seek(SeekFrom::Start(offset))
		.map_err(|err| err.to_string())?;
	file.write_all(data).map_err(|err| err.to_string())?;

	Ok(FileWriteAck {
		bytes_written: data.len() as u64,
	})
}

fn collect_cpu_info() -> Result<Vec<CpuInfo>, String> {
	let mut system = System::new_all();
	system.refresh_cpu_usage();
	let cpus = system
		.cpus()
		.iter()
		.map(|cpu| CpuInfo {
			name: cpu.name().to_string(),
			usage: cpu.cpu_usage(),
			frequency_hz: cpu.frequency(),
		})
		.collect();
	Ok(cpus)
}

fn collect_disk_info() -> Result<Vec<DiskInfo>, String> {
	let disks = Disks::new_with_refreshed_list();
	let infos = disks
		.list()
		.iter()
		.map(|disk| {
			let total_space = disk.total_space();
			let available_space = disk.available_space();
			let usage_percent = if total_space == 0 {
				0.0
			} else {
				let used = total_space.saturating_sub(available_space);
				((used as f64 / total_space as f64) * 100.0) as f32
			};
			let usage = disk.usage();
			DiskInfo {
				name: disk.name().to_string_lossy().to_string(),
				mount_path: disk.mount_point().to_string_lossy().to_string(),
				filesystem: disk.file_system().to_string_lossy().to_string(),
				total_space,
				available_space,
				usage_percent,
				total_read_bytes: usage.total_read_bytes,
				total_written_bytes: usage.total_written_bytes,
				read_only: disk.is_read_only(),
				removable: disk.is_removable(),
				kind: format!("{:?}", disk.kind()),
			}
		})
		.collect();
	Ok(infos)
}

fn collect_interface_info() -> Result<Vec<InterfaceInfo>, String> {
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
	Ok(infos)
}

#[derive(Debug, Clone)]
struct UserRecord {
	username: String,
	password_hash: String,
	salt: String,
	roles: HashSet<String>,
	permissions: HashSet<PermissionGrant>,
	tokens: HashSet<String>,
}

#[derive(Debug, Clone)]
struct TokenRecord {
	id: String,
	secret: String,
	username: String,
	label: Option<String>,
	permissions: HashSet<PermissionGrant>,
	issued_at: u64,
	expires_at: Option<u64>,
	revoked: bool,
	issued_by: String,
}

#[derive(Debug, Clone)]
struct SessionRecord {
	session_id: String,
	username: String,
	roles: HashSet<String>,
	permissions: HashSet<PermissionGrant>,
	expires_at: Option<u64>,
	token_id: Option<String>,
}

#[derive(Debug, Clone)]
enum Capability {
	FileRead(String),
	FileWrite(String),
	System,
	Disks,
	Network,
}

impl Capability {
	fn is_read_only(&self) -> bool {
		matches!(
			self,
			Capability::FileRead(_) | Capability::System | Capability::Disks | Capability::Network
		)
	}
}

impl PermissionGrant {
	fn allows(&self, capability: &Capability) -> bool {
		match self {
			PermissionGrant::Owner => true,
			PermissionGrant::Viewer => capability.is_read_only(),
			PermissionGrant::Files { path, access } => match capability {
				Capability::FileRead(request_path) => path_matches(path, request_path),
				Capability::FileWrite(request_path) => {
					matches!(access, FileAccess::ReadWrite) && path_matches(path, request_path)
				}
				Capability::System | Capability::Disks | Capability::Network => false,
			},
			PermissionGrant::SystemInfo => matches!(capability, Capability::System),
			PermissionGrant::DiskInfo => matches!(capability, Capability::Disks),
			PermissionGrant::NetworkInfo => matches!(capability, Capability::Network),
		}
	}
}

impl SessionRecord {
	fn is_expired(&self, now: u64) -> bool {
		self.expires_at.map(|exp| exp <= now).unwrap_or(false)
	}

	fn allows(&self, capability: &Capability) -> bool {
		self.permissions.iter().any(|perm| perm.allows(capability))
	}

	fn to_info(&self) -> SessionInfo {
		let mut roles: Vec<String> = self.roles.iter().cloned().collect();
		roles.sort();
		SessionInfo {
			session_id: self.session_id.clone(),
			username: self.username.clone(),
			roles,
			permissions: self.permissions.iter().cloned().collect(),
			expires_at: self.expires_at,
		}
	}
}

fn default_permissions_for_roles(roles: &HashSet<String>) -> HashSet<PermissionGrant> {
	let mut permissions = HashSet::new();
	if roles.contains(OWNER_ROLE) {
		permissions.insert(PermissionGrant::Owner);
		return permissions;
	}
	if roles.contains(VIEWER_ROLE) {
		permissions.insert(PermissionGrant::Viewer);
		permissions.insert(PermissionGrant::SystemInfo);
		permissions.insert(PermissionGrant::DiskInfo);
		permissions.insert(PermissionGrant::NetworkInfo);
		permissions.insert(PermissionGrant::Files {
			path: String::from("/"),
			access: FileAccess::Read,
		});
	}
	permissions
}

fn normalize_role(role: &str) -> String {
	role.trim().to_lowercase()
}

fn now_timestamp() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map(|duration| duration.as_secs())
		.unwrap_or(0)
}

fn hash_password(salt: &str, password: &str) -> String {
	let mut hasher = Sha256::new();
	hasher.update(salt.as_bytes());
	hasher.update(password.as_bytes());
	let digest = hasher.finalize();
	let mut output = String::with_capacity(digest.len() * 2);
	for byte in digest {
		output.push_str(&format!("{:02x}", byte));
	}
	output
}

fn verify_password(salt: &str, password: &str, expected_hash: &str) -> bool {
	hash_password(salt, password) == expected_hash
}

fn normalize_path(path: &str) -> String {
	let trimmed = path.trim();
	if trimmed.is_empty() {
		return String::from("/");
	}
	let mut normalized = trimmed.replace('\\', "/");
	while normalized.ends_with('/') && normalized.len() > 1 {
		normalized.pop();
	}
	if normalized.is_empty() {
		normalized.push('/');
	}
	normalized
}

fn path_matches(grant: &str, request: &str) -> bool {
	if grant.is_empty() || grant == "/" || grant == "*" {
		return true;
	}
	let grant_norm = normalize_path(grant);
	let request_norm = normalize_path(request);
	let grant_cmp = grant_norm.trim_start_matches('/');
	let request_cmp = request_norm.trim_start_matches('/');
	if request_norm == grant_norm || request_cmp == grant_cmp {
		return true;
	}
	let prefix = format!("{}/", grant_norm);
	let prefix_cmp = format!("{}/", grant_cmp);
	request_norm.starts_with(&prefix) || request_cmp.starts_with(&prefix_cmp)
}

/// Load or generate an Ed25519 keypair and persist it to disk.
pub fn load_or_generate_keypair(path: &Path) -> Result<identity::Keypair> {
	// Ensure parent directory exists if a directory component was provided.
	if let Some(parent) = path.parent() {
		if !parent.as_os_str().is_empty() && !parent.exists() {
			std::fs::create_dir_all(parent)?;
			log::info!("created key directory {}", parent.display());
		}
	}
	if path.exists() {
		let bytes = fs::read(path)?;
		let key = Keypair::from_protobuf_encoding(&bytes)?;
		Ok(key.into())
	} else {
		let key = identity::Keypair::generate_ed25519();
		let bytes = key.to_protobuf_encoding()?;
		fs::write(path, &bytes)?;
		Ok(key.into())
	}
}

fn libp2p_multiaddr(address: &Multiaddr, local_ip: IpAddr, peer_id: &PeerId) -> Multiaddr {
	let mut reachable = Multiaddr::empty();
	for protocol in address.iter() {
		match protocol {
			Protocol::Ip4(ip) if ip.is_unspecified() => match local_ip {
				IpAddr::V4(local) => reachable.push(Protocol::Ip4(local)),
				IpAddr::V6(_) => reachable.push(protocol.clone()),
			},
			Protocol::Ip6(ip) if ip.is_unspecified() => match local_ip {
				IpAddr::V6(local) => reachable.push(Protocol::Ip6(local)),
				IpAddr::V4(_) => reachable.push(protocol.clone()),
			},
			_ => reachable.push(protocol.clone()),
		}
	}
	reachable.push(Protocol::P2p(peer_id.clone().into()));
	reachable
}

pub fn build_swarm(id_keys: identity::Keypair, peer_id: PeerId) -> Result<Swarm<AgentBehaviour>> {
	let swarm = SwarmBuilder::with_existing_identity(id_keys)
		.with_tokio()
		.with_tcp(
			tcp::Config::default(),
			noise::Config::new,
			yamux::Config::default,
		)?
		.with_behaviour(|_| AgentBehaviour::new(peer_id))?
		.with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(u64::MAX)))
		.build();
	Ok(swarm)
}
