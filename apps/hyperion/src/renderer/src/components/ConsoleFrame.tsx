import type { ReactNode } from "react";

import type { ConnectionStatus } from "../lib/connection";
import type { DisplayDefinition, DisplayId } from "../lib/displays";
import { LinkStatus } from "./LinkStatus";
import { UtcClock } from "./UtcClock";

interface ConsoleFrameProps {
  /** The station's displays in navigation order; must include `activeDisplay`. */
  readonly displays: ReadonlyArray<DisplayDefinition>;
  readonly activeDisplay: DisplayId;
  /** Called when the operator picks a display from the navigation bar. */
  readonly onSelectDisplay: (id: DisplayId) => void;
  readonly linkStatus: ConnectionStatus;
  readonly children: ReactNode;
}

/**
 * The fixed frame shared by every console: header strip, work area and navigation bar.
 *
 * @remarks
 * See "Layout" in `docs/frontend/ux-guidelines.md`. The header strip is identical at every station
 * and titles the active display. The navigation bar is a row of display tabs, each a button
 * showing its key; the active one is marked by `aria-current` and a top rule, not by colour alone.
 */
export function ConsoleFrame({
  displays,
  activeDisplay,
  onSelectDisplay,
  linkStatus,
  children,
}: ConsoleFrameProps) {
  const active = displays.find((display) => display.id === activeDisplay);
  if (active === undefined) {
    throw new Error(`active display "${activeDisplay}" is not among the station's displays`);
  }
  return (
    <div className="console">
      <header className="console__header">
        <div className="console__identity">
          <span className="console__ship">HYPERION</span>
          <h1 className="console__title">{active.title}</h1>
        </div>
        <div className="console__status">
          <UtcClock />
          <LinkStatus status={linkStatus} />
        </div>
      </header>
      <main className="console__work">{children}</main>
      <nav className="console__nav" aria-label="Displays">
        <ul>
          {displays.map((display) => (
            <li key={display.id}>
              <button
                type="button"
                className="console__tab"
                aria-current={display.id === activeDisplay ? "page" : undefined}
                onClick={() => {
                  onSelectDisplay(display.id);
                }}
              >
                <span className="console__key">{display.key}</span>{" "}
                <span className="console__tab-title">{display.title}</span>
              </button>
            </li>
          ))}
        </ul>
      </nav>
    </div>
  );
}
