
use std::collections::HashMap;
use std::convert::Infallible;

use agent_register::AgentRegister;
use either_body::*;

use futures::future::Either;
use grpc_server::EpicShelterImpl;
use log::LevelFilter;
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tonic::client::GrpcService;
use warp::Filter;
use warp::hyper::Server;
use epic_shelter_generated_protos::epic_shelter::epic_shelter_server::EpicShelterServer;
use warp::hyper::Version;
use warp::hyper::service::make_service_fn;
use futures::future;
use futures::future::TryFutureExt;
use warp::ws::Message;
use std::sync::Arc;
use handle_ws::handle_ws;

mod either_body;
mod handle_ws;
mod types;
mod message_handler;
mod agent_register;
mod grpc_server;
mod agent;
mod agent_types;

type Users = Arc<RwLock<HashMap<usize, mpsc::UnboundedSender<Result<Message, warp::Error>>>>>;


#[tokio::main]
async fn main() {
    env_logger::Builder::new()
        .filter_level(LevelFilter::Debug)
        .format_timestamp(None)
        .init();

    let db = sled::open("./serverdb").unwrap();



    let addr = "[::1]:45000".parse().unwrap();

	let agent_register = AgentRegister::new();

    let epic_shelter = EpicShelterServer::new(EpicShelterImpl::new(db.clone(), agent_register.clone()));

	// let agent_register = Arc::new(agent_register);

	// let agent_register = warp::any().map(move || agent_register.clone());

    // let routes = warp::get().and(

    // )

    // routes.and(other)

    Server::bind(&addr)
        .serve(make_service_fn(move |_| {
			log::info!("make_service_fn");

			let agent_register = agent_register.clone();
			let agent_register = warp::any().map(move || agent_register.clone());

			let ws_route = warp::path!("api" / "ws")
				.and(warp::ws())
				.and(agent_register)
				.map(|ws: warp::ws::Ws, agent_register| {
					ws.on_upgrade(move |socket| handle_ws(socket, agent_register))
				});

            let mut epic_shelter = epic_shelter.clone();
			let mut warp = warp::service(ws_route);

            future::ok::<_, Infallible>(tower::service_fn(
                move |req: hyper::Request<hyper::Body>| match req.version() {
                    Version::HTTP_11 | Version::HTTP_10 => Either::Left(
                        warp.call(req)
                            .map_ok(|res| res.map(EitherBody::Left))
                            .map_err(Error::from),
                    ),
                    Version::HTTP_2 => Either::Right(          
                        epic_shelter.call(req)
                            .map_ok(|res| res.map(EitherBody::Right))
                            .map_err(Error::from),
                    ),
                    _ => unimplemented!()
                }
            ))
        })).await.unwrap();
}

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