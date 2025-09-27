use anyhow::Result;
use futures::prelude::*;
use libp2p::core::ConnectedPoint;
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
use std::env;
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

const FILE_META_PROTOCOL: &str = "/puppy/filemeta/1.0.0";
const CONTROL_PROTOCOL: &str = "/puppy/control/1.0.0";
const MAX_FILE_CHUNK: u64 = 4 * 1024 * 1024; // 4 MiB per transfer chunk
const OWNER_ROLE: &str = "owner";
const VIEWER_ROLE: &str = "viewer";
const DEFAULT_SESSION_TTL: u64 = 60 * 60; // 1 hour sessions for credential auth

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileMetaRequest {
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileMetaResponse {
	DirEntries(Vec<FileEntry>),
	FileStat(FileEntry),
	FileChunk(FileChunk),
	WriteAck(FileWriteAck),
	Cpus(Vec<CpuInfo>),
	Disks(Vec<DiskInfo>),
	Interfaces(Vec<InterfaceInfo>),
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
	},
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlPlaneResponse {
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

type FileMetaBehaviour = request_response::json::Behaviour<FileMetaRequest, FileMetaResponse>;
type ControlBehaviour =
	request_response::json::Behaviour<ControlPlaneRequest, ControlPlaneResponse>;

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "AgentEvent", event_process = false)]
pub struct AgentBehaviour {
	ping: ping::Behaviour,
	file_meta: FileMetaBehaviour,
	control_plane: ControlBehaviour,
	mdns: mdns::tokio::Behaviour,
}

#[derive(Debug)]
pub enum AgentEvent {
	Ping(ping::Event),
	FileMeta(RequestResponseEvent<FileMetaRequest, FileMetaResponse>),
	Control(RequestResponseEvent<ControlPlaneRequest, ControlPlaneResponse>),
	Mdns(mdns::Event),
}

impl From<ping::Event> for AgentEvent {
	fn from(event: ping::Event) -> Self {
		AgentEvent::Ping(event)
	}
}

impl From<RequestResponseEvent<FileMetaRequest, FileMetaResponse>> for AgentEvent {
	fn from(event: RequestResponseEvent<FileMetaRequest, FileMetaResponse>) -> Self {
		AgentEvent::FileMeta(event)
	}
}

impl From<RequestResponseEvent<ControlPlaneRequest, ControlPlaneResponse>> for AgentEvent {
	fn from(event: RequestResponseEvent<ControlPlaneRequest, ControlPlaneResponse>) -> Self {
		AgentEvent::Control(event)
	}
}

impl From<mdns::Event> for AgentEvent {
	fn from(event: mdns::Event) -> Self {
		AgentEvent::Mdns(event)
	}
}

impl AgentBehaviour {
	fn new(local_peer_id: PeerId) -> Self {
		let file_protocols = std::iter::once((
			StreamProtocol::new(FILE_META_PROTOCOL),
			ProtocolSupport::Full,
		));
		let control_protocols =
			std::iter::once((StreamProtocol::new(CONTROL_PROTOCOL), ProtocolSupport::Full));
		let file_meta = request_response::json::Behaviour::new(
			file_protocols,
			RequestResponseConfig::default(),
		);
		let control_plane = request_response::json::Behaviour::new(
			control_protocols,
			RequestResponseConfig::default(),
		);
		let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)
			.expect("mDNS init failed");
		Self {
			ping: ping::Behaviour::default(),
			file_meta,
			control_plane,
			mdns,
		}
	}
}

