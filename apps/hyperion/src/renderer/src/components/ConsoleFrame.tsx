import type { ReactNode } from "react";

import type { ConnectionStatus } from "../lib/connection";
import { LinkStatus } from "./LinkStatus";
import { UtcClock } from "./UtcClock";

interface ConsoleFrameProps {
  /** Unique title of the display shown in the work area. */
  readonly title: string;
  /** Titles of the station's displays in navigation order; must include `title`. */
  readonly displays: ReadonlyArray<string>;
  readonly linkStatus: ConnectionStatus;
  readonly children: ReactNode;
}

/**
 * The fixed frame shared by every console: header strip, work area and navigation bar.
 *
 * @remarks
 * See "Layout" in `docs/frontend/ux-guidelines.md`. The header strip is identical at every station.
 */
export function ConsoleFrame({ title, displays, linkStatus, children }: ConsoleFrameProps) {
  return (
    <div className="console">
      <header className="console__header">
        <div className="console__identity">
          <span className="console__ship">HYPERION</span>
          <h1 className="console__title">{title}</h1>
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
            <li key={display} aria-current={display === title ? "page" : undefined}>
              {display}
            </li>
          ))}
        </ul>
      </nav>
    </div>
  );
}
