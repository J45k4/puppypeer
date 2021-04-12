use std::sync::Arc;
use std::time::Duration;
use epic_shelter_generated_protos::epic_shelter::Command;
use epic_shelter_generated_protos::epic_shelter::FetchFileMetadataRequest;
use epic_shelter_generated_protos::epic_shelter::FetchFileMetadataResponse;
use epic_shelter_generated_protos::epic_shelter::PushFsChangesRequest;
use epic_shelter_generated_protos::epic_shelter::PushFsChangesResponse;
use epic_shelter_generated_protos::epic_shelter::SendClientInfoRequest;
use epic_shelter_generated_protos::epic_shelter::SendClientInfoResponse;
use epic_shelter_generated_protos::epic_shelter::SubscribeToCommandsRequest;
use epic_shelter_generated_protos::epic_shelter::epic_shelter_client::EpicShelterClient;
use failure::bail;
use tokio::sync::Mutex;
use tokio::sync::broadcast;
use tokio::sync::oneshot;
use tokio::time::sleep;
use tonic::Streaming;
use tonic::transport::Channel;

struct ServerClientInner {
    stopper: Option<oneshot::Sender<()>>
}

impl Drop for ServerClientInner {
    fn drop(&mut self) {
        if let Some(stopper) = self.stopper.take() {
            stopper.send(()).unwrap_or_default();
        }
    }
}

pub struct ServerClient {
    address: String,
    inner: Arc<ServerClientInner>,
    sub_chan: broadcast::Sender<Command>,
    client: Arc<Mutex<Option<EpicShelterClient<Channel>>>>
}

impl Clone for ServerClient {
    fn clone(&self) -> Self {
        ServerClient{
            address: self.address.clone(),
            inner: self.inner.clone(),
            sub_chan: self.sub_chan.clone(),
            client: self.client.clone()
        }
    }
}

impl ServerClient {
    pub async fn connect(address: &str) -> Result<ServerClient, failure::Error> {
        // let client = EpicShelterClient::connect(address.to_owned()).await?;

        let (chan, _) = broadcast::channel(20);
        let (stopper, stopper_receiver) = oneshot::channel();

        let client= ServerClient{
            address: address.to_owned(),
            inner: Arc::new(ServerClientInner {
                stopper: Some(stopper)
            }),
            sub_chan: chan.clone(),
            client: Arc::new(Mutex::new(None))
        };


        create_subscribe_worker(client.clone(), chan, stopper_receiver);

        Ok(client)
    }
    
    pub async fn subscribe_to_commands(&self) -> broadcast::Receiver<Command> {
        self.sub_chan.subscribe()
    }

    pub async fn send_client_info(&self, req: SendClientInfoRequest) -> Result<SendClientInfoResponse, failure::Error>  {
        let mut client = self.get_client().await?;

        let r = client.send_client_info(req).await?;

        Ok(r.into_inner())
    }

    pub async fn push_fs_changes(&self, req: PushFsChangesRequest) -> Result<PushFsChangesResponse, failure::Error> {
        let mut client = self.get_client().await?;

        let r = client.push_fs_changes(req).await?;

        Ok(r.into_inner())
    }
    
    pub async fn upload_data(&self, file_hash: &str, offset: u64, data: &[u8]) {

    }

    pub async fn fetch_file_metadata(&self, req: FetchFileMetadataRequest) -> Result<FetchFileMetadataResponse, failure::Error> {
        let mut client = self.get_client().await?;

        let r = client.fetch_file_metadata(req).await?;

        Ok(r.into_inner())
    }

    async fn try_to_connect(&self) -> Result<(), failure::Error> {
        let mut c = self.client.lock().await;
        
        let client = EpicShelterClient::connect(self.address.clone()).await?;

        *c = Some(client);

        Ok(())
    }

    async fn get_client(&self) -> Result<EpicShelterClient<Channel>, failure::Error> {
        match self.client.lock().await.clone() {
            Some(c) => Ok(c),
            None => bail!("Client is not connected")
        }
    }

    async fn create_sub(&self) -> Result<Streaming<Command>, failure::Error> {
        let mut client = self.get_client().await?;

        Ok(client.subscribe_to_commands(SubscribeToCommandsRequest{}).await?.into_inner())
    }
}

fn create_subscribe_worker(
    client: ServerClient, 
    sub_send: broadcast::Sender<Command>, 
    stopper: oneshot::Receiver<()>
) {
    tokio::spawn(async move {
        let client = client;
        let stopper = stopper;

        tokio::pin!(stopper);

        loop {
            loop {
                log::info!("Trying to connect");

                match client.try_to_connect().await {
                    Ok(_) => break,
                    Err(_) => {}
                };

                sleep(Duration::from_secs(2)).await;
            }

            log::info!("Client connected");

            loop {
                let mut stream = match client.create_sub().await {
                    Ok(r) => r,
                    Err(_) => continue
                };

                log::info!("Client connected2");
    
                loop {
                    tokio::select! {
                        _ = &mut stopper => {
                            return;
                        }
                        r = stream.message() => match r {
                            Ok(Some(c)) => {
                                log::debug!("Received command {:?}", c);

                                sub_send.send(c).unwrap();
                            } 
                            Ok(None) => {
                                log::debug!("Received non ??");
                            }
                            Err(e) => {
                                log::error!("Client disconnected");
            
                                break;
                            }                        
                        }
                    };
                }
            }
        }
    });
}