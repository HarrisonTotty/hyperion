import type { DetailLevelDto } from "@hyperion/protocol";
import { type ReactNode, useState } from "react";

import { EarthMassUnit } from "../../components/EarthMassUnit";
import {
  type BodyDistanceUnit,
  formatBodyDistance,
  formatGravity,
  formatMassMearth,
  formatNumber,
  formatPeriod,
  formatRadiusKm,
  formatSignificant,
  formatTemperatureK,
  formatUniverseTimeDhms,
  TIME_SYSTEM_LABEL,
} from "../../lib/format";
import {
  bodyKindLabel,
  bodyStateLabel,
  destructionCauseLabel,
  detailLevelLabel,
  moonOriginLabel,
  planetClassLabel,
  sectionStateLabel,
} from "../../lib/system/bodyWords";
import type { BodyRecord, BulkProperties, Section, SystemBody } from "../../lib/system/model";
import { formatBodyIdHex } from "../../lib/seed";
import { ReadoutRow, shown as value } from "./ReadoutRow";

/** Kilometres in a metre. */
const KM_PER_M = 1e-3;

/** Seconds in a day of 86,400 s, the `d` of every period. */
const SECONDS_PER_DAY = 86_400;

/** Decimals an eccentricity and an inclination are read to, and a mass fraction's percentage. */
const ECCENTRICITY_DECIMALS = 4;
const ANGLE_DECIMALS = 1;
const PERCENT_DECIMALS = 1;

/**
 * One section of a record, rendered from its tag and never from a guess (the orchestrator's ruling
 * 34): `ok` its rows; `not_resolved` and `not_modelled` its name with `NOT RESOLVED` or
 * `NOT YET MODELLED`, once; `not_applicable` nothing at all, as a gas giant has no surface.
 *
 * @param name - The section's name, which a stand-in reads beside: `BULK`, `SURFACE`.
 * @param rows - Its rows, when it is `ok`.
 */
function sectionRows<T>(
  name: string,
  section: Section<T>,
  rows: (value: T) => ReactNode,
): ReactNode {
  let result: ReactNode;
  switch (section.state) {
    case "ok":
      result = rows(section.value);
      break;
    case "not_resolved":
    case "not_modelled":
      result = (
        <ReadoutRow key={name} label={name} shown={value(sectionStateLabel(section.state))} wide />
      );
      break;
    case "not_applicable":
      result = null;
      break;
  }
  return result;
}

/** A count of bodies, or `NONE` for an empty list, which is data. */
function countOf(ids: ReadonlyArray<string>): string {
  return ids.length === 0 ? "NONE" : formatNumber(ids.length, 0);
}

/** A mass fraction as a percentage, without its unit: `32.3`. */
function percent(fraction: number): string {
  return formatNumber(fraction * 100, PERCENT_DECIMALS);
}

/** The rows of a bulk section. */
function bulkRows(bulk: BulkProperties): ReactNode {
  const { massFractions: fractions } = bulk;
  return (
    <>
      <ReadoutRow label="CLASS" shown={value(planetClassLabel(bulk.planetClass))} />
      <ReadoutRow label="RADIUS" shown={value(formatRadiusKm(bulk.radiusM * KM_PER_M), "km")} />
      <ReadoutRow label="DENSITY" shown={value(formatSignificant(bulk.densityKgM3), "kg/m³")} />
      <ReadoutRow label="GRAVITY" shown={value(formatGravity(bulk.surfaceGravityMS2), "m/s²")} />
      <ReadoutRow
        label="T EQ"
        shown={value(formatTemperatureK(bulk.equilibriumTemperatureK), "K")}
      />
      <ReadoutRow label="IRON" shown={value(percent(fractions.iron), "%")} />
      <ReadoutRow label="ROCK" shown={value(percent(fractions.rock), "%")} />
      <ReadoutRow label="WATER" shown={value(percent(fractions.water), "%")} />
      <ReadoutRow label="ENVELOPE" shown={value(percent(fractions.envelope), "%")} />
    </>
  );
}

/** Props of {@link BodyRecordReadings}. */
export interface BodyRecordReadingsProps {
  /** The whole record once `body_detail` has answered, and the list's entry until then. */
  readonly body: SystemBody | BodyRecord;
  /** The detail level `body` holds. */
  readonly granted: DetailLevelDto;
  /** What it orbits, in words; `null` for a free-floating object. */
  readonly parentName: string | null;
  /** Its distance from what it orbits at the display time, m; `null` without an orbit. */
  readonly distanceM: number | null;
  /** Its orbit's inclination to the orbit map's plane, or to its planet's equator for a moon, rad. */
  readonly inclinationRad: number | null;
}

