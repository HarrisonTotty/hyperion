import { type ReactNode, useId, useLayoutEffect, useRef } from "react";

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

/** Whether `element` stands in a part of `root` not displayed: `hidden`, or `display: none`. */
function hiddenWithin(element: HTMLElement, root: HTMLElement): boolean {
  for (
    let each: HTMLElement | null = element;
    each !== null && each !== root;
    each = each.parentElement
  ) {
    if (each.hasAttribute("hidden") || each.style.display === "none") {
      return true;
    }
  }
  return false;
}

/**
 * The fixed frame shared by every console: header strip, work area and navigation bar.
 *
 * @remarks
 * See "Layout" in `docs/frontend/ux-guidelines.md`. The header strip is identical at every station
 * and titles the active display. The navigation bar is a row of display tabs, each a button
 * showing its key; the active one is marked by `aria-current` and a top rule, not by colour alone.
 * The work area carries a modifier class named after the active display, so that a display with a
 * fixed layout of its own, such as `GALAXY`, can replace the default grid of panels. A display left
 * by its key while the focus is in it hands the focus to the navigation tab of the display shown.
 */
export function ConsoleFrame({
  displays,
  activeDisplay,
  onSelectDisplay,
  linkStatus,
  children,
}: ConsoleFrameProps) {
  const baseId = useId();
  const tabId = (id: DisplayId): string => `${baseId}-${id}`;
  const workRef = useRef<HTMLElement>(null);
  // A display chosen by its key hides the one the focus may be in, which keeps it on a hidden
  // element in jsdom and loses it in Chromium; before paint it goes to the tab of the display now
  // shown, where a click on that tab would have left it (WCAG 2.4.3).
  useLayoutEffect(() => {
    const focused = document.activeElement;
    const work = workRef.current;
    if (
      focused instanceof HTMLElement &&
      work !== null &&
      work.contains(focused) &&
      hiddenWithin(focused, work)
    ) {
      document.getElementById(`${baseId}-${activeDisplay}`)?.focus();
    }
  }, [baseId, activeDisplay]);
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
      <main className={`console__work console__work--${active.id}`} ref={workRef}>
        {children}
      </main>
      <nav className="console__nav" aria-label="Displays">
        <ul>
          {displays.map((display) => (
            <li key={display.id}>
              <button
                id={tabId(display.id)}
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
