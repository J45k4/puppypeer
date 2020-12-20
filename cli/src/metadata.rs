use std::{fs::{File, create_dir_all}, path::{Path, PathBuf}};
use std::io::prelude::*;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub struct FileMetadata {
    pub file_hash: Option<String>,
    pub received_at: Option<u64>,
    pub mime_type: Option<String>,
    pub size: Option<u64>,
    pub tags: Vec<String>,
    pub sources: Vec<FileSource>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct FileSource {
    pub host_name: String,
    pub full_file_path: Option<String>,
    pub root_folder: Option<String>,
    pub relative_file_path: Option<String>,
    pub file_name: Option<String>,
    pub file_extension: Option<String>,
    pub received_at: Option<u64>,
    pub created_at: Option<u64>,
    pub modified_at: Option<u64>,
    pub accessed_at: Option<u64>,
    pub last_updated_at: Option<u64>,
    pub readonly: Option<bool>
}

impl FileMetadata {
    pub fn new() -> FileMetadata {
        FileMetadata{
            ..Default::default()
        }
    }

    pub fn get_or_create(path_str: &str) -> FileMetadata {
        let path = Path::new(path_str);

        if path.exists() == false || path.is_dir() {
            return FileMetadata::new();
        }

        FileMetadata::from_path(path_str)
    }

    pub fn from_path(path: &str) -> FileMetadata {
        let mut f = File::open(path).unwrap();

        let mut s = String::new();

        f.read_to_string(&mut s).unwrap();

        let file_metadata: FileMetadata = serde_json::from_str(&s).unwrap();

        file_metadata
    }

    pub fn find_file_source(&mut self, host_name: &str) -> Option<FileSource> {
        for file_source in &self.sources{
            if file_source.host_name == host_name {
                return Some(file_source.clone());
            }
        }

        return None;
    }

    pub fn save_file_source(&mut self, new_file_source: FileSource) {
        self.sources.retain(|f| f.host_name != new_file_source.host_name);

        self.sources.push(new_file_source);
    }

    pub fn save(&self, path_str: &str) {
        let s = serde_json::to_string(&self).unwrap();

        let mut path = PathBuf::from(path_str);
        path.pop();

        create_dir_all(path).unwrap();

        let mut f = File::create(path_str).unwrap();

        f.write(s.as_bytes()).unwrap();
    }
}
