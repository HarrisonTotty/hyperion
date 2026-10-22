import type { UniverseTime } from "@hyperion/protocol";
import { useState } from "react";

import { EarthMassUnit } from "../../components/EarthMassUnit";
import { SolarMassUnit } from "../../components/SolarMassUnit";
import { StaleSlot } from "../../components/StaleSlot";
import { StarList } from "../../components/StarList";
import {
  formatAge,
  type FormattedAge,
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
import { TIME_WIDTH_CH } from "./chartReadings";
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
   * Whether the chart's answer, which the selected system's row is read from, is a snapshot the
   * link no longer backs: each value read from the answer is then muted and trails its `S`, the
   * summary's with it, since a summary the link no longer backs is a snapshot too.
   */
  readonly stale: boolean;
  /**
   * Whether `stars` is a stale snapshot of its own: an answer for a time or a mode the chart has
   * since left, or kept after the newer request failed; the summary's values, `MASS`, `[Fe/H]`,
   * the primary's under `STAR A` and the star list's, are then muted and trail the guide's `S`,
   * as they are with `stale`.
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
  /** Whether the answer the bound is read from is stale: each part then trails its `S`. */
  readonly stale: boolean;
}

/**
 * The stated bound on an observed system's apparent position (plan 12, Design note 3), one reading
 * of two parts joined by a middle dot, as the guide joins a reading's parts: the bound in the
 * distance ladder, its unit held from one reading to the next with the distance units' hysteresis,
 * adjusted during render as the habitable zones' rows are, and the angle it subtends at the
 * observer, its `″` set against the digits as `°` is (`0.632 AU · 0.00150″`). Both are values, in
 * `--text`. A bound, not a `±`, which the guide keeps for a standard uncertainty: a reading of its
 * own after the position's, on a row of its own. Each part keeps a slot for its stale `S`, after
 * the bound's unit and after the `″` (plan 05, P05.T14).
 */
function CurvatureBoundReading({ boundLy, boundArcsec, stale }: CurvatureBoundReadingProps) {
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
        <span className={stale ? "stale" : undefined}>
          <span className="system-readout__value">{bound.value}</span>{" "}
          <span className="system-readout__unit">{bound.unit}</span>
          <StaleSlot stale={stale} />
          {" · "}
          <span className="system-readout__value">{formatCurvatureBoundArcsec(boundArcsec)}″</span>
          <StaleSlot stale={stale} />
        </span>
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
  /**
   * Whether the time is the emitted time of a system read as observed, in
   * {@link EMITTED_WIDTH_CH}'s slot; else it is the chart time, in {@link TIME_WIDTH_CH}'s.
   */
  readonly observed: boolean;
  /** Whether the answer the time is read from is stale: the time then trails its `S`. */
  readonly stale: boolean;
}

/**
 * The universe time an age is given at, after a middle dot, as a reading's parts are joined (the
 * guide's `AT`): `· AT UT +0.00 yr`, the chart time, or the emitted time for a system read as
 * observed (plan 12, P12.T11.c).
 *
 * @remarks
 * Its figures are a value's, in `--text` and untracked, its words and unit muted beside them. The
 * time keeps its system's label and its unit on one line, `UT -20,012.75 yr`, so that a narrow
 * column breaks the reading after `AT` rather than run it past its edge. The age takes the
 * readout's row in both modes, and the time stands right-aligned in a slot that holds its longest
 * (§ Numbers: a field has a fixed width sized for its longest value): the chart time in
 * {@link TIME_WIDTH_CH}'s, the furniture's `UT` slot, which holds the clock window's `-1000.00`
 * and `+1000.00`, so that the chart time takes one width wherever it stands; the emitted time, as
 * observed, in {@link EMITTED_WIDTH_CH}'s, which holds any emitted time to the source horizon's,
 * `-229,000.00` among them (P12.T11.c). So the time keeps one width as it changes, and the reading
 * stands on one line wherever the row holds it (plan 12, P12.T11.e). The time is the answer's, so
 * it keeps a slot for its stale `S` after its unit, inside the part kept whole, so that the mark
 * never wraps away from it: `22.7 Myr S · AT UT +0.00 yr S` (plan 05, P05.T14).
 */
function AtTime({ timeYr, observed, stale }: AtTimeProps) {
  const widthCh = observed ? EMITTED_WIDTH_CH : TIME_WIDTH_CH;
  return (
    <>
      · AT{" "}
      <span className="system-readout__whole">
        {TIME_SYSTEM_LABEL}{" "}
        <span
          className="system-readout__value system-readout__value--fixed"
          style={{ minWidth: `${widthCh}ch` }}
        >
          {formatUniverseTimeYr(timeYr)}
        </span>{" "}
        yr
        <StaleSlot stale={stale} />
      </span>
    </>
  );
}

/**
 * What the age reading shows: the em dash with nothing selected, `NOT YET FORMED` before the
 * system formed, a status in the value's place that takes no stale mark, or the age with its unit.
 */
