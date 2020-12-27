use std::{fs::{copy, create_dir_all}, path::Path, process::exit};

use clap::ArgMatches;
use gethostname::gethostname;
use metadata::FileSource;
use tree_magic::from_filepath;
use utility::{get_current_timestamp, get_millis_timestamp};
use walkdir::WalkDir;
use super::*;


pub fn exec_copy(args: &ArgMatches) {
    let input_folder_path_str = args.value_of("input-folder").unwrap();
    let output_folder_path_str = args.value_of("output-folder").unwrap();  

    log::info!("Input folder path {}", input_folder_path_str);
    log::info!("Output folder path {}", output_folder_path_str);

    let input_folder_path = Path::new(input_folder_path_str);
    let output_folder_path = Path::new(output_folder_path_str);

    if input_folder_path.exists() == false {
        log::error!("Input folder does not exists {}", input_folder_path_str);

        exit(1);
    }

    if input_folder_path.is_dir() == false {
        log::error!("Input path is not folder {}", input_folder_path_str);

        exit(1);
    }

    create_dir_all(input_folder_path).unwrap();

    let host_name_os = gethostname();
    let host_name = host_name_os.to_str().unwrap();

    let w = WalkDir::new(input_folder_path);

    for entry in w {
        let unwrapped_entry = entry.unwrap();

        let metadata = unwrapped_entry.metadata().unwrap();

        if !metadata.is_file() {
            continue;
        }

        let file_path = unwrapped_entry.path();

        let file_path_str = file_path.display().to_string();

        let hash = common::calc_file_hash(&file_path_str).unwrap();

        let metadata_file_path = output_folder_path.join(format!("epic-shelter-metadata-{}.json", hash));
    
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

        let file_name = file_path.file_name().unwrap().to_str().unwrap().to_string();

        if file_source.file_name.is_none() {
            file_source.file_name = Some(file_name.clone());
        }

        if file_source.file_extension.is_none() {
            let splitted: Vec<&str> = file_name.split(".").collect();

            if splitted.len()  > 1 {
                let file_ext = splitted[splitted.len() - 1];
                file_source.file_extension = Some(file_ext.to_string());
            }
        }

        file_source.last_updated_at = Some(get_current_timestamp());
        file_source.full_file_path = Some(file_path_str.to_string());
        file_source.root_folder = Some(output_folder_path_str.to_string());
        file_source.relative_file_path = Some(file_path.strip_prefix(input_folder_path_str).unwrap().to_str().unwrap().to_string());

        file_metadata.save_file_source(file_source);

        let dst_file_name = format!("epic-shelter-data-{}", hash);

        let dst_file_path = output_folder_path.join(dst_file_name);

        file_metadata.save(metadata_file_path_str);

        if dst_file_path.exists() {
            log::info!("Duplicate file {}", file_path.to_string_lossy());

            continue;
        }

        log::info!("Copying file {} to {:?}", file_path_str, dst_file_path);

        copy(file_path_str, dst_file_path).unwrap();
    }
}