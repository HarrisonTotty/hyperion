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
  crustLidLabel,
  destructionCauseLabel,
  detailLevelLabel,
  moonOriginLabel,
  planetClassLabel,
  redoxClassLabel,
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
  CrustComposition,
  GasShare,
  Modelled,
  Population,
  Section,
  SilicateVapour,
  SubstanceArea,
  SurfaceCondensate,
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

/**
 * A fraction as a percentage to the guide's three significant figures, without its unit: a mole
 * fraction's `0.934`, a share of the surface's `71.0`, and `1.00E-4` below 0.001 %.
 */
function significantPercent(fraction: number): string {
  return formatSignificant(fraction * 100);
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
 * A share's percentage as its part of a reading of shares reads it, without its unit: its figure,
 * or `ABOVE 99.9` for a share below the whole whose figure rounds to the whole's, so that no part
 * claims the whole beside another, as the guide's survey coverage reads (signoff-11, item 21;
 * signoff-12, item 43).
 *
 * @remarks
 * Ruled on the figure as shown, not on a threshold of the fraction, so that the rule and the
 * reading agree at every rounding edge, against the same formatter's figure for the whole. `99.9`
 * is the last figure below the whole at both precisions the readout reads shares to, three
 * significant figures and one decimal; a share read to another would need its own. `ABOVE` keeps
 * its figure across a no-break space, as `%` does.
 *
 * @param fraction - The share, from 0 to 1.
 * @param figure - The share's percentage as shown, without its unit, from its fraction.
 */
function sharePercentage(fraction: number, figure: (fraction: number) => string): string {
  const shown = figure(fraction);
  return fraction < 1 && shown === figure(1) ? `ABOVE${NO_BREAK_SPACE}99.9` : shown;
}

/**
 * A mixture's gases as their reading, in the order the wire gives them, largest first: each named
 * by {@link substanceName} in a label's case, never by its key, with its mole fraction to the
 * guide's three significant figures (`Ar 0.934 %`), in E notation below 0.001 % (`1.00E-4 %`);
 * `NONE` for no gas, which is data.
 *
 * @remarks
 * A gas below the whole whose three figures round to `100` reads `ABOVE 99.9 %`, as the guide's
 * survey coverage does, so that no part claims the whole beside another; only a gas that is the
 * whole reads `100 %` (signoff-11, item 21).
 */
