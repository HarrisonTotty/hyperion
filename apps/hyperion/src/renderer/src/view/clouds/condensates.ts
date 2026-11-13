/**
 * A cloud layer's condensate (plan R11, R11.T7.a; Design notes 9 and 10): which of a body's decks its
 * clouds draw, that deck's optics at 550 nm, its in-cloud optical depth and its heights, and the
 * stand-ins that serve where the record does not model them.
 *
 * @remarks
 * - **The registry's optics** (decision-composition): a deck's asymmetry g and co-albedo 1 − ϖ₀ at
 *   550 nm are its thickest mode's, R08's `aerosolModeOptics` of the mode's particles, which the
 *   assembly computes for the mode's medium term; its phase is its material's, the deck's own
 *   (`CondensateDeck.phase`), not ISCCP's switch at 260 K, which the composition ruling superseded.
 * - **The in-cloud optical depth** is the record's: the sum of the τ(550) of the deck's modes, each
 *   in-cloud (`AerosolOpticalDepth.tau550`, before its coverage applies), where every one of them is
 *   modelled.
 * - **The stand-ins** ({@link STAND_IN_CONDENSATES}) serve a deck whose optics or optical depth the
 *   record does not model, and a kept scene's cover, whose decks name no condensate: plan R11's five,
 *   each at its stated effective radius, with g and 1 − ϖ₀ the registry's at that size (pinned by
 *   `condensates.test.ts` against `aerosolModeOptics`), which corrects the plan's co-albedo of 10⁻⁵
 *   for liquid water to 5.4 × 10⁻⁷ (T7.a.1's review), and its in-cloud τ the plan's citation's.
 * - **One deck a body**: the layer draws the deck whose sky-mean τ, in-cloud τ times coverage, is the
 *   largest, the one that most changes the picture (lean, 2026-11-02).
 * - Pure: no state, no platform API; `f64` throughout.
 */
import type { AerosolInventory, AerosolPhase } from "../../lib/system/model";

/** A condensate's single-scattering optics at 550 nm, each dimensionless. */
export interface CloudOptics {
  /** g, the phase function's asymmetry parameter, in [0, 1). */
  readonly asymmetry: number;
  /** 1 − ϖ₀, the co-albedo, in [0, 1]: kept apart from ϖ₀, which `f32` cannot hold near 1. */
  readonly coAlbedo: number;
}

/** One of a body's condensate decks as its clouds read it. */
export interface CloudDeck {
  /** The condensate, by its registry key. */
  readonly condensate: string;
  /** Its phase, which its material keeps throughout. */
  readonly phase: AerosolPhase;
  /**
   * Its in-cloud τ(550), the sum of its modes', dimensionless; `null` where any of them is not
   * modelled, or it has none.
   */
  readonly inCloudOpticalDepth550: number | null;
  /** The share of the sky it covers, its modes' largest, in [0, 1]. */
  readonly coverage: number;
  /** Its base's height above the datum, m. */
  readonly baseAltitudeM: number;
  /** Its top's height above the datum, m, at or above its base. */
  readonly topAltitudeM: number;
  /** Its thickest mode's optics from the registry; `null` where the assembly drew none. */
  readonly optics: CloudOptics | null;
}

/**
 * What of a body's record its clouds read (R11.T7.b), carried with its atmosphere's entry: plan
 * 14's cloud fraction, its air's surface pressure and its decks.
 */
export interface BodyClouds {
  /** The share of the sky its clouds cover, P14.T24.b's, in [0, 1]. */
  readonly cloudFraction: number;
  /** The air's surface pressure, Pa, which picks a water-ice stand-in. */
  readonly surfacePressurePa: number;
  /** Its decks, in its inventory's order. */
  readonly decks: ReadonlyArray<CloudDeck>;
}

/** Where a layer's optics or optical depth came from. */
export type CloudSource = "record" | "stand-in";

/** The condensate a body's cloud layer draws, every member resolved. */
export interface CloudCondensate extends CloudOptics {
  readonly condensate: string;
  readonly phase: AerosolPhase;
  /** τ̄, the mean in-cloud τ(550), above 0. */
  readonly inCloudOpticalDepth550: number;
  readonly baseAltitudeM: number;
  readonly topAltitudeM: number;
  /** Whether g and 1 − ϖ₀ are the record's deck's (its registry optics) or a stand-in's. */
  readonly opticsSource: CloudSource;
  /** Whether τ̄ is the record's or a stand-in's. */
  readonly opticalDepthSource: CloudSource;
}

/** The names of plan R11's five stand-in condensates (T7.a). */
export type StandInCondensateName =
  | "liquidWater"
  | "terrestrialIce"
  | "martianWaterIce"
  | "martianCarbonDioxideIce"
  | "liquidMethane";

