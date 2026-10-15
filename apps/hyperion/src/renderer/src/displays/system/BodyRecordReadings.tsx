import { type DetailLevelDto, SECONDS_PER_JULIAN_YEAR } from "@hyperion/protocol";
import { type ReactNode, useState } from "react";

import { EarthMassUnit } from "../../components/EarthMassUnit";
import {
  type BodyDistanceUnit,
  formatBodyDistance,
  formatBodySpan,
  formatGravity,
  formatMassMearth,
  formatNumber,
  formatPeriod,
  formatPressure,
  formatRadiusKm,
  formatSci,
  formatSignificant,
  formatTemperatureK,
  TIME_SYSTEM_LABEL,
} from "../../lib/format";
import {
  beltCompositionLabel,
  beltSiteLabel,
  bodyKindLabel,
  bodyStateLabel,
  destructionCauseLabel,
  detailLevelLabel,
  moonOriginLabel,
  planetClassLabel,
  ringKindLabel,
  ringMaterialLabel,
  sectionStateLabel,
  surfaceStateLabel,
} from "../../lib/system/bodyWords";
import type {
  BodyEnvelope,
  BodyRecord,
  BodySurface,
  BulkProperties,
  GasShare,
  Population,
  Section,
  SystemBody,
} from "../../lib/system/model";
import { formatBodyIdHex } from "../../lib/seed";
import { substanceName } from "../../lib/substances";
import { BodyIdRow } from "./BodyIdRow";
import { formatEventTime } from "./displayTime";
import { MISSING, ReadoutRow, type Shown, shown as value } from "./ReadoutRow";

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

/**
 * The `DETAIL SEED` row: the seed in upper case; the guide's Missing state, the em dash, while the
 * server does not send it, since it is a single value inside a hooks section that is shown (until
 * rendering plan R09 derives it); and no row where it does not apply.
 */
function detailSeedRow(seed: Section<string>): ReactNode {
  let reading: Shown;
  switch (seed.state) {
    case "ok":
      reading = value(seed.value.toUpperCase());
      break;
    case "not_resolved":
    case "not_modelled":
      reading = MISSING;
      break;
    case "not_applicable":
      return null;
  }
  return <ReadoutRow label="DETAIL SEED" shown={reading} wide />;
}

/** A count of bodies, or `NONE` for an empty list, which is data. */
function countOf(ids: ReadonlyArray<string>): string {
  return ids.length === 0 ? "NONE" : formatNumber(ids.length, 0);
}

/** A mass fraction as a percentage, without its unit: `32.3`. */
function percent(fraction: number): string {
  return formatNumber(fraction * 100, PERCENT_DECIMALS);
}

/** What joins the parts of a reading of several, as the guide's Voice joins them. */
const PART_JOIN = " · ";

/** The no-break space (U+00A0), which both B612 faces hold, inside each part of such a reading. */
const NO_BREAK_SPACE = "\u00A0";

/** One part of a reading of shares: what it is, in words or by its formula, and its percentage. */
interface Share {
  readonly name: string;
  readonly percentage: string;
}

/**
 * Shares as one reading, each its name and its percentage with its `%`, joined by middle dots:
 * `N2 78.1 % · O2 20.9 % · Ar 0.934 %`. The quantity is named once, in the row's label, never on
 * the `%` (the guide's Voice and nomenclature; decision-substance-names).
 *
 * @remarks
 * A part's figure keeps its `%` across a no-break space, as the guide's `±` reading keeps its parts
 * together, and so does a name of one word, a formula's or a layer's, its figure: a narrow column
 * breaks such a reading only after a middle dot. A name of several words (`UNLISTED MATERIAL`) may
 * break between its words and before its figure, since bound whole it is wider than the column at
 * 1280 × 720 and would scroll the readings sideways.
 */
function sharesReading(shares: ReadonlyArray<Share>): string {
  return shares
    .map(({ name, percentage }) => {
      const nameJoin = name.includes(" ") ? " " : NO_BREAK_SPACE;
      return `${name}${nameJoin}${percentage}${NO_BREAK_SPACE}%`;
    })
    .join(PART_JOIN);
}

