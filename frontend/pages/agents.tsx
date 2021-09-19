import React, { Fragment } from "react"
import { Container } from "react-bootstrap"
import { AgentsTable } from "../src/agents-table"
import { Navigationbar } from "../src/navigationbar"

const Agents = () => {
    return (
		<Fragment>
			<Navigationbar />
			<Container>
				<AgentsTable />
			</Container>
		</Fragment>
        
    )
}

export default Agents