/** A stand-in condensate: its registry key and phase, its size and its optics at 550 nm. */
export interface StandInCondensate extends CloudOptics {
  readonly name: StandInCondensateName;
  readonly condensate: string;
  readonly phase: AerosolPhase;
  /** The shape class its particles take in the registry: a droplet or a crystal. */
  readonly shape: "sphere" | "crystal";
  /** r_eff of a gamma distribution of v_eff 0.1, µm. */
  readonly effectiveRadiusUm: number;
  /** A typical in-cloud τ(550), the geometric mean of the cited range, dimensionless. */
  readonly inCloudOpticalDepth550: number;
}

/**
 * Plan R11's stand-in condensates (T7.a), each at 550 nm. g and 1 − ϖ₀ are the registry's
 * (R08's `aerosolModeOptics`, a gamma distribution of v_eff 0.1 at the stated r_eff: Mie on the
 * registry's index for a droplet, Yang et al. 2013's roughened aggregate for a crystal), computed
 * 2026-11-02 and held to it by `condensates.test.ts`; each g lies in the plan's cited range. The
 * in-cloud τ is the geometric mean of the plan's cited range. The sizes and ranges are the plan's
 * research, cited from memory: the re-check of 2026-11-13 reached no source (no web access from the
 * lane), and the references below are from memory too.
 *
 * - Liquid water: r_eff 10 µm, the plan's g 0.85–0.87 and median column τ about 8 (Hansen and
 *   Travis 1974, Space Sci. Rev. 16, 527); the registry gives g 0.8654 and 1 − ϖ₀ 5.43 × 10⁻⁷,
 *   against the plan's 10⁻⁵, and an independent Mie run (m = 1.333 + 1.96 × 10⁻⁹ i, v_eff 0.1)
 *   4.3–5.0 × 10⁻⁷ and g 0.864 (2026-11-13). Hansen and Travis's review may not hold the τ.
 * - Terrestrial ice: r_eff 30 µm, g about 0.75, τ 0.1–3 (Yang et al. 2013, J. Atmos. Sci. 70, 330).
 * - Martian water ice: r_eff 1–4 µm (2.5 here), g 0.7–0.8, τ 0.1–1 (Clancy et al. 2003, J. Geophys.
 *   Res. 108(E9), 5098).
 * - Martian CO₂ ice: r_eff 0.5–3 µm (1.5 here), g 0.6–0.8, τ 0.01–0.6 (Montmessin et al. 2007,
 *   J. Geophys. Res. 112, E11S90); the registry's Yang 2013 table stands in for CO₂ as an analogue.
 * - Liquid methane: g 0.85–0.9, τ a few, taken as 3 (Griffith et al. 1998, Nature 395, 575); r_eff
 *   50 µm, a drizzle droplet's (lean, 2026-11-02: the plan gives no size).
 */
export const STAND_IN_CONDENSATES: Readonly<Record<StandInCondensateName, StandInCondensate>> = {
  liquidWater: {
    name: "liquidWater",
    condensate: "H2O",
    phase: "liquid",
    shape: "sphere",
    effectiveRadiusUm: 10,
    asymmetry: 0.8654,
    coAlbedo: 5.43e-7,
    inCloudOpticalDepth550: 8,
  },
  terrestrialIce: {
    name: "terrestrialIce",
    condensate: "H2O",
    phase: "solid",
    shape: "crystal",
    effectiveRadiusUm: 30,
    asymmetry: 0.7513,
    coAlbedo: 1.5e-7,
    inCloudOpticalDepth550: Math.sqrt(0.1 * 3),
  },
  martianWaterIce: {
    name: "martianWaterIce",
    condensate: "H2O",
    phase: "solid",
    shape: "crystal",
    effectiveRadiusUm: 2.5,
    asymmetry: 0.7409,
    coAlbedo: 2.12e-8,
    inCloudOpticalDepth550: Math.sqrt(0.1 * 1),
  },
  martianCarbonDioxideIce: {
    name: "martianCarbonDioxideIce",
    condensate: "CO2",
    phase: "solid",
    shape: "crystal",
    effectiveRadiusUm: 1.5,
    asymmetry: 0.741,
    coAlbedo: 1.66e-8,
    inCloudOpticalDepth550: Math.sqrt(0.01 * 0.6),
  },
  liquidMethane: {
    name: "liquidMethane",
    condensate: "CH4",
    phase: "liquid",
    shape: "sphere",
    effectiveRadiusUm: 50,
    asymmetry: 0.898,
    coAlbedo: 6.3e-8,
    inCloudOpticalDepth550: 3,
  },
};

/**
 * The surface pressure below which a water-ice deck takes the Martian stand-in's small crystals
 * rather than terrestrial cirrus's, Pa: 2 kPa, between Mars's 0.6 kPa and Titan's and Earth's
 * 147 and 101 kPa (lean, 2026-11-02; a thin air holds less vapour to grow crystals on).
 */
export const THIN_AIR_ICE_PRESSURE_PA = 2_000;