fn handle_file_request(req: FileMetaRequest) -> FileMetaResponse {
	match req {
		FileMetaRequest::ListDir { path } => match fs::read_dir(&path) {
			Ok(entries) => {
				let mut files = Vec::new();
				for entry in entries.flatten() {
					let metadata = entry.metadata().ok();
					let name = entry.file_name().to_string_lossy().to_string();
					let is_dir = metadata.as_ref().map(|md| md.is_dir()).unwrap_or(false);
					let size =
						metadata.and_then(|md| if md.is_dir() { None } else { Some(md.len()) });
					let extension = entry
						.path()
						.extension()
						.map(|s| s.to_string_lossy().to_string());

					files.push(FileEntry {
						name,
						is_dir,
						extension,
						size,
					});
				}
				FileMetaResponse::DirEntries(files)
			}
			Err(err) => FileMetaResponse::Error(err.to_string()),
		},
		FileMetaRequest::StatFile { path } => match fs::metadata(&path) {
			Ok(metadata) => {
				let name = Path::new(&path)
					.file_name()
					.map(|s| s.to_string_lossy().to_string())
					.unwrap_or_else(|| path.clone());
				let is_dir = metadata.is_dir();
				let size = if is_dir { None } else { Some(metadata.len()) };
				let extension = Path::new(&path)
					.extension()
					.map(|s| s.to_string_lossy().to_string());
				FileMetaResponse::FileStat(FileEntry {
					name,
					is_dir,
					extension,
					size,
				})
			}
			Err(err) => FileMetaResponse::Error(err.to_string()),
		},
		FileMetaRequest::ReadFile {
			path,
			offset,
			length,
		} => match read_file_chunk(&path, offset, length) {
			Ok(chunk) => FileMetaResponse::FileChunk(chunk),
			Err(err) => FileMetaResponse::Error(err),
		},
		FileMetaRequest::WriteFile { path, offset, data } => {
			match write_file_range(&path, offset, &data) {
				Ok(ack) => FileMetaResponse::WriteAck(ack),
				Err(err) => FileMetaResponse::Error(err),
			}
		}
		FileMetaRequest::ListCpus => match collect_cpu_info() {
			Ok(cpus) => FileMetaResponse::Cpus(cpus),
			Err(err) => FileMetaResponse::Error(err),
		},
		FileMetaRequest::ListDisks => match collect_disk_info() {
			Ok(disks) => FileMetaResponse::Disks(disks),
			Err(err) => FileMetaResponse::Error(err),
		},
		FileMetaRequest::ListInterfaces => match collect_interface_info() {
			Ok(interfaces) => FileMetaResponse::Interfaces(interfaces),
			Err(err) => FileMetaResponse::Error(err),
		},
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

#[derive(Debug, Default)]
pub struct AuthManager {
	users: HashMap<String, UserRecord>,
	tokens: HashMap<String, TokenRecord>,
	tokens_by_secret: HashMap<String, String>,
	sessions: HashMap<PeerId, SessionRecord>,
}

impl AuthManager {
	pub fn handle_control_request(
		&mut self,
		peer: &PeerId,
		request: ControlPlaneRequest,
	) -> ControlPlaneResponse {
		self.purge_expired();
		match request {
			ControlPlaneRequest::Authenticate { method } => self.authenticate(peer, method),
			ControlPlaneRequest::CreateUser {
				username,
				password,
				roles,
				permissions,
			} => self.create_user(peer, username, password, roles, permissions),
			ControlPlaneRequest::CreateToken {
				username,
				label,
				expires_in,
				permissions,
			} => self.create_token(peer, username, label, expires_in, permissions),
			ControlPlaneRequest::GrantAccess {
				username,
				permissions,
				merge,
			} => self.grant_access(peer, username, permissions, merge),
			ControlPlaneRequest::ListUsers => self.list_users(peer),
			ControlPlaneRequest::ListTokens { username } => self.list_tokens(peer, username),
			ControlPlaneRequest::RevokeToken { token_id } => self.revoke_token(peer, token_id),
			ControlPlaneRequest::RevokeUser { username } => self.revoke_user(peer, username),
		}
	}

	pub fn logout(&mut self, peer: &PeerId) {
		self.sessions.remove(peer);
	}

	fn authorize_file_request(
		&mut self,
		peer: &PeerId,
		request: &FileMetaRequest,
	) -> Result<(), String> {
		if self.users.is_empty() {
			return Ok(());
		}

		self.purge_expired();
		let session = match self.ensure_authenticated(peer) {
			Ok(session) => session,
			Err(ControlPlaneResponse::AuthFailure { reason }) => return Err(reason),
			Err(_) => return Err(String::from("Peer must authenticate")),
		};

		if let Some(capability) = Self::capability_for_request(request) {
			if session.allows(&capability) {
				Ok(())
			} else {
				Err(String::from(
					"Peer lacks permission for requested operation",
				))
			}
		} else {
			Ok(())
		}
	}

	fn authenticate(&mut self, peer: &PeerId, method: AuthMethod) -> ControlPlaneResponse {
		match method {
			AuthMethod::Token { token } => self.authenticate_token(peer, token),
			AuthMethod::Credentials { username, password } => {
				self.authenticate_credentials(peer, username, password)
			}
		}
	}

	fn authenticate_token(&mut self, peer: &PeerId, token: String) -> ControlPlaneResponse {
		self.purge_expired();
		let Some(token_id) = self.tokens_by_secret.get(&token).cloned() else {
			return ControlPlaneResponse::AuthFailure {
				reason: String::from("Invalid or expired token"),
			};
		};
		let Some(record) = self.tokens.get(&token_id).cloned() else {
			self.tokens_by_secret.remove(&token);
			return ControlPlaneResponse::AuthFailure {
				reason: String::from("Invalid or expired token"),
			};
		};
		if record.revoked {
			self.tokens_by_secret.remove(&record.secret);
			return ControlPlaneResponse::AuthFailure {
				reason: String::from("Token revoked"),
			};
		}
		let now = now_timestamp();
		if record.expires_at.map(|exp| exp <= now).unwrap_or(false) {
			self.tokens_by_secret.remove(&record.secret);
			return ControlPlaneResponse::AuthFailure {
				reason: String::from("Token expired"),
			};
		}
		let Some(user) = self.users.get(&record.username) else {
			return ControlPlaneResponse::AuthFailure {
				reason: String::from("Token references missing user"),
			};
		};
		let session = SessionRecord {
			session_id: Uuid::new_v4().to_string(),
			username: user.username.clone(),
			roles: user.roles.clone(),
			permissions: record.permissions.clone(),
			expires_at: record.expires_at,
			token_id: Some(record.id.clone()),
		};
		let info = self.store_session(peer, session);
		ControlPlaneResponse::AuthSuccess { session: info }
	}

	fn authenticate_credentials(
		&mut self,
		peer: &PeerId,
		username: String,
		password: String,
	) -> ControlPlaneResponse {
		let username = username.trim().to_string();
		let Some(user) = self.users.get(&username).cloned() else {
			return ControlPlaneResponse::AuthFailure {
				reason: String::from("Unknown user"),
			};
		};
		if !verify_password(&user.salt, &password, &user.password_hash) {
			return ControlPlaneResponse::AuthFailure {
				reason: String::from("Invalid credentials"),
			};
		}
		let session = SessionRecord {
			session_id: Uuid::new_v4().to_string(),
			username: user.username.clone(),
			roles: user.roles.clone(),
			permissions: user.permissions.clone(),
			expires_at: Some(now_timestamp().saturating_add(DEFAULT_SESSION_TTL)),
			token_id: None,
		};
		let info = self.store_session(peer, session);
		ControlPlaneResponse::AuthSuccess { session: info }
	}

	fn store_session(&mut self, peer: &PeerId, session: SessionRecord) -> SessionInfo {
		let info = session.to_info();
		self.sessions.insert(peer.clone(), session);
		info
	}

	fn create_user(
		&mut self,
		peer: &PeerId,
		username: String,
		password: String,
		roles: Vec<String>,
		permissions: Vec<PermissionGrant>,
	) -> ControlPlaneResponse {
		let username = username.trim().to_string();
		if username.is_empty() {
			return ControlPlaneResponse::Error(String::from("Username cannot be empty"));
		}
		if password.is_empty() {
			return ControlPlaneResponse::Error(String::from("Password cannot be empty"));
		}
		let bootstrap = self.users.is_empty();
		if !bootstrap {
			if let Err(err) = self.ensure_owner(peer) {
				return err;
			}
		}
		if self.users.contains_key(&username) {
			return ControlPlaneResponse::Error(String::from("User already exists"));
		}
		let mut role_set: HashSet<String> = roles
			.into_iter()
			.map(|role| normalize_role(&role))
			.filter(|role| !role.is_empty())
			.collect();
		if bootstrap {
			role_set.insert(OWNER_ROLE.to_string());
		}
		if role_set.is_empty() {
			role_set.insert(VIEWER_ROLE.to_string());
		}
		let permission_set = self.final_permissions(permissions, &role_set);
		let salt = Uuid::new_v4().to_string();
		let password_hash = hash_password(&salt, &password);
		let record = UserRecord {
			username: username.clone(),
			password_hash,
			salt,
			roles: role_set.clone(),
			permissions: permission_set,
			tokens: HashSet::new(),
		};
		self.users.insert(username.clone(), record);
		ControlPlaneResponse::UserCreated { username }
	}

	fn create_token(
		&mut self,
		peer: &PeerId,
		username: String,
		label: Option<String>,
		expires_in: Option<u64>,
		permissions: Vec<PermissionGrant>,
	) -> ControlPlaneResponse {
		let username = username.trim().to_string();
		let session = match self.ensure_authenticated(peer) {
			Ok(session) => session,
			Err(err) => return err,
		};
		if session.username != username && !session.roles.contains(OWNER_ROLE) {
			return ControlPlaneResponse::AuthFailure {
				reason: String::from("Owner role required to issue tokens for other users"),
			};
		}
		let Some(user) = self.users.get(&username).cloned() else {
			return ControlPlaneResponse::Error(String::from("User not found"));
		};
		let mut permission_set: HashSet<PermissionGrant> = permissions.into_iter().collect();
		if permission_set.is_empty() {
			permission_set = user.permissions.clone();
		}
		let response_permissions: Vec<PermissionGrant> = permission_set.iter().cloned().collect();
		let now = now_timestamp();
		let expires_at = expires_in.and_then(|secs| now.checked_add(secs));
		let token_id = Uuid::new_v4().to_string();
		let token_secret = Uuid::new_v4().to_string().replace('-', "");
		let record = TokenRecord {
			id: token_id.clone(),
			secret: token_secret.clone(),
			username: username.clone(),
			label,
			permissions: permission_set.clone(),
			issued_at: now,
			expires_at,
			revoked: false,
			issued_by: session.username.clone(),
		};
		self.tokens_by_secret
			.insert(token_secret.clone(), token_id.clone());
		self.tokens.insert(token_id.clone(), record);
		if let Some(user_entry) = self.users.get_mut(&username) {
			user_entry.tokens.insert(token_id.clone());
		}
		ControlPlaneResponse::TokenIssued {
			token: token_secret,
			token_id,
			username,
			permissions: response_permissions,
			expires_at,
		}
	}

	fn grant_access(
		&mut self,
		peer: &PeerId,
		username: String,
		permissions: Vec<PermissionGrant>,
		merge: bool,
	) -> ControlPlaneResponse {
		let username = username.trim().to_string();
		if let Err(err) = self.ensure_owner(peer) {
			return err;
		}
		let Some(user) = self.users.get_mut(&username) else {
			return ControlPlaneResponse::Error(String::from("User not found"));
		};
		let mut updated_permissions: HashSet<PermissionGrant> = if merge {
			user.permissions.clone()
		} else {
			HashSet::new()
		};
		if merge {
			for permission in permissions {
				updated_permissions.insert(permission);
			}
		} else if permissions.is_empty() {
			updated_permissions = default_permissions_for_roles(&user.roles);
		} else {
			updated_permissions = permissions.into_iter().collect();
		}
		user.permissions = updated_permissions.clone();
		self.refresh_sessions_for(&username);
		self.purge_expired();
		ControlPlaneResponse::AccessGranted {
			username,
			permissions: updated_permissions.into_iter().collect(),
		}
	}

	fn list_users(&mut self, peer: &PeerId) -> ControlPlaneResponse {
		if self.users.is_empty() {
			return ControlPlaneResponse::Users(Vec::new());
		}
		if let Err(err) = self.ensure_owner(peer) {
			return err;
		}
		let mut summaries: Vec<UserSummary> = self
			.users
			.values()
			.map(|user| UserSummary {
				username: user.username.clone(),
				roles: {
					let mut roles: Vec<String> = user.roles.iter().cloned().collect();
					roles.sort();
					roles
				},
				permissions: user.permissions.iter().cloned().collect(),
			})
			.collect();
		summaries.sort_by(|a, b| a.username.cmp(&b.username));
		ControlPlaneResponse::Users(summaries)
	}

	fn list_tokens(&mut self, peer: &PeerId, username: Option<String>) -> ControlPlaneResponse {
		let session = match self.ensure_authenticated(peer) {
			Ok(session) => session,
			Err(err) => return err,
		};
		let user_filter = username.map(|name| name.trim().to_string());
		if let Some(ref requested) = user_filter {
			if requested != &session.username && !session.roles.contains(OWNER_ROLE) {
				return ControlPlaneResponse::AuthFailure {
					reason: String::from("Owner role required to inspect other users' tokens"),
				};
			}
		}
		let now = now_timestamp();
		let mut tokens: Vec<TokenInfo> = self
			.tokens
			.values()
			.filter(|token| {
				if let Some(ref requested) = user_filter {
					token.username == *requested
				} else if session.roles.contains(OWNER_ROLE) {
					true
				} else {
					token.username == session.username
				}
			})
			.map(|token| TokenInfo {
				id: token.id.clone(),
				username: token.username.clone(),
				label: token.label.clone(),
				permissions: token.permissions.iter().cloned().collect(),
				expires_at: token.expires_at,
				revoked: token.revoked || token.expires_at.map(|exp| exp <= now).unwrap_or(false),
				issued_at: token.issued_at,
				issued_by: token.issued_by.clone(),
			})
			.collect();
		tokens.sort_by(|a, b| a.issued_at.cmp(&b.issued_at));
		ControlPlaneResponse::Tokens(tokens)
	}

	fn revoke_token(&mut self, peer: &PeerId, token_id: String) -> ControlPlaneResponse {
		let session = match self.ensure_authenticated(peer) {
			Ok(session) => session,
			Err(err) => return err,
		};
		let Some(token) = self.tokens.get_mut(&token_id) else {
			return ControlPlaneResponse::Error(String::from("Token not found"));
		};
		if token.username != session.username && !session.roles.contains(OWNER_ROLE) {
			return ControlPlaneResponse::AuthFailure {
				reason: String::from("Owner role required to revoke other users' tokens"),
			};
		}
		token.revoked = true;
		self.tokens_by_secret.remove(&token.secret);
		self.sessions
			.retain(|_, session| session.token_id.as_ref() != Some(&token_id));
		ControlPlaneResponse::TokenRevoked { token_id }
	}

	fn revoke_user(&mut self, peer: &PeerId, username: String) -> ControlPlaneResponse {
		if let Err(err) = self.ensure_owner(peer) {
			return err;
		}
		let username = username.trim().to_string();
		let Some(user) = self.users.remove(&username) else {
			return ControlPlaneResponse::Error(String::from("User not found"));
		};
		for token_id in user.tokens {
			if let Some(token) = self.tokens.remove(&token_id) {
				self.tokens_by_secret.remove(&token.secret);
			}
		}
		self.sessions
			.retain(|_, session| session.username != username);
		ControlPlaneResponse::UserRemoved { username }
	}

	fn ensure_authenticated(
		&mut self,
		peer: &PeerId,
	) -> Result<SessionRecord, ControlPlaneResponse> {
		let now = now_timestamp();
		match self.sessions.get(peer) {
			Some(session) if !session.is_expired(now) => Ok(session.clone()),
			Some(_) => {
				self.sessions.remove(peer);
				Err(ControlPlaneResponse::AuthFailure {
					reason: String::from("Session expired"),
				})
			}
			None => Err(ControlPlaneResponse::AuthFailure {
				reason: String::from("Peer not authenticated"),
			}),
		}
	}

	fn ensure_owner(&mut self, peer: &PeerId) -> Result<SessionRecord, ControlPlaneResponse> {
		let session = self.ensure_authenticated(peer)?;
		if session.roles.contains(OWNER_ROLE) {
			Ok(session)
		} else {
			Err(ControlPlaneResponse::AuthFailure {
				reason: String::from("Owner role required"),
			})
		}
	}

	fn refresh_sessions_for(&mut self, username: &str) {
		let maybe_user = self.users.get(username).cloned();
		if maybe_user.is_none() {
			self.sessions
				.retain(|_, session| session.username != username);
			return;
		}
		let user = maybe_user.unwrap();
		let now = now_timestamp();
		let mut stale: Vec<PeerId> = Vec::new();
		for (peer, session) in self.sessions.iter_mut() {
			if session.username != username {
				continue;
			}
			session.roles = user.roles.clone();
			if let Some(token_id) = &session.token_id {
				match self.tokens.get(token_id) {
					Some(token) => {
						session.permissions = token.permissions.clone();
						session.expires_at = token.expires_at;
						if token.revoked || token.expires_at.map(|exp| exp <= now).unwrap_or(false)
						{
							stale.push(peer.clone());
						}
					}
					None => stale.push(peer.clone()),
				}
			} else {
				session.permissions = user.permissions.clone();
			}
		}
		for peer in stale {
			self.sessions.remove(&peer);
		}
	}

	fn purge_expired(&mut self) {
		let now = now_timestamp();
		self.tokens_by_secret.retain(|_, token_id| {
			if let Some(token) = self.tokens.get(token_id) {
				!(token.revoked || token.expires_at.map(|exp| exp <= now).unwrap_or(false))
			} else {
				false
			}
		});
		let mut stale: Vec<PeerId> = Vec::new();
		for (peer, session) in self.sessions.iter() {
			if session.is_expired(now) {
				stale.push(peer.clone());
				continue;
			}
			if let Some(token_id) = &session.token_id {
				match self.tokens.get(token_id) {
					Some(token) => {
						if token.revoked || token.expires_at.map(|exp| exp <= now).unwrap_or(false)
						{
							stale.push(peer.clone());
						}
					}
					None => stale.push(peer.clone()),
				}
			}
		}
		for peer in stale {
			self.sessions.remove(&peer);
		}
	}

	fn final_permissions(
		&self,
		explicit: Vec<PermissionGrant>,
		roles: &HashSet<String>,
	) -> HashSet<PermissionGrant> {
		if explicit.is_empty() {
			default_permissions_for_roles(roles)
		} else {
			explicit.into_iter().collect()
		}
	}

	fn capability_for_request(request: &FileMetaRequest) -> Option<Capability> {
		match request {
			FileMetaRequest::ListDir { path }
			| FileMetaRequest::StatFile { path }
			| FileMetaRequest::ReadFile { path, .. } => Some(Capability::FileRead(path.clone())),
			FileMetaRequest::WriteFile { path, .. } => Some(Capability::FileWrite(path.clone())),
			FileMetaRequest::ListCpus => Some(Capability::System),
			FileMetaRequest::ListDisks => Some(Capability::Disks),
			FileMetaRequest::ListInterfaces => Some(Capability::Network),
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
