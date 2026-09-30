import { BodiesPanel } from "./BodiesPanel";
import { OrbitMapPanel } from "./OrbitMapPanel";
import type { SystemTarget } from "./systemTarget";
import { TimeControl } from "./TimeControl";
import { useSystemView } from "./useSystemView";

/** Props of {@link SystemView}. */
export interface SystemViewProps {
  readonly target: SystemTarget;
}

/**
 * One opening of the `SYSTEM` display, on a fixed layout that fills the work area without
 * scrolling: the orbit map and the display time in the first column, the bodies in the second
 * (plan 14, P14.T41–T44).
 *
 * @remarks
 * The map and the time are used together, the time's steps moving what the map shows, so they share
 * a column, the map taking what the time leaves; the list and the readout are paired with the map,
 * as the chart's are, and stand beside it.
 */
export function SystemView({ target }: SystemViewProps) {
  const view = useSystemView(target);
  return (
    <div className="system">
      <div className="system__column system__column--map">
        <OrbitMapPanel view={view} />
        <TimeControl time={view.displayTime} />
      </div>
      <div className="system__column system__column--bodies">
        <BodiesPanel view={view} />
      </div>
    </div>
  );
}
