import type { UniverseIdHex } from "@hyperion/protocol";
import { memo, useState } from "react";

import type { CentreLy } from "../../lib/galaxy/model";
import { linkDownReason, useServerLink } from "../../lib/serverLink";
import { useUniverse } from "../../lib/universe";
import type { SystemTarget } from "../system/systemTarget";
import { CentreEntry } from "./CentreEntry";
import { type GalaxyPage, GalaxyPages } from "./GalaxyPages";
import { SystemsPanel } from "./SystemsPanel";
import { UniversePanel } from "./UniversePanel";
import { useLocalChart } from "./useLocalChart";

/** Where the map cursor starts: the galactic centre. */
const ORIGIN_LY: CentreLy = [0, 0, 0];

/**
 * The operator's choice to show `UNIVERSE` whole or folded, and the open universe it was made
 * for: a choice lasts until another universe is opened.
 */
interface UniverseChoice {
  readonly openId: UniverseIdHex | null;
  readonly expanded: boolean;
}

/** The page on show and the open universe it was chosen for: a choice lasts until another opens. */
interface PageChoice {
  readonly openId: UniverseIdHex | null;
  readonly page: GalaxyPage;
}

/** Props of {@link GalaxyDisplay}. */
interface GalaxyDisplayProps {
  /**
   * Opens the `SYSTEM` display on a system of the chart; stable across renders, so that the
   * memoised display is not rendered again by `App`.
   */
  readonly onOpenSystem: (target: SystemTarget) => void;
}

function GalaxyPanels({ onOpenSystem }: GalaxyDisplayProps) {
  const { open, createUnconfirmed } = useUniverse();
  const { status } = useServerLink();
  const openId = open?.id ?? null;
  const [universeChoice, setUniverseChoice] = useState<UniverseChoice | null>(null);
  const [pageChoice, setPageChoice] = useState<PageChoice | null>(null);
  // Picked on the map and entered in CURSOR, which sit side by side.
  const [cursorLy, setCursorLy] = useState<CentreLy>(ORIGIN_LY);
  // Chosen on the map and charted by the local chart, so held by the display both are part of.
  const [chartCentreLy, setChartCentreLy] = useState<CentreLy | null>(null);
  const chart = useLocalChart(chartCentreLy);
  // Whole while no universe is open, and while a create's result is unconfirmed, so that its
  // report is seen; folded once one is open.
  const universeExpanded =
    createUnconfirmed ||
    (universeChoice !== null && universeChoice.openId === openId
      ? universeChoice.expanded
      : openId === null);
  const page = pageChoice !== null && pageChoice.openId === openId ? pageChoice.page : "map";

  return (
    <div className="galaxy">
      <div className="galaxy__column galaxy__column--universe">
        <UniversePanel
          expanded={universeExpanded}
          onToggle={() => {
            setUniverseChoice({ openId, expanded: !universeExpanded });
          }}
        />
        <GalaxyPages
          hidden={universeExpanded}
          page={page}
          onPage={(next) => {
            setPageChoice({ openId, page: next });
          }}
          cursorLy={cursorLy}
          onCursor={setCursorLy}
          centreLy={chartCentreLy}
          chart={chart}
        />
      </div>
      <div className="galaxy__column galaxy__column--chart">
        {open === null ? null : (
          <CentreEntry
            cursorLy={cursorLy}
            onCursor={setCursorLy}
            onCentre={(centreLy) => {
              setChartCentreLy(centreLy);
              // The chart is what CENTRE CHART acts on, so it is what the column then shows.
              setPageChoice({ openId, page: "chart" });
            }}
            heldBack={linkDownReason(status)}
          />
        )}
        {open === null ? null : <SystemsPanel chart={chart} onOpenSystem={onOpenSystem} />}
      </div>
    </div>
  );
}

/**
 * The `GALAXY` display: universe, parameters and galaxy map, the cursor, and the local chart, on a
 * fixed layout that fills the work area without scrolling.
 *
 * @remarks
 * Two columns. The first holds `UNIVERSE`, folded to one line once a universe is open, and below
 * it the `PARAMETERS`, `GALAXY MAP` and `LOCAL CHART` pages, which share a panel; `UNIVERSE` shown
 * whole takes the column, and the pages are out of view until it folds again. The second holds the
 * `CURSOR` panel, beside the map and after it in reading order, and `SYSTEMS`, the list and readout
 * the chart is paired with. The display holds the map cursor, which the map and `CURSOR` share, the
 * chart's centre, which `CENTRE CHART` sets, the page on show, which `CENTRE CHART` turns to the
 * chart, and, through `useLocalChart`, the chart itself, since its picture and its list stand in
 * different columns. All of it lasts across a visit to another display (plan 05, design note D1).
 * With no universe open, `UNIVERSE` is whole and says `NO UNIVERSE OPEN`, and nothing else is shown.
 * Memoised: its one prop, the way to open the `SYSTEM` display, is stable, and it reads the server
 * link and the universe from context, so a latency update, which re-renders `App`, does not
 * re-render it.
 */
export const GalaxyDisplay = memo(GalaxyPanels);
