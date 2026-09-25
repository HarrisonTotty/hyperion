import { SolarMassUnit } from "../../components/SolarMassUnit";
import { StaleMark } from "../../components/StaleMark";
import { StarList } from "../../components/StarList";
import {
  formatAge,
  formatBearingDeg,
  formatLengthLy,
  formatListPosition,
  formatMassMsun,
  formatSigned,
  formatUniverseTimeYr,
  TIME_SYSTEM_LABEL,
} from "../../lib/format";
import type { ChartSystem } from "../../lib/galaxy/model";
import { populationLabel } from "../../lib/galaxy/wire";
import { useItemsInView } from "../../lib/itemsInView";
import type { SystemModel } from "../../lib/system/model";
import { formatHex64 } from "../../lib/seed";
import { AXIS_TOLERANCE_LY, cylindrical, type LocalFrame, toLocal } from "../../spatial/frame";
import { PrimaryReadings } from "./PrimaryReadings";
import { SystemReading as Reading } from "./SystemReading";

/** Whether the selected system is within the drive range, in words; `null` with none selected. */
function rangeWords(system: ChartSystem | null, driveRangeLy: number): string | null {
  if (system === null) {
    return null;
  }
  return system.distanceLy <= driveRangeLy ? "IN RANGE" : "OUT OF RANGE";
}

/** Props of {@link SystemReadout}. */
export interface SystemReadoutProps {
  /** The selected system, or `null` when none is. */
  readonly system: ChartSystem | null;
  /** The named directions at the chart centre, which the offsets are taken along. */
  readonly frame: LocalFrame;
  /** The range the system counts as in (plan 05, design note D9). */
  readonly driveRangeLy: number;
  /** The chart time, in years from the epoch: the time the age is given at. */
  readonly timeYr: number;
  /** Decimals every distance on this chart is written with (plan 05, design note D21). */
  readonly distanceDecimals: number;
  /**
   * The selected system's stars as `system_summary` answered for it, or `null` before the answer,
   * without a selection, and when the answer cannot be used (plan 06, P06.T36).
   */
  readonly stars: SystemModel | null;
  /**
   * Whether `stars` is a stale snapshot: an answer for a time the chart has since left, or kept
   * after the newer request failed; its readings are then muted and trail the guide's `S`.
   */
  readonly starsStale: boolean;
}

/**
 * Everything known about the selected system: what it is called, where it is, how heavy and how
 * old, and, once `system_summary` has answered, what its stars are (plan 05, P05.T10; plan 06,
 * P06.T36; plan 11, P11.T14).
 *
 * @remarks
 * A live region read as a whole, so that a selection made on the chart or in the list is announced
 * once rather than value by value. It is a `div` with `role="status"`, not the `output` that would
 * otherwise be the semantic element for it: an `output` holds phrasing content, and the readout is
 * a `dl` (the orchestrator's ruling 14). `COREWARD`, `SPINWARD` and `NORTH` are the signed offsets
 * along the named directions at the chart centre; `NORTH` is the offset from the reference plane,
 * the only non-visual source of the chart's fill cue, and the legend's `FILLED NORTH OF PLANE`
 * names the same direction. `RADIUS`, `ANGLE` and `HEIGHT` are the system's own place in the
 * `GALACTIC` frame, whose coordinates the guide names in those words; the angle is missing on the
 * galactic axis, where it is undefined. The age is the age at the chart time, which is given beside
 * it, since a query at another time gives another age. Whether the system is within the drive
 * range is in words, never colour alone. With nothing selected every value is an em dash.
 *
 * With the stars' answer, the primary's mass now stands beside its initial mass and the system's
 * metallicity, `[Fe/H]` in `dex`, follows its population, each the em dash until then, so that no
 * row moves as the answer comes; then, under `STAR A`, the primary as
 * {@link PrimaryReadings} reads it, and the star list of every star and orbit ({@link StarList}).
 * A system not yet formed at the chart's time reads `NOT YET FORMED` for its stars. Where the
 * request stands, pending, refused, timed out or cut off, is said beside the readout and not in it,
 * since its `RETRY` is a control (rulings 13 and 14); until the answer the rows it fills are
 * absent, never zero. An answer kept for a time the chart has left, or after a newer request
 * failed, is a stale snapshot: its readings are muted and trail the guide's `S`.
 *
 * The readings outrun the column at 1280 × 720, so they scroll inside their own region with the
 * readings in view and their total under it, as the guide asks of a list and ruling 70.6 allows a
 * readout: each reading and each of the star list's tables counts one. A new selection returns the
 * region to its top. The region takes the focus by `Tab` alone, so that the list keeps it.
 */
