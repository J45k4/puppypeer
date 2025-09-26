use common::calc_file_hash_async;
use tokio::fs::metadata;
use tokio::sync::mpsc;
use walkdir::WalkDir;
use crate::types::FileMetadata;

pub struct ScanEntry {
    file_path: String,
    file_metadata: FileMetadata
}

pub async fn scan_directory(
    tx: mpsc::Sender<ScanEntry>, 
    folder_path: &str, 
    calculate_hash: bool
) -> Result<(), failure::Error> {
    for entry in WalkDir::new(folder_path) {
        match entry {
            Ok(d) => {
                let metadata = d.metadata()?;

                if metadata.is_file() {
                    let file_path = d.path().to_string_lossy();

                    let mut file_metadata = FileMetadata{
                        file_size: metadata.len(),
                        file_hash: None
                    };

                    if calculate_hash {
                        let file_hash = calc_file_hash_async(&file_path).await?;

                        file_metadata.file_hash = Some(file_hash);
                    }

                    let scan_entry = ScanEntry{
                        file_path: file_path.to_string(),
                        file_metadata: file_metadata
                    };

                    tx.send(scan_entry).await;
                }
            }
            Err(e) => {}
        }
    }

    Ok(())
} 