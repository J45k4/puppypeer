import { useEffect, useState } from "react"
import { sendMessageToServer, subToConnEvents } from "./conn"
import { AgentState } from "./types"

const agentsMap = new Map<string, AgentState>()

export const useAgents = () => {
    const [agents, setAgents] = useState<AgentState[]>(Array.from(agentsMap.values()))

    useEffect(() => {

        const s = subToConnEvents({
            next: (msg) => {
                if (msg.type === "agentState") {
                    setAgents(Array.from(agentsMap.values()))
                }
            }
        })

        return () => {
            s.unsubscribe()
        }
    }, [])

    return agents
}

subToConnEvents({
	next: (e) => {
		if (e.type === "agentState") {
			agentsMap.set(e.agent_id, e)
		}
	},
	error: () => {}
})

export const subscribeToAgents = () => {
    sendMessageToServer({
        type: "subscribeToAgents"    
    })
}