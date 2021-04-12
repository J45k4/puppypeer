use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct FileMetadata {
    pub file_hash: Option<Vec<u8>>,
    pub file_size: u64
}