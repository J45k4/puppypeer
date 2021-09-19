import React, { useState } from "react"
import { subToConnEvents } from "../conn"
import { Trie } from "../trie"
import { FolderEntry } from "../types"

const folderEntries: FolderEntry[] = []

export const useFolderEntries = (folderPath: string) => {
	const [ entries, setEntries ] = useState<FolderEntry[]>(folderEntries.filter(
		p => p.path === folderPath
	))

	return entries
}


subToConnEvents({
	next: (event) => {
		if (event.type === "folderEntry") {
			folderEntries.push(event)
		}
	},
	error: () => {

	}
})