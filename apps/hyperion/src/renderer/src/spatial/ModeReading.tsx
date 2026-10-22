import { Reading, type SpatialReading } from "./Reading";

/** A display's mode, as its spatial view shows it after the frame, the time and the scale bar. */
export interface SpatialMode {
  /** The mode's name: `NOW` or `OBSERVED FROM`. */
  readonly name: string;
  /** Where the mode observes from, written as the centre is; empty for `NOW`. */
  readonly from: ReadonlyArray<SpatialReading>;
}

/** Props of {@link ModeReading}. */
export interface ModeReadingProps {
  /** The mode the answer on show was given in, never the mode being chosen. */
  readonly mode: SpatialMode;
  /**
   * Whether the answer the mode was given in is stale, passed to the observer's readings: given,
   * each keeps the slot for its `S` (see {@link Reading}); absent, they have none.
   */
  readonly stale?: boolean | undefined;
}

/**
 * The mode of the answer on show, `MODE NOW` or `MODE OBSERVED FROM` and the observer's `RADIUS`,
 * `ANGLE` and `HEIGHT`: one reading, in a spatial view's furniture and on a graph's line that reads
 * the same answer (plan 12, P12.T11 and P12.T11.d).
 *
 * @remarks
 * A term and its definition, for a `dl` of readings, whose observer's readings run on after the
 * mode's name in a `dl` nested in its value. The name is a word, as the frame's is, so it takes no
 * slot and never the stale `S`; the observer's coordinates are figures, each a {@link Reading} given
 * this reading's `stale`.
 */
export function ModeReading({ mode, stale }: ModeReadingProps) {
  return (
    <div className="field spatial-reading spatial-view__mode">
      <dt className="field__label">MODE</dt>{" "}
      <dd>
        {mode.name}
        {mode.from.length === 0 ? null : (
          <dl className="spatial-view__observer">
            {mode.from.map((reading) => (
              <Reading key={reading.label} reading={reading} stale={stale} />
            ))}
          </dl>
        )}
      </dd>
    </div>
  );
}
