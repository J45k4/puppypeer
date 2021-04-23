import React from "react"
import { useAgents } from "./use-agents-state"

export const AgentsTable = () => {
	const agents = useAgents()

	return (
		<table>
			<thead>
				<tr>
					<th>
						Agentid
                	</th>
					<th>
						Computer
                	</th>
					<th>
						Connected
                	</th>
					<th>
						Send bytes
                	</th>
					<th>
						Received bytes
               	 	</th>
					<th>
						Send speed
                	</th>
				</tr>
			</thead>
			<tbody>
				{agents.map(p => {
					return (
						<tr key={p.agent_id}>
							<td>
								{p.agent_id}
							</td>
							<td>
								{p.computerName}
							</td>
							<td>
								{p.connected}
							</td>
							<td>
								{p.sendBytes}
							</td>
							<td>
								{p.receivedByts}
							</td>
							<td>
								{p.sendSpeed}
							</td>
						</tr>
					)
				})}
			</tbody>
		</table>
	)
}