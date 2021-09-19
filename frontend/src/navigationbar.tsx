import React from "react"
import { Nav, Navbar } from "react-bootstrap"
import Link from "next/link"

export const Navigationbar = () => {
	return (
		<Navbar
			collapseOnSelect
			expand="lg"
			style={{
				backgroundColor: "white",
				borderRadius: "0px",
				marginBottom: "10px",
				boxShadow:
					"0 1px 5px 0 rgba(0, 0, 0, 0.2), 0 2px 2px 0 rgba(0, 0, 0, 0.14),0 3px 1px -2px rgba(0, 0, 0, 0.12)"
			}}>
				<Link href="/">
					<Navbar.Brand style={{
						cursor: "pointer"
					}}>Epic shelter</Navbar.Brand>
				</Link>
				<Navbar.Toggle aria-controls="responsive-navbar-nav" />
				<Navbar.Collapse>
					<Nav className="mr-auto">
						<Link href="/agents" passHref>
							<Nav.Link>
								Agents
							</Nav.Link>
						</Link>
					</Nav>
				</Navbar.Collapse>
		</Navbar>
	)
}