use std::{fmt::format, io::BufReader, path::Path};

use clap::ArgMatches;
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
    
        let form = Form::new()
            .part("file", s);

        let client = Client::new();

        let hash = common::calc_file_hash(&file_path_str).unwrap();

        let url = format!("{}/v1/file/{}/content", base_url, hash);

        log::info!("Posting file to {}", url);

        client.post(&url)
            .multipart(form)
            .send().await.unwrap();
    }


    // Body::wrap_stream(stream)

    // let form = reqwest::multipart::Form::new();

    // form.part(name, part)
}