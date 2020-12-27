use actix_multipart::Multipart;
use actix_web::{App, Error, HttpRequest, HttpResponse, HttpServer, middleware, web};
use common::provide_file_handle;
use futures::{StreamExt, TryStreamExt};
use tokio::io::{self, AsyncWriteExt};
use std::io::Write;

use actix_web::{post, get};
use tokio::fs::File;

use crate::config::get_file_data_path;

#[get("/v1/file/{hash}/metadata")]
async fn get_file_metadata(web::Path(hash): web::Path<String>) -> Result<String, ()> {
    Ok(hash)
}

#[post("/v1/file/{hash}/content")]
async fn post_file_content(web::Path(hash): web::Path<String>, mut payload: Multipart) -> String {
    let data_file_path = get_file_data_path(&hash);

    log::info!("Saving file contents to {}", data_file_path);
    
    while let Ok(Some(mut field)) = payload.try_next().await {
        let mime = field.content_type().clone();

        let mime_string = mime.to_string();
        let mime_str = mime_string.as_str();

        match mime_str {
            "application/octet-stream" => {
                let mut file = provide_file_handle(&data_file_path);

                let content_type = field.content_disposition().unwrap();
    
                let filename = content_type.get_filename().unwrap();
                let filepath = format!("./tmp/{}", sanitize_filename::sanitize(&filename));
        


                // File::create is blocking operation, use threadpool
                let mut f = web::block(|| std::fs::File::create(filepath))
                    .await
                    .unwrap();
        
                // Field in turn is stream of *Bytes* object
                while let Some(chunk) = field.next().await {
                    let data = chunk.unwrap();
                    println!("data bytes {}", data.len());
        
                    // file.write_all(&data).unwrap();
                    //filesystem operations are blocking, we have to use threadpool
                    f = web::block(move || f.write_all(&data).map(|_| f)).await.unwrap();
                }  
            },
            _ => {
                panic!("Unsupported mime type");
            }
        }

        println!("mime {}", mime);
    }


    "asdsf".to_string()
}

#[post("/v1/file/{hash}/content/offset/{offset}")]
async fn post_file_content_from_offset(web::Path((hash, offset)): web::Path<(String, u32)>) -> String {
    "asdfg".to_string()
}

#[get("/v1/file/{hash}/content")]
async fn get_file_contents() -> String {
    "filecontent".to_string()
}

#[get("/ping")]
async fn ping() -> String {
    "Ok".to_string()
}