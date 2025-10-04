use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChunk {
	pub offset: u64,
	pub data: Vec<u8>,
	pub eof: bool,
}