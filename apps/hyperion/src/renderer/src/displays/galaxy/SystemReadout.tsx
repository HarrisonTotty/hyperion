import type { UniverseTime } from "@hyperion/protocol";
import { useState } from "react";

import { EarthMassUnit } from "../../components/EarthMassUnit";
import { SolarMassUnit } from "../../components/SolarMassUnit";
import { StaleMark } from "../../components/StaleMark";
import { StarList } from "../../components/StarList";
import {
  formatAge,
  formatBearingDeg,
  formatLengthLy,
  type FormattedSubstellarMass,
  formatListPosition,
  formatNumber,
  formatSci,
  formatSigned,
  formatSubstellarMass,
  formatUniverseTimeYr,
  TIME_SYSTEM_LABEL,
} from "../../lib/format";
import { type ChartSystem, isFormedPlanet, observedAgeMyr } from "../../lib/galaxy/model";
import {
  type BoundUnit,
  EMITTED_WIDTH_CH,
  formatCurvatureBound,
  formatCurvatureBoundArcsec,
  formatLightAgeYr,
  LIGHT_AGE_WIDTH_CH,
} from "../../lib/galaxy/observation";
import { populationLabel } from "../../lib/galaxy/wire";
import { useItemsInView } from "../../lib/itemsInView";
import type { SystemModel } from "../../lib/system/model";
import { formatHex64 } from "../../lib/seed";
import {
  AXIS_TOLERANCE_LY,
  cylindrical,
  type LocalFrame,
  localFrameAt,
  toLocal,
} from "../../geometry/frame";
import { PrimaryReadings } from "./PrimaryReadings";
import type { LineExtinction } from "./useExtinction";
import { SystemReading as Reading } from "./SystemReading";

/** Whether the selected system is within the drive range, in words; `null` with none selected. */
function rangeWords(system: ChartSystem | null, driveRangeLy: number): string | null {
  if (system === null) {
    return null;
  }
  return system.distanceLy <= driveRangeLy ? "IN RANGE" : "OUT OF RANGE";
}

/** The drawn unit of a mass as its kind is read in: `M☉`, or `M⊕` for a planet. */
function massUnit(mass: FormattedSubstellarMass) {
  return mass.unit === "mearth" ? <EarthMassUnit /> : <SolarMassUnit />;
}

/** Decimals of a speed in km/s, as the guide writes one (`12.4 km/s`). */
const SPEED_DECIMALS = 1;

/** Decimals of an extinction or a reddening in magnitudes (plan 07, P07.T11.c). */
const MAGNITUDE_DECIMALS = 2;

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
  /**
   * The time the stars describe, from which a death is counted: when their light left, for an
   * answer asked in observed mode (plan 12, P12.T6.b); the answer's own time when absent or `null`.
   */
  readonly starsAt?: UniverseTime | null | undefined;
  /**
   * What lies between the chart's centre and the selected system at the chart's time, or `null`
   * before the answer, without a selection, for a system the server names no such system and after
   * a failed request (plan 07, P07.T11.c); where the request stands is said beside the readout.
   */
  readonly extinction: LineExtinction | null;
}

/** The state of a readout of a system as its light shows it (plan 12, P12.T11). */
export const AS_OBSERVED = "AS OBSERVED";

/** The label of the stated curvature bound on where an observed system appears (plan 12, P12.T11). */
export const CURVATURE_BOUND_LABEL = "CURVATURE BOUND";

/** The label of the time the light an observed system is read by left it, in universe time. */
export const EMITTED_LABEL = `EMITTED ${TIME_SYSTEM_LABEL}`;

interface CurvatureBoundReadingProps {
  readonly boundLy: number;
  readonly boundArcsec: number;
}

/**
 * The stated bound on an observed system's apparent position (plan 12, Design note 3), one reading
 * of two parts joined by a middle dot, as the guide joins a reading's parts: the bound in the
 * distance ladder, its unit held from one reading to the next with the distance units' hysteresis,
 * adjusted during render as the habitable zones' rows are, and the angle it subtends at the
 * observer, its `″` set against the digits as `°` is (`0.632 AU · 0.00150″`). Both are values, in
 * `--text`. A bound, not a `±`, which the guide keeps for a standard uncertainty: a reading of its
 * own after the position's, on a row of its own.
 */
