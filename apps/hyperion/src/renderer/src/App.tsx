import { Activity, type ReactElement, useState } from "react";

import { ConnectionPanel } from "./components/ConnectionPanel";
import { ConsoleFrame } from "./components/ConsoleFrame";
import { GalaxyDisplay } from "./displays/galaxy/GalaxyDisplay";
import { type ConnectionState, DEFAULT_SERVER_URL, useServerConnection } from "./lib/connection";
import { DISPLAYS, type DisplayId } from "./lib/displays";
import { useDisplayKeys } from "./lib/useDisplayKeys";

const SERVER_URL = import.meta.env.VITE_HYPERION_SERVER_URL ?? DEFAULT_SERVER_URL;

function displayContent(id: DisplayId, connection: ConnectionState): ReactElement {
  let content: ReactElement;
  switch (id) {
    case "link":
      content = (
        <ConnectionPanel url={SERVER_URL} clientVersion={__APP_VERSION__} connection={connection} />
      );
      break;
    case "galaxy":
      content = <GalaxyDisplay />;
      break;
  }
  return content;
}

/**
 * Root of the bridge client. Owns the server connection and the choice of display, and lays out
 * the consoles.
 *
 * @remarks
 * Every display stays mounted under React's `Activity`: a hidden one keeps its state (a chart's
 * centre, its camera, its selection) while its effects are torn down and it leaves the
 * accessibility tree, and switching back is instant (plan 05, design note D1).
 */
export function App() {
  const connection = useServerConnection(SERVER_URL, __APP_VERSION__);
  const [activeDisplay, setActiveDisplay] = useState<DisplayId>("link");
  useDisplayKeys(DISPLAYS, setActiveDisplay);
  return (
    <ConsoleFrame
      displays={DISPLAYS}
      activeDisplay={activeDisplay}
      onSelectDisplay={setActiveDisplay}
      linkStatus={connection.status}
    >
      {DISPLAYS.map(({ id }) => (
        <Activity key={id} mode={id === activeDisplay ? "visible" : "hidden"}>
          {displayContent(id, connection)}
        </Activity>
      ))}
    </ConsoleFrame>
  );
}
