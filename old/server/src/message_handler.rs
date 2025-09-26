use crate::types::MessageFromClient;


pub async fn hdl_msg_from_cli(message: MessageFromClient) {
    match message {
        MessageFromClient::SubscribeToAgents => {
            log::info!("Subscribe to agents");
        }
    }
}