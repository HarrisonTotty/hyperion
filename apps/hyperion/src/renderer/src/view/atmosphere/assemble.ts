/**
 * A body's medium assembled from its record, with its labels (plan R08, R08.T10.a; Design notes 6,
 * 12 and 14; `decision-r08-t10a-leans.md`): the column on the body's gravity, the molecular,
 * absorber and aerosol terms, the labels of Design note 12 and the reasons behind
 * `ATMOSPHERE: APPROXIMATE`, for the optics worker (`optics.worker.ts`) and the atmosphere cache.
 *
 * @remarks
 * **The labels** follow the section the body's air comes from (decision-r08-giant-label;
 * decision-p14-t35e-wire): its surface section, or a gas envelope's own where the surface is
 * `not_applicable`. Withheld reads `NOT RESOLVED`, `not_modelled` or absent `NOT YET MODELLED`, and
 * a gas envelope sent `ok` keeps `NOT YET MODELLED` with no medium until R08.T16.a draws envelopes
 * (`decision-r08-t10a-leans.md` item 5). An `airless` surface draws nothing and says
 * `EXOSPHERE: NOT YET MODELLED` (`signoff-2.md` item 6); a surface holding melt (`magma_ocean`, or a
 * crust whose `meltAreaFraction` is above 0) whose `silicateVapour` is `not_modelled` says
 * `SILICATE VAPOUR: NOT YET MODELLED` beside its other notes; a surface with air and no inventory
 * says `AEROSOLS AND ABSORBERS: NOT YET MODELLED`. A kept scene's body set airless says nothing,
 * and one left unset `NOT YET MODELLED`.
 *
 * **The medium** for an `ok` surface with gases:
 *
 * - its gravity, R08.T3.d's `bodyGravity` of the record's figure, mass, spin at the record's time
 *   and bulk gravity, at R_ref = (2a + c) ÷ 3 ({@link mediumTableRadiusM});
 * - its column, `hydrostaticColumn` at the record's p_s and μ, isothermal at its T_s: the seam
 *   until P14.T24.e's published adiabat and R08.T3.a's follow-up, which ignores the record's
 *   `verticalStructure` even where it is sent;
 * - the molecular term of the record's gases, passed unconverted;
 * - every gas as an absorber layer spanning the column, x n_s ∫ρ dz from p_s up, so that the pairs
 *   and the `air` partner see the whole bulk (plan R08, T4's record "For R08.T10.a"), and each of
 *   the inventory's absorbers on its own layer and profile; a gas the registry does not flag
 *   `absorber` reports nothing for want of optics (Ar, Ne, Kr, Xe). A deck's vapour is cut at the
 *   deck's top; its cold-trapped vapour above is not drawn, and says so;
 * - the negligible-absorbers bound (`negligible.ts`) on the layers as listed: a pending species,
 *   which has no optics, takes no column and no curve whatever the bound finds, and no reason
 *   where it is negligible; a negligible species drawn with estimated optics keeps its term and
 *   drops its reason; each is named in {@link AssembledMedium.negligible}. The layers are never
 *   filtered by the bound, so that a species with a pending part keeps its measured data and a
 *   bulk gas its pairs;
 * - Thomson scattering on e⁻'s layer, and an aerosol term for each of the inventory's modes;
 * - at most `MAX_TERMS` (8) terms: the molecular term, then the others of greatest vertical depth
 *   (an absorber by `absorberRankingDepth`, a mode by its own τ(550), Thomson by σ_T U, the earlier
 *   listed first on a tie), the rest left out and reported.
 *
 * **The reasons** behind `ATMOSPHERE: APPROXIMATE` are player text, each naming its substance by
 * `substanceName`, shown under Design note 12's rule: while less than
 * `CLOUD_DECK_SPLIT_OPTICAL_DEPTH` of vertical optical depth at 550 nm lies above the layer's or the
 * mode's top, counting the molecular, Thomson and aerosol terms (gas absorption is each light's,
 * and left out). The provisional thick line (`thick/regime.ts`) adds one of its own.
 */

import type { AerosolShapeDto } from "@hyperion/protocol";

import { substanceName } from "../../lib/substances";
import { EARTH_MASS_KG } from "../../lib/system/bodiesWire";
import type {
  AerosolInventory,
  BodyDetail,
  BodyEnvelope,
  BodyRecord,
  BodySurface,
  BulkProperties,
  AerosolMode as RecordAerosolMode,
  SizeDistribution as RecordSizeDistribution,
  Section,
} from "../../lib/system/model";
import { spinRateAt } from "../../lib/system/rotation";
import {
  ABSORBER_CROSS_SECTIONS,
  ABSORBER_PAIR_CROSS_SECTIONS,
  AIR_PARTNER,
  type AbsorberApproximation,
  type AbsorberCurve,
  type AbsorberLayer,
  type AbsorberSpec,
  absorberCurve,
  absorberRankingDepth,
  type AbsorberSpecs,
  absorberSpecs,
  absorberTerm,
  layerMoments,
  THOMSON_CROSS_SECTION_M2,
  thomsonTerm,
} from "./absorbers";
import { committedLineSpecies, type KDistributionTable } from "./absorbers/kDistribution";
import {
  type AerosolModeSpec,
  type AerosolParticles,
  aerosolTerm,
  aerosolTermInRange,
  phaseApproximations,
} from "./aerosol";
import { aggregateShapeInRange } from "./aggregate";
import { type BodyAtmosphereInput, type BodyLight, clampedGroundAlbedo } from "./atmosphereInput";
import { type AtmosphereColumn, columnPressureBar, hydrostaticColumn } from "./column";
import { ATMOSPHERE_LABELS, type AtmosphereLabel } from "./labels";
import {
  type MaterialPhase,
  materialApproximations,
  phasedName,
  type ShapeClass,
} from "./materials/materials";
import {
  type AtmosphereMedium,
  checkTabulatedTops,
  columnLengthM,
  type DensityProfile,
  type MediumTerm,
} from "./medium";
import { negligibleAbsorbers, type NegligibleVerdict } from "./negligible";
import { bodyGravity, type BodyGravity, type SpheroidFigure } from "./oblate";
import { molecularMixture, molecularTerm, type RayleighApproximation } from "./rayleigh";
import { REGISTRY_OPTICS_FLAGS } from "./registryFlags";
import type { ModeOptics, SizeDistribution } from "./sizeDistribution";
import type { BakeSpectrum } from "./spectralColour";
import { CLOUD_DECK_SPLIT_OPTICAL_DEPTH, provisionalThick, THICK_REASON } from "./thick/regime";

export {
  ATMOSPHERE_OPTICS_VERSION,
  atmosphereKey,
  type BodyAtmosphereInput,
  type BodyLight,
  clampedGroundAlbedo,
  type KeptAtmosphere,
} from "./atmosphereInput";
export { THICK_REASON } from "./thick/regime";

/**
 * The most terms a medium may have: `tables.ts`'s `MAX_TERMS` (`common.wgsl`'s), 8, held equal by
 * `assemble.test.ts` rather than imported, so that this module's graph stays free of the engine.
 */
