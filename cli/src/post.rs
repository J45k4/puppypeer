use std::{io::BufReader, path::Path};

use clap::ArgMatches;
use reqwest::{Body, multipart::Part};
use tokio::prelude::*;
use walkdir::WalkDir;


pub async fn exec_post(args: &ArgMatches<'_>) {
    let url = args.value_of("url");
    let input_folder_path_str = args.value_of("input-folder").unwrap();

    let input_folder_path = Path::new(input_folder_path_str);

    if input_folder_path.exists() == false {
        log::error!("Input folder does not exist {}", input_folder_path_str);
    }

    let w = WalkDir::new(input_folder_path);

    for entry in w {
        let unwrapped_entry = entry.unwrap();

        let metadata = unwrapped_entry.metadata().unwrap();

        if !metadata.is_file() {
            continue;
        }

        let file_path = unwrapped_entry.path();

        let file_path_str = file_path.display().to_string();

        let file_handle = tokio::fs::File::open(file_path);

        let mut reader = BufReader::new(file_handle);

        Part::stream(Body::wrap_stream(file_handle.);
    }


    // Body::wrap_stream(stream)

    // let form = reqwest::multipart::Form::new();

    // form.part(name, part)
}