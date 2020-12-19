use std::{fs::copy, path::Path, time::SystemTime};

use clap::{App, Arg};
use gethostname::gethostname;
use metadata::{FileSource};
use tree_magic::from_filepath;
use walkdir::WalkDir;
use std::time::UNIX_EPOCH;

mod hash_utility;
mod metadata;

fn get_millis_timestamp(dt: std::time::SystemTime) -> Result<u64, Box<dyn std::error::Error>> {
    Ok(dt.duration_since(UNIX_EPOCH)?.as_millis() as u64)
}

fn get_current_timestamp() -> u64 {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    return since_the_epoch.as_millis() as u64;
}

fn main() {
    let matches = App::new("Epic shelter")
        .version("0.0.1")
        .arg(Arg::with_name("input-folder").long("input-folder").value_name("input folder"))
        .arg(Arg::with_name("output-folder").long("output-folder").value_name("output-folder"))
        .get_matches();

    let input_folder = matches.value_of("input-folder").unwrap();
    let output_folder = matches.value_of("output-folder").unwrap();

    let host_name_os = gethostname();
    let host_name = host_name_os.to_str().unwrap();

    let dst_folder_path = Path::new(output_folder);
    let metadata_folder = dst_folder_path.join(".epic-shelter");

    let root_folder_path = input_folder;

    let w = WalkDir::new(root_folder_path);

    for entry in w {
        let unwrapped_entry = entry.unwrap();

        let metadata = unwrapped_entry.metadata().unwrap();

        if !metadata.is_file() {
            continue;
        }

        let file_path = unwrapped_entry.path();

        let file_path_str = file_path.display().to_string();

        let hash = hash_utility::calc_file_hash(&file_path_str).unwrap();

        let metadata_file_path = metadata_folder.join(format!("{}.json", hash));
    
        let metadata_file_path_str = metadata_file_path.to_str().unwrap();

        let mut file_metadata = metadata::FileMetadata::get_or_create(metadata_file_path_str);        

        let size = metadata.len();
        let modified = get_millis_timestamp(metadata.modified().unwrap()).unwrap();
        let accessed = get_millis_timestamp(metadata.accessed().unwrap()).unwrap();
        let created = get_millis_timestamp(metadata.created().unwrap()).unwrap();
        let readonly = metadata.permissions().readonly();

        if file_metadata.size.is_none() {
            file_metadata.size = Some(size);
        }

        if file_metadata.file_hash.is_none() {
            file_metadata.file_hash = Some(hash.clone());
        }

        if file_metadata.received_at.is_none() {
            file_metadata.received_at = Some(get_current_timestamp());
        }

        if file_metadata.mime_type.is_none() {
            let result = from_filepath(file_path);
            file_metadata.mime_type = Some(result);
        }

        let mut file_source = {
            let f = file_metadata.find_file_source(host_name);

            match f {
                Some(p) => p,
                None => {
                    FileSource {
                        host_name: host_name.to_string(),
                        ..Default::default()
                    }
                }
            }
        };

        if file_source.received_at.is_none() {
            file_source.received_at = Some(get_current_timestamp());
        }

        if file_source.modified_at.is_none() {
            file_source.modified_at = Some(modified);
        }

        if file_source.accessed_at.is_none() {
            file_source.accessed_at = Some(accessed);
        }

        if file_source.created_at.is_none() {
            file_source.created_at = Some(created);
        }

        if file_source.readonly.is_none() {
            file_source.readonly = Some(readonly);
        }

        if file_source.file_name.is_none() {
            file_source.file_name = Some(file_path.file_name().unwrap().to_str().unwrap().to_string());
        }

        file_source.full_file_path = Some(file_path_str.to_string());
        file_source.root_folder = Some(root_folder_path.to_string());

        file_metadata.save_file_source(file_source);

        let dst_file_path = dst_folder_path.join(hash);

        file_metadata.save(metadata_file_path_str);

        if dst_file_path.exists() {
            println!("Duplicate file {}", file_path.to_string_lossy());

            continue;
        }

        copy(file_path_str, dst_file_path).unwrap();
    }
}
