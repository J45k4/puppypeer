use futures::SinkExt;
use futures::StreamExt;
use futures::stream::SplitSink;
use warp::ws::Message;
use warp::ws::WebSocket;

use crate::agent::Agent;
use crate::agent_register::AgentRegister;
use crate::types::MessageFromClient;
use crate::types::MessageToClient;

struct WsState {
	subscribed_to_agents: bool
}

async fn handle_ws_message(
	agent_register: AgentRegister,
	msg: Option<Result<Message, warp::Error>>,
	tx: &mut SplitSink<WebSocket, Message>,
	state: &mut WsState
) {
	let msg = match msg {
		Some(Ok(m)) => {
			log::debug!("Received message {:?}", m);

			if !m.is_text() {
				return;
			}

			let msg = m.to_str().unwrap();
            let msg: MessageFromClient = serde_json::from_str(msg).unwrap();
			
			msg
		}
		_ => {
			return;
		}
	};

	match msg {
	    MessageFromClient::SubscribeToAgents => {
			log::info!("Subscribe to agents");

			state.subscribed_to_agents = true;

			let agents = agent_register.get_agents().await;

			for agent in agents {
				let agent_state = agent.get_state().await;

				log::debug!("Sending agent {:?}", agent_state);

				let msg = MessageToClient::AgentState(agent_state);
				let msg = serde_json::to_string(&msg).unwrap();
				let msg = Message::text(msg);
			
				tx.send(msg).await.unwrap_or_default();
			}
		}
	}
}

async fn handle_agent_change(
	agent: Agent,
	tx: &mut SplitSink<WebSocket, Message>
) {
	let agent_state = agent.get_state().await;

	let msg = MessageToClient::AgentState(agent_state);
	let msg = serde_json::to_string(&msg).unwrap();
	let msg = Message::text(msg);

	tx.send(msg).await.unwrap_or_default();
}

pub async fn handle_ws(ws: warp::ws::WebSocket) {
    // let (mut tx, rx) = ws.split();

	// let mut state = WsState{
	// 	subscribed_to_agents: false
	// };

	// let agent_changes = agent_register.sub_to_changes();

	// tokio::pin!(rx);
	// tokio::pin!(agent_changes);

    // loop {
	// 	tokio::select! {
	// 		ws_msg = rx.next() => {
	// 			handle_ws_message(agent_register.clone(), ws_msg, &mut tx, &mut state).await;
	// 		}
	// 		Ok(agent) = agent_changes.recv() => {
	// 			handle_agent_change(agent, &mut tx).await;
	// 		}
	// 	};
    // }
}

pub struct WsHandler {

}

impl WsHandler {
	
}