import { subToConnEvents } from '../src/conn'
import { subscribeToAgents } from '../src/use-agents-state'
import '../styles/globals.css'
import 'bootstrap/dist/css/bootstrap.min.css';

if (typeof window !== "undefined") {
	subToConnEvents({
		next: (event) => {
			console.log("event", event)

			if (event.type === "websocketConnected") {
				subscribeToAgents()
			}
		}
	})
}


function MyApp({ Component, pageProps }) {
	return <Component {...pageProps} />
}

export default MyApp
