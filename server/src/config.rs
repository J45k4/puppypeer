use std::path::Path;

pub fn get_data_folder_path() -> String {
    r"G:\epic-data".to_string()
}

pub fn get_file_data_path(hash: &str) -> String {
    Path::new(&get_data_folder_path())
        .join(format!("epic-shelter-data-{}", hash))
        .to_string_lossy().to_string()
}

pub fn get_file_metadata_path(hash: &str) -> String {
    Path::new(&get_data_folder_path())
        .join(format!("./tmp/epic-shelter-metadata-{}.json", hash))
        .to_string_lossy().to_string()
}