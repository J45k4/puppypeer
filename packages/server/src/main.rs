
use std::convert::Infallible;
use std::pin::Pin;
use std::time::Duration;

use either_body::*;
use epic_shelter_generated_protos::epic_shelter::Command;
use epic_shelter_generated_protos::epic_shelter::PushFsChangesResponse;
use epic_shelter_generated_protos::epic_shelter::SendClientInfoResponse;
use epic_shelter_generated_protos::epic_shelter::SubscribeToCommandsRequest;
use futures::Stream;
use futures::future::Either;
use log::LevelFilter;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tonic::Response;
use tonic::Status;
use tonic::client::GrpcService;
use warp::Filter;
use warp::hyper::Server;
use tonic::transport::Server as TonicServer;
use epic_shelter_generated_protos::epic_shelter::epic_shelter_server::EpicShelterServer;
use epic_shelter_generated_protos::epic_shelter::epic_shelter_server::EpicShelter;
use warp::hyper::Version;
use warp::hyper::service::make_service_fn;
use futures::future;
use futures::future::TryFutureExt;

mod either_body;

pub struct EpicShelterImpl {
    db: sled::Db
}

impl EpicShelterImpl {
    fn new(db: sled::Db) -> EpicShelterImpl {
        EpicShelterImpl{
            db: db,
        }
    }
}

#[async_trait::async_trait]
impl EpicShelter for EpicShelterImpl {
    async fn push_fs_changes(
            &self,
            request: tonic::Request<epic_shelter_generated_protos::epic_shelter::PushFsChangesRequest>,
        ) -> Result<tonic::Response<epic_shelter_generated_protos::epic_shelter::PushFsChangesResponse>, tonic::Status> {
        
            println!("Handling this shit {:?}", request);

        Ok(tonic::Response::new(PushFsChangesResponse{}))
    }

    type subscribe_to_commandsStream = Pin<Box<dyn Stream<Item = Result<Command, Status>> + Send + Sync + 'static>>;

    async fn subscribe_to_commands(
        &self,
        request: tonic::Request<SubscribeToCommandsRequest>,
    ) -> Result<tonic::Response<Self::subscribe_to_commandsStream>, tonic::Status> {
        let (tx, rx) = mpsc::channel(4);

        tokio::spawn(async move {
            let tx = tx;
            loop {
                match tx.send(Ok(Command{ 
                    ..Default::default()
                })).await {
                    Ok(_) => {},
                    Err(_) => return
                }

                sleep(Duration::from_secs(1)).await;
            }
        });

        Ok(Response::new(Box::pin(
            tokio_stream::wrappers::ReceiverStream::new(rx),
        )))
    }

    async fn send_client_info(
            &self,
            request: tonic::Request<epic_shelter_generated_protos::epic_shelter::SendClientInfoRequest>,
        ) -> Result<Response<epic_shelter_generated_protos::epic_shelter::SendClientInfoResponse>, Status> {
        //self.db.insert(key, value)

        println!("Client version {}", request.get_ref().version);

        Ok(tonic::Response::new(SendClientInfoResponse{}))
    }

    async fn fetch_file_metadata(
            &self,
            request: tonic::Request<epic_shelter_generated_protos::epic_shelter::FetchFileMetadataRequest>,
        ) -> Result<Response<epic_shelter_generated_protos::epic_shelter::FetchFileMetadataResponse>, Status> {
        todo!()
    }
}

#[tokio::main]
async fn main() {
    env_logger::Builder::new()
        .filter_level(LevelFilter::Info)
        .format_timestamp(None)
        .init();

    let db = sled::open("./serverdb").unwrap();

    let addr = "[::1]:45000".parse().unwrap();

    let epic_shelter = EpicShelterServer::new(EpicShelterImpl::new(db.clone()));
    let mut warp = warp::service(warp::path("hello").map(|| "hello, world!"));

    Server::bind(&addr)
        .serve(make_service_fn(move |_| {
            let mut epic_shelter = epic_shelter.clone();

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