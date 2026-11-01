import type { BodyInspection } from "../../view/scenes/inspectionScene";
import { BodiesPanel } from "./BodiesPanel";
import { EventsPanel } from "./EventsPanel";
import { OrbitMapPanel } from "./OrbitMapPanel";
import type { SystemTarget } from "./systemTarget";
import { TimeControl } from "./TimeControl";
import { useSystemView } from "./useSystemView";

/** Props of {@link SystemView}. */
export interface SystemViewProps {
  readonly target: SystemTarget;
  /** Opens a body in `VIEW`, as the orbit map's `VIEW BODY` asks (the owner's request). */
  readonly onViewBody: (inspection: BodyInspection) => void;
}

/**
 * One opening of the `SYSTEM` display, on a fixed layout that fills the work area without
 * scrolling: the orbit map, the display time and the events in the first column, the bodies in the
 * second (plan 14, P14.T41–T44).
 *
 * @remarks
 * The map and the time are used together, the time's steps moving what the map shows, so they share
 * a column, the map taking what the time and the events leave; the events count down from the
 * display time, so they stand under it. The list and the readout are paired with the map, as the
 * chart's are, and stand beside it.
 */
export function SystemView({ target, onViewBody }: SystemViewProps) {
  const view = useSystemView(target);
  return (
    <div className="system">
      <div className="system__column system__column--map">
        <OrbitMapPanel view={view} onViewBody={onViewBody} />
        <TimeControl time={view.displayTime} />
        <EventsPanel view={view} />
      </div>
      <div className="system__column system__column--bodies">
        <BodiesPanel view={view} />
      </div>
    </div>
  );
}