/**
 * A mixture's gases as their reading, in the order the wire gives them, largest first: each named
 * by {@link substanceName} in a label's case, never by its key, with its mole fraction to the
 * guide's three significant figures (`Ar 0.934 %`), in E notation below 0.001 % (`1.00E-4 %`);
 * `NONE` for no gas, which is data.
 */
function gasesShown(gases: ReadonlyArray<GasShare>): Shown {
  return gases.length === 0
    ? value("NONE")
    : value(
        sharesReading(
          gases.map((gas) => ({
            name: substanceName(gas.species, "upper"),
            percentage: formatSignificant(gas.moleFraction * 100),
          })),
        ),
      );
}

/**
 * A surface pressure with its unit (`101 kPa`, `9.20 MPa`), or `NONE` for an airless body's zero,
 * which is data: no air, never a reading of `0.00 Pa`.
 */
function pressureShown(pressurePa: number): Shown {
  if (pressurePa === 0) {
    return value("NONE");
  }
  const pressure = formatPressure(pressurePa);
  return value(pressure.value, pressure.unit);
}

/**
 * The rows of a surface section (plan 14, P14.T43.b, _Amended for T35.e_): its state in words under
 * the section's own name, so that `SURFACE` keeps its place whether the section is shown or stands
 * in; the mean surface temperature; the surface pressure; and the air's gases by mole fraction.
 *
 * @remarks
 * The temperature and the pressure take a line each, so that their long labels share the label
 * track that wraps rather than widening the second one.
 *
 * An airless body's air is computed, with none in it: its pressure and its gases read `NONE`, and
 * its exosphere, a quantity of its own that the generator does not yet compute (P14.T55.c), reads
 * `NOT YET MODELLED`, so that `NONE` is never read as a body known to be clear of gas (the guide's
 * Data states). The note follows the `airless` state, as the view's `EXOSPHERE: NOT YET MODELLED`
 * does (rendering plan R08, R08.T10.a). The section's other members (its material, climate regime
 * and global figures) are on the wire and not yet read here.
 */
function surfaceRows(surface: BodySurface): ReactNode {
  return (
    <>
      <ReadoutRow label="SURFACE" shown={value(surfaceStateLabel(surface.state))} wide />
      <ReadoutRow
        label="MEAN SURFACE TEMPERATURE"
        shown={value(formatTemperatureK(surface.meanSurfaceTemperatureK), "K")}
        wide
      />
      <ReadoutRow label="SURFACE PRESSURE" shown={pressureShown(surface.surfacePressurePa)} wide />
      <ReadoutRow label="MOLE FRACTION" shown={gasesShown(surface.gases)} wide />
      {surface.state === "airless" ? (
        <ReadoutRow label="EXOSPHERE" shown={value(sectionStateLabel("not_modelled"))} wide />
      ) : null}
    </>
  );
}

/**
 * The rows of a gas-envelope body's `envelope` section, the air of a giant or a sub-Neptune, whose
 * surface section does not apply. It reads as `ATMOSPHERE`, as the view's notes name the same air
 * (`ATMOSPHERE: NOT YET MODELLED`), since the `BULK` section's `ENVELOPE` is the structure model's
 * layer, which is modelled: `ATMOSPHERE GAS ENVELOPE`, under the section's own name as `SURFACE`'s
 * state is, then its composition by mole fraction, as a surface's gases read.
 *
 * @remarks
 * Its other figures (the Guillot profile's temperatures and parameters, the adiabat, the decks and
 * the haze) are on the wire and not yet read here. Every server sends the section `not_modelled`
 * until P14.T24.d fills it, and {@link sectionRows} reads it so. `GAS ENVELOPE` holds while the
 * section is a gas-envelope body's alone, as the protocol defines it; should P14.T53.b give a
 * supercritical-water world this section, the lead row is to read the wire's state instead.
 */
function envelopeRows(envelope: BodyEnvelope): ReactNode {
  return (
    <>
      <ReadoutRow label="ATMOSPHERE" shown={value("GAS ENVELOPE")} wide />
      <ReadoutRow label="MOLE FRACTION" shown={gasesShown(envelope.composition)} wide />
    </>
  );
}

