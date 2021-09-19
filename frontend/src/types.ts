
export interface AgentState {
    type: "agentState"
    agent_id: string
    connected: boolean
    computerName: string
    sendBytes: number
    receivedByts: number
    sendSpeed: number
}

export interface FolderEntry {
	type: "folderEntry"
	entryType: "FILE" | "FOLDER"
	name: string
	path: string
	modifiedAt: Date
	createdAt: Date
}

export interface subscribeToAgents {
    type: "subscribeToAgents"
}

export type MessageToServer = subscribeToAgents

export type MessageFromServer = AgentState

export interface WebsocketConnected {
    type: "websocketConnected"
}

export interface WebsocketDisconnected {
    type: "websocketDisconnected"
}

// export interface WebsocketError {
// 	type: "websocketError"
// 	message: string
// }

export type ConnectionEvent = AgentState |
	FolderEntry |
    WebsocketConnected |
    WebsocketDisconnected