export const MEDIUM_TERM_LIMIT = 8;

/**
 * The most light slots a body's curves fill: R07's `MAX_BODY_LIGHTS` (`lighting/hostLights.ts`), 2,
 * held equal by `assemble.test.ts` rather than imported, as {@link MEDIUM_TERM_LIMIT} is.
 */
export const LIGHT_SLOT_LIMIT = 2;

/** The wavelength the τ rule and the inventory's optical depths are at, nm. */
const TAU_WAVELENGTH_NM = 550;

/**
 * The body a medium's tables are built for: `tables.ts`'s `AtmosphereBody`, its datum spheroid and
 * R08.T3.d's gravity (held assignable to it by `assemble.test.ts`).
 */
export interface AssembledBody {
  readonly figure: SpheroidFigure;
  readonly gravity: BodyGravity;
}

/**
 * Each light slot's curves of growth, in `lightsAt`'s order, one a medium's absorber term in its
 * order: `tables.ts`'s `AbsorberCurveSlots` (held equal to it by `assemble.test.ts`).
 */
export type CurveSlots = ReadonlyArray<ReadonlyArray<AbsorberCurve>>;

/** What the per-planet tables take beside a drawn medium. */
export interface DrawnMedium {
  readonly body: AssembledBody;
  /** R_ref, m: the tables' ground, (2a + c) ÷ 3 ({@link mediumTableRadiusM}). */
  readonly bottomRadiusM: number;
  /** The specs of the medium's absorber terms, in their order (`tables.ts`'s `absorberSlots`). */
  readonly absorbers: ReadonlyArray<AbsorberSpec>;
  /** The light slots' spectra, in `lightsAt`'s order. */
  readonly spectra: ReadonlyArray<BakeSpectrum>;
  /**
   * Per light slot, the absorbers' curves ({@link CurveSlots}): empty as assembled, filled by
   * {@link withCurves} with {@link absorberCurveSlots}' or the worker pool's.
   */
  readonly curves: CurveSlots;
}

/** What every assembly gives, drawn or not. */
interface AssembledLabels {
  /** The body's labels, each once, in `labels.ts`'s `ATMOSPHERE_LABELS` order. */
  readonly labels: ReadonlyArray<AtmosphereLabel>;
  /** Why the body carries `atmosphereApproximate`, as player text, each once; empty without it. */
  readonly reasons: ReadonlyArray<string>;
  /** The species and pair keys the negligible-absorbers bound found negligible, in layer order. */
  readonly negligible: ReadonlyArray<string>;
}

/**
 * A body's medium and labels (the Provides' `AssembledMedium`): no medium, or the medium with what
 * its tables take ({@link DrawnMedium}).
 */
export type AssembledMedium =
  | (AssembledLabels & { readonly medium: undefined; readonly drawn: undefined })
  | (AssembledLabels & { readonly medium: AtmosphereMedium; readonly drawn: DrawnMedium });

/** What the assembly takes beside its input. */
export interface AssembleOptions {
  /**
   * The line species' k-distribution tables, which the optics worker loads
   * (`absorbers/kDistribution.ts`'s `loadKDistributions` of {@link lineSpeciesOf}); none if absent,
   * when every line species reports `noCrossSections`.
   */
  readonly kDistributions?: ReadonlyArray<KDistributionTable>;
}

/** A body's absorber specs alone, for a curve-share job ({@link absorberSpecsOf}). */
export interface AssembledAbsorbers {
  /** As {@link DrawnMedium.absorbers}: empty for an input that draws no medium. */
  readonly specs: ReadonlyArray<AbsorberSpec>;
  /** As {@link DrawnMedium.spectra}. */
  readonly spectra: ReadonlyArray<BakeSpectrum>;
}

/**
 * A figure's mean radius R₁ = (2a + c) ÷ 3, m: the per-planet tables' ground, `hillaire.ts`'s
 * `tableRadiusM` (NIMA TR8350.2, Table 3.3's "mean radius of semi-axes"), written here so that this
 * module's graph stays free of the engine and held equal to it by `assemble.test.ts`.
 */
export function mediumTableRadiusM(figure: SpheroidFigure): number {
  return (2 * figure.equatorialRadiusM + figure.polarRadiusM) / 3;
}

// ---------------------------------------------------------------------------------------------
// The reasons, as the operator reads them.

/** A pair's member's name in a note: the `air` partner as the word, a species by `substanceName`. */
function memberName(member: string): string {
  return member === AIR_PARTNER ? "air" : substanceName(member);
}

/** A key's name in a note: a species by `substanceName`, a pair as its members' names. */
function keyName(key: string): string {
  const [first, second] = key.split("+");
  if (isPairKey(key) && first !== undefined && second !== undefined) {
    return `${memberName(first)}–${memberName(second)} pairs`;
  }
  return substanceName(key);
}

/** Whether a key is a pair's, its members joined by `+` (`O2+O2`, `H2O+air`). */
function isPairKey(key: string): boolean {
  const members = key.split("+");
  return members.length === 2 && members.every((m) => m.length > 0);
}

/** A key's possessive in a note: a pair's "their", a species' "its". */
function possessive(key: string): string {
  return isPairKey(key) ? "their" : "its";
}

/** The note of a key a newer server sent, which this client has no row for. */
function unknownReason(key: string): string {
  return `${keyName(key)}: not known to this console, not drawn`;
}

/** The note of an absorber that draws nothing for want of data. */
function noAbsorptionReason(key: string): string {
  return `${keyName(key)}: no absorption data on file, ${possessive(key)} absorption not drawn`;
}

/** The note of an absorber with estimated optics. */
function estimatedReason(key: string): string {
  return `${keyName(key)}: ${possessive(key)} absorption estimated, not measured`;
}

/**
 * The note of an absorber left out past the stored columns or the term limit, in the guide's words
 * (its `ATMOSPHERE: APPROXIMATE` row: "the console can draw no more layers of absorbing gas").
 */
function leftOutReason(key: string): string {
  return `${keyName(key)}: left out, the console can draw no more layers of absorbing gas`;
}

/** The note of Thomson scattering left out past the term limit. */
const THOMSON_LEFT_OUT_REASON = `${substanceName("e-")} scattering: left out, the console can draw no more layers`;

/**
 * A mode's particles in a note: their material's name with its phase word (`materials.ts`'s
 * `phasedName`), `H2O ice particles`, `liquid H2SO4 particles`, `solid Fe particles`.
 */
function particlesName(material: string, phase: MaterialPhase): string {
  return `${phasedName(material, phase)} particles`;
}

/**
 * The reasons of a mode whose Mie size quadrature did not converge in any channel or wavelength
 * drawn (R08.T5.f's `ModeOptics.sizeConvergence`): `atmosphereApproximate`'s, one a mode, naming
 * its particles by their phase (a water cloud's droplets and an ice cloud's crystals read apart)
 * and the first such wavelength in whole nm.
 *
 * @param optics - The mode's optics at every wavelength it is drawn at.
 */