/** Props of {@link SpanRow}. */
interface SpanRowProps {
  readonly label: string;
  /** The span's inner and outer ends, m. */
  readonly innerM: number;
  readonly outerM: number;
}

/**
 * An annulus's two edges as one reading, `66.0 – 137 Mm`, its unit in the row's unit slot
 * (ruling 113.3). The unit is the outer end's, held from one reading to the next with
 * `formatBodySpan`'s hysteresis, adjusted during render as the `DIST` row's is.
 */
function SpanRow({ label, innerM, outerM }: SpanRowProps) {
  const [unit, setUnit] = useState<BodyDistanceUnit>(
    () => formatBodySpan(innerM * KM_PER_M, outerM * KM_PER_M, null).unit,
  );
  const span = formatBodySpan(innerM * KM_PER_M, outerM * KM_PER_M, unit);
  if (span.unit !== unit) {
    setUnit(span.unit);
  }
  return <ReadoutRow label={label} shown={value(span.value, span.unit)} wide />;
}

/** A gap's row: the resonance it is cleared at in its label, `GAP 2:1`, and where it lies. */
function gapRow(resonance: readonly [number, number], radiusM: number): ReactNode {
  const label = `GAP ${resonance[0]}:${resonance[1]}`;
  const at = formatBodyDistance(radiusM * KM_PER_M, null);
  return <ReadoutRow key={`${label}:${radiusM}`} label={label} shown={value(at.value, at.unit)} />;
}

/**
 * The rows of a population section: a ring's kind, material, edges from its planet's centre,
 * optical depth and gaps; a belt's site, edges from its host (the belt proper's and a scattered
 * component's apart), composition, size slope, largest body, mean eccentricity and inclination,
 * dust luminosity, gaps and members; the halo's edges, comets and comet rate.
 */
function populationRows(population: Population): ReactNode {
  let rows: ReactNode;
  switch (population.kind) {
    case "ring":
      rows = (
        <>
          <ReadoutRow label="CLASS" shown={value(ringKindLabel(population.ringKind))} />
          <ReadoutRow label="COMPOSITION" shown={value(ringMaterialLabel(population.material))} />
          <SpanRow label="EDGES" innerM={population.innerEdgeM} outerM={population.outerEdgeM} />
          <ReadoutRow
            label="OPTICAL DEPTH"
            shown={value(formatSignificant(population.opticalDepth))}
          />
          {population.gaps.map((gap) => gapRow(gap.resonance, gap.radiusM))}
        </>
      );
      break;
    case "belt":
      rows = (
        <>
          <ReadoutRow label="SITE" shown={value(beltSiteLabel(population.site))} />
          <ReadoutRow
            label="COMPOSITION"
            shown={value(beltCompositionLabel(population.composition))}
          />
          <SpanRow label="EDGES" innerM={population.innerEdgeM} outerM={population.outerEdgeM} />
          {population.scattered === null ? null : (
            <>
              <SpanRow
                label="BELT PROPER"
                innerM={population.main.innerEdgeM}
                outerM={population.main.outerEdgeM}
              />
              <SpanRow
                label="SCATTERED DISC"
                innerM={population.scattered.innerEdgeM}
                outerM={population.scattered.outerEdgeM}
              />
            </>
          )}
          <ReadoutRow label="SIZE INDEX" shown={value(formatNumber(population.sizeSlope, 2))} />
          <ReadoutRow
            label="LARGEST DIAMETER"
            shown={value(formatRadiusKm(population.largestDiameterM * KM_PER_M), "km")}
          />
          <ReadoutRow
            label="MEAN ECC"
            shown={value(formatNumber(population.meanEccentricity, ECCENTRICITY_DECIMALS))}
          />
          <ReadoutRow
            label="MEAN INC"
            shown={value(
              `${formatNumber((population.meanInclinationRad * 180) / Math.PI, ANGLE_DECIMALS)}°`,
            )}
          />
          <ReadoutRow
            label="DUST LUMINOSITY"
            shown={
              population.fractionalLuminosity === 0
                ? value("NONE")
                : value(formatSci(population.fractionalLuminosity), "× HOST")
            }
          />
          {population.gaps.map((gap) => gapRow(gap.resonance, gap.radiusM))}
          {sectionRows("MEMBERS", population.members, (members) => (
            <ReadoutRow label="MEMBERS" shown={value(countOf(members))} />
          ))}
        </>
      );
      break;
    case "cometary_halo":
      rows = (
        <>
          <SpanRow label="EDGES" innerM={population.innerEdgeM} outerM={population.outerEdgeM} />
          <ReadoutRow
            label="COMETS OVER 1 km"
            shown={value(formatSignificant(population.comets))}
          />
          <ReadoutRow
            label="COMET RATE"
            shown={value(
              formatSignificant(population.cometRatePerS * SECONDS_PER_JULIAN_YEAR),
              "/yr",
            )}
          />
        </>
      );
      break;
  }
  return rows;
}

