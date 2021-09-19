import React from "react"
import { Card } from "react-bootstrap"
import { useFolderEntries } from "./use-folder-entries"

const Folder = (props: {
	folderPath: string
}) => {
	const entries = useFolderEntries(props.folderPath)

	return (
		<div>
			{entries.map(p => (
				<div>
					{p.name}
				</div>
			))}
			<div>
				C:
			</div>
			<div>
				D:
			</div>
			<div>
				E:
			</div>
			<div>
				F:
			</div>
		</div>
	)
}

export const Filesystem = (props: {
	agentId: string
}) => {
	return (
		<Card>
			<Card.Body>
				<Card.Title>Filesystem</Card.Title>
				<Folder folderPath={props.agentId} />
			</Card.Body>
		</Card>
	)
}