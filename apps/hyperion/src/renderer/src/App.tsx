import { Activity, type ReactElement, useCallback, useState } from "react";

import { ConnectionPanel } from "./components/ConnectionPanel";
import { ConsoleFrame } from "./components/ConsoleFrame";
import { GraphicsPanel } from "./components/GraphicsPanel";
import { UniverseProvider } from "./components/UniverseProvider";
import { GalaxyDisplay } from "./displays/galaxy/GalaxyDisplay";
import { SystemDisplay } from "./displays/system/SystemDisplay";
import { ViewDisplay } from "./displays/view/ViewDisplay";
import type { SystemOpening, SystemTarget } from "./displays/system/systemTarget";
import { type ConnectionState, useServerConnection } from "./lib/connection";
import { DISPLAYS, type DisplayId } from "./lib/displays";
import { ServerLinkContext, useServerLinkValue } from "./lib/serverLink";
import { useDisplayKeys } from "./lib/useDisplayKeys";
import { GraphicsStatusProvider } from "./view/engine/GraphicsStatusProvider";
import { navigatorGpu } from "./view/engine/status";

/** What `App` hands the displays besides the server link and the universe, which are contexts. */
interface DisplayInputs {
  readonly connection: ConnectionState;
  readonly serverUrl: string;
  readonly systemOpening: SystemOpening | null;
  readonly openSystem: (target: SystemTarget) => void;
}

function displayContent(id: DisplayId, inputs: DisplayInputs): ReactElement {
  let content: ReactElement;
  switch (id) {
    case "link":
      content = (
        <>
          <ConnectionPanel
            url={inputs.serverUrl}
            clientVersion={__APP_VERSION__}
            connection={inputs.connection}
          />
          <GraphicsPanel />
        </>
      );
      break;
    case "galaxy":
      content = <GalaxyDisplay onOpenSystem={inputs.openSystem} />;
      break;
    case "system":
      content = <SystemDisplay opening={inputs.systemOpening} />;
      break;
    case "view":
      content = <ViewDisplay />;
      break;
  }
  return content;
}

/**
 * Root of the bridge client. Owns the server connection and the choice of display, and lays out
 * the consoles.
 *
 * @remarks
 * The server it links to is the one the main process resolved from the command line, handed over by
 * the preload, as are the launch's graphics mode and the GPU process's crashes, which feed the one
 * graphics status store beside the adapter's answer (R01.T4).
 *
 * Every display stays mounted under React's `Activity`: a hidden one keeps its state (a chart's
 * centre, its camera, its selection) while its effects are torn down and it leaves the
 * accessibility tree, and switching back is instant (plan 05, design note D1). Displays reach the
 * server through `ServerLinkContext` and the open universe through `UniverseContext`; both values
 * are memoised, so that a latency update re-renders the `LINK` display and the header but no
 * display that only makes requests.
 *
 * `OPEN SYSTEM` on the `GALAXY` display opens the `SYSTEM` display on its selected system at the
 * chart's time (plan 14, P14.T41.a): `App` keeps the latest opening, counted so that each is a new
 * display state, and switches to the display. The callback is stable, so the memoised `GALAXY`
 * display is not rendered again for it.
 */
export function App() {
  const serverUrl = window.hyperion.serverUrl;
  const connection = useServerConnection(serverUrl, __APP_VERSION__);
  const link = useServerLinkValue(connection);
  const [activeDisplay, setActiveDisplay] = useState<DisplayId>("link");
  const [systemOpening, setSystemOpening] = useState<SystemOpening | null>(null);
  useDisplayKeys(DISPLAYS, setActiveDisplay);
  const openSystem = useCallback((target: SystemTarget): void => {
    setSystemOpening((previous) => ({ target, sequence: (previous?.sequence ?? 0) + 1 }));
    setActiveDisplay("system");
  }, []);
  const inputs: DisplayInputs = { connection, serverUrl, systemOpening, openSystem };
  return (
    <GraphicsStatusProvider graphics={window.hyperion.graphics} gpu={navigatorGpu()}>
      <ServerLinkContext value={link}>
        <UniverseProvider>
          <ConsoleFrame
            displays={DISPLAYS}
            activeDisplay={activeDisplay}
            onSelectDisplay={setActiveDisplay}
            linkStatus={connection.status}
            // The VIEW display draws only kept test scenes until R02.T17 brings the server's.
            modeBanner={activeDisplay === "view" ? "TRAINING" : null}
          >
            {DISPLAYS.map(({ id }) => (
              <Activity key={id} mode={id === activeDisplay ? "visible" : "hidden"}>
                {displayContent(id, inputs)}
              </Activity>
            ))}
          </ConsoleFrame>
        </UniverseProvider>
      </ServerLinkContext>
    </GraphicsStatusProvider>
  );
}
