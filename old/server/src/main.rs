
use std::collections::HashMap;
use std::convert::Infallible;

use agent_register::AgentRegister;
use either_body::*;

use futures::StreamExt;
use futures::future::Either;
use grpc_server::EpicShelterImpl;
use log::LevelFilter;
use tokio::sync::RwLock;
use tokio::sync::broadcast;
use std::sync::Mutex;
use tokio::sync::mpsc;
use tonic::client::GrpcService;
use warp::Filter;
use warp::hyper::Server;
use epic_shelter_generated_protos::epic_shelter::epic_shelter_server::EpicShelterServer;
use warp::hyper::Version;
use warp::hyper::service::make_service_fn;
use warp::hyper::service::service_fn;
use futures::future;
use futures::future::TryFutureExt;
use warp::ws::Message;
use std::sync::Arc;
use handle_ws::handle_ws;
use warp::hyper::{Body, Request, Response};

mod either_body;
mod handle_ws;
mod types;
mod message_handler;
mod agent_register;
mod grpc_server;
mod agent;
mod agent_types;

type Users = Arc<RwLock<HashMap<usize, mpsc::UnboundedSender<Result<Message, warp::Error>>>>>;

struct Dep {
	// chan: broadcast::Sender<u64>
}

#[tokio::main]
async fn main() {
    env_logger::Builder::new()
        .filter_level(LevelFilter::Info)
        .format_timestamp(None)
        .init();

    let db = sled::open("./serverdb").unwrap();

	let addr = "[::1]:4433".parse().unwrap();

	let mut server_config = quinn::ServerConfig::default();

	let mut server_config = quinn::ServerConfigBuilder::new(server_config);

	let mut endpoint = quinn::Endpoint::builder();

	// quinn::ServerConfigBuilder::new()

	let mut endpoint = quinn::Endpoint::builder();
	endpoint.listen(server_config.build());

	let (endpoint, mut incoming) = endpoint.bind(&addr).unwrap();
    eprintln!("listening on {}", endpoint.local_addr().unwrap());


    while let Some(conn) = incoming.next().await {
        log::info!("connection incoming");
        // tokio::spawn(
        //     handle_connection(root.clone(), conn).unwrap_or_else(move |e| {
        //         error!("connection failed: {reason}", reason = e.to_string())
        //     }),
        // );
    }


	let agent_register = AgentRegister::new();

	let ws_route = warp::path!("api" / "ws")
	.and(warp::ws())
	.map(|ws: warp::ws::Ws| {
		ws.on_upgrade(move |socket| handle_ws(socket))
	});

	let mut warp = warp::service(ws_route);

    Server::bind(&addr)
        .serve(make_service_fn(move |_| {
			log::info!("make_service_fn");

			future::ok::<_, Infallible>(tower::service_fn(
				move |req: hyper::Request<hyper::Body>| {
					warp.call(req)
						.map_err(Error::from)
				}
			))


        })).await.unwrap();
}


            // let mut epic_shelter = epic_shelter.clone();

			// async move {
			// 	Ok::<_, Infallible>(service_fn(move |req: Request<Body>| async move {
			// 		warp.call(req)
			// 			.map_err(Error::from)
			// 	}))
			// }

				// future::ok::<_, Infallible>(tower::service_fn(
				// 	move |req: hyper::Request<hyper::Body>| {
				// 		log::info!("tower_service_fn");
	
				// 		warp.call(req)
				// 			// .map_ok(|res| res.map(EitherBody::Left))
				// 			.map_err(Error::from)
						
				// 		// match req.version() {
				// 		// 	Version::HTTP_11 | Version::HTTP_10 => Either::Left(
				// 		// 		warp.call(req)
				// 		// 			.map_ok(|res| res.map(EitherBody::Left))
				// 		// 			.map_err(Error::from),
				// 		// 	),
				// 		// 	// Version::HTTP_2 => Either::Right(          
				// 		// 	// 	epic_shelter.call(req)
				// 		// 	// 		.map_ok(|res| res.map(EitherBody::Right))
				// 		// 	// 		.map_err(Error::from),
				// 		// 	// ),
				// 		// 	_ => unimplemented!()
				// 		// }
				// 	}
				// ))

// match req.version() {
//     Version::HTTP_11 | Version::HTTP_10 => Either::Left(
//         warp.call(req)
//             .map_ok(|res| res.map(EitherBody::Left))
//             .map_err(Error::from),
//     ),
//     Version::HTTP_2 => Either::Right(
//         tonic
//             .call(req)
//             .map_ok(|res| res.map(EitherBody::Right))
//             .map_err(Error::from),
//     ),
//     _ => unimplemented!(),
// }