/**
 * A gas envelope's medium, its pure parts (plan R08, R08.T16.a; Design notes 9, 13 and 17;
 * `decision-deep-decks-split.md` §3.3): the profile its column is built on, each mode's share of its
 * τ above a pressure, the cut of its inventory at its floor, the levels its modes' ends and steep
 * profiles take, and the datum its column stands on.
 *
 * @remarks
 * An envelope has no ground, so its medium is always split at its floor, the shallowest level at
 * which τ(550) from the top, the gas's and every mode's at its coverage by its published profile,
 * reaches `CLOUD_DECK_SPLIT_OPTICAL_DEPTH`; `assemble.ts` finds it on the column and builds the
 * medium from the inventory cut there ({@link cutAtFloor}).
 *
 * - **A mode's profile.** With one size a deck, a mode's τ is spread as its column mass is, as
 *   (p ÷ p_b)^f dp between its top p_t and its base p_b, f its `fSed`; so the share of it above p
 *   is ((p ÷ p_b)^(f+1) − (p_t ÷ p_b)^(f+1)) ÷ (1 − (p_t ÷ p_b)^(f+1)) ({@link shareAbove}).
 * - **The column** runs deep enough that T15's deck reads the medium below the floor to τ 1,000
 *   from the top (`thick/deck.ts`' `DECK_FLOOR_OPTICAL_DEPTH`), and up past every mode it carries
 *   (the giants' soot lies at 0.1–0.27 Pa). Its levels lie at each mode's ends, the cut's among
 *   them, within {@link MODE_EDGE_LN_P} of a level beyond each end, so that a term laid on them
 *   (`aerosol.ts`' `aerosolTerm`, which ramps a layer to 0 over the interval past each end) holds
 *   its τ inside its ends; and through a steep profile, a sheet of f in the hundreds, at
 *   {@link STEEP_PROFILE_STEP} e-folds of its τ's density a level ({@link modeLevelsPa}).
 * - **The datum.** Plan 14's figure of a giant is its 1-bar level spheroid (Design note 17, "Gas
 *   giants"), and the column's base lies far below it, so the medium stands on that figure lowered
 *   by the 1-bar level's h\* ({@link loweredFigure}), as `thick/deckRead.ts`' `deckDatumOf` raises
 *   a deck's.
 */

import type { AerosolInventory, AerosolMode, BodyEnvelope } from "../../lib/system/model";
import { exp, log, pow } from "../../lib/pinnedMath";
import type { EnvelopeProfile } from "./column";
import { type BodyGravity, gravityRatio, type SpheroidFigure } from "./oblate";

/**
 * The pressure of a gas envelope's datum, Pa: 1 bar, the level its record's figure and bulk radius
 * give (plan 14; Design note 17, "Gas giants": "Their datum is the 1-bar level spheroid").
 */
export const ENVELOPE_DATUM_PA = 1e5;

/**
 * The highest a gas envelope's column's top lies, Pa: its 1-bar datum × `COLUMN_TOP_PRESSURE_RATIO`
 * (10⁻⁷), as a surface's column's top lies over its ground; a tenth of its highest mode's or
 * absorber's top where that is higher ({@link envelopeTopPa}).
 */
export const ENVELOPE_TOP_PA = 1e-2;

/**
 * τ in the green channel from the top that a gas envelope's column reaches at its base: 1.1 times
 * `thick/deck.ts`' `DECK_FLOOR_OPTICAL_DEPTH`, 1,000, so that its deck's floor, found at τ 1,000 in
 * the reference frame (s = 1) and so at one height in every band, lies inside it, with room for
 * the τ the floor's search reads beside the molecular term's.
 */
export const ENVELOPE_BASE_OPTICAL_DEPTH = 1_100;

/** The deepest a gas envelope's column's base lies, Pa: 100 kbar. */
export const ENVELOPE_BASE_MAX_PA = 1e10;

