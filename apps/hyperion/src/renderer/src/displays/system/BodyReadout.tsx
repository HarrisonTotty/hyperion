import { formatListPosition } from "../../lib/format";
import { useItemsInView } from "../../lib/itemsInView";
import { BodyRecordReadings } from "./BodyRecordReadings";
import { HostReadings } from "./HostReadings";
import { MISSING, ReadoutRow } from "./ReadoutRow";
import type { Selected } from "./useSystemView";

/** Props of {@link BodyReadout}. */
export interface BodyReadoutProps {
  /** What is selected, or `null` when nothing is. */
  readonly selected: Selected | null;
}

/** The identity of what is selected: a new one starts the readout afresh, at its top. */
function selectedId(selected: Selected | null): string {
  if (selected === null) {
    return "none";
  }
  return selected.kind === "host" ? selected.host.id : selected.body.id;
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
 *
 * A planet's record runs to about sixteen lines, more than the bodies' column holds at 1280 × 720,
 * so the readings scroll inside their own region, with the readings in view and their total under
 * it, as the guide asks of a list (the orchestrator's ruling 65). The region takes the focus from
 * the keyboard only by `Tab`, so that selecting in the list leaves the focus on the list; a new
 * selection returns the region to its top. The live region stays mounted through every selection,
 * since one that is replaced may not be announced.
 */
export function BodyReadout({ selected }: BodyReadoutProps) {
  const id = selectedId(selected);
  const contentKey =
    selected === null
      ? id
      : selected.kind === "host"
        ? `${id}:${selected.zones.length}`
        : `${id}:${String(selected.whole)}:${selected.granted}`;
  const { ref, range } = useItemsInView("dt", contentKey, id);

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
    <>
      <section
        className="body-readout__scroll"
        ref={ref}
        aria-label="Readings"
        // A scrolling region with nothing focusable in it must take focus itself to be scrolled
        // from the keyboard (WCAG 2.1.1); the rule allows tabIndex on interactive roles only.
        // oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex
        tabIndex={0}
      >
        {/* `output`'s content model is phrasing content, so it cannot hold this `dl`: a role is
            added where no native element fits. Atomic, so that the whole selection is read. */}
        {/* oxlint-disable-next-line jsx-a11y/prefer-tag-over-role */}
        <div className="body-readout" role="status" aria-label="Selected body" aria-atomic="true">
          <dl className="readout body-readout__values">{readings}</dl>
        </div>
      </section>
      {range === null ? null : (
        <p className="list-position">
          {formatListPosition(range.firstVisible, range.lastVisible, range.total)}
        </p>
      )}
    </>
  );
}
