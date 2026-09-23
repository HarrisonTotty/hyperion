import { Activity, type KeyboardEvent, useId, useLayoutEffect, useRef } from "react";

import type { CentreLy } from "../../lib/galaxy/model";
import { GalaxyMapPanel } from "./GalaxyMapPanel";
import { LocalChartPanel } from "./LocalChartPanel";
import { PageTabFocus } from "./pageTabFocus";
import { ParametersPanel } from "./ParametersPanel";
import type { LocalChartState } from "./useLocalChart";

/** The pages that share the column, in the order the selector offers them. */
export type GalaxyPage = "parameters" | "map" | "chart";

const GALAXY_PAGES: ReadonlyArray<GalaxyPage> = ["parameters", "map", "chart"];

/** Each page's name: full words, as the guide has panels titled. */
const PAGE_TITLE: Readonly<Record<GalaxyPage, string>> = {
  parameters: "PARAMETERS",
  map: "GALAXY MAP",
  chart: "LOCAL CHART",
};

/** The ID of a page's tab, from the pages' own ID. */
function tabIdOf(baseId: string, page: GalaxyPage): string {
  return `${baseId}-tab-${page}`;
}

/** The ID of a page, from the pages' own ID. */
function pageIdOf(baseId: string, page: GalaxyPage): string {
  return `${baseId}-page-${page}`;
}

/** Moves the focus to a page's tab. */
function focusTab(baseId: string, page: GalaxyPage): void {
  document.getElementById(tabIdOf(baseId, page))?.focus();
}

/** The page an arrow, `Home` or `End` key moves the selection to from `page`, or `null`. */
function pageAfterKey(key: string, page: GalaxyPage): GalaxyPage | null {
  const index = GALAXY_PAGES.indexOf(page);
  let next: number;
  switch (key) {
    case "ArrowLeft":
      next = (index + GALAXY_PAGES.length - 1) % GALAXY_PAGES.length;
      break;
    case "ArrowRight":
      next = (index + 1) % GALAXY_PAGES.length;
      break;
    case "Home":
      next = 0;
      break;
    case "End":
      next = GALAXY_PAGES.length - 1;
      break;
    default:
      return null;
  }
  return GALAXY_PAGES[next] ?? null;
}

interface GalaxyPagesProps {
  /** Whether the pages are out of view, as they are while `UNIVERSE` is shown whole. */
  readonly hidden: boolean;
  /** The page on show, which the display owns, since `CENTRE CHART` chooses the chart. */
  readonly page: GalaxyPage;
  readonly onPage: (page: GalaxyPage) => void;
  /** The map cursor, in the `GALACTIC` frame. */
  readonly cursorLy: CentreLy;
  /** Moves the cursor, after a pick on a map or an arrow key on it. */
  readonly onCursor: (cursorLy: CentreLy) => void;
  /** The chart's centre, marked on both maps, or `null` before one is chosen. */
  readonly centreLy: CentreLy | null;
  /** The local chart, which the display owns with the list and readout beside it. */
  readonly chart: LocalChartState;
}

/**
 * The `PARAMETERS`, `GALAXY MAP` and `LOCAL CHART` pages, sharing one panel behind a page selector,
 * so that the display's two large pictures each have the column's full width and height.
 *
 * @remarks
 * The selector is a tab list and the panel's title: the chosen page's name is marked by a rule
 * beneath it as well as by `aria-selected`, so its state is a shape and not a colour alone. It is
 * one stop in the tab order; the arrow keys, `Home` and `End` choose a page there, and a click or
 * tap chooses the page under it. The map is shown until the operator chooses otherwise, and
 * `CENTRE CHART` shows the chart. Every page stays mounted, the ones not chosen hidden, so that none
 * asks the server again on its return and the chart keeps its camera. A page hidden with the focus
 * in it, as by `C` on a focused map, hands the focus to the tab of the page shown; and each page
 * offers its tab, through {@link PageTabFocus}, to a control of its own that goes as it is pressed.
 */
export function GalaxyPages({
  hidden,
  page,
  onPage,
  cursorLy,
  onCursor,
  centreLy,
  chart,
}: GalaxyPagesProps) {
  const baseId = useId();
  const tabId = (each: GalaxyPage): string => tabIdOf(baseId, each);
  const panelId = (each: GalaxyPage): string => pageIdOf(baseId, each);
  const pagesRef = useRef<HTMLDivElement>(null);

  // A page chosen from elsewhere hides the one the focus may be in, as `C` pressed on a focused map
  // hides the map's page to show the chart's. The focus would stay on a hidden element, which
  // Chromium then takes away and jsdom does not; before paint, it moves to the tab of the page now
  // shown, as it follows a choice made on the tabs. A map's density reading, which announces only
  // while its picture has focus, so hears the picture's `blur` from this move, in any browser,
  // rather than rely on how the browser treats a focused element that is hidden.
  // The same holds for the focus on the tab of the page that was shown, which is no longer the
  // tab list's one stop.
  useLayoutEffect(() => {
    const focused = document.activeElement;
    const within = focused?.closest('[role="tabpanel"], [role="tab"]');
    if (
      within !== null &&
      within !== undefined &&
      pagesRef.current?.contains(within) === true &&
      within.id !== pageIdOf(baseId, page) &&
      within.id !== tabIdOf(baseId, page)
    ) {
      focusTab(baseId, page);
    }
  }, [baseId, page]);

  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>): void => {
    const next = pageAfterKey(event.key, page);
    if (next === null) {
      return;
    }
    event.preventDefault();
    onPage(next);
    // The focus follows the choice, as it does in a tab list.
    focusTab(baseId, next);
  };

  return (
    <div className="panel galaxy__pages galaxy-pages" hidden={hidden} ref={pagesRef}>
      <div className="galaxy-pages__tabs" role="tablist" aria-label="Galaxy pages">
        {GALAXY_PAGES.map((each) => (
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
              onPage(each);
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
        <PageTabFocus
          value={() => {
            focusTab(baseId, "parameters");
          }}
        >
          <ParametersPanel />
        </PageTabFocus>
      </div>
      <div
        id={panelId("map")}
        role="tabpanel"
        aria-labelledby={tabId("map")}
        className="galaxy-pages__page galaxy-pages__page--map"
        hidden={page !== "map"}
      >
        <PageTabFocus
          value={() => {
            focusTab(baseId, "map");
          }}
        >
          <GalaxyMapPanel cursorLy={cursorLy} onCursor={onCursor} centreLy={centreLy} />
        </PageTabFocus>
      </div>
      <div
        id={panelId("chart")}
        role="tabpanel"
        aria-labelledby={tabId("chart")}
        className="galaxy-pages__page galaxy-pages__page--chart"
        hidden={page !== "chart"}
      >
        {/*
         * Under `Activity`, as a hidden display is (design note D1): the chart keeps its camera and
         * its selection, and the spatial view's effects are torn down while the page is not on show,
         * so its single keys (D3) and its frames belong to the page the operator is looking at.
         */}
        <Activity mode={page === "chart" ? "visible" : "hidden"}>
          <PageTabFocus
            value={() => {
              focusTab(baseId, "chart");
            }}
          >
            <LocalChartPanel chart={chart} />
          </PageTabFocus>
        </Activity>
      </div>
    </div>
  );
}
