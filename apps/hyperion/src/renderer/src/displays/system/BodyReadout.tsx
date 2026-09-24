import { BodyRecordReadings } from "./BodyRecordReadings";
import { HostReadings } from "./HostReadings";
import { MISSING, ReadoutRow } from "./ReadoutRow";
import type { Selected } from "./useSystemView";

/** Props of {@link BodyReadout}. */
export interface BodyReadoutProps {
  /** What is selected, or `null` when nothing is. */
  readonly selected: Selected | null;
}

/**
 * Everything the server says of the selected body: for a host, its star and its zones; for a body,
 * its record, every section from its tag (plan 14, P14.T43.b).
 *
 * @remarks
 * A live region read as a whole, so that a selection made on the map or in the list is announced
 * once. It is a `div` with `role="status"`, not an `output`, since it holds a `dl` (the
 * orchestrator's ruling 14), and it holds no control (ruling 13): the state of the record's own
 * request stands outside it. Each body's readings are keyed by its ID, so that a new selection
 * starts afresh. With nothing selected the designation is an em dash.
 */
export function BodyReadout({ selected }: BodyReadoutProps) {
  let readings;
  if (selected === null) {
    readings = <ReadoutRow label="DESIG" shown={MISSING} wide />;
  } else if (selected.kind === "host") {
    readings = <HostReadings host={selected.host} zones={selected.zones} />;
  } else {
    readings = (
      <BodyRecordReadings
        key={selected.body.id}
        body={selected.body}
        granted={selected.granted}
        parentName={selected.parentName}
        distanceM={selected.distanceM}
        inclinationRad={selected.inclinationRad}
      />
    );
  }
  return (
    // `output`'s content model is phrasing content, so it cannot hold this `dl`: a role is added
    // where no native element fits. Atomic, so that the whole selection is read.
    // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
    <div className="body-readout" role="status" aria-label="Selected body" aria-atomic="true">
      <dl className="readout body-readout__values">{readings}</dl>
    </div>
  );
}