/**
 * The stand-in of a condensate in a phase, under an air of `surfacePressurePa`; `null` for a
 * condensate none of the five stands in for, which the layer then draws as liquid water's
 * ({@link cloudCondensateOf}).
 */
export function standInCondensate(
  condensate: string,
  phase: AerosolPhase,
  surfacePressurePa: number,
): StandInCondensate | null {
  const { liquidWater, terrestrialIce, martianWaterIce, martianCarbonDioxideIce, liquidMethane } =
    STAND_IN_CONDENSATES;
  if (condensate === "H2O") {
    if (phase === "liquid") {
      return liquidWater;
    }
    return surfacePressurePa < THIN_AIR_ICE_PRESSURE_PA ? martianWaterIce : terrestrialIce;
  }
  if (condensate === "CO2" && phase === "solid") {
    return martianCarbonDioxideIce;
  }
  if (condensate === "CH4" && phase === "liquid") {
    return liquidMethane;
  }
  return null;
}

/** A deck's sky-mean τ, its in-cloud τ times its coverage; 0 where its τ is not modelled. */
function skyMeanOpticalDepth(deck: CloudDeck): number {
  return (deck.inCloudOpticalDepth550 ?? 0) * deck.coverage;
}

/**
 * The condensate a body's cloud layer draws from its decks: the deck of largest sky-mean τ, the
 * first listed on a tie (the inventory lists the deepest first), each member the record's where
 * modelled and its stand-in's otherwise; `null` with no deck.
 *
 * @remarks
 * A deck whose condensate has no stand-in and whose optics or τ the record leaves out takes liquid
 * water's for what is missing, a stated convention. A deck of zero thickness is drawn at its base.
 *
 * @param surfacePressurePa - The air's surface pressure, which picks a water-ice stand-in.
 */
export function cloudCondensateOf(
  decks: ReadonlyArray<CloudDeck>,
  surfacePressurePa: number,
): CloudCondensate | null {
  let chosen: CloudDeck | undefined;
  for (const deck of decks) {
    if (chosen === undefined || skyMeanOpticalDepth(deck) > skyMeanOpticalDepth(chosen)) {
      chosen = deck;
    }
  }
  if (chosen === undefined) {
    return null;
  }
  const standIn =
    standInCondensate(chosen.condensate, chosen.phase, surfacePressurePa) ??
    STAND_IN_CONDENSATES.liquidWater;
  const optics = chosen.optics ?? standIn;
  const tau = chosen.inCloudOpticalDepth550;
  const recordTau = tau !== null && tau > 0;
  return {
    condensate: chosen.condensate,
    phase: chosen.phase,
    asymmetry: optics.asymmetry,
    coAlbedo: optics.coAlbedo,
    inCloudOpticalDepth550: recordTau ? tau : standIn.inCloudOpticalDepth550,
    baseAltitudeM: chosen.baseAltitudeM,
    topAltitudeM: Math.max(chosen.topAltitudeM, chosen.baseAltitudeM),
    opticsSource: chosen.optics === null ? "stand-in" : "record",
    opticalDepthSource: recordTau ? "record" : "stand-in",
  };
}

/**
 * A record's condensate decks as its clouds read them (the module's remarks), in the inventory's
 * order.
 *
 * @param heightAtPa - The column's height above the datum at a pressure, m (the assembly's
 *   `heightAtPa`, which lays a deck below the datum at 0 and one above the top at the top).
 * @param opticsOf - The registry's optics at 550 nm of the inventory's mode at an index, `null` where
 *   the assembly drew it with none.
 */
export function cloudDecksOf(
  inventory: AerosolInventory,
  heightAtPa: (pressurePa: number) => number,
  opticsOf: (modeIndex: number) => CloudOptics | null,
): CloudDeck[] {
  return inventory.decks.map((deck): CloudDeck => {
    let tau: number | null = 0;
    let coverage = 0;
    let thickest = -1;
    let optics: CloudOptics | null = null;
    for (const [index, mode] of inventory.modes.entries()) {
      if (mode.deck !== deck.condensate) {
        continue;
      }
      coverage = Math.max(coverage, mode.coverage);
      const depth = mode.opticalDepth;
      if (depth.kind !== "value") {
        tau = null;
        continue;
      }
      if (tau !== null) {
        tau += depth.value.tau550;
      }
      if (depth.value.tau550 > thickest) {
        thickest = depth.value.tau550;
        optics = opticsOf(index);
      }
    }
    const baseAltitudeM = heightAtPa(deck.basePa);
    return {
      condensate: deck.condensate,
      phase: deck.phase,
      inCloudOpticalDepth550: thickest < 0 ? null : tau,
      coverage,
      baseAltitudeM,
      topAltitudeM: Math.max(heightAtPa(deck.topPa), baseAltitudeM),
      optics,
    };
  });
}
