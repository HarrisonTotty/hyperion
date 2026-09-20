import { ConnectionPanel } from "./components/ConnectionPanel";
import { DEFAULT_SERVER_URL, useServerConnection } from "./lib/connection";

const SERVER_URL = import.meta.env.VITE_HYPERION_SERVER_URL ?? DEFAULT_SERVER_URL;

/** Root of the bridge client. Owns the server connection and lays out the consoles. */
export function App() {
  const connection = useServerConnection(SERVER_URL, __APP_VERSION__);
  return (
    <main className="bridge">
      <header className="bridge__header">
        <h1>HYPERION</h1>
        <span className="bridge__version">v{__APP_VERSION__}</span>
      </header>
      <ConnectionPanel url={SERVER_URL} connection={connection} />
    </main>
  );
}
