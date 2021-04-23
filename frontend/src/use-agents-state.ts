import { useEffect, useState } from "react"
import { sendMessageToServer, subToConnEvents } from "./conn"
import { AgentState, MessageToServer } from "./types"


export const useAgents = () => {
    const [agents, setAgents] = useState<AgentState[]>([])

    useEffect(() => {
        const agentsMap = new Map<string, AgentState>()

        const s = subToConnEvents({
            next: (msg) => {
                if (msg.type === "agentState") {
					console.log("storing this agent state")
                    agentsMap.set(msg.agent_id, msg)
					console.log("agentsMap", agentsMap)
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

export const subscribeToAgents = () => {
    sendMessageToServer({
        type: "subscribeToAgents"    
    })
}