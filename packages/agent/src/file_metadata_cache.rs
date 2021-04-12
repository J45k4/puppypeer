use std::convert::TryInto;
use sled::Db;
use sled::IVec;
use sled::Tree;

use crate::types::FileMetadata;

fn decode_file_id(file_id: IVec) -> Option<u64> {
    let bytes: [u8; 8] = match file_id.to_vec().try_into() {
        Ok(a) => a,
        Err(_) => return None
    };

    Some(u64::from_le_bytes(bytes))
}

pub struct FileMetadataCache {
    db: Db,
    file_metadatas: Tree,
    file_hashes: Tree,
    file_paths: Tree
}

impl FileMetadataCache {
    pub fn new(db: sled::Db) -> FileMetadataCache {
        FileMetadataCache{
            file_metadatas: db.open_tree(b"file_metadata").unwrap(),
            file_hashes: db.open_tree(b"file_hash").unwrap(),
            file_paths: db.open_tree(b"file_path").unwrap(),
            db: db
        }
    }

    pub fn remove_file_path(&self, path: &str) -> Result<(), failure::Error> {
        self.file_paths.remove(path)?;

        Ok(())
    }

    pub fn get_file_metadata_from_path(&self, file_path: &str) -> Result<Option<FileMetadata>, failure::Error> {
        let file_id = match self.get_file_id_with_path(file_path)? {
            Some(r) => r,
            None => return Ok(None)
        };

        Ok(self.get_file_metadata(file_id)?)
    }

    pub fn remove_hash(&self, file_hash: &[u8]) -> Result<Option<u64>, failure::Error> {
        Ok(match self.get_file_id_with_file_hash(&file_hash)? {
            Some(r) => {
                self.file_hashes.remove(file_hash)?;

                Some(r)
            }
            None => None
        })
    }

    pub fn set_hash(&self, file_path: &str, file_hash: Vec<u8>) -> Result<(), failure::Error> {
        let file_id = match self.get_file_id_with_path(file_path)? {
            Some(r) => r,
            None => {
                let file_id = self.db.generate_id()?;

                self.file_paths.insert(file_path, &file_id.to_le_bytes())?;

                file_id
            }
        };

        match self.get_file_metadata(file_id)? {
            Some(file_metadata) => {
                let mut file_metadata = file_metadata.clone();
                file_metadata.file_hash = Some(file_hash.clone());

                self.set_file_metadata_with_file_id(file_id, file_metadata)?;
            }
            None => {

            }
        };

        match self.get_file_id_with_file_hash(&file_hash)? {
            Some(_) => {},
            None => {
                self.file_hashes.insert(file_hash, &file_id.to_be_bytes())?;
            }
        };

        Ok(())
    }

    pub fn set_file_metadata(&self, file_path: &str, file_metadata: FileMetadata) -> Result<(), failure::Error> {
        let file_id = match self.get_file_id_with_path(file_path)? {
            Some(r) => r,
            None => {
                let file_id = self.db.generate_id()?;

                self.file_paths.insert(file_path, &file_id.to_le_bytes());

                file_id
            }
        };

        self.set_file_metadata_with_file_id(file_id, file_metadata)?;

        Ok(())
    }

    fn get_file_metadata(&self, file_id: u64) -> Result<Option<FileMetadata>, failure::Error> {
        let raw = match self.file_metadatas.get(file_id.to_le_bytes())? {
            Some(r) => r,
            None => return Ok(None)
        };

        let file_metadata: FileMetadata = bincode::deserialize(&raw)?;

        Ok(Some(file_metadata))
    }

    fn get_file_id_with_path(&self, file_path: &str) -> Result<Option<u64>, failure::Error> {
        Ok(match self.file_paths.get(file_path)? {
            Some(r) => decode_file_id(r),
            None => None
        })
    }

    fn get_file_id_with_file_hash(&self, file_hash: &[u8]) -> Result<Option<u64>, failure::Error> {
        Ok(match self.file_hashes.get(file_hash)? {
            Some(r) => decode_file_id(r),
            None => None
        })
    }

    fn set_file_metadata_with_file_id(&self, file_id: u64, file_metadata: FileMetadata) -> Result<(), failure::Error> {
        let b = bincode::serialize(&file_metadata)?;

        self.file_metadatas.insert(file_id.to_le_bytes(), b)?;
        
        Ok(())
    }
}