function CurvatureBoundReading({ boundLy, boundArcsec }: CurvatureBoundReadingProps) {
  const [held, setHeld] = useState<BoundUnit | null>(
    () => formatCurvatureBound(boundLy, null).unit,
  );
  const bound = formatCurvatureBound(boundLy, held);
  if (bound.unit !== held) {
    setHeld(bound.unit);
  }
  return (
    <>
      <dt>{CURVATURE_BOUND_LABEL}</dt>
      <dd className="system-readout__wide">
        <span className="system-readout__value">{bound.value}</span>{" "}
        <span className="system-readout__unit">{bound.unit}</span>
        {" · "}
        <span className="system-readout__value">{formatCurvatureBoundArcsec(boundArcsec)}″</span>
      </dd>
    </>
  );
}

/** A reading of the present that the readout states as Estimated, `~`, for a system observed. */
function estimatedIf(estimated: boolean, text: string | null): string | null {
  return text === null || !estimated ? text : `~${text}`;
}

interface AtTimeProps {
  /** The universe time the age is given at, in years from the epoch. */
  readonly timeYr: number;
  /** Whether the age is read as observed, in the readout's wide slot. */
  readonly observed: boolean;
}

/**
 * The universe time an age is given at, after a middle dot, as a reading's parts are joined (the
 * guide's `AT`): `· AT UT +0.00 yr`, the chart time, or the emitted time for a system read as
 * observed (plan 12, P12.T11.c).
 *
 * @remarks
 * Its figures are a value's, in `--text` and untracked, its words and unit muted beside them. The
 * time keeps its system's label and its unit on one line, `UT -20,012.75 yr`, so that a narrow
 * column breaks the reading after `AT` rather than run it past its edge. Read as observed, the age
 * takes the readout's row and the time stands right-aligned in {@link EMITTED_WIDTH_CH}'s slot,
 * which holds any emitted time to the source horizon's, `-229,000.00` among them, so that the
 * reading keeps one width and stands on one line wherever the row holds it (§ Numbers: a field has
 * a fixed width sized for its longest value).
 */
function AtTime({ timeYr, observed }: AtTimeProps) {
  const time = formatUniverseTimeYr(timeYr);
  return (
    <>
      · AT{" "}
      <span className="system-readout__whole">
        {TIME_SYSTEM_LABEL}{" "}
        {observed ? (
          <span
            className="system-readout__value system-readout__value--fixed"
            style={{ minWidth: `${EMITTED_WIDTH_CH}ch` }}
          >
            {time}
          </span>
        ) : (
          <span className="system-readout__value">{time}</span>
        )}{" "}
        yr
      </span>
    </>
  );
}

