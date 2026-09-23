import type { ReactNode } from "react";

import { SolarMassUnit } from "../../components/SolarMassUnit";
import {
  formatAge,
  formatBearingDeg,
  formatLengthLy,
  formatMassMsun,
  formatSigned,
  formatUniverseTimeYr,
  TIME_SYSTEM_LABEL,
} from "../../lib/format";
import type { ChartSystem } from "../../lib/galaxy/model";
import { populationLabel } from "../../lib/galaxy/wire";
import { formatHex64 } from "../../lib/seed";
import { AXIS_TOLERANCE_LY, cylindrical, type LocalFrame, toLocal } from "../../spatial/frame";

interface ReadingProps {
  readonly label: string;
  /** The value's digits, or `null` for one that does not exist, shown as an em dash. */
  readonly value: string | null;
  /** The unit after the value: `ly`, `°`, or an element such as the drawn `M☉`. */
  readonly unit?: ReactNode;
  /** Whether the value takes the rest of the row, as a designation and an ID do. */
  readonly wide?: boolean;
  /** What the value is qualified by, such as the time an age is given at. */
  readonly beside?: ReactNode;
}

function Reading({ label, value, unit, wide = false, beside }: ReadingProps) {
  return (
    <>
      <dt>{label}</dt>
      <dd className={wide ? "system-readout__wide" : undefined}>
        {value === null ? (
          <span className="readout__missing">—</span>
        ) : (
          <>
            <span className="system-readout__value">{value}</span>
            {unit === undefined ? null : (
              <>
                {/* The degree sign is written against the digits; other units after a space. */}
                {unit === "°" ? null : " "}
                <span className="system-readout__unit">{unit}</span>
              </>
            )}
            {beside === undefined ? null : <span className="system-readout__beside">{beside}</span>}
          </>
        )}
      </dd>
    </>
  );
}

/** Whether the selected system is within the drive range, in words; `null` with none selected. */
function rangeWords(system: ChartSystem | null, driveRangeLy: number): string | null {
  if (system === null) {
    return null;
  }
  return system.distanceLy <= driveRangeLy ? "IN RANGE" : "OUT OF RANGE";
}

interface SystemReadoutProps {
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
}

/**
 * Everything known about the selected system: what it is called, where it is, how heavy and how
 * old.
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
 */
export function SystemReadout({
  system,
  frame,
  driveRangeLy,
  timeYr,
  distanceDecimals,
}: SystemReadoutProps) {
  const local = system === null ? null : toLocal(frame, system.relLy);
  const galactic = system === null ? null : cylindrical(system.positionLy);
  const onAxis = galactic !== null && !(galactic.radiusLy > AXIS_TOLERANCE_LY);
  const age = system === null ? null : formatAge(system.ageMyr);
  const atTime = `AT ${TIME_SYSTEM_LABEL} ${formatUniverseTimeYr(timeYr)} yr`;

  return (
    // `output`'s content model is phrasing content, so it cannot hold this `dl`: the rules' "use
    // the semantic element before ARIA" adds a role where no native element fits, and here none
    // does. Atomic, so the whole selection is read, not the values that happen to have changed.
    // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
    <div className="system-readout" role="status" aria-label="Selected system" aria-atomic="true">
      <dl className="readout system-readout__values">
        <Reading label="DESIG" value={system?.designation ?? null} wide />
        <Reading label="ID" value={system === null ? null : formatHex64(system.id)} wide />
        <Reading
          label="DIST"
          value={system === null ? null : formatLengthLy(system.distanceLy, distanceDecimals)}
          unit="ly"
        />
        {/*
         * `DRIVE RANGE`, the setting's one name, and no `SET`: the reading draws a within or beyond
         * word, not the setting's value, and `SET` marks a drawn value (the orchestrator's ruling
         * 15, and design note D9).
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
        <Reading label="AGE" value={age?.value ?? null} unit={age?.unit} beside={atTime} />
        <Reading
          label="POPULATION"
          value={system === null ? null : populationLabel(system.population)}
          wide
        />
      </dl>
    </div>
  );
}
