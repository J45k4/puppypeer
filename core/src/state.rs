use libp2p::PeerId;

pub const FLAG_READ: u8 = 0x01;
pub const FLAG_WRITE: u8 = 0x02;
pub const FLAG_EXECUTE: u8 = 0x04;
pub const FLAG_SEARCH: u8 = 0x08;

pub struct FolderRule {
    path: String,
    flags: u8
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

pub enum Rule {
    Owner,
    Folder(FolderRule),
}

pub struct RelationshipRule {
    rule: Rule,
    expires_at: Option<i64>,
}

pub struct Relationship {
    src: PeerId,
    target: PeerId,
    rules: Vec<RelationshipRule>,
}

pub struct TokenAuth {
    token: String,
}

pub enum AuthMethod {
    Token { token: String },
    Credentials { username: String, password: String },
}

pub struct Auth {
    method: AuthMethod,
    expires_at: Option<i64>,
    rules: Vec<Rule>,
}

pub struct Connection {
    peer: PeerId,
}

pub struct State {
    me: PeerId,
    relationships: Vec<Relationship>,
    auths: Vec<Auth>,
    connections: Vec<Connection>,
}

impl State {
    pub fn authenticate(&mut self, peer_id: PeerId, method: AuthMethod) {

    }

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
}