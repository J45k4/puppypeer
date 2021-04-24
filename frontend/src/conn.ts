import { ConnectionEvent, MessageFromServer, MessageToServer } from "./types"
import { PartialObserver, Subject } from "rxjs"

const connEvents = new Subject<ConnectionEvent>()

export const subToConnEvents = (o: PartialObserver<ConnectionEvent>) => connEvents.subscribe(o)

let socket: WebSocket

export const sendMessageToServer = (msg: MessageToServer) => {
    if (!socket) {
        return
    }

    socket.send(JSON.stringify(msg))
}

type ServerMessageSubscriber = (msg: MessageFromServer) => void

const newMessageSubscribers = new Set()

export const subscribeToServerMessages = (subscriber: ServerMessageSubscriber) => {
    newMessageSubscribers.add(subscriber)
}

export const unsubscribeFromServerMessages = (subscriber: ServerMessageSubscriber) => {
    newMessageSubscribers.delete(subscriber)
}

const token = "qwert"

function createClient() {
	if (socket) {
		return
	}

    // var scheme = document.location.protocol == "https:" ? "wss" : "ws";
    // var port = document.location.port ? ":" + document.location.port : "";
    // var wsURL = scheme + "://" + document.location.hostname + port + "/api/socket?token=" + token

    const wsURL = "ws://localhost:45000/api/ws"

    socket = new WebSocket(wsURL)

    socket.onopen = () => {
        connEvents.next({
            type: "websocketConnected"
        })
    }

    socket.onerror = (e) => {
		socket = undefined
    }

    socket.onclose = () => {
		socket = undefined

        connEvents.next({
			type: "websocketDisconnected"
        })

        setTimeout(() => {
            createClient();
        }, 2000)
    }

    socket.onmessage = (msg) => {
        const parsedMessage = JSON.parse(msg.data)

        connEvents.next(parsedMessage)
    }
}

if (typeof document !== "undefined") {
    createClient();
}