/**
 * The rows of a bulk section.
 *
 * @remarks
 * The layers of the structure model are one reading under `MASS FRACTION`, which names their
 * quantity once (signoff-5, item 8): `IRON 32.3 % · ROCK 67.7 % · WATER 0.0 % · ENVELOPE 0.0 %`.
 * A layer is not a substance and keeps its word, and the layers keep the model's order, from the
 * core out, so that each stands in the same place for every body.
 */
function bulkRows(bulk: BulkProperties): ReactNode {
  const { massFractions: fractions } = bulk;
  const layers: ReadonlyArray<Share> = [
    { name: "IRON", percentage: percent(fractions.iron) },
    { name: "ROCK", percentage: percent(fractions.rock) },
    { name: "WATER", percentage: percent(fractions.water) },
    { name: "ENVELOPE", percentage: percent(fractions.envelope) },
  ];
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
      {bulk.effectiveTemperatureK === null ? null : (
        <ReadoutRow
          label="T EFF"
          shown={value(formatTemperatureK(bulk.effectiveTemperatureK), "K")}
        />
      )}
      <ReadoutRow label="MASS FRACTION" shown={value(sharesReading(layers))} wide />
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
  /**
   * Its orbit's inclination to the orbit map's system plane, or to its planet's equator for a moon,
   * rad; `null` when there is no plane to read it to.
   */
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
 * radius in `km`, density, gravity, equilibrium temperature and its layers' mass fractions; its
 * moons and rings, counted; for a ring, a belt or the cometary halo, what the population is and
 * where it lies; and, from the whole record only, its surface (its state, mean temperature,
 * pressure and gases), a gas-envelope body's envelope in the surface's place, read as its
 * `ATMOSPHERE` (its gases), and its hooks, whose detail seed is the one hook the wire carries, the
 * em dash until the server derives it. A section withheld or not modelled reads so, once, in place
 * of its rows; one that does not apply is left out. A server since P14.T35.e sends one of `surface`
 * and `envelope` as not applicable for every planet it resolves to the `surface` level, so the
 * planet's air is read from the other; an older server sends no envelope, which reads
 * `ATMOSPHERE NOT YET MODELLED`.
 *
 * The distance is the one reading the display derives: from the server's elements at the display
 * time, by the solver that agrees with the server's to 10⁻⁹ (P14.T39), since the answer is up to a
 * year old; it keeps its unit with `formatBodyDistance`'s hysteresis as the time moves. The
 * inclination of a planet is to the plane the map is drawn on, the angle between the two normals the
 * server gives; a moon's is to its planet's equator, the plane normal to the planet's pole, or to
 * the planet's orbit normal where its rotation is not modelled (P14.T56.b), since a satellite's
 * elements are in its planet's body frame, whose axes are the galactic axes.
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
      <BodyIdRow text={formatBodyIdHex(body.id)} />
      {sectionRows("NAME", body.label, (label) => (
        <ReadoutRow label="NAME" shown={value(label)} wide />
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
          shown={value(`${TIME_SYSTEM_LABEL} ${formatEventTime(state.at)}`)}
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
      {sectionRows("SMALL BODIES", body.population, populationRows)}
      {whole === null ? null : (
        <>
          {sectionRows("SURFACE", whole.surface, surfaceRows)}
          {sectionRows("ATMOSPHERE", whole.envelope, envelopeRows)}
          {sectionRows("GENERATOR INPUTS", whole.hooks, (hooks) => detailSeedRow(hooks.detailSeed))}
        </>
      )}
    </>
  );
}
