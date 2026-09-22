import type { UniverseIdHex } from "@hyperion/protocol";
import { type KeyboardEvent, useId, useState } from "react";

import type { CentreLy } from "../../lib/galaxy/model";
import { useUniverse } from "../../lib/universe";
import { GalaxyMapPanel } from "./GalaxyMapPanel";
import { ParametersPanel } from "./ParametersPanel";

/** The pages that share the column, in the order the selector offers them. */
type Page = "parameters" | "map";

const PAGES: ReadonlyArray<Page> = ["parameters", "map"];

/** Each page's name: full words, as the guide has panels titled. */
const PAGE_TITLE: Readonly<Record<Page, string>> = {
  parameters: "PARAMETERS",
  map: "GALAXY MAP",
};

/**
 * The operator's choice of page and the open universe it was made for: a choice lasts until
 * another universe is opened, which shows the map.
 */
interface PageChoice {
  readonly openId: UniverseIdHex | null;
  readonly page: Page;
}

/** The page an arrow, `Home` or `End` key moves the selection to from `page`, or `null`. */
function pageAfterKey(key: string, page: Page): Page | null {
  const index = PAGES.indexOf(page);
  let next: number;
  switch (key) {
    case "ArrowLeft":
      next = (index + PAGES.length - 1) % PAGES.length;
      break;
    case "ArrowRight":
      next = (index + 1) % PAGES.length;
      break;
    case "Home":
      next = 0;
      break;
    case "End":
      next = PAGES.length - 1;
      break;
    default:
      return null;
  }
  return PAGES[next] ?? null;
}

interface GalaxyPagesProps {
  /** Whether the pages are out of view, as they are while `UNIVERSE` is shown whole. */
  readonly hidden: boolean;
  /** The map cursor, in the `GALACTIC` frame. */
  readonly cursorLy: CentreLy;
  /** Moves the cursor, after a pick on a map or an arrow key on it. */
  readonly onCursor: (cursorLy: CentreLy) => void;
  /** The chart's centre, marked on both maps, or `null` before one is chosen. */
  readonly centreLy: CentreLy | null;
}

/**
 * The `PARAMETERS` and `GALAXY MAP` pages, sharing one panel behind a page selector, so that the
 * map, the display's primary picture, has the column's full width and height.
 *
 * @remarks
 * The selector is a tab list and the panel's title: the chosen page's name is marked by a rule
 * beneath it as well as by `aria-selected`, so its state is a shape and not a colour alone. It is
 * one stop in the tab order; the arrow keys, `Home` and `End` choose a page there, and a click or
 * tap chooses the page under it. The map is shown until the operator chooses otherwise, and again
 * once another universe is opened. Both pages stay mounted, the one not chosen hidden, so that
 * neither asks the server again on its return.
 */
export function GalaxyPages({ hidden, cursorLy, onCursor, centreLy }: GalaxyPagesProps) {
  const baseId = useId();
  const { open } = useUniverse();
  const openId = open?.id ?? null;
  const [choice, setChoice] = useState<PageChoice | null>(null);
  const page = choice !== null && choice.openId === openId ? choice.page : "map";
  const tabId = (each: Page): string => `${baseId}-tab-${each}`;
  const panelId = (each: Page): string => `${baseId}-page-${each}`;

  const choose = (next: Page): void => {
    setChoice({ openId, page: next });
  };
  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>): void => {
    const next = pageAfterKey(event.key, page);
    if (next === null) {
      return;
    }
    event.preventDefault();
    choose(next);
    // The focus follows the choice, as it does in a tab list.
    document.getElementById(tabId(next))?.focus();
  };

  return (
    <div className="panel galaxy__pages galaxy-pages" hidden={hidden}>
      <div className="galaxy-pages__tabs" role="tablist" aria-label="Galaxy pages">
        {PAGES.map((each) => (
          <button
            key={each}
            id={tabId(each)}
            type="button"
            role="tab"
            className="galaxy-pages__tab"
            aria-selected={each === page}
            aria-controls={panelId(each)}
            // One stop in the tab order: the chosen page's tab.
            tabIndex={each === page ? 0 : -1}
            onClick={() => {
              choose(each);
            }}
            onKeyDown={onKeyDown}
          >
            {PAGE_TITLE[each]}
          </button>
        ))}
      </div>
      <div
        id={panelId("parameters")}
        role="tabpanel"
        aria-labelledby={tabId("parameters")}
        className="galaxy-pages__page galaxy-pages__page--parameters"
        hidden={page !== "parameters"}
      >
        <ParametersPanel />
      </div>
      <div
        id={panelId("map")}
        role="tabpanel"
        aria-labelledby={tabId("map")}
        className="galaxy-pages__page galaxy-pages__page--map"
        hidden={page !== "map"}
      >
        <GalaxyMapPanel cursorLy={cursorLy} onCursor={onCursor} centreLy={centreLy} />
      </div>
    </div>
  );
}
