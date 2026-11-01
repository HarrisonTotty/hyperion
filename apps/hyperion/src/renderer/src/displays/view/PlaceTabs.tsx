import type { KeyboardEvent } from "react";

import { StatusText } from "../../components/StatusText";

/** The page that stands in the camera panel's place while `CONTROLS` names an instrument. */
export type PlacePage = "camera" | "ground";

/**
 * Why `GROUND` is held back while the instrument `designator` states no body's terrain: its
 * `FRAME` is no body with a surface (plan R10, R10.T12.d; lean, 2026-10-31).
 */
export function groundHeldBack(designator: string): string {
  return `NOT AVAILABLE: ${designator}'s FRAME is no body with a surface`;
}

/** The IDs that tie the place's tabs to its page and to `GROUND`'s reason. */
export interface PlaceIds {
  /** A tab's ID, by which the page names itself. */
  readonly tab: (page: PlacePage) => string;
  /** The page's ID, which each tab controls. */
  readonly panel: string;
  /** The held-back `GROUND`'s reason's ID, which describes it. */
  readonly reason: string;
}

/** Props of {@link PlaceTabs}. */
export interface PlaceTabsProps {
  /** The page standing: `GROUND` only where it is not held back. */
  readonly page: PlacePage;
  /** Why `GROUND` is held back ({@link groundHeldBack}), or `null` where it is offered. */
  readonly groundReason: string | null;
  readonly onPage: (page: PlacePage) => void;
  /** The instrument's name, the place's designator (`INSTRUMENT 1`). */
  readonly designator: string;
  readonly ids: PlaceIds;
}

/** The tabs, by the page each shows. */
const TABS: ReadonlyArray<{ readonly page: PlacePage; readonly label: string }> = [
  { page: "camera", label: "CAMERA" },
  { page: "ground", label: "GROUND" },
];

/** The page an arrow, `Home` or `End` key moves the choice to from `page`, or `null`. */
function pageAfterKey(key: string, page: PlacePage): PlacePage | null {
  switch (key) {
    case "ArrowLeft":
    case "ArrowRight":
      return page === "camera" ? "ground" : "camera";
    case "Home":
      return "camera";
    case "End":
      return "ground";
    default:
      return null;
  }
}

/**
 * The camera panel's place's head while `CONTROLS` names an instrument (plan R10, R10.T12.d; guide
 * draft D2): the page tabs `CAMERA` and `GROUND`, the chosen one in `--accent` over a rule, then the
 * instrument's designator.
 *
 * @remarks
 * A tab list, as the ship's other page selector, `GALAXY`'s, is (ux-reviewer's ruling, 2026-10-31):
 * one stop in the tab order, the chosen page's tab, the arrow keys, `Home` and `End` choosing a page
 * there with the focus following, and a click choosing the page under it. `GROUND` is held back
 * rather than disabled while the instrument states no body's terrain, struck through, and described
 * by its reason, which stands over the place while the tab is hovered or focused, a press's focus
 * included, taking none of its room (guide draft D2): at the floor the place has no line to spare
 * for it (lean, 2026-11-01). The arrows reach the held-back tab without choosing it, so that the
 * keyboard shows its reason too, and a click or a tap focuses it, so that a press does.
 */
export function PlaceTabs({ page, groundReason, onPage, designator, ids }: PlaceTabsProps) {
  const offered = (each: PlacePage): boolean => each === "camera" || groundReason === null;
  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>): void => {
    const next = pageAfterKey(event.key, page);
    if (next === null) {
      return;
    }
    event.preventDefault();
    if (offered(next)) {
      onPage(next);
    }
    // The focus follows the arrow, as it does in a tab list, onto a held-back tab too, which it
    // does not choose, so that the focus shows its reason.
    document.getElementById(ids.tab(next))?.focus();
  };
  return (
    <div className="panel__title view-place__head">
      <div className="view-place__tabs" role="tablist" aria-label={`Place ${designator}`}>
        {TABS.map((tab) => {
          const held = !offered(tab.page);
          return (
            <button
              key={tab.page}
              id={ids.tab(tab.page)}
              type="button"
              role="tab"
              className="view-place__tab"
              aria-selected={page === tab.page}
              aria-controls={ids.panel}
              aria-disabled={held ? "true" : undefined}
              aria-describedby={held ? ids.reason : undefined}
              // One stop in the tab order: the chosen page's tab.
              tabIndex={page === tab.page ? 0 : -1}
              onClick={() => {
                if (!held) {
                  onPage(tab.page);
                }
              }}
              onKeyDown={onKeyDown}
            >
              {tab.label}
            </button>
          );
        })}
      </div>
      <span className="panel__designator">{designator}</span>
      {groundReason === null ? null : (
        <p className="view-place__reason" id={ids.reason}>
          <StatusText text={groundReason} />
        </p>
      )}
    </div>
  );
}
