use epic_shelter_generated_protos::epic_shelter::command::CommandType;
use tokio::sync::broadcast::error::RecvError;

use crate::client::ServerClient;

pub async fn handle_commands(client: ServerClient) {
    let mut command_stream = client.subscribe_to_commands().await;

    loop {
        match command_stream.recv().await {
            Ok(c) => {
                log::debug!("Received command");

                if let Some(command_type) = c.command_type {
                    match command_type {
                        CommandType::ScanFolder(_) => {}
                        CommandType::MoveFiles(_) => {}
                        CommandType::UploadFile(_) => {}
                        CommandType::RemoveMe(_) => {
                            return;
                        }
                    }
                }
            }
            Err(RecvError::Closed) => {
                println!("Channel closed");
            }
            Err(RecvError::Lagged(t)) => {
                log::info!("I am laggging :( {}", t);
            }
        };
    }
}