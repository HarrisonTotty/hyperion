import { ConnectionPanel } from "./components/ConnectionPanel";
import { ConsoleFrame } from "./components/ConsoleFrame";
import { DEFAULT_SERVER_URL, useServerConnection } from "./lib/connection";

const SERVER_URL = import.meta.env.VITE_HYPERION_SERVER_URL ?? DEFAULT_SERVER_URL;

const DISPLAYS = ["Link"] as const;

/** Root of the bridge client. Owns the server connection and lays out the consoles. */
export function App() {
  const connection = useServerConnection(SERVER_URL, __APP_VERSION__);
  return (
    <ConsoleFrame title="Link" displays={DISPLAYS} linkStatus={connection.status}>
      <ConnectionPanel url={SERVER_URL} clientVersion={__APP_VERSION__} connection={connection} />
    </ConsoleFrame>
  );
}