/**
 * How far beyond each of a mode's ends its column takes a level, in ln p: 10⁻⁸, so that the ramp
 * `aerosolTerm` lays to 0 past an end holds under 10⁻⁸ (f + 1) ÷ 2 of the mode's τ: 3.6 × 10⁻⁵ at
 * the steepest volatile deck's f of about 7,100.
 */
export const MODE_EDGE_LN_P = 1e-8;

/**
 * The e-folds of a steep mode's τ density, ∝ (p ÷ p_b)^(f+1) in p, its column's levels step by
 * through it: 0.1, at which linear interpolation between levels holds its τ to about 0.1² ÷ 8,
 * 1.3 × 10⁻³ of itself, in each interval.
 */
export const STEEP_PROFILE_STEP = 0.1;

/**
 * The share of a steep mode's τ density, below its base's, past which its levels stop: 10⁻⁶,
 * 13.8 e-folds, 138 levels at {@link STEEP_PROFILE_STEP}.
 */
export const STEEP_PROFILE_DEPTH = 1e-6;

/** What of a mode its profile reads: its ends and its exponent. */
export type ModeProfile = Pick<AerosolMode, "basePa" | "topPa" | "fSed">;

/**
 * The share of a mode's τ that lies above pressure `pressurePa` by its published profile (the
 * module's remarks): 0 at and above its top, 1 at and below its base.
 */
export function shareAbove(mode: ModeProfile, pressurePa: number): number {
  const { basePa, topPa } = mode;
  if (pressurePa <= topPa) {
    return 0;
  }
  if (pressurePa >= basePa) {
    return 1;
  }
  const power = mode.fSed + 1;
  const topShare = pow(topPa / basePa, power);
  return (pow(pressurePa / basePa, power) - topShare) / (1 - topShare);
}

/** A mode's τ(550) over the whole sky: its in-cloud τ times its coverage; 0 where not modelled. */
export function skyOpticalDepth550(mode: AerosolMode): number {
  return mode.opticalDepth.kind === "value" ? mode.opticalDepth.value.tau550 * mode.coverage : 0;
}

/**
 * The Stefan–Boltzmann constant σ, W m⁻² K⁻⁴: 5.670374419 × 10⁻⁸, exact in the 2019 SI (CODATA
 * 2018 and 2022), the sim's `units::consts::STEFAN_BOLTZMANN`.
 */
const STEFAN_BOLTZMANN_W_M2_K4 = 5.670_374_419e-8;

/**
 * The stellar flux a gas envelope's self-luminosity cut reads, W m⁻²: σ T_irr⁴, the flux its
 * record's irradiation temperature stands for (T_irr⁴ = T_eq⁴ ÷ f, the wire's), which its own
 * reflection leaves out, so at most the star's at the body. An envelope's record carries no stellar
 * flux of its own, as a surface's does.
 */
export function envelopeStellarFluxWM2(envelope: BodyEnvelope): number {
  const t2 = envelope.irradiationTemperatureK * envelope.irradiationTemperatureK;
  return STEFAN_BOLTZMANN_W_M2_K4 * t2 * t2;
}

/**
 * A gas envelope's temperature profile (`column.ts`' `envelope`): Guillot's grey profile of the
 * record's parameters at the bulk section's gravity above the join, the adiabat's nodes below.
 */
export function envelopeProfileOf(envelope: BodyEnvelope, bulkGravityMS2: number): EnvelopeProfile {
  return {
    kind: "envelope",
    grey: envelope.profile,
    internalK: envelope.internalTemperatureK,
    irradiationK: envelope.irradiationTemperatureK,
    gravityMS2: bulkGravityMS2,
    nodes: envelope.adiabat,
  };
}

/**
 * A gas envelope's column's top, Pa: {@link ENVELOPE_TOP_PA}, or a tenth of the highest top of its
 * inventory's modes and absorbers where that lies above it, so that the column holds them all.
 */
export function envelopeTopPa(inventory: AerosolInventory): number {
  const tops = [
    ...inventory.modes.map((m) => m.topPa),
    ...inventory.absorbers.map((a) => a.topPa),
  ].filter((p) => p > 0);
  return Math.min(ENVELOPE_TOP_PA, 0.1 * Math.min(...tops, Number.POSITIVE_INFINITY));
}

