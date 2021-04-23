
import { sendMessageToServer } from '../src/conn'
import styles from '../styles/Home.module.css'

export default function Home() {
  return (
    <div className={styles.container}>
      <button onClick={() => {
        sendMessageToServer({
          
        })
      }}>
        Do something
      </button>
    </div>
  )
}