export function sizeConvergenceReasons(
  material: string,
  phase: MaterialPhase,
  optics: ReadonlyArray<ModeOptics>,
): ReadonlyArray<string> {
  const unconverged = optics.find((o) => o.sizeConvergence?.converged === false);
  return unconverged === undefined
    ? []
    : [
        `${particlesName(material, phase)}: their sizes summed short of convergence at ${Math.round(unconverged.wavelengthNm)} nm`,
      ];
}

// ---------------------------------------------------------------------------------------------
// Where the body's air comes from, and the labels that says.

/** No medium, with the labels that say why. */
interface NoAir {
  readonly kind: "none";
  readonly labels: ReadonlyArray<AtmosphereLabel>;
}

/** A surface with air to draw, with its labels beside the reasons'. */
interface Air {
  readonly kind: "air";
  readonly record: BodyRecord;
  readonly surface: BodySurface;
  readonly bulk: BulkProperties;
  readonly labels: ReadonlyArray<AtmosphereLabel>;
}

/** The labels a section's withheld or unmodelled state gives, none for any other. */
function sectionLabels(section: Section<unknown>): ReadonlyArray<AtmosphereLabel> {
  let labels: ReadonlyArray<AtmosphereLabel>;
  switch (section.state) {
    case "not_resolved":
      labels = ["atmosphereNotResolved"];
      break;
    case "not_modelled":
      labels = ["atmosphereNotYetModelled"];
      break;
    case "ok":
    case "not_applicable":
      labels = [];
      break;
  }
  return labels;
}

/**
 * A gas envelope's labels: its own section's state, and `NOT YET MODELLED` while `ok`, since no
 * envelope is drawn before R08.T16.a (`decision-r08-t10a-leans.md` item 5).
 */
function envelopeLabels(envelope: Section<BodyEnvelope>): ReadonlyArray<AtmosphereLabel> {
  return envelope.state === "ok" ? ["atmosphereNotYetModelled"] : sectionLabels(envelope);
}

/** Whether a surface holds melt (Design note 12): `magma_ocean`, or a crust with a melt area. */
function holdsMelt(surface: BodySurface): boolean {
  return (
    surface.state === "magma_ocean" ||
    (surface.crust.kind === "value" && surface.crust.value.meltAreaFraction > 0)
  );
}

/** Where a record's air comes from. */
function airOf(detail: BodyDetail): NoAir | Air {
  const { record } = detail;
  const { surface } = record;
  if (surface.state === "not_applicable") {
    return { kind: "none", labels: envelopeLabels(record.envelope) };
  }
  if (surface.state !== "ok") {
    return { kind: "none", labels: sectionLabels(surface) };
  }
  const value = surface.value;
  const silicate: ReadonlyArray<AtmosphereLabel> =
    holdsMelt(value) && value.silicateVapour.kind === "not_modelled"
      ? ["silicateVapourNotYetModelled"]
      : [];
  if (value.state === "airless") {
    return { kind: "none", labels: ["exosphereNotYetModelled", ...silicate] };
  }
  if (value.gases.length === 0) {
    return { kind: "none", labels: silicate };
  }
  if (record.bulk.state !== "ok") {
    // The air cannot be laid on a body whose size is withheld or not modelled.
    return { kind: "none", labels: sectionLabels(record.bulk) };
  }
  const aerosols: ReadonlyArray<AtmosphereLabel> =
    value.aerosols.kind === "not_modelled" ? ["aerosolsNotYetModelled"] : [];
  return {
    kind: "air",
    record,
    surface: value,
    bulk: record.bulk.value,
    labels: [...aerosols, ...silicate],
  };
}

/** Labels in `ATMOSPHERE_LABELS`' order, each once. */
function ordered(labels: Iterable<AtmosphereLabel>): ReadonlyArray<AtmosphereLabel> {
  const held = new Set(labels);
  return ATMOSPHERE_LABELS.filter((label) => held.has(label));
}

// ---------------------------------------------------------------------------------------------
// The body, its column and its absorber layers.

/** A record's body: its figure (or a sphere of its bulk radius) and its gravity at the time. */
function bodyOf(record: BodyRecord, bulk: BulkProperties, detail: BodyDetail): AssembledBody {
  const figureSection = record.figure;
  const figure: SpheroidFigure =
    figureSection.state === "ok"
      ? {
          equatorialRadiusM: figureSection.value.equatorialRadiusM,
          polarRadiusM: figureSection.value.polarRadiusM,
        }
      : { equatorialRadiusM: bulk.radiusM, polarRadiusM: bulk.radiusM };
  const massKg =
    record.massMearth.state === "ok" ? record.massMearth.value * EARTH_MASS_KG : undefined;
  const angularVelocityRadS =
    record.rotation.state === "ok" && massKg !== undefined
      ? spinRateAt(record.rotation.value, detail.time)
      : null;
  const gravity = bodyGravity(
    {
      figure,
      massKg: massKg ?? Number.NaN,
      angularVelocityRadS,
      bulkGravityMS2: bulk.surfaceGravityMS2,
      figureLaw:
        figureSection.state === "ok"
          ? {
              law: figureSection.value.law,
              momentOfInertiaFactor: figureSection.value.momentOfInertiaFactor,
            }
          : null,
    },
    mediumTableRadiusM(figure),
  );
  return { figure, gravity };
}

/** A record's column, isothermal at its T_s: the seam until P14.T24.e's nodes. */
function columnOf(surface: BodySurface, body: AssembledBody): AtmosphereColumn {
  const meanMolarMassGPerMol = surface.meanMolarMassGPerMol;
  if (meanMolarMassGPerMol === null) {
    throw new RangeError("a surface with gases carries their mean molar mass");
  }
  return hydrostaticColumn({
    surfacePa: surface.surfacePressurePa,
    temperature: { kind: "isothermal", temperatureK: surface.meanSurfaceTemperatureK },
    meanMolarMassGPerMol,
    referenceGravityMS2: body.gravity.referenceGravityMS2,
    referenceRadiusM: mediumTableRadiusM(body.figure),
  });
}

/** A column's datum and top pressures, Pa. */
function columnEnds(column: AtmosphereColumn): {
  readonly datumPa: number;
  readonly topPa: number;
} {
  return {
    datumPa: column.pressuresPa[0] ?? Number.NaN,
    topPa: column.pressuresPa[column.pressuresPa.length - 1] ?? Number.NaN,
  };
}

/** A reason, shown while less than the split's depth lies above `topPa` (Design note 12's rule). */
interface PendingReason {
  /** One reason a slot: a kind and a key, so that a key's two facts are both kept. */
  readonly slot: string;
  readonly reason: string;
  /** The top of what it concerns, Pa; the column's top for the whole column. */
  readonly topPa: number;
}

/** Collects reasons, the first given for each slot kept. */
class Reasons {
  readonly #bySlot = new Map<string, PendingReason>();