/** An inventory cut at a floor, and the modes the cut drops. */
export interface CutInventory {
  readonly inventory: AerosolInventory;
  /** The modes that take no term and no label, in the inventory's order. */
  readonly dropped: ReadonlyArray<AerosolMode>;
}

/**
 * `inventory` cut at an envelope's floor, `floorPa` (`decision-deep-decks-split.md` §3.3, item 3):
 * a mode whose top lies above the floor enters, one that straddles it based at the floor with the
 * τ its profile puts above it ({@link shareAbove}), cut there and not rescaled; a mode whose top
 * lies at or below the floor is dropped, with no term and no label, as is one of no τ whose top
 * lies there; one of no τ whose top lies above keeps its place, to be labelled. Its decks and
 * absorbers stand as they are.
 */
export function cutAtFloor(inventory: AerosolInventory, floorPa: number): CutInventory {
  const modes: AerosolMode[] = [];
  const dropped: AerosolMode[] = [];
  for (const mode of inventory.modes) {
    if (mode.topPa >= floorPa) {
      dropped.push(mode);
      continue;
    }
    const depth = mode.opticalDepth;
    if (mode.basePa <= floorPa || depth.kind === "not_modelled") {
      modes.push(mode);
      continue;
    }
    modes.push({
      ...mode,
      basePa: floorPa,
      opticalDepth: {
        kind: "value",
        value: { ...depth.value, tau550: depth.value.tau550 * shareAbove(mode, floorPa) },
      },
    });
  }
  return { inventory: { ...inventory, modes }, dropped };
}

/**
 * The pressures at which a gas envelope's column lays a level for the modes that enter it, Pa
 * (the module's remarks): each mode's top and base, a level {@link MODE_EDGE_LN_P} beyond each,
 * and through a profile steeper than `columnStepLnP` allows, levels {@link STEEP_PROFILE_STEP}
 * e-folds of its τ density apart from its base up, until it falls under
 * {@link STEEP_PROFILE_DEPTH} of the base's; those strictly between `topPa` and `basePa` alone.
 *
 * @param columnStepLnP - The column's own spacing of levels, in ln p.
 */
export function modeLevelsPa(
  modes: ReadonlyArray<AerosolMode>,
  topPa: number,
  basePa: number,
  columnStepLnP: number,
): ReadonlyArray<number> {
  const levels: number[] = [];
  for (const mode of modes) {
    if (mode.opticalDepth.kind === "not_modelled" || !(mode.basePa > mode.topPa)) {
      continue;
    }
    const edge = exp(MODE_EDGE_LN_P);
    levels.push(mode.basePa * edge, mode.basePa, mode.topPa, mode.topPa / edge);
    const stepLnP = STEEP_PROFILE_STEP / (mode.fSed + 1);
    if (stepLnP < columnStepLnP) {
      const steps = Math.ceil(log(1 / STEEP_PROFILE_DEPTH) / STEEP_PROFILE_STEP);
      for (let j = 1; j <= steps; j += 1) {
        const p = mode.basePa * exp(-j * stepLnP);
        if (p <= mode.topPa) {
          break;
        }
        levels.push(p);
      }
    }
  }
  return levels.filter((p) => p > topPa && p < basePa);
}

/**
 * A figure lowered by `depthM` of h\*, the gravity-scaled height (Design note 17): the spheroid
 * (a − d ÷ s_e, c − d ÷ s_p) through the level surface that depth below the datum at the equator
 * and the poles, s the body's gravity ratio there, as `thick/deckRead.ts`' `deckDatumOf` raises a
 * deck's.
 */
export function loweredFigure(
  figure: SpheroidFigure,
  gravity: BodyGravity,
  depthM: number,
): SpheroidFigure {
  return {
    equatorialRadiusM: figure.equatorialRadiusM - depthM / gravityRatio(gravity, 0),
    polarRadiusM: figure.polarRadiusM - depthM / gravityRatio(gravity, Math.PI / 2),
  };
}
