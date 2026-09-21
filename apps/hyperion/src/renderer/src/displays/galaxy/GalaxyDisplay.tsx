import { memo, useId } from "react";

import { useUniverse } from "../../lib/universe";
import { ParametersPanel } from "./ParametersPanel";
import { UniversePanel } from "./UniversePanel";

interface DataPanelProps {
  readonly title: string;
  /** The panel's area in the display's grid. */
  readonly className: string;
  readonly universeOpen: boolean;
}

/**
 * A data panel whose content is built by a later task: the galaxy map (P05.T8) or the local chart
 * (P05.T11).
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

function GalaxyPanels() {
  const { open } = useUniverse();
  return (
    <div className="galaxy">
      <UniversePanel />
      <ParametersPanel />
      <DataPanel title="Galaxy Map" className="galaxy__map" universeOpen={open !== null} />
      <DataPanel title="Local Chart" className="galaxy__chart" universeOpen={open !== null} />
    </div>
  );
}

/**
 * The `GALAXY` display: universe, parameters, galaxy map and local chart, on a fixed grid that
 * fills the work area without scrolling.
 *
 * @remarks
 * The universe and its parameters share the first column, the galaxy map the second, and the
 * local chart, with its system list and readout, the rest. With no universe open every data panel
 * says so. Memoised: it takes no props and reads the server link and the universe from context,
 * so a latency update, which re-renders `App`, does not re-render it.
 */
export const GalaxyDisplay = memo(GalaxyPanels);