type AgeShown =
  | { readonly kind: "missing" }
  | { readonly kind: "status"; readonly text: string }
  | { readonly kind: "value"; readonly age: FormattedAge };

/** The age reading: the age then, `NOT YET FORMED` before the system formed, or the age now. */
function ageReading(system: ChartSystem | null): AgeShown {
  if (system === null) {
    return { kind: "missing" };
  }
  const then = observedAgeMyr(system);
  if (then !== null && then < 0) {
    return { kind: "status", text: "NOT YET FORMED" };
  }
  return { kind: "value", age: formatAge(then ?? system.ageMyr) };
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
 * time gives another age; it takes a row of its own, the chart time in the furniture's `UT` slot,
 * so that the time keeps one width and the reading one line as the chart time changes (P12.T11.e),
 * and where the readout sets two readings to a row `MASS` stands alone before it. Whether the
 * system is within the drive range is in words, never colour alone. With nothing selected every
 * value is an em dash.
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
 * absent, never zero.
 *
 * A stale value is muted and trails the guide's `S`, one after each value, in a slot each reading
 * keeps while live, so that the mark's coming moves nothing (plan 05, P05.T14): after the unit
 * and before the words beside it, as `AGE 22.7 Myr S · AT UT +0.00 yr S` reads, and after each
 * of `CURVATURE BOUND`'s parts. Words that are values take it as digits do (`IN RANGE`, a kind, a
 * population, `PLANET`, `NONE`), and so do `DESIG` and `ID`; a status in a value's place
 * (`NOT YET FORMED`, `NOT YET MODELLED`) and the em dash keep their slot and take none, a status
 * muted with its source all the same, and the headings and `AS OBSERVED` none. The chart's readings, its row's and `FROM CHART CENTRE`'s, are
 * stale with the chart's answer (`stale`); the summary's, `MASS`, `[Fe/H]` (but a free-floating
 * planet's, which comes with its row), the primary's and the star list's, are stale with it or
 * when the summary is a snapshot of its own (`starsStale`): kept for a time the chart has left, or
 * after a newer request failed.
 *
 * A row the chart was answered in observed mode reads `AS OBSERVED` (plan 12, P12.T11), on a line
 * above the scrolling readings, so that it stays on show however far they are scrolled, and in the
 * live region, so that a selection is announced with it; a row without its observation reads as
 * above whatever was asked. Then the system's `LIGHT AGE`, in
 * `yr`, and the time that light left it, `EMITTED UT`, follow its ID, each in a slot that holds the
 * oldest light of the source horizon; its `AGE` is its age then, an Observed value, given at the
 * emitted time, or `NOT YET FORMED` before it formed (the port ruling, §6.2), on a row of its own
 * as in now, the time in a slot that holds the earliest (P12.T11.c); its present position,
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
  stale,
  starsStale,
  starsAt = null,
  extinction,
}: SystemReadoutProps) {
  // The summary is a snapshot when the chart's answer is, as well as when it lags the chart.
  const summaryStale = stale || starsStale;
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
  let ageValue: string | null;
  let ageUnit: string | undefined;
  switch (age.kind) {
    case "missing":
      ageValue = null;
      ageUnit = undefined;
      break;
    case "status":
      ageValue = age.text;
      ageUnit = undefined;
      break;
    case "value":
      ageValue = age.age.value;
      ageUnit = age.age.unit;
      break;
  }
  // The age is given at the time it describes: when the light left, as observed.
  // The time is the answer's, a value beside the age, so it is marked even beside a status.
  const atTime = (
    <AtTime timeYr={observation?.emittedYr ?? timeYr} observed={observed} stale={stale} />
  );
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
            <Reading label="DESIG" value={system?.designation ?? null} wide stale={stale} />
            <Reading
              label="ID"
              value={system === null ? null : formatHex64(system.id)}
              wide
              stale={stale}
            />
            {observation === null ? null : (
              <>
                <Reading
                  label="LIGHT AGE"
                  value={formatLightAgeYr(observation.lightAgeYr)}
                  unit="yr"
                  widthCh={LIGHT_AGE_WIDTH_CH}
                  stale={stale}
                />
                <Reading
                  label={EMITTED_LABEL}
                  value={formatUniverseTimeYr(observation.emittedYr)}
                  unit="yr"
                  widthCh={EMITTED_WIDTH_CH}
                  stale={stale}
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
              stale={stale}
            />
            {/*
             * `DRIVE RANGE`, the setting's one name, and no `SET`: the reading draws a within or
             * beyond word, not the setting's value, and `SET` marks a drawn value (the
             * orchestrator's ruling 15, and design note D9).
             */}
            {/* A word read from the answer's distance, so a value, and marked as one. */}
            <Reading label="DRIVE RANGE" value={rangeWords(system, driveRangeLy)} stale={stale} />
            <Reading
              label="NORTH"
              value={estimatedIf(
                observed,
                local === null ? null : formatSigned(local.north, distanceDecimals),
              )}
              unit="ly"
              stale={stale}
            />
            <Reading
              label="COREWARD"
              value={estimatedIf(
                observed,
                local === null ? null : formatSigned(local.coreward, distanceDecimals),
              )}
              unit="ly"
              stale={stale}
            />
            <Reading
              label="SPINWARD"
              value={estimatedIf(
                observed,
                local === null ? null : formatSigned(local.spinward, distanceDecimals),
              )}
              unit="ly"
              stale={stale}
            />
            <Reading
              label="INIT MASS"
              value={initialMass?.value ?? null}
              unit={initialMass === null ? <SolarMassUnit /> : massUnit(initialMass)}
              stale={stale}
            />
            {/* Always there, the em dash until the stars' answer, so that no row moves when it comes. */}
            <Reading
              label="MASS"
              value={mass?.value ?? null}
              unit={mass === null ? <SolarMassUnit /> : massUnit(mass)}
              stale={summaryStale}
            />
            <Reading
              label="AGE"
              value={ageValue}
              unit={ageUnit}
              beside={atTime}
              // It takes the row in both modes, so that its time in its slot never wraps it
              // (P12.T11.c as observed; P12.T11.e in now).
              wide
              // `NOT YET FORMED` is a status, which keeps its slot and takes no mark.
              stale={stale}
              status={age.kind === "status"}
            />
            <Reading
              label="POPULATION"
              value={system === null ? null : populationLabel(system.population)}
              wide
              stale={stale}
            />
            <Reading
              label="[Fe/H]"
              symbol
              value={feHDex === null ? null : formatSigned(feHDex, 2)}
              unit="dex"
              // A free-floating planet's comes with its row; a star system's with its summary.
              stale={system?.kind === "rogue_planet" ? stale : summaryStale}
            />
            {/* A status in a value's place: its slot kept, no mark, and muted with the summary. */}
            {stars === null || formed !== null ? null : (
              <Reading label="STARS" value="NOT YET FORMED" wide stale={summaryStale} status />
            )}
            {/*
             * A free-floating planet has no stars and no summary: its kind in words, and its bulk
             * properties are plan 14's, so the readout says they are not modelled rather than leave
             * the space to read as none (plan 13, design note 12).
             */}
            {planetRow === null ? null : <Reading label="KIND" value="PLANET" wide stale={stale} />}
            {/* `NOT YET MODELLED` and `NOT YET FORMED` are statuses: their slots kept, no mark. */}
            {planetRow === null ? null : (
              <Reading label={planetRow.label} value={planetRow.value} wide stale={stale} status />
            )}
          </dl>
          {/* `VEL` and its components, in km/s beside the offsets' ly, in a list of their own. */}
          <dl className="readout system-readout__values" aria-label="Velocity">
            <Reading
              label="VEL"
              value={speed === null ? null : formatNumber(speed, SPEED_DECIMALS)}
              unit="km/s"
              stale={stale}
            />
            <Reading
              label="COREWARD"
              value={velocity === null ? null : formatSigned(velocity.coreward, SPEED_DECIMALS)}
              unit="km/s"
              stale={stale}
            />
            <Reading
              label="SPINWARD"
              value={velocity === null ? null : formatSigned(velocity.spinward, SPEED_DECIMALS)}
              unit="km/s"
              stale={stale}
            />
            <Reading
              label="NORTH"
              value={velocity === null ? null : formatSigned(velocity.north, SPEED_DECIMALS)}
              unit="km/s"
              stale={stale}
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
              stale={stale}
            />
            <Reading
              label="ANGLE"
              value={estimatedIf(
                observed,
                galactic === null || onAxis ? null : formatBearingDeg(galactic.angleDeg, 1),
              )}
              stale={stale}
            />
            <Reading
              label="HEIGHT"
              value={estimatedIf(
                observed,
                system === null ? null : formatSigned(system.positionLy.z, 1),
              )}
              unit="ly"
              stale={stale}
            />
            {observation === null || system === null ? null : (
              <CurvatureBoundReading
                key={system.id}
                boundLy={observation.curvatureBoundLy}
                boundArcsec={observation.curvatureBoundArcsec}
                stale={stale}
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
              stale={stale}
            />
            <Reading
              label="E(B−V)"
              value={
                extinction === null ? null : formatNumber(extinction.eBVMag, MAGNITUDE_DECIMALS)
              }
              unit="mag"
              stale={stale}
            />
            <Reading
              label="A(K)"
              value={
                extinction === null ? null : formatNumber(extinction.aKMag, MAGNITUDE_DECIMALS)
              }
              unit="mag"
              stale={stale}
            />
            <Reading
              label="N(H)"
              value={extinction === null ? null : formatSci(extinction.hydrogenColumnPerCm2)}
              unit="/cm²"
              stale={stale}
            />
          </dl>
          {primary === null || formed === null ? null : (
            <>
              {/* A heading, not a value: its readings carry their own marks. */}
              <h3 className="system-readout__heading">STAR A</h3>
              <dl className="readout system-readout__values">
                <PrimaryReadings host={primary} at={starsAt ?? formed.time} stale={summaryStale} />
              </dl>
              <StarList model={formed} stale={summaryStale} />
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
