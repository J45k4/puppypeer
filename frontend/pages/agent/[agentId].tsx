import React, { Fragment } from "react"
import { Container } from "react-bootstrap"
import { Filesystem } from "../../src/agent/filesystem"
import { Navigationbar } from "../../src/navigationbar"

export default function AgentPage() {
	return (
		<Fragment>
			<Navigationbar/>
			<Container>
				<Filesystem />
			</Container>	
		</Fragment>
	)
}