export function SystemReadout({
  system,
  frame,
  driveRangeLy,
  timeYr,
  distanceDecimals,
  stars,
  starsStale,
}: SystemReadoutProps) {
  const local = system === null ? null : toLocal(frame, system.relLy);
  const galactic = system === null ? null : cylindrical(system.positionLy);
  const onAxis = galactic !== null && !(galactic.radiusLy > AXIS_TOLERANCE_LY);
  const age = system === null ? null : formatAge(system.ageMyr);
  const atTime = `AT ${TIME_SYSTEM_LABEL} ${formatUniverseTimeYr(timeYr)} yr`;
  const formed = stars !== null && stars.formed ? stars : null;
  const primary = formed?.hosts[0] ?? null;
  const selectedKey = system?.id ?? "none";
  const { ref, range } = useItemsInView(
    "dt, .star-list__table",
    `${selectedKey}:${stars === null ? "none" : `${stars.time.seconds}:${stars.hosts.length}`}`,
    selectedKey,
  );

  return (
    <>
      <section
        className="system-readout__scroll"
        ref={ref}
        aria-label="Readings"
        // A scrolling region with nothing focusable in it must take focus itself to be scrolled
        // from the keyboard (WCAG 2.1.1); the rule allows tabIndex on interactive roles only.
        // oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex
        tabIndex={0}
      >
        <div
          className="system-readout"
          // `output`'s content model is phrasing content, so it cannot hold this `dl`: the rules'
          // "use the semantic element before ARIA" adds a role where no native element fits.
          // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
          role="status"
          aria-label="Selected system"
          // Atomic, so the whole selection is read, not the values that happen to have changed.
          aria-atomic="true"
        >
          <dl className="readout system-readout__values">
            <Reading label="DESIG" value={system?.designation ?? null} wide />
            <Reading label="ID" value={system === null ? null : formatHex64(system.id)} wide />
            <Reading
              label="DIST"
              value={system === null ? null : formatLengthLy(system.distanceLy, distanceDecimals)}
              unit="ly"
            />
            {/*
             * `DRIVE RANGE`, the setting's one name, and no `SET`: the reading draws a within or
             * beyond word, not the setting's value, and `SET` marks a drawn value (the
             * orchestrator's ruling 15, and design note D9).
             */}
            <Reading label="DRIVE RANGE" value={rangeWords(system, driveRangeLy)} />
            <Reading
              label="NORTH"
              value={local === null ? null : formatSigned(local.north, distanceDecimals)}
              unit="ly"
            />
            <Reading
              label="COREWARD"
              value={local === null ? null : formatSigned(local.coreward, distanceDecimals)}
              unit="ly"
            />
            <Reading
              label="SPINWARD"
              value={local === null ? null : formatSigned(local.spinward, distanceDecimals)}
              unit="ly"
            />
            <Reading
              label="RADIUS"
              value={galactic === null ? null : formatLengthLy(galactic.radiusLy, 1)}
              unit="ly"
            />
            <Reading
              label="ANGLE"
              value={galactic === null || onAxis ? null : formatBearingDeg(galactic.angleDeg, 1)}
            />
            <Reading
              label="HEIGHT"
              value={system === null ? null : formatSigned(system.positionLy.z, 1)}
              unit="ly"
            />
            <Reading
              label="INIT MASS"
              value={system === null ? null : formatMassMsun(system.initialMassMsun)}
              unit={<SolarMassUnit />}
            />
            {/* Always there, the em dash until the stars' answer, so that no row moves when it comes. */}
            <Reading
              label="MASS"
              value={
                primary === null || primary.kind === "no_remnant"
                  ? null
                  : formatMassMsun(primary.massMsun)
              }
              unit={<SolarMassUnit />}
              stale={starsStale}
            />
            <Reading label="AGE" value={age?.value ?? null} unit={age?.unit} beside={atTime} />
            <Reading
              label="POPULATION"
              value={system === null ? null : populationLabel(system.population)}
              wide
            />
            <Reading
              label="[Fe/H]"
              symbol
              value={stars === null ? null : formatSigned(stars.feHDex, 2)}
              unit="dex"
              stale={starsStale}
            />
            {stars === null || formed !== null ? null : (
              <Reading label="STARS" value="NOT YET FORMED" wide stale={starsStale} />
            )}
          </dl>
          {primary === null || formed === null ? null : (
            <>
              <h3 className="system-readout__heading">STAR A{starsStale ? <StaleMark /> : null}</h3>
              <dl className="readout system-readout__values">
                <PrimaryReadings host={primary} at={formed.time} stale={starsStale} />
              </dl>
              <StarList model={formed} stale={starsStale} />
            </>
          )}
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