  add(slot: string, reason: string, topPa: number): void {
    if (!this.#bySlot.has(slot)) {
      this.#bySlot.set(slot, { slot, reason, topPa });
    }
  }

  /** Sets a slot's reason over any given before: what is left out is said over how it is drawn. */
  replace(slot: string, reason: string, topPa: number): void {
    this.#bySlot.set(slot, { slot, reason, topPa });
  }

  has(slot: string): boolean {
    return this.#bySlot.has(slot);
  }

  all(): ReadonlyArray<PendingReason> {
    return [...this.#bySlot.values()];
  }
}

/** The lowest pressure of a deck: the least top of the modes that name it, Pa. */
function deckTopPa(inventory: AerosolInventory, condensate: string): number | undefined {
  const tops = inventory.modes.filter((m) => m.deck === condensate).map((m) => m.topPa);
  return tops.length === 0 ? undefined : Math.min(...tops);
}

/** A body's absorber layers and the reasons laying them gave. */
interface Layers {
  readonly layers: ReadonlyArray<AbsorberLayer>;
  /** Each listed species' top on its column, Pa, for the τ rule. */
  readonly topsPa: ReadonlyMap<string, number>;
}

/**
 * The absorber layers of a surface's gases and inventory on its column: each gas from p_s up at
 * its mole fraction (to its deck's top where its vapour condenses), and each of the inventory's
 * absorbers on its own layer, which replaces a gas's of the same species. A record layer wholly
 * above the column's top is dropped with a reason, one wholly below its datum without
 * (`decision-r08-t10a-leans.md` item 11); one that straddles the column passes, `absorberSpecs`
 * laying its whole column inside.
 */
function layersOf(surface: BodySurface, column: AtmosphereColumn, reasons: Reasons): Layers {
  const { datumPa, topPa: columnTopPa } = columnEnds(column);
  const inventory = surface.aerosols.kind === "value" ? surface.aerosols.value : undefined;
  const recordAbsorbers = inventory?.absorbers ?? [];
  const recorded = new Set(recordAbsorbers.map((a) => a.species));
  const numberColumnPerM2 = (topPa: number): number =>
    column.surfaceNumberDensityPerM3 * layerMoments(column, datumPa, topPa, 1).integralM;
  const layers: AbsorberLayer[] = [];
  const topsPa = new Map<string, number>();
  for (const gas of surface.gases) {
    if (recorded.has(gas.species)) {
      continue;
    }
    const deck = inventory?.decks.find((d) => d.condensate === gas.species);
    const deckTop =
      inventory === undefined || deck === undefined ? undefined : deckTopPa(inventory, gas.species);
    const cut = deckTop !== undefined && deckTop > columnTopPa && deckTop < datumPa;
    const topPa = cut ? deckTop : 0;
    layers.push({
      species: gas.species,
      columnPerM2: gas.moleFraction * numberColumnPerM2(topPa),
      basePa: datumPa,
      topPa,
    });
    topsPa.set(gas.species, Math.max(topPa, columnTopPa));
    if (cut && deck !== undefined && deck.vapourMoleFractionAbove > 0) {
      reasons.add(
        `vapour:${gas.species}`,
        `${substanceName(gas.species)}: its vapour above its cloud deck, not drawn`,
        deckTop,
      );
    }
  }
  const aboveReason = (species: string): void => {
    reasons.add(
      `layer:${species}`,
      `${substanceName(species)}: its layer lies above the air drawn, not drawn`,
      columnTopPa,
    );
  };
  for (const absorber of recordAbsorbers) {
    // A layer of no column, or of base equal to its top (legal on the wire, as a mode's is), has
    // nothing to draw; `absorberSpecs` refuses the second.
    if (absorber.columnPerM2 === 0 || absorber.basePa === absorber.topPa) {
      continue;
    }
    if (absorber.basePa <= columnTopPa) {
      aboveReason(absorber.species);
      continue;
    }
    if (absorber.topPa >= datumPa) {
      continue;
    }
    const profile = absorber.profile?.map((node) => ({
      pressurePa: node.pressurePa,
      mixingRatio: node.relativeMixingRatio,
    }));
    // A profile whose mixing ratio is 0 wherever the column is: its absorber lies above the column's
    // top, or below the ground; `absorberSpecs` refuses it.
    if (
      profile !== undefined &&
      !(layerMoments(column, absorber.basePa, absorber.topPa, 1, profile).integralM > 0)
    ) {
      if (absorber.topPa < columnTopPa) {
        aboveReason(absorber.species);
      }
      continue;
    }
    layers.push({
      species: absorber.species,
      columnPerM2: absorber.columnPerM2,
      basePa: absorber.basePa,
      topPa: absorber.topPa,
      ...(profile === undefined ? {} : { profile }),
    });
    topsPa.set(absorber.species, Math.max(absorber.topPa, columnTopPa));
  }
  return { layers, topsPa };
}

/** A key's top for the τ rule: a species' layer's, a pair's higher member's, else the column's. */
function keyTopPa(key: string, topsPa: ReadonlyMap<string, number>, columnTopPa: number): number {
  const members = isPairKey(key) ? key.split("+") : [key];
  const tops = members.flatMap((m) => {
    const top = topsPa.get(m);
    return top === undefined ? [] : [top];
  });
  return tops.length === 0 ? columnTopPa : Math.max(...tops);
}

/** The Rayleigh term's approximations as reasons, a registry `rayleigh: false` absorber's dropped. */
function rayleighReasons(
  approximations: ReadonlyArray<RayleighApproximation>,
  columnTopPa: number,
  reasons: Reasons,
): void {
  for (const { species, provenance } of approximations) {
    const flags = REGISTRY_OPTICS_FLAGS.get(species);
    if (flags === undefined) {
      reasons.add(`unknown:${species}`, unknownReason(species), columnTopPa);
      continue;
    }
    switch (provenance) {
      case "estimated":
        reasons.add(
          `scattering:${species}`,
          `${substanceName(species)}: its scattering estimated, not measured`,
          columnTopPa,
        );
        break;
      case "none":
        // e⁻, H⁻ and the ions scatter by no Rayleigh row: their optics are other terms' (Thomson,
        // their continua and lines), which report their own.
        if (!(flags.absorber && !flags.rayleigh)) {
          reasons.add(
            `scattering:${species}`,
            `${substanceName(species)}: no scattering data on file, its scattering not drawn`,
            columnTopPa,
          );
        }
        break;
    }
  }
}

/** Each key's verdicts of the negligible-absorbers bound. */
function verdictsByKey(
  verdicts: ReadonlyArray<NegligibleVerdict>,
): ReadonlyMap<string, ReadonlyArray<NegligibleVerdict>> {
  const byKey = new Map<string, NegligibleVerdict[]>();
  for (const verdict of verdicts) {
    const list = byKey.get(verdict.key) ?? [];
    list.push(verdict);
    byKey.set(verdict.key, list);
  }
  return byKey;
}

/** Whether every one of a key's verdicts is negligible. */
function allNegligible(verdicts: ReadonlyArray<NegligibleVerdict> | undefined): boolean {
  return verdicts !== undefined && verdicts.length > 0 && verdicts.every((v) => v.negligible);
}

/** The absorber specs' approximations and the bound's verdicts as reasons. */
function absorberReasons(
  approximations: ReadonlyArray<AbsorberApproximation>,
  byKey: ReadonlyMap<string, ReadonlyArray<NegligibleVerdict>>,
  drawnKeys: ReadonlySet<string>,
  topOf: (key: string) => number,
  reasons: Reasons,
): void {
  for (const { species: key, reason } of approximations) {
    const flags = isPairKey(key) ? undefined : REGISTRY_OPTICS_FLAGS.get(key);
    if (!isPairKey(key) && flags === undefined) {
      reasons.add(`unknown:${key}`, unknownReason(key), topOf(key));
      continue;
    }
    // A bulk gas the registry asks no absorption of (Ar) is listed for the pairs alone, and
    // draws nothing whether its profile takes a column or not.
    if (flags?.absorber === false) {
      continue;
    }
    switch (reason) {
      case "noCrossSections":
        // A wholly pending species has no optics, so `absorberSpecs` gives it no column and no
        // curve already; the bound finds whether leaving it undrawn needs a note.
        if (!allNegligible(byKey.get(key))) {
          reasons.add(`absorption:${key}`, noAbsorptionReason(key), topOf(key));
        }
        break;
      case "noColumnFree":
        reasons.replace(`absorption:${key}`, leftOutReason(key), topOf(key));
        break;
      case "temperatureOutsideTable":
        if (!allNegligible(byKey.get(key))) {
          reasons.add(
            `absorption:${key}`,
            `${keyName(key)}: ${possessive(key)} absorption held at the nearest temperature on file`,
            topOf(key),
          );
        }
        break;
      case "estimatedOptics":
        if (!allNegligible(byKey.get(key))) {
          reasons.add(`absorption:${key}`, estimatedReason(key), topOf(key));
        }
        break;
    }
  }
  for (const [key, verdicts] of byKey) {
    if (reasons.has(`absorption:${key}`) || reasons.has(`unknown:${key}`)) {
      continue;
    }
    for (const verdict of verdicts) {
      if (verdict.negligible) {
        continue;
      }
      let text: string;
      switch (verdict.test) {
        case "absorption":
          text = !drawnKeys.has(key)
            ? noAbsorptionReason(key)
            : verdict.entries.some((entry) => entry.origin === "pending")
              ? `${keyName(key)}: part of ${possessive(key)} absorption has no data on file, not drawn`
              : estimatedReason(key);
          break;
        case "heldChi":
        case "fixedGMix":
        case "broadener":
          text = estimatedReason(key);
          break;
      }
      reasons.add(`absorption:${key}`, text, topOf(key));
      break;
    }
  }
}

/** The keys a body's specs draw: each column spec's species and each pair spec's pairs. */
function drawnKeysOf(specs: ReadonlyArray<AbsorberSpec>): ReadonlySet<string> {
  const keys = new Set<string>();
  for (const spec of specs) {
    switch (spec.kind) {
      case "column":
        for (const s of spec.species) {
          keys.add(s.species);
        }
        break;
      case "pair":
        for (const p of spec.pairs) {
          keys.add(p.pair);
        }
        break;
    }
  }
  return keys;
}

// ---------------------------------------------------------------------------------------------
// The aerosol modes.

/** The wire's shape class in the client's words. */
function shapeClassOf(shape: AerosolShapeDto): ShapeClass {
  let shapeClass: ShapeClass;
  switch (shape) {
    case "sphere":
      shapeClass = "sphere";
      break;
    case "non_spherical_mineral":
      shapeClass = "nonSphericalMineral";
      break;
    case "crystal":
      shapeClass = "crystal";
      break;
    case "aggregate":
      shapeClass = "aggregate";
      break;
  }
  return shapeClass;
}

/** µm per m: the wire's radii are in metres, the optics' in micrometres. */
const UM_PER_M = 1e6;

/**
 * A mode's particles as the optics take them: an aggregate of its monomers, or a size distribution
 * of its material and shape. A distribution sent with the `aggregate` shape takes its file's
 * shape, since its sizes are no aggregate's.
 */
function particlesOf(mode: RecordAerosolMode, sizes: RecordSizeDistribution): AerosolParticles {
  const form = { phase: mode.phase };
  let particles: AerosolParticles;
  switch (sizes.kind) {
    case "aggregate":
      particles = {
        kind: "aggregate",
        mode: {
          material: mode.material,
          form,
          monomerRadiusUm: sizes.monomerRadiusM * UM_PER_M,
          monomerCount: sizes.monomerCount,
          fractalDimension: sizes.fractalDimension,
          fractalPrefactor: sizes.fractalPrefactor,
        },
      };
      break;
    case "gamma":
    case "log_normal": {
      const distribution: SizeDistribution = {
        kind: sizes.kind === "gamma" ? "gamma" : "logNormal",
        effectiveRadiusUm: sizes.effectiveRadiusM * UM_PER_M,
        effectiveVariance: sizes.effectiveVariance,
      };
      const shape = shapeClassOf(mode.shape);
      particles = {
        kind: "distribution",
        mode: {
          material: mode.material,
          form,
          ...(shape === "aggregate" ? {} : { shape }),
          sizes: distribution,
        },
      };
      break;
    }
  }
  return particles;
}

/** A mode of the inventory that is drawn: its index, its term's name and its layer on the column. */
interface PlannedMode {
  readonly index: number;
  readonly mode: RecordAerosolMode;
  readonly spec: AerosolModeSpec;
}

/**
 * The note of a mode whose particles lie outside the optics on file (`aerosol.ts`'s
 * `aerosolTermInRange`): sizes past Mie's reach, a non-spherical table's largest size or the MMF's
 * monomer, or an aggregate's fractal dimension outside the MMF's range.
 */
function outsideOpticsReason(mode: RecordAerosolMode, particles: AerosolParticles): string {
  const name = particlesName(mode.material, mode.phase);
  return particles.kind === "aggregate" && !aggregateShapeInRange(particles.mode)
    ? `${name}: their fractal dimension lies outside the optics on file, not drawn`
    : `${name}: their sizes lie outside the optics on file, not drawn`;
}

/**
 * The inventory's modes to draw: each of τ above 0 and base below its top, inside the column,
 * whose particles the optics on file take. A mode of τ 0 or of base equal to top is skipped
 * (review-r08-shapes-b C3: legal on the wire); one wholly below the datum is hidden by the ground;
 * one wholly above the column's top is dropped and reported; one whose sizes or optical depth the
 * generator does not model is dropped and reported (P14.T54.b); and one whose modelled particles
 * lie outside its optics' range at a wavelength the term is drawn at (`aerosolTermInRange`: Mie's reach, a
 * non-spherical table's largest size, the MMF's monomer and fractal dimension), which a legal
 * record may hold and which `aerosolTerm` would refuse, is dropped and reported under the τ rule
 * (R08.T10.b), so that only a bug raises the computation's fault (`decision-r08-t10a-leans.md`
 * item 6). {@link assembleMedium} and {@link absorberSpecsOf} both take the modes from here, before
 * the term limit ranks them.
 */
function modesOf(
  inventory: AerosolInventory | undefined,
  column: AtmosphereColumn,
  reasons: Reasons,
): ReadonlyArray<PlannedMode> {
  const { datumPa, topPa: columnTopPa } = columnEnds(column);
  const planned: PlannedMode[] = [];
  const names = new Map<string, number>();
  for (const [index, mode] of (inventory?.modes ?? []).entries()) {
    const sizes = mode.sizeDistribution;
    const depth = mode.opticalDepth;
    const tau550 = depth.kind === "value" ? depth.value.tau550 : undefined;
    if (tau550 === 0 || mode.basePa === mode.topPa || mode.topPa >= datumPa) {
      continue;
    }
    if (mode.basePa <= columnTopPa) {
      reasons.add(
        `mode:${index}`,
        `${particlesName(mode.material, mode.phase)} lie above the air drawn, not drawn`,
        columnTopPa,
      );
      continue;
    }
    const topPa = Math.max(mode.topPa, columnTopPa);
    // The optics need both, and a member the generator does not model is labelled, never stood in.
    if (sizes.kind === "not_modelled" || tau550 === undefined) {
      reasons.add(
        `mode:${index}`,
        `${particlesName(mode.material, mode.phase)}: their ${sizes.kind === "not_modelled" ? "sizes" : "optical depth"} not modelled, not drawn`,
        topPa,
      );
      continue;
    }
    // A modelled distribution outside its optics' range is dropped under the τ rule (R08.T10.b).
    const particles = particlesOf(mode, sizes.value);
    if (!aerosolTermInRange(particles)) {
      reasons.add(`mode:${index}`, outsideOpticsReason(mode, particles), topPa);
      continue;
    }
    const seen = (names.get(mode.material) ?? 0) + 1;
    names.set(mode.material, seen);
    planned.push({
      index,
      mode,
      spec: {
        name: seen === 1 ? `aerosol:${mode.material}` : `aerosol:${mode.material}:${seen}`,
        particles,
        opticalDepth550: tau550,
        basePa: mode.basePa,
        topPa,
        mixingExponent: mode.fSed,
      },
    });
  }
  return planned;
}

// ---------------------------------------------------------------------------------------------
// The plan: everything but the aerosols' optics.

/** A term that competes for the medium's places past the molecular term. */
type Candidate =
  | { readonly kind: "thomson"; readonly layer: AbsorberLayer }
  | { readonly kind: "absorber"; readonly spec: AbsorberSpec }
  | { readonly kind: "mode"; readonly planned: PlannedMode };

/** A candidate's vertical depth, by which the term limit ranks it. */
function candidateDepth(candidate: Candidate): number {
  let depth: number;
  switch (candidate.kind) {
    case "thomson":
      depth = THOMSON_CROSS_SECTION_M2 * candidate.layer.columnPerM2;
      break;
    case "absorber":
      depth = absorberRankingDepth(candidate.spec);
      break;
    case "mode":
      depth = candidate.planned.spec.opticalDepth550;
      break;
  }
  return depth;
}

/** A body's medium as planned, before its aerosols' optics. */
interface Plan {
  readonly air: Air;
  readonly body: AssembledBody;
  readonly column: AtmosphereColumn;
  readonly spectra: ReadonlyArray<BakeSpectrum>;
  readonly molecular: MediumTerm;
  readonly thomson: AbsorberLayer | undefined;
  readonly specs: ReadonlyArray<AbsorberSpec>;
  readonly modes: ReadonlyArray<PlannedMode>;
  readonly reasons: Reasons;
  readonly negligible: ReadonlyArray<string>;
}

/** The light slots' spectra, at most {@link LIGHT_SLOT_LIMIT}. */
function spectraOf(lights: ReadonlyArray<BodyLight>): ReadonlyArray<BakeSpectrum> {
  if (lights.length > LIGHT_SLOT_LIMIT) {
    throw new RangeError(
      `a body's medium takes at most ${LIGHT_SLOT_LIMIT} light slots, not ${lights.length}`,
    );
  }
  return lights.map((light) => light.spectrum);
}

/** Everything of a body's medium but its aerosols' optics (build steps 1–6 and the term limit). */
function planOf(
  detail: BodyDetail,
  air: Air,
  lights: ReadonlyArray<BodyLight>,
  options: AssembleOptions,
): Plan {
  const spectra = spectraOf(lights);
  const { record, surface, bulk } = air;
  const body = bodyOf(record, bulk, detail);
  const column = columnOf(surface, body);
  const columnTopPa = columnEnds(column).topPa;
  const reasons = new Reasons();
  const molecular = molecularTerm(column, surface.gases);
  rayleighReasons(molecular.approximations, columnTopPa, reasons);
  const { layers, topsPa } = layersOf(surface, column, reasons);
  const kDistributions = options.kDistributions ?? [];
  const bulkMoleFractions: ReadonlyMap<string, number> = new Map(
    surface.gases.map((g) => [g.species, g.moleFraction]),
  );
  // The layers are never filtered by the bound: a wholly pending species has no optics, so it
  // takes no column and no curve here already, and a species with a pending part keeps its
  // measured data, a bulk gas its pairs. The medium drawn is the one the bound judged.
  const absorbers: AbsorberSpecs = absorberSpecs(
    layers,
    column,
    ABSORBER_CROSS_SECTIONS,
    ABSORBER_PAIR_CROSS_SECTIONS,
    { kDistributions, bulkMoleFractions },
  );
  const { specs, approximations } = absorbers;
  const verdicts =
    spectra.length === 0
      ? []
      : negligibleAbsorbers(
          { column, layers, bulkMoleFractions, suns: spectra, kDistributions },
          { specs: absorbers },
        );
  const byKey = verdictsByKey(verdicts);
  const negligible = [...byKey].filter(([, v]) => allNegligible(v)).map(([key]) => key);
  absorberReasons(
    approximations,
    byKey,
    drawnKeysOf(specs),
    (key) => keyTopPa(key, topsPa, columnTopPa),
    reasons,
  );
  const electrons = layers.find((l) => l.species === "e-" && l.columnPerM2 > 0);
  const inventory = surface.aerosols.kind === "value" ? surface.aerosols.value : undefined;
  const modes = modesOf(inventory, column, reasons);

  // The term limit: the molecular term, then the deepest of the rest, the earlier on a tie.
  const candidates: Candidate[] = [
    ...(electrons === undefined ? [] : [{ kind: "thomson" as const, layer: electrons }]),
    ...specs.map((spec) => ({ kind: "absorber" as const, spec })),
    ...modes.map((planned) => ({ kind: "mode" as const, planned })),
  ];
  const places = MEDIUM_TERM_LIMIT - 1;
  const chosen =
    candidates.length <= places
      ? new Set(candidates)
      : new Set(
          candidates
            .map((candidate, index) => ({ candidate, index, depth: candidateDepth(candidate) }))
            .toSorted((a, b) => (b.depth === a.depth ? a.index - b.index : b.depth - a.depth))
            .slice(0, places)
            .map(({ candidate }) => candidate),
        );
  for (const candidate of candidates) {
    if (chosen.has(candidate)) {
      continue;
    }
    switch (candidate.kind) {
      case "thomson":
        reasons.add("left:thomson", THOMSON_LEFT_OUT_REASON, keyTopPa("e-", topsPa, columnTopPa));
        break;
      case "absorber":
        for (const key of drawnKeysOf([candidate.spec])) {
          if (REGISTRY_OPTICS_FLAGS.get(key)?.absorber !== false) {
            reasons.replace(
              `absorption:${key}`,
              leftOutReason(key),
              keyTopPa(key, topsPa, columnTopPa),
            );
          }
        }
        break;
      case "mode": {
        const { mode, index, spec } = candidate.planned;
        reasons.add(
          `mode:${index}`,
          `${particlesName(mode.material, mode.phase)}: left out, the console can draw no more layers`,
          spec.topPa,
        );
        break;
      }
    }
  }
  return {
    air,
    body,
    column,
    spectra,
    molecular: molecular.term,
    thomson: candidates.some((c) => c.kind === "thomson" && chosen.has(c)) ? electrons : undefined,
    specs: candidates.flatMap((c) => (c.kind === "absorber" && chosen.has(c) ? [c.spec] : [])),
    modes: candidates.flatMap((c) => (c.kind === "mode" && chosen.has(c) ? [c.planned] : [])),
    reasons,
    negligible,
  };
}

// ---------------------------------------------------------------------------------------------
// The τ rule.

/** The height of a pressure on a column, m: linear in ln p between its levels, 0 below the datum. */
function heightAtPa(column: AtmosphereColumn, pressurePa: number): number {
  const { pressuresPa, altitudesM } = column;
  const last = pressuresPa.length - 1;
  if (pressurePa >= (pressuresPa[0] ?? Number.NaN)) {
    return 0;
  }
  if (pressurePa <= (pressuresPa[last] ?? Number.NaN)) {
    return column.topHeightM;
  }
  let i = 0;
  while (i < last - 1 && (pressuresPa[i + 1] ?? Number.NaN) > pressurePa) {
    i += 1;
  }
  const p0 = pressuresPa[i] ?? Number.NaN;
  const p1 = pressuresPa[i + 1] ?? Number.NaN;
  const t = Math.log(p0 / pressurePa) / Math.log(p0 / p1);
  const z0 = altitudesM[i] ?? Number.NaN;
  const z1 = altitudesM[i + 1] ?? Number.NaN;
  return z0 + (z1 - z0) * t;
}

/** One scattering term's share of the τ rule: its extinction at 550 nm at relative density 1. */
interface DepthTerm {
  readonly density: DensityProfile;
  readonly extinctionPerM: number;
}

/**
 * The vertical optical depth at 550 nm above a pressure (Design note 12's τ rule), over the
 * molecular, Thomson and aerosol terms: each term's extinction times ∫ ρ dh from the pressure's
 * height to the medium's top.
 */
function depthAbove(
  column: AtmosphereColumn,
  topHeightM: number,
  terms: ReadonlyArray<DepthTerm>,
): (pressurePa: number) => number {
  const totals = terms.map((t) => columnLengthM(t.density, topHeightM));
  return (pressurePa) => {
    const heightM = Math.min(heightAtPa(column, pressurePa), topHeightM);
    let depth = 0;
    for (const [i, term] of terms.entries()) {
      depth +=
        term.extinctionPerM * ((totals[i] ?? Number.NaN) - columnLengthM(term.density, heightM));
    }
    return Math.max(depth, 0);
  };
}

// ---------------------------------------------------------------------------------------------
// The assembly.

/** No medium, with its labels. */
function noMedium(labels: ReadonlyArray<AtmosphereLabel>): AssembledMedium {
  return {
    medium: undefined,
    drawn: undefined,
    labels: ordered(labels),
    reasons: [],
    negligible: [],
  };
}

/** The phase reasons of a mode, its substance's name first where a note does not begin with it. */
function namedFirst(material: string, reason: string): string {
  const name = substanceName(material);
  return reason.startsWith(name) ? reason : `${name}: ${reason}`;
}

/**
 * A body's medium and labels from its record, or a kept scene's body's labels (the Provides'
 * `assembleMedium`; the module's remarks).
 *
 * @remarks
 * Pure and synchronous: the optics worker calls it once a body and inventory, with the line
 * species' tables loaded ({@link lineSpeciesOf}), and computes the curves after
 * ({@link absorberCurveSlots}, or the pool's shares and {@link withCurves}). A legal record never
 * makes it throw: a throw is a bug, which the cache turns into the computation's fault
 * (`decision-r08-t10a-leans.md` item 6).
 *
 * @throws RangeError for more lights than {@link LIGHT_SLOT_LIMIT}, a ground albedo not finite, or
 *   a record that breaks the wire's own rules or no body obeys: gases that do not sum to 1; a
 *   surface with gases and no mean molar mass; an absorber listed twice, with a base above its top
 *   or with a profile that does not run from its base to its top; a `rotational` figure spun past
 *   its break-up, where its equator's normal gravity is not above 0 (`oblate.ts`'s
 *   `oblateSlicing`, which the generator's `capped` law stays clear of); or a column no
 *   hydrostatic atmosphere holds under the isothermal seam, k T_s ln 10⁷ ≥ μ m_u g_ref R_ref.
 */
export function assembleMedium(
  input: BodyAtmosphereInput,
  options: AssembleOptions = {},
): AssembledMedium {
  if (input.kind === "kept") {
    let labels: ReadonlyArray<AtmosphereLabel>;
    switch (input.atmosphere.kind) {
      case "unset":
        labels = ["atmosphereNotYetModelled"];
        break;
      case "airless":
        labels = [];
        break;
    }
    return noMedium(labels);
  }
  const groundAlbedo = clampedGroundAlbedo(input.groundAlbedo);
  const air = airOf(input.detail);
  if (air.kind === "none") {
    spectraOf(input.lights);
    return noMedium(air.labels);
  }
  const plan = planOf(input.detail, air, input.lights, options);
  const { column, reasons } = plan;
  const aerosols = plan.modes.map((planned) => ({
    planned,
    term: aerosolTerm(planned.spec, column),
  }));
  const thomson = plan.thomson === undefined ? undefined : thomsonTerm(plan.thomson, column);
  const lines = plan.specs.some(
    (spec) => spec.kind === "column" && spec.species.some((s) => s.lines !== undefined),
  );
  const medium: AtmosphereMedium = {
    name: plan.air.record.designation,
    topHeightM: column.topHeightM,
    groundAlbedo,
    terms: [
      plan.molecular,
      ...(thomson === undefined ? [] : [thomson]),
      ...plan.specs.map((spec) => absorberTerm(spec, column)),
      ...aerosols.map(({ term }) => term.term),
    ],
    ...(lines ? { pressureBar: columnPressureBar(column) } : {}),
  };
  checkTabulatedTops(medium);

  const sigma550M2 = molecularMixture(plan.air.surface.gases).crossSectionM2(TAU_WAVELENGTH_NM);
  const above = depthAbove(column, medium.topHeightM, [
    { density: column.density, extinctionPerM: sigma550M2 * column.surfaceNumberDensityPerM3 },
    ...(thomson === undefined
      ? []
      : [{ density: thomson.density, extinctionPerM: thomson.scattering[1] }]),
    ...aerosols.map(({ planned, term }) => ({
      density: term.term.density,
      extinctionPerM:
        planned.spec.opticalDepth550 / columnLengthM(term.term.density, column.topHeightM),
    })),
  ]);
  const shown: string[] = [];
  for (const { reason, topPa } of reasons.all()) {
    if (above(topPa) < CLOUD_DECK_SPLIT_OPTICAL_DEPTH) {
      shown.push(reason);
    }
  }
  for (const { planned, term } of aerosols) {
    const { material, phase } = planned.mode;
    const opticalDepthAbove550 = above(planned.spec.topPa);
    for (const { reason } of materialApproximations([
      { material, form: { phase }, opticalDepthAbove550 },
    ])) {
      shown.push(reason);
    }
    for (const { reason } of phaseApproximations([
      { material, optics: term.channels, opticalDepthAbove550 },
    ])) {
      shown.push(namedFirst(material, reason));
    }
    if (opticalDepthAbove550 < CLOUD_DECK_SPLIT_OPTICAL_DEPTH) {
      shown.push(...sizeConvergenceReasons(material, phase, [...term.channels, term.at550]));
    }
  }
  if (provisionalThick(medium)) {
    shown.push(THICK_REASON);
  }
  const reasonsShown = [...new Set(shown)];
  const approximate: ReadonlyArray<AtmosphereLabel> =
    reasonsShown.length > 0 ? ["atmosphereApproximate"] : [];
  return {
    medium,
    drawn: {
      body: plan.body,
      bottomRadiusM: mediumTableRadiusM(plan.body.figure),
      absorbers: plan.specs,
      spectra: plan.spectra,
      curves: [],
    },
    labels: ordered([...plan.air.labels, ...approximate]),
    reasons: reasonsShown,
    negligible: plan.negligible,
  };
}

/**
 * A body's absorber specs alone, as {@link assembleMedium}'s {@link DrawnMedium.absorbers} gives
 * them, the term limit applied, with no aerosol optics (no Mie): for a curve-share job on an optics
 * worker that has not assembled the body.
 *
 * @throws RangeError as {@link assembleMedium}: for more lights than {@link LIGHT_SLOT_LIMIT}, or a
 *   record that breaks the wire's rules (gases that do not sum to 1, a surface with gases and no
 *   mean molar mass, an absorber listed twice or ill-formed), a `rotational` figure spun past its
 *   break-up, or an unbound isothermal column.
 */
export function absorberSpecsOf(
  input: BodyAtmosphereInput,
  options: AssembleOptions = {},
): AssembledAbsorbers {
  if (input.kind === "kept") {
    return { specs: [], spectra: [] };
  }
  const air = airOf(input.detail);
  if (air.kind === "none") {
    return { specs: [], spectra: spectraOf(input.lights) };
  }
  const plan = planOf(input.detail, air, input.lights, options);
  return { specs: plan.specs, spectra: plan.spectra };
}

/**
 * The line species an input's medium lists that the client holds committed k-distribution tables
 * for (`absorbers/kDistribution.ts`'s `committedLineSpecies`), ascending: what the optics worker
 * loads before {@link assembleMedium}. Empty for an input that draws no medium.
 */
export function lineSpeciesOf(input: BodyAtmosphereInput): ReadonlyArray<string> {
  if (input.kind === "kept") {
    return [];
  }
  const air = airOf(input.detail);
  if (air.kind === "none") {
    return [];
  }
  const { surface } = air;
  const listed = new Set([
    ...surface.gases.map((g) => g.species),
    ...(surface.aerosols.kind === "value"
      ? surface.aerosols.value.absorbers.map((a) => a.species)
      : []),
  ]);
  return committedLineSpecies().filter((species) => listed.has(species));
}

/**
 * Each light slot's channel curves of a drawn medium's absorbers, computed whole on one thread
 * (`absorbers.ts`'s `absorberCurve`): the curves {@link withCurves} takes where no worker pool
 * shares them.
 */
export function absorberCurveSlots(drawn: DrawnMedium): CurveSlots {
  return drawn.spectra.map((spectrum) =>
    drawn.absorbers.map((absorber) => absorberCurve(absorber, spectrum)),
  );
}

/**
 * The light slots' curves from each absorber's curves by light, as `absorbers.ts`'s
 * `absorberCurvesBySun` or `absorberCurvesFromSums` give them: the worker pool's join.
 *
 * @param perAbsorber - One entry an absorber in {@link DrawnMedium.absorbers}' order, each one a
 *   light in slot order.
 * @throws RangeError for an absorber whose curves are not one a light.
 */
export function curveSlotsOf(
  perAbsorber: ReadonlyArray<ReadonlyArray<{ readonly channels: AbsorberCurve }>>,
  lightCount: number,
): CurveSlots {
  for (const [k, curves] of perAbsorber.entries()) {
    if (curves.length !== lightCount) {
      throw new RangeError(`absorber ${k} has ${curves.length} lights' curves, not ${lightCount}`);
    }
  }
  return Array.from({ length: lightCount }, (_, j) =>
    perAbsorber.map((curves) => {
      const curve = curves[j]?.channels;
      if (curve === undefined) {
        throw new RangeError(`absorber curves for light slot ${j} are missing`);
      }
      return curve;
    }),
  );
}

/**
 * An assembled medium with its curves.
 *
 * @throws RangeError for curves given to no medium, or not one slot a light and one curve an
 *   absorber of the kind of its spec.
 */
export function withCurves(assembled: AssembledMedium, curves: CurveSlots): AssembledMedium {
  if (assembled.medium === undefined) {
    if (curves.length > 0) {
      throw new RangeError("curves are given to a body that draws no medium");
    }
    return assembled;
  }
  const { drawn } = assembled;
  if (curves.length !== drawn.spectra.length) {
    throw new RangeError(
      `a medium of ${drawn.spectra.length} light slots takes ${curves.length} slots of curves`,
    );
  }
  for (const [j, slot] of curves.entries()) {
    const fits =
      slot.length === drawn.absorbers.length &&
      slot.every((curve, k) => curve.kind === drawn.absorbers[k]?.kind);
    if (!fits) {
      throw new RangeError(
        `light slot ${j}'s curves do not match the medium's ${drawn.absorbers.length} absorbers`,
      );
    }
  }
  return { ...assembled, drawn: { ...drawn, curves } };
}