/**
 * A body's readings, every section from its tag (plan 14, P14.T43.b; the orchestrator's ruling
 * 34).
 *
 * @remarks
 * The designation and ID; the label; the kind in words, a moon's origin, and the state, with what
 * destroyed it and since when in universe time; what it orbits; the detail level its record holds.
 * Then each section as its tag says: the mass in `M⊕` (ruling 64.3); the orbit's semi-major axis,
 * period, eccentricity and inclination, and the distance from what it orbits now; the bulk's class,
 * radius in `km`, density, gravity, equilibrium temperature and composition by mass; its moons and
 * rings, counted; and, from the whole record only, its surface and its hooks, whose surface seed is
 * the one hook the wire carries. A section withheld or not modelled reads so, once, in place of its
 * rows; one that does not apply is left out.
 *
 * The distance is the one reading the display derives: from the server's elements at the display
 * time, by the solver that agrees with the server's to 10⁻⁹ (P14.T39), since the answer is up to a
 * year old; it keeps its unit with `formatBodyDistance`'s hysteresis as the time moves. The
 * inclination of a planet is to the plane the map is drawn on, the angle between the two normals the
 * server gives.
 */
export function BodyRecordReadings({
  body,
  granted,
  parentName,
  distanceM,
  inclinationRad,
}: BodyRecordReadingsProps) {
  const distance = distanceM === null ? null : formatBodyDistance(distanceM * KM_PER_M, null);
  const [distanceUnit, setDistanceUnit] = useState<BodyDistanceUnit | null>(distance?.unit ?? null);
  // The unit is held from one reading to the next, adjusted during render, so that a distance that
  // moves across a boundary with the display time does not flicker between units.
  const held = distanceM === null ? null : formatBodyDistance(distanceM * KM_PER_M, distanceUnit);
  if (held !== null && held.unit !== distanceUnit) {
    setDistanceUnit(held.unit);
  }
  const state = body.state;
  const whole = "surface" in body ? body : null;
  return (
    <>
      <ReadoutRow label="DESIG" shown={value(body.designation)} wide />
      <ReadoutRow label="ID" shown={value(formatBodyIdHex(body.id))} wide />
      {sectionRows("LABEL", body.label, (label) => (
        <ReadoutRow label="LABEL" shown={value(label)} wide />
      ))}
      <ReadoutRow label="KIND" shown={value(bodyKindLabel(body.kind))} wide />
      {body.kind.kind === "moon" ? (
        <ReadoutRow label="ORIGIN" shown={value(moonOriginLabel(body.kind.origin))} wide />
      ) : null}
      <ReadoutRow label="STATE" shown={value(bodyStateLabel(state))} wide />
      {state.kind === "destroyed" ? (
        <ReadoutRow label="CAUSE" shown={value(destructionCauseLabel(state.cause))} wide />
      ) : null}
      {state.kind === "destroyed" || state.kind === "unbound" ? (
        <ReadoutRow
          label="SINCE"
          shown={value(`${TIME_SYSTEM_LABEL} ${formatUniverseTimeDhms(state.at)}`)}
          wide
        />
      ) : null}
      <ReadoutRow label="PARENT" shown={value(parentName ?? "NONE")} wide />
      <ReadoutRow label="DETAIL" shown={value(detailLevelLabel(granted))} wide />
      {sectionRows("MASS", body.massMearth, (massMearth) => (
        <ReadoutRow label="MASS" shown={value(formatMassMearth(massMearth), <EarthMassUnit />)} />
      ))}
      {sectionRows("ORBIT", body.orbit, ({ orbit }) => {
        const axis = formatBodyDistance(orbit.semiMajorAxisM * KM_PER_M, null);
        const period = formatPeriod(orbit.periodS / SECONDS_PER_DAY);
        return (
          <>
            <ReadoutRow label="SMA" shown={value(axis.value, axis.unit)} />
            <ReadoutRow label="PERIOD" shown={value(period.value, period.unit)} />
            <ReadoutRow
              label="ECC"
              shown={value(formatNumber(orbit.eccentricity, ECCENTRICITY_DECIMALS))}
            />
            <ReadoutRow
              label="INC"
              shown={
                inclinationRad === null
                  ? { kind: "missing" }
                  : value(`${formatNumber((inclinationRad * 180) / Math.PI, ANGLE_DECIMALS)}°`)
              }
            />
            <ReadoutRow
              label="DIST"
              shown={held === null ? { kind: "missing" } : value(held.value, held.unit)}
            />
          </>
        );
      })}
      {sectionRows("BULK", body.bulk, bulkRows)}
      {sectionRows("MOONS", body.moons, (moons) => (
        <ReadoutRow label="MOONS" shown={value(countOf(moons))} />
      ))}
      {sectionRows("RINGS", body.rings, (rings) => (
        <ReadoutRow label="RINGS" shown={value(countOf(rings))} />
      ))}
      {whole === null ? null : (
        <>
          {sectionRows("SURFACE", whole.surface, () => null)}
          {sectionRows("HOOKS", whole.hooks, (hooks) => (
            <ReadoutRow label="SURFACE SEED" shown={value(hooks.surfaceSeed.toUpperCase())} wide />
          ))}
        </>
      )}
    </>
  );
}
