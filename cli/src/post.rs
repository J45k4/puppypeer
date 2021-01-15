use std::{fmt::format, io::BufReader, path::Path};

use clap::ArgMatches;
use common::get_millis_timestamp;
use futures::Stream;
use reqwest::{Body, Client, multipart::{Form, Part}};
use tokio::prelude::*;
use walkdir::WalkDir;
use tokio_util::codec;
use tokio_util::codec::{Framed, BytesCodec};

pub async fn exec_post(args: &ArgMatches<'_>) {
    let base_url = args.value_of("url").unwrap();
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

        let file_handle = tokio::fs::File::open(file_path).await.unwrap();

        let framed = Framed::new(file_handle, BytesCodec::new());

        let body = Body::wrap_stream(framed);
    
        let file_name = file_path.file_name()
            .unwrap().to_str().unwrap().to_string();

        let s = Part::stream(body).file_name(file_name);

        let modified = get_millis_timestamp(metadata.modified().unwrap()).unwrap();
        let accessed = get_millis_timestamp(metadata.accessed().unwrap()).unwrap();
        let created = get_millis_timestamp(metadata.created().unwrap()).unwrap();
        let readonly = metadata.permissions().readonly();

        let relative_path = file_path.strip_prefix(input_folder_path_str).unwrap().to_str().unwrap().to_string();
    
        let form = Form::new()
            .part("file", s)
            .part("modified_at", Part::text(modified.to_string()).mime_str("text/plain").unwrap())
            .part("accessed_at", Part::text(accessed.to_string()).mime_str("text/plain").unwrap())
            .part("created_at", Part::text(created.to_string()).mime_str("text/plain").unwrap())
            .part("readonly", Part::text(readonly.to_string()).mime_str("text/plain").unwrap())
            .part("full_file_path", Part::text(file_path_str.clone()).mime_str("text/plain").unwrap())
            .part("relative_file_path", Part::text(relative_path).mime_str("text/plain").unwrap())
            .part("root_folder_path", Part::text(input_folder_path_str.to_string()).mime_str("text/plain").unwrap());

        let client = Client::new();

        let hash = common::calc_file_hash(&file_path_str).unwrap();

        let url = format!("{}/v1/file/{}/content", base_url, hash);

        log::info!("Posting file to {}", url);

        client.post(&url)
            .multipart(form)
            .send().await.unwrap();
    }
}