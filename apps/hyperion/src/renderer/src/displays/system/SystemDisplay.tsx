import { memo, useId } from "react";

import { useUniverse } from "../../lib/universe";
import type { SystemOpening } from "./systemTarget";
import { SystemView } from "./SystemView";

/** Props of {@link SystemDisplay}. */
interface SystemDisplayProps {
  /** The latest opening, from `OPEN SYSTEM` on the `GALAXY` display; `null` before the first. */
  readonly opening: SystemOpening | null;
}

function SystemPanels({ opening }: SystemDisplayProps) {
  const titleId = useId();
  const { open } = useUniverse();
  // A system of a universe no longer open is not the display's to show.
  const target =
    opening !== null && open !== null && opening.target.universe === open.id ? opening : null;
  if (target === null) {
    return (
      <div className="system system--empty">
        <section className="panel system__map" aria-labelledby={titleId}>
          <h2 className="panel__title" id={titleId}>
            Orbit map
          </h2>
          <p className="panel__empty">
            NO SYSTEM SELECTED: select one on GALAXY and press OPEN SYSTEM
          </p>
        </section>
      </div>
    );
  }
  return <SystemView key={target.sequence} target={target.target} />;
}

/**
 * The `SYSTEM` display: one system, its hosts and bodies, the orbit map, the body list and readout,
 * and the display time (plan 14, phase I; the owner's draft of the guide's nomenclature).
 *
 * @remarks
 * It shows the system `OPEN SYSTEM` last opened on the `GALAXY` display, at the chart's time; each
 * opening is keyed apart, so that opening a system again starts it afresh, held at the chart's time.
 * With none opened, or once another universe is open, it reads `NO SYSTEM SELECTED` and says how to
 * choose one. Memoised: its prop is the opening, which `App` keeps in state, and it reads the server
 * link and the universe from context, so a latency update does not re-render it.
 */
export const SystemDisplay = memo(SystemPanels);
