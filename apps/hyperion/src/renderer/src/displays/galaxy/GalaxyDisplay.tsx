import type { UniverseIdHex } from "@hyperion/protocol";
import { memo, useId, useState } from "react";

import type { CentreLy } from "../../lib/galaxy/model";
import { linkDownReason, useServerLink } from "../../lib/serverLink";
import { useUniverse } from "../../lib/universe";
import { CentreEntry } from "./CentreEntry";
import { GalaxyPages } from "./GalaxyPages";
import { UniversePanel } from "./UniversePanel";

interface DataPanelProps {
  readonly title: string;
  /** The panel's place in the display's layout. */
  readonly className: string;
  readonly universeOpen: boolean;
}

/**
 * A data panel whose content is built by a later task: the local chart (P05.T11).
 *
 * @remarks
 * It shows only what is true: its title, and `NO UNIVERSE OPEN` until a universe is. Nothing is
 * drawn in it until its content exists.
 */
function DataPanel({ title, className, universeOpen }: DataPanelProps) {
  const titleId = useId();
  return (
    <section className={`panel ${className}`} aria-labelledby={titleId}>
      <h2 className="panel__title" id={titleId}>
        {title}
      </h2>
      {universeOpen ? null : <p className="panel__empty">NO UNIVERSE OPEN</p>}
    </section>
  );
}

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

function GalaxyPanels() {
  const { open, createUnconfirmed } = useUniverse();
  const { status } = useServerLink();
  const openId = open?.id ?? null;
  const [universeChoice, setUniverseChoice] = useState<UniverseChoice | null>(null);
  // Picked on the map and entered in CURSOR, which sit side by side.
  const [cursorLy, setCursorLy] = useState<CentreLy>(ORIGIN_LY);
  // Chosen on the map and charted by the local chart, so held by the display both are part of.
  const [chartCentreLy, setChartCentreLy] = useState<CentreLy | null>(null);
  // Whole while no universe is open, and while a create's result is unconfirmed, so that its
  // report is seen; folded once one is open.
  const universeExpanded =
    createUnconfirmed ||
    (universeChoice !== null && universeChoice.openId === openId
      ? universeChoice.expanded
      : openId === null);

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
          cursorLy={cursorLy}
          onCursor={setCursorLy}
          centreLy={chartCentreLy}
        />
      </div>
      <div className="galaxy__column galaxy__column--chart">
        {open === null ? null : (
          <CentreEntry
            cursorLy={cursorLy}
            onCursor={setCursorLy}
            onCentre={setChartCentreLy}
            heldBack={linkDownReason(status)}
          />
        )}
        <DataPanel title="Local Chart" className="galaxy__chart" universeOpen={open !== null} />
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
 * it the `PARAMETERS` and `GALAXY MAP` pages, which share a panel; `UNIVERSE` shown whole takes the
 * column, and the pages are out of view until it folds again. The second holds the `CURSOR` panel,
 * beside the map and after it in reading order, and the local chart. The display holds the map
 * cursor, which the map and `CURSOR` share, and the chart's centre, which `CENTRE CHART` sets and
 * the local chart (P05.T11.g) takes as its `centreLy`, so that both last across a visit to another
 * display (plan 05, design note D1). With no universe open, `UNIVERSE` is whole and the local chart
 * says `NO UNIVERSE OPEN`. Memoised: it takes no props and reads the server link and the universe
 * from context, so a latency update, which re-renders `App`, does not re-render it.
 */
export const GalaxyDisplay = memo(GalaxyPanels);
