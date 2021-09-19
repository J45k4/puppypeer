import React from "react"
import { useAgents } from "./use-agents-state"

import Link from "next/link"
import { Card, Table } from "react-bootstrap"

export const AgentsTable = () => {
	const agents = useAgents()

	return (
		<Card>
			<Card.Body>
				<Table>
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
										<Link href={`/agent/${p.agent_id}`}>
											{p.agent_id}
										</Link>
									</td>
									<td>
										{p.computerName}
									</td>
									<td>
										{p.connected ? "yes" : "no"}
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
				</Table>
			</Card.Body>
		</Card>
	)
}