/** The age reading: the age then, `NOT YET FORMED` before the system formed, or the age now. */
function ageReading(system: ChartSystem | null): {
  readonly value: string | null;
  readonly unit: string | undefined;
  readonly formed: boolean;
} {
  if (system === null) {
    return { value: null, unit: undefined, formed: true };
  }
  const then = observedAgeMyr(system);
  if (then !== null && then < 0) {
    return { value: "NOT YET FORMED", unit: undefined, formed: false };
  }
  const age = formatAge(then ?? system.ageMyr);
  return { value: age.value, unit: age.unit, formed: true };
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
 * `GALACTIC` frame, whose coordinates the guide names in those words, and stand under that
 * heading, as the guide groups them, after the system's own readings; the angle is missing on the
 * galactic axis, where it is undefined. The age is the age at the chart time, which is given beside
 * it after a middle dot ({@link AtTime}: `AGE 22.7 Myr · AT UT +0.00 yr`), since a query at another
 * time gives another age. Whether the system is within the drive
 * range is in words, never colour alone. With nothing selected every value is an em dash.
 *
 * `VEL` is the system's speed at the epoch, and the next list gives its velocity's `COREWARD`,
 * `SPINWARD` and `NORTH` components, signed, in km/s, along the named directions at the system's
 * own position, where its rotation is spinward (plan 08, P08.T7.b); the system moves at it, and
 * the chart, which asks again when its time changes, shows it moved.
 *
 * Under `FROM CHART CENTRE`, the dust along the line from the chart's centre to the system at
 * the chart's time (plan 07, P07.T11.c): `A(V)`, `E(B−V)` and `A(K)` in `mag` to two decimals and
 * `N(H)`, the hydrogen column, in `/cm²` in E notation, each the em dash until the answer.
 *
 * With the stars' answer, the primary's mass now stands beside its initial mass and the system's
 * metallicity, `[Fe/H]` in `dex`, follows its population, each the em dash until then, so that no
 * row moves as the answer comes; then, under `STAR A`, the primary as
 * {@link PrimaryReadings} reads it, and the star list of every star and orbit ({@link StarList}).
 * A system not yet formed at the chart's time reads `NOT YET FORMED` for its stars.
 *
 * A free-floating brown dwarf reads as a system of one star: its masses in M☉ to three decimals
 * (`0.052`), and, once `system_summary` answers, its kind, class and temperature under `STAR A`. A
 * free-floating planet asks nothing of the server: its mass is in Earth masses, its `KIND` is
 * `PLANET`, its metallicity the one its row carries (P13.T5.d), and `BULK` reads
 * `NOT YET MODELLED`, or `STATE` `NOT YET FORMED` before it forms (ruling 134.2) (plan 13, design notes 12 and 14; P13.T8.d). Where the
 * request stands, pending, refused, timed out or cut off, is said beside the readout and not in it,
 * since its `RETRY` is a control (rulings 13 and 14); until the answer the rows it fills are
 * absent, never zero. An answer kept for a time the chart has left, or after a newer request
 * failed, is a stale snapshot: its readings are muted and trail the guide's `S`.
 *
 * A row the chart was answered in observed mode reads `AS OBSERVED` (plan 12, P12.T11), on a line
 * above the scrolling readings, so that it stays on show however far they are scrolled, and in the
 * live region, so that a selection is announced with it; a row without its observation reads as
 * above whatever was asked. Then the system's `LIGHT AGE`, in
 * `yr`, and the time that light left it, `EMITTED UT`, follow its ID, each in a slot that holds the
 * oldest light of the source horizon; its `AGE` is its age then, an Observed value, given at the
 * emitted time, or `NOT YET FORMED` before it formed (the port ruling, §6.2), on a row of its own,
 * the time in a slot that holds the earliest (P12.T11.c); its present position,
 * `DIST`, the offsets and `RADIUS`, `ANGLE` and `HEIGHT`, is Estimated, `~`, with the stated
 * `CURVATURE BOUND` on where it appears after them, in the distance ladder and, after a middle
 * dot, in `″`; and `VEL`
 * and its components, present state the crew cannot know, are the em dash. Its stars are the
 * summary's asked in the same mode, as they come, and a death is counted from the emitted time.
 * `FROM CHART CENTRE` stays the dust from the chart's centre at the chart's time.
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
  starsAt = null,
  extinction,
}: SystemReadoutProps) {
  // Read as observed only where the row carries its observation, whatever was asked: a server that
  // ignores the mode answers in `now` (the port ruling, §6.2).
  const observation = system?.observed ?? null;
  const observed = observation !== null;
  const local = system === null ? null : toLocal(frame, system.relLy);
  // The velocity is the present's, which the crew cannot know as observed, and the wire carries
  // none at emission yet: its readings are missing in observed mode.
  const moving = system === null || observed ? null : system;
  const velocity =
    moving === null ? null : toLocal(localFrameAt(moving.positionLy), moving.velocityKmS);
  const speed =
    moving === null
      ? null
      : Math.hypot(moving.velocityKmS.x, moving.velocityKmS.y, moving.velocityKmS.z);
  const galactic = system === null ? null : cylindrical(system.positionLy);
  const onAxis = galactic !== null && !(galactic.radiusLy > AXIS_TOLERANCE_LY);
  const age = ageReading(system);
  // The age is given at the time it describes: when the light left, as observed.
  const atTime = <AtTime timeYr={observation?.emittedYr ?? timeYr} observed={observed} />;
  const formed = stars !== null && stars.formed ? stars : null;
  const primary = formed?.hosts[0] ?? null;
  const initialMass =
    system === null ? null : formatSubstellarMass(system.initialMassMsun, system.layer);
  const mass =
    system === null || primary === null || primary.kind === "no_remnant"
      ? null
      : formatSubstellarMass(primary.massMsun, system.layer);
  // A free-floating planet's bulk properties are plan 14's (plan 13, design note 12).
  // `BULK` for a formed planet, `STATE` for one not yet formed (the orchestrator's ruling 134.2).
  let planetRow: { readonly label: string; readonly value: string } | null = null;
  if (system !== null && system.kind === "rogue_planet") {
    planetRow = isFormedPlanet(system)
      ? { label: "BULK", value: "NOT YET MODELLED" }
      : { label: "STATE", value: "NOT YET FORMED" };
  }
  // A free-floating planet's metallicity comes with its row, a star system's with its summary.
  let feHDex: number | null = null;
  if (system?.kind === "rogue_planet") {
    feHDex = system.feHDex;
  } else if (stars !== null) {
    feHDex = stars.feHDex;
  }
  const selectedKey = system?.id ?? "none";
  const { ref, range } = useItemsInView(
    "dt, .star-list__table",
    `${selectedKey}:${stars === null ? "none" : `${stars.time.seconds}:${stars.hosts.length}`}`,
    selectedKey,
  );

  return (
    <>
      {/*
       * Above the scrolling readings, so that the state of every reading stays on show however far
       * they are scrolled (§ Data states: a mode is always shown); the live region says it too.
       */}
      {observed ? (
        <p className="system-readout__state" aria-hidden="true">
          {AS_OBSERVED}
        </p>
      ) : null}
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
          {/* Said with the selection; the line on show stands above the scrolling readings. */}
          {observed ? <p className="visually-hidden">{AS_OBSERVED}</p> : null}
          <dl className="readout system-readout__values">
            <Reading label="DESIG" value={system?.designation ?? null} wide />
            <Reading label="ID" value={system === null ? null : formatHex64(system.id)} wide />
            {observation === null ? null : (
              <>
                <Reading
                  label="LIGHT AGE"
                  value={formatLightAgeYr(observation.lightAgeYr)}
                  unit="yr"
                  widthCh={LIGHT_AGE_WIDTH_CH}
                />
                <Reading
                  label={EMITTED_LABEL}
                  value={formatUniverseTimeYr(observation.emittedYr)}
                  unit="yr"
                  widthCh={EMITTED_WIDTH_CH}
                />
              </>
            )}
            <Reading
              label="DIST"
              value={estimatedIf(
                observed,
                system === null ? null : formatLengthLy(system.distanceLy, distanceDecimals),
              )}
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
              value={estimatedIf(
                observed,
                local === null ? null : formatSigned(local.north, distanceDecimals),
              )}
              unit="ly"
            />
            <Reading
              label="COREWARD"
              value={estimatedIf(
                observed,
                local === null ? null : formatSigned(local.coreward, distanceDecimals),
              )}
              unit="ly"
            />
            <Reading
              label="SPINWARD"
              value={estimatedIf(
                observed,
                local === null ? null : formatSigned(local.spinward, distanceDecimals),
              )}
              unit="ly"
            />
            <Reading
              label="INIT MASS"
              value={initialMass?.value ?? null}
              unit={initialMass === null ? <SolarMassUnit /> : massUnit(initialMass)}
            />
            {/* Always there, the em dash until the stars' answer, so that no row moves when it comes. */}
            <Reading
              label="MASS"
              value={mass?.value ?? null}
              unit={mass === null ? <SolarMassUnit /> : massUnit(mass)}
              stale={starsStale}
            />
            <Reading
              label="AGE"
              value={age.value}
              unit={age.unit}
              beside={atTime}
              // As observed it takes the row, so that its emitted time never wraps it (P12.T11.c).
              wide={observed || !age.formed}
            />
            <Reading
              label="POPULATION"
              value={system === null ? null : populationLabel(system.population)}
              wide
            />
            <Reading
              label="[Fe/H]"
              symbol
              value={feHDex === null ? null : formatSigned(feHDex, 2)}
              unit="dex"
              stale={system?.kind !== "rogue_planet" && starsStale}
            />
            {stars === null || formed !== null ? null : (
              <Reading label="STARS" value="NOT YET FORMED" wide stale={starsStale} />
            )}
            {/*
             * A free-floating planet has no stars and no summary: its kind in words, and its bulk
             * properties are plan 14's, so the readout says they are not modelled rather than leave
             * the space to read as none (plan 13, design note 12).
             */}
            {planetRow === null ? null : <Reading label="KIND" value="PLANET" wide />}
            {planetRow === null ? null : (
              <Reading label={planetRow.label} value={planetRow.value} wide />
            )}
          </dl>
          {/* `VEL` and its components, in km/s beside the offsets' ly, in a list of their own. */}
          <dl className="readout system-readout__values" aria-label="Velocity">
            <Reading
              label="VEL"
              value={speed === null ? null : formatNumber(speed, SPEED_DECIMALS)}
              unit="km/s"
            />
            <Reading
              label="COREWARD"
              value={velocity === null ? null : formatSigned(velocity.coreward, SPEED_DECIMALS)}
              unit="km/s"
            />
            <Reading
              label="SPINWARD"
              value={velocity === null ? null : formatSigned(velocity.spinward, SPEED_DECIMALS)}
              unit="km/s"
            />
            <Reading
              label="NORTH"
              value={velocity === null ? null : formatSigned(velocity.north, SPEED_DECIMALS)}
              unit="km/s"
            />
          </dl>
          {/* The guide's § Voice: a readout groups the three under the heading `GALACTIC`. */}
          <h3 className="system-readout__heading">GALACTIC</h3>
          <dl className="readout system-readout__values">
            <Reading
              label="RADIUS"
              value={estimatedIf(
                observed,
                galactic === null ? null : formatLengthLy(galactic.radiusLy, 1),
              )}
              unit="ly"
            />
            <Reading
              label="ANGLE"
              value={estimatedIf(
                observed,
                galactic === null || onAxis ? null : formatBearingDeg(galactic.angleDeg, 1),
              )}
            />
            <Reading
              label="HEIGHT"
              value={estimatedIf(
                observed,
                system === null ? null : formatSigned(system.positionLy.z, 1),
              )}
              unit="ly"
            />
            {observation === null || system === null ? null : (
              <CurvatureBoundReading
                key={system.id}
                boundLy={observation.curvatureBoundLy}
                boundArcsec={observation.curvatureBoundArcsec}
              />
            )}
          </dl>
          {/*
           * The dust between the chart's centre and the system (plan 07, P07.T11.c): A(V), E(B−V)
           * and A(K) in mag, N(H) in /cm² in E notation, the guide's parenthesised subscripts;
           * each the em dash until the answer, never zero.
           */}
          <h3 className="system-readout__heading">FROM CHART CENTRE</h3>
          <dl className="readout system-readout__values">
            <Reading
              label="A(V)"
              value={
                extinction === null ? null : formatNumber(extinction.aVMag, MAGNITUDE_DECIMALS)
              }
              unit="mag"
            />
            <Reading
              label="E(B−V)"
              value={
                extinction === null ? null : formatNumber(extinction.eBVMag, MAGNITUDE_DECIMALS)
              }
              unit="mag"
            />
            <Reading
              label="A(K)"
              value={
                extinction === null ? null : formatNumber(extinction.aKMag, MAGNITUDE_DECIMALS)
              }
              unit="mag"
            />
            <Reading
              label="N(H)"
              value={extinction === null ? null : formatSci(extinction.hydrogenColumnPerCm2)}
              unit="/cm²"
            />
          </dl>
          {primary === null || formed === null ? null : (
            <>
              <h3 className="system-readout__heading">STAR A{starsStale ? <StaleMark /> : null}</h3>
              <dl className="readout system-readout__values">
                <PrimaryReadings host={primary} at={starsAt ?? formed.time} stale={starsStale} />
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
