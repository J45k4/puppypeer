use std::{fs::OpenOptions, path::PathBuf};
use std::fs::{File, create_dir_all};


pub fn provide_file_handle(file_path: &str) -> File {
    let mut path = PathBuf::from(file_path);
    
    if path.exists() && path.is_file() {
        return OpenOptions::new()
        .read(true)
        .write(true)
        .open(file_path).unwrap();
    }

    path.pop();

    if path.exists() == false {
        create_dir_all(file_path).unwrap();
    }

    File::create(file_path).unwrap()
}