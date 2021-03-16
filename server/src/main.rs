use std::io::Write;

use actix_multipart::Multipart;
use actix_web::{App, Error, HttpRequest, HttpResponse, HttpServer, middleware, web};
use futures::{StreamExt, TryStreamExt};

use actix_web::{post, get};
use simple_logger::SimpleLogger;

mod handlers;
mod config;

async fn save_file(mut payload: Multipart) -> Result<HttpResponse, Error> {
    // iterate over multipart stream
    while let Ok(Some(mut field)) = payload.try_next().await {
        let mime = field.content_type();

        println!("mime {}", mime.type_());

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

            // filesystem operations are blocking, we have to use threadpool
            f = web::block(move || f.write_all(&data).map(|_| f)).await?;
        }
    }
    
    println!("save_file ready");

    Ok(HttpResponse::Ok().into())
}

fn ping() -> HttpResponse {
    HttpResponse::Ok().body("ok")
}

fn index() -> HttpResponse {
    let html = r#"<html>
        <head><title>Upload Test</title></head>
        <body>
            <form target="/" method="post" enctype="multipart/form-data">
                <input type="file" multiple name="file"/>
                <button type="submit">Submit</button>
            </form>
        </body>
    </html>"#;

    HttpResponse::Ok().body(html)
}


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .unwrap();

    std::env::set_var("RUST_LOG", "actix_server=info,actix_web=info");
    std::fs::create_dir_all("./tmp").unwrap();

    let ip = "0.0.0.0:45000";

    HttpServer::new(|| {
        App::new().wrap(middleware::Logger::default()).service(
            web::resource("/")
                .route(web::get().to(index))
                .route(web::post().to(save_file)),
        ).service(
            web::resource("/ping")
                .route(web::get().to(ping)),
        ).service(
            web::resource("/v1/file/")
        ).service(handlers::get_file_metadata)
        .service(handlers::post_file_content)
    })
    .bind(ip)?
    .run()
    .await
}