function gasesShown(gases: ReadonlyArray<GasShare>): Shown {
  return gases.length === 0
    ? value("NONE")
    : value(
        sharesReading(
          gases.map((gas) => ({
            name: substanceName(gas.species, "upper"),
            percentage: sharePercentage(gas.moleFraction, significantPercent),
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
 * Whether a surface holds melt, where a vapour of its own, the melt's silicate vapour, lies over
 * it, as the wire states it: the state `magma_ocean` (P14.T35.f), or a crust whose
 * `melt_area_fraction` is above 0 (P14.T54.a), as a locked lava world's dayside pool is under the
 * state `airless`.
 *
 * @remarks
 * A crust not modelled, as every server before P14.T54.a sends it, adds no melt, and an ice lid's
 * melt area is 0, since its liquid is a condensate, not a melt of the crust. P14.T53.b decides
 * `steam_envelope`, which is not read here.
 */
function holdsMelt(surface: BodySurface): boolean {
  return (
    surface.state === "magma_ocean" ||
    (surface.crust.kind === "value" && surface.crust.value.meltAreaFraction > 0)
  );
}

/** What a melt's own vapour, as the wire sends it, lets its surface's rows read. */
interface VapourReading {
  /** Whether an empty volatile air reads Missing, the em dash, rather than `NONE`. */
  readonly emptyAirMissing: boolean;
  /** Whether the `SILICATE VAPOUR NOT YET MODELLED` row stands. */
  readonly notModelledRow: boolean;
}

/**
 * How a melt's own vapour reads: not modelled, Missing air and its row; computed as none, `NONE`
 * and no row (decision-backlog-2 §2.4); a value, Missing air and no row.
 */
function vapourReading(vapour: Modelled<SilicateVapour>): VapourReading {
  let reading: VapourReading;
  switch (vapour.kind) {
    case "not_modelled":
      reading = { emptyAirMissing: true, notModelledRow: true };
      break;
    case "none":
      reading = { emptyAirMissing: false, notModelledRow: false };
      break;
    case "value":
      // `NOT YET MODELLED` would be false, and this client has no figures to read: P14.T55.b, which
      // gives the vapour its members, adds its row (plan 14, P14.T43.e's record).
      reading = { emptyAirMissing: true, notModelledRow: false };
      break;
  }
  return reading;
}

/**
 * The registry key of the crust that lies at the surface, the member its lid names.
 *
 * @throws Error where the lid names a member that is `null`, which the wire's contract forbids and
 * the decoder (`bodiesWire.ts`'s `toCrust`) refuses, so that only a broken client reaches it.
 */
function lidSubstance(crust: CrustComposition): string {
  let key: string | null;
  switch (crust.lid) {
    case "primary":
      key = crust.primary;
      break;
    case "secondary":
      key = crust.secondary;
      break;
    case "bulk":
      key = crust.bulk;
      break;
  }
  if (key === null) {
    throw new Error(`the crust's lid names its ${crust.lid} member, which is null`);
  }
  return key;
}

/**
 * The melt's share of the surface as `MELT COVER` reads it: its percentage to three significant
 * figures with its unit (`25.0 %`), in E notation below 0.001 %, `ABOVE 99.9 %` for a share below
 * the whole that would round to it, `100 %` only for the whole, and `NONE` for 0, which is data.
 */
function meltCoverShown(meltAreaFraction: number): Shown {
  return meltAreaFraction === 0
    ? value("NONE")
    : value(sharePercentage(meltAreaFraction, significantPercent), "%");
}

/**
 * The rows of a surface's crust: `CRUST`, which crust lies at the surface and its substance, joined
 * by a middle dot as the view's `MATERIAL` reading is (`SECONDARY · REDUCED BASALT`); then
 * `MANTLE REDOX` and `MELT COVER`. A crust not modelled reads `CRUST NOT YET MODELLED`, and one
 * computed as none `CRUST NONE`, once, in place of the three.
 */
function crustRows(crust: Modelled<CrustComposition>): ReactNode {
  let rows: ReactNode;
  switch (crust.kind) {
    case "not_modelled":
      rows = <ReadoutRow label="CRUST" shown={value(sectionStateLabel("not_modelled"))} wide />;
      break;
    case "none":
      rows = <ReadoutRow label="CRUST" shown={value("NONE")} wide />;
      break;
    case "value": {
      const composition = crust.value;
      const substance = substanceName(lidSubstance(composition), "upper");
      rows = (
        <>
          <ReadoutRow
            label="CRUST"
            shown={value(`${crustLidLabel(composition.lid)}${PART_JOIN}${substance}`)}
            wide
          />
          <ReadoutRow label="MANTLE REDOX" shown={value(redoxClassLabel(composition.redox))} wide />
          <ReadoutRow
            label="MELT COVER"
            shown={meltCoverShown(composition.meltAreaFraction)}
            wide
          />
        </>
      );
      break;
    }
  }
  return rows;
}

/**
 * A cover by substance, the surface's liquids or its ices, as one reading of shares: largest first,
 * ties in the wire's order, each named by {@link substanceName} in a label's case with no phase
 * word, since the row's label names the phase, and its share at `MELT COVER`'s precision; a part at
 * 0 left out, and `NONE` where no part is above 0, which is data.
 *
 * @remarks
 * Sorted here, by a stable sort, since the wire orders the cover by the reservoirs' mass, not by
 * area.
 */
function coverShown(areas: ReadonlyArray<SubstanceArea>): Shown {
  const parts = areas
    .filter((area) => area.areaFraction > 0)
    .toSorted((a, b) => b.areaFraction - a.areaFraction);
  return parts.length === 0
    ? value("NONE")
    : value(
        sharesReading(
          parts.map((area) => ({
            name: substanceName(area.substance, "upper"),
            percentage: sharePercentage(area.areaFraction, significantPercent),
          })),
        ),
      );
}

/**
 * Whether the wire states the ice in a surface's cold traps: some condensate carries its trapped
 * share, which no server sends before P14.T54.b.
 */
function coldTrapsKnown(condensates: Modelled<ReadonlyArray<SurfaceCondensate>>): boolean {
  return (
    condensates.kind === "value" &&
    condensates.value.some((condensate) => condensate.trappedAreaFraction.kind === "value")
  );
}

/**
 * The rows of a surface section (plan 14, P14.T43.b, _Amended for T35.e_): its state in words under
 * the section's own name, so that `SURFACE` keeps its place whether the section is shown or stands
 * in; the mean surface temperature; the surface pressure; the air's gases by mole fraction; and then
 * the ground's crust and cover (P14.T43.g).
 *
 * @remarks
 * The temperature and the pressure take a line each, so that `SURFACE PRESSURE`, the section's
 * longest label, stays in the first label track rather than widening the second.
 *
 * An airless body's air is computed, with none in it: its pressure and its gases read `NONE`
 * unless it holds melt (below), and its exosphere, a quantity of its own that the generator does
 * not yet compute (P14.T55.c), reads `NOT YET MODELLED`, so that `NONE` is never read as a body
 * known to be clear of gas (the guide's Data states). The note follows the `airless` state, as the
 * view's `EXOSPHERE: NOT YET MODELLED` does (rendering plan R08, R08.T10.a).
 *
 * A surface holding melt, the state `magma_ocean` (P14.T35.f) or a crust's melt area above 0
 * (P14.T54.a), has a vapour of its own, the melt's silicate vapour, which no rule computes before
 * P14.T55.b. While the wire leaves it out, its row reads `SILICATE VAPOUR NOT YET MODELLED`, and an
 * empty volatile air reads its pressure and gases Missing, the em dash, never `NONE`, since the
 * pressure over a melt is at least the melt's own vapour pressure (the guide's Data states;
 * decision-backlog-2 §2.4, decision-triton-and-e101 §2.3). A volatile air it holds, a young magma
 * ocean's steam and CO2, reads its figures beside the row; a vapour computed as none reads `NONE` as
 * an airless body's air does. A locked lava world, `airless` with a dayside pool, reads both notes,
 * `EXOSPHERE` and `SILICATE VAPOUR` each on its own line, as the view does (rendering plan R08,
 * Design note 12). `SILICATE VAPOUR` is signed off (signoff-12, item 27), as one name with the
 * view's `SILICATE VAPOUR: NOT YET MODELLED` note, which R08.T10.a adds.
 *
 * A vapour the wire sends as a value, which no server does before P14.T55.b, reads its empty
 * volatile air as Missing too, and has no row: its figures are T55.b's to read.
 *
 * After the air's rows come the ground's, in this order (P14.T43.g, decision-ux-23-surface-rows):
 * the crust's rows ({@link crustRows}), `CRUST`, `MANTLE REDOX`, the mantle's class and not the
 * ground's, which may be more oxidised, and `MELT COVER`, the crust's melt area; then the cover by
 * substance, `LIQUID COVER` and `ICE COVER` ({@link coverShown}), read from every server, as the
 * decision rules, though a server before P14.T54.a, which models no condensates, sends its water
 * alone (plan 14, P14.T43.g's deferred findings); and, on an `airless` surface,
 * `COLD TRAPS NOT YET MODELLED` unless a condensate carries its trapped share
 * ({@link coldTrapsKnown}), as T54.a labels an airless body's cold traps, so that its `ICE COVER` is
 * never read as all the ice it holds.
 *
 * The section's other members are on the wire and not yet read here: its material, climate regime
 * and global figures; the crusts that do not lie at the surface and the crust's provinces, which
 * the view's `MATERIAL` reading names point by point once the terrain is served; the mantle's redox
 * figure ({@link CrustComposition.mantleRedoxIw}), which waits on a name the guide does not have;
 * and the condensates' own figures, their reservoirs, layers and seasons.
 */
function surfaceRows(surface: BodySurface): ReactNode {
  const vapour = holdsMelt(surface) ? vapourReading(surface.silicateVapour) : null;
  // Ruled on the empty gases, not on the pressure (decision-backlog-2 §2.4): only a vapour computed
  // as none lets the volatile air's zero read as `NONE` over a melt.
  const airMissing = vapour !== null && vapour.emptyAirMissing && surface.gases.length === 0;
  return (
    <>
      <ReadoutRow label="SURFACE" shown={value(surfaceStateLabel(surface.state))} wide />
      <ReadoutRow
        label="T SURF"
        shown={value(formatTemperatureK(surface.meanSurfaceTemperatureK), "K")}
        wide
      />
      <ReadoutRow
        label="SURFACE PRESSURE"
        shown={airMissing ? MISSING : pressureShown(surface.surfacePressurePa)}
        wide
      />
      <ReadoutRow
        label="MOLE FRACTION"
        shown={airMissing ? MISSING : gasesShown(surface.gases)}
        wide
      />
      {surface.state === "airless" ? (
        <ReadoutRow label="EXOSPHERE" shown={value(sectionStateLabel("not_modelled"))} wide />
      ) : null}
      {vapour?.notModelledRow === true ? (
        <ReadoutRow label="SILICATE VAPOUR" shown={value(sectionStateLabel("not_modelled"))} wide />
      ) : null}
      {crustRows(surface.crust)}
      <ReadoutRow label="LIQUID COVER" shown={coverShown(surface.surfaceLiquids)} wide />
      <ReadoutRow label="ICE COVER" shown={coverShown(surface.surfaceIces)} wide />
      {surface.state === "airless" && !coldTrapsKnown(surface.condensates) ? (
        <ReadoutRow label="COLD TRAPS" shown={value(sectionStateLabel("not_modelled"))} wide />
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
 *
 * A layer below the whole whose one-decimal figure rounds to `100.0` reads `ABOVE 99.9 %`, as
 * `MOLE FRACTION`'s gases do, so that no layer claims the whole beside another: a rock at 0.99996
 * reads `IRON 0.0 % · ROCK ABOVE 99.9 % · WATER 0.0 % · ENVELOPE 0.0 %`. Only a body of one layer
 * alone reads `100.0 %`, and a layer that rounds to `0.0 %` keeps it, as a rounded share does
 * (signoff-12, item 43).
 */
function bulkRows(bulk: BulkProperties): ReactNode {
  const { massFractions: fractions } = bulk;
  const layer = (name: string, fraction: number): Share => ({
    name,
    percentage: sharePercentage(fraction, percent),
  });
  const layers: ReadonlyArray<Share> = [
    layer("IRON", fractions.iron),
    layer("ROCK", fractions.rock),
    layer("WATER", fractions.water),
    layer("ENVELOPE", fractions.envelope),
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
 * pressure and gases, and, over a melt, its own vapour's state; then the crust at the surface, the
 * mantle's redox class, the melt's share of the surface, the liquids and ices that cover it by
 * substance, and, on an airless body, its cold traps' state), a gas-envelope body's envelope in
 * the surface's place, read as its `ATMOSPHERE` (its gases), and its hooks, whose detail seed is
 * the one hook the wire carries, the em dash until the server derives it. A section withheld or not
 * modelled reads so, once, in place of its rows; one that does not apply is left out. A server
 * since P14.T35.e sends one of `surface` and `envelope` as not applicable for every planet it
 * resolves to the `surface` level, so the planet's air is read from the other; an older server
 * sends no envelope, which reads `ATMOSPHERE NOT YET MODELLED`.
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
