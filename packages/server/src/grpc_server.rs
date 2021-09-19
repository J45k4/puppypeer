use std::pin::Pin;
use epic_shelter_generated_protos::epic_shelter::Command;
use epic_shelter_generated_protos::epic_shelter::PushFsChangesResponse;
use epic_shelter_generated_protos::epic_shelter::SendClientInfoResponse;
use epic_shelter_generated_protos::epic_shelter::ServerEvent;
use epic_shelter_generated_protos::epic_shelter::SubscribeToCommandsRequest;
use epic_shelter_generated_protos::epic_shelter::epic_shelter_server::EpicShelter;
use epic_shelter_generated_protos::epic_shelter::ClientEvent;
use futures::Stream;
use tokio::sync::mpsc;
use tonic::Response;
use tonic::Status;
use tokio_stream::wrappers::ReceiverStream;

use crate::agent::Agent;
use crate::agent_register::AgentRegister;

pub struct EpicShelterImpl {
    db: sled::Db,
	agent_register: AgentRegister
}

impl EpicShelterImpl {
    pub fn new(db: sled::Db, agent_register: AgentRegister) -> EpicShelterImpl {
        EpicShelterImpl{
            db: db,
			agent_register: agent_register
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
		log::info!("Subscribed to commands");

		let register = self.agent_register.clone();

		let request = request.into_inner();

		let agent = if let Some(a) = register.get_agent(&request.agent_id).await {
			a
		} else {
			Agent::new(&request.agent_id)
		};

		agent.set_connected(true).await;

		let (tx, mut rx) = mpsc::channel(10);

    	let (buff_tx, buff_rx) = mpsc::channel(10);

		agent.set_cmd_sender(tx).await;

		register.emit_agent_change(agent.clone()).await;

        tokio::spawn(async move {
            loop {
				let cmd = match rx.recv().await {
					Some(v) => v,
					None => break
				};

                match buff_tx.send(Ok(Command{ 
                    ..Default::default()
                })).await {
                    Ok(_) => {},
                    Err(err) => {
						log::info!("Error happened with the stream {:?}", err);

						agent.set_connected(false).await;

						return
					}
                };
            }
        });

        Ok(Response::new(Box::pin(
            ReceiverStream::new((buff_rx))
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

	type eventsStream = Pin<Box<dyn Stream<Item = Result<ServerEvent, Status>> + Send + Sync + 'static>>;

	async fn events(
		&self,
		request: tonic::Request<tonic::Streaming<ClientEvent>>
	) -> Result<Response<Self::eventsStream>, Status> {
		let (tx, rx) = mpsc::channel(20);

		let s = ReceiverStream::new(rx);

		Ok(Response::new(Box::pin(s) as Self::eventsStream))
	}
}