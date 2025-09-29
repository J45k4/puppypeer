use anyhow::bail;
use libp2p::{Multiaddr, PeerId, swarm::ConnectionId};

pub const FLAG_READ: u8 = 0x01;
pub const FLAG_WRITE: u8 = 0x02;
pub const FLAG_EXECUTE: u8 = 0x04;
pub const FLAG_SEARCH: u8 = 0x08;

#[derive(Clone, Debug)]
pub struct FolderRule {
	path: String,
	flags: u8,
}

impl FolderRule {
	pub fn new(path: String, flags: u8) -> Self {
		Self { path, flags }
	}

	pub fn can_read(&self) -> bool {
		self.flags & FLAG_READ != 0
	}

	pub fn can_write(&self) -> bool {
		self.flags & FLAG_WRITE != 0
	}

	pub fn can_execute(&self) -> bool {
		self.flags & FLAG_EXECUTE != 0
	}
}

#[derive(Clone, Debug)]
pub enum Rule {
	Owner,
	Folder(FolderRule),
}

#[derive(Clone, Debug)]
pub struct RelationshipRule {
	rule: Rule,
	expires_at: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct Relationship {
	src: PeerId,
	target: PeerId,
	rules: Vec<RelationshipRule>,
}

pub struct TokenAuth {
	token: String,
}

#[derive(Clone, Debug)]
pub enum AuthMethod {
	Token { token: String },
	Credentials { username: String, password: String },
}

#[derive(Clone, Debug)]
pub struct Auth {
	method: AuthMethod,
	expires_at: Option<i64>,
	rules: Vec<Rule>,
}

#[derive(Clone, Debug)]
pub struct Connection {
	pub peer_id: PeerId,
	pub connection_id: ConnectionId,
}

#[derive(Clone, Debug)]
pub struct DiscoveredPeer {
	pub peer_id: PeerId,
	pub multiaddr: Multiaddr,
}

#[derive(Clone, Debug)]
pub struct Peer {
	pub id: PeerId,
	pub name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct User {
	pub name: String,
	pub passw: String,
}

#[derive(Clone, Debug)]
pub struct State {
	pub me: PeerId,
	pub relationships: Vec<Relationship>,
	pub auths: Vec<Auth>,
	pub connections: Vec<Connection>,
	pub discovered_peers: Vec<DiscoveredPeer>,
	pub peers: Vec<Peer>,
	pub users: Vec<User>,
}

impl Default for State {
	fn default() -> Self {
		Self {
			me: PeerId::random(),
			relationships: Vec::new(),
			auths: Vec::new(),
			connections: Vec::new(),
			discovered_peers: Vec::new(),
			peers: Vec::new(),
			users: Vec::new(),
		}
	}
}

impl State {
	pub fn authenticate(&mut self, peer_id: PeerId, method: AuthMethod) {}

	pub fn has_fs_access(&self, src: PeerId, path: &str, access: u8) -> bool {
		if src == self.me {
			return true;
		}

		for rel in &self.relationships {
			if rel.src == src || rel.target == src {
				for rule in &rel.rules {
					match &rule.rule {
						Rule::Owner => {
							return true;
						}
						Rule::Folder(folder_rule) => {
							if path.starts_with(&folder_rule.path) {
								if (access & FLAG_READ != 0 && folder_rule.can_read())
									|| (access & FLAG_WRITE != 0 && folder_rule.can_write())
									|| (access & FLAG_EXECUTE != 0 && folder_rule.can_execute())
								{
									return true;
								}
							}
						}
					}
				}
			}
		}

		false
	}

	pub fn peer_discovered(&mut self, peer_id: PeerId, multiaddr: Multiaddr) {
		if !self.discovered_peers.iter().any(|p| p.peer_id == peer_id) {
			self.discovered_peers
				.push(DiscoveredPeer { peer_id, multiaddr });
		}
	}

	pub fn peer_expired(&mut self, peer_id: PeerId, multiaddr: Multiaddr) {
		self.discovered_peers
			.retain(|p| !(p.peer_id == peer_id && p.multiaddr == multiaddr));
	}

	pub fn create_user(&mut self, username: String, password: String) -> anyhow::Result<()> {
		if self.users.iter().any(|u| u.name == username) {
			bail!("User already exists");
		}
		self.users.push(User {
			name: username,
			passw: password,
		});
		Ok(())
	}
}
