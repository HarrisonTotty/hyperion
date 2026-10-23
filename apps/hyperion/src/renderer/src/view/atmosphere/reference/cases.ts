/**
 * The reference tracer's cloud-free cases (plan R08, R08.T12.b.1; Design notes 10, 16 and 17):
 * each case's medium, figure, sun, curves of growth, geometry set and aggregates, from the
 * client's own media and optics, for `hyperion-fit atmosphere-reference` to trace and R08.T13 to
 * hold the CPU twin to.
 *
 * @remarks
 * The media are R08.T10's fixtures (`decision-r08-t12b-scope.md`, ruling 3): R08.T10.a's kept
 * scenes read these media, with the thick ones R08.T12.b.2 adds, so the twin, the reference and
 * the drawn world read one medium on one figure. This module imports nothing from the tests.
 *
 * - `earth`: {@link EARTH_REFERENCE} (Earth with its measured ozone since R08.T6.h, Design note
 *   16's "Earth with ozone") on a sphere of R05's `tableRadiusM` for WGS 84, (2a + c) ÷ 3, the
 *   figure the per-planet tables draw it on.
 * - `earth-ozone-slab`: {@link EARTH_SLAB_REFERENCE}, T6.d's 300 DU slab, on the same sphere.
 * - `mars`: {@link MARS_REFERENCE}, Mars's air on R08.T4.a's column with its dust well mixed, on
 *   a sphere of the column's 3,389.5 km.
 * - `saturn-oblate`: {@link SATURN_OBLATE_REFERENCE}, the Saturn-class air on Saturn's level
 *   spheroid, Design note 17's thin case.
 *
 * Every case traces at the fitted channels, `medium.ts`'s `CHANNEL_WAVELENGTHS_NM`, under the
 * reference Sun (`earth.ts`'s `REFERENCE_SUN`) at its per-channel illuminance at 1 au
 * ({@link referenceSunIlluminanceLx}), lx: the metric is relative, and the radiances come out in
 * the case's unit per steradian. The 15-bin spectral references are R08.T17's.
 */

import {
  ATOMIC_MASS_CONSTANT_KG,
  GRAVITATIONAL_CONSTANT_M3_PER_KG_S2,
} from "../../../lib/system/constants";
import type { Rgb } from "../../photometry/toneCurve";
import type { AbsorberCurve } from "../absorbers";
import { aerosolTerm } from "../aerosol";
import {
  MARS_AIR,
  MARS_DUST_MODE,
  MARS_DUST_OPTICAL_DEPTH_550,
  MARS_FIT_COLUMN,
} from "../channels";
import { gasProperties, hydrostaticColumn, meanMolarMassGPerMol } from "../column";
import {
  EARTH_REFERENCE,
  EARTH_REFERENCE_CURVES,
  EARTH_SLAB_REFERENCE,
  EARTH_SLAB_REFERENCE_CURVES,
  referenceSunIlluminanceLx,
  WGS84_LEVEL_SPHEROID,
} from "../earth";
import { tableRadiusM } from "../hillaire";
import { type AtmosphereMedium, localScaleHeightM, type MediumTerm } from "../medium";
import { bodyGravity, type BodyGravity, gravityRatio, type LevelSpheroid } from "../oblate";
import { BOLTZMANN_J_PER_K, type GasFractions, molecularTerm } from "../rayleigh";
import {
  aerialSet,
  discSet,
  GROUND_OBSERVER_HEIGHT_M,
  groundSet,
  limbGeometry,
  type LocalDirection,
  type ReferenceAggregate,
  REFERENCE_DETECTOR_HALF_ANGLE_DEG,
  type ReferenceGeometry,
  sphereAggregates,
  sphereGroundSet,
  sphereLimbSet,
} from "./geometries";

/** The figure a case's medium is layered on. */
export type CaseFigure =
  | {
      readonly kind: "sphere";
      /** The ground's radius, m. */
      readonly radiusM: number;
    }
  | {
      readonly kind: "spheroid";
      /** The level spheroid's datum and gravity, as R08.T3.d's `bodyGravity` gives them. */
      readonly gravity: BodyGravity & { readonly spheroid: LevelSpheroid };
    };

/** One case of the reference tracer (the module's remarks). */
export interface ReferenceCase {
  /** The case's name: its files are `<name>.case.json` and `<name>.reference.json`. */
  readonly name: string;
  readonly medium: AtmosphereMedium;
  readonly figure: CaseFigure;
  /**
   * The medium's absorber terms' curves of growth under the case's sun, one an absorber term in
   * the medium's order of them (light slot 0 of the tables' `AbsorberCurveSlots`).
   */
  readonly curves: ReadonlyArray<AbsorberCurve>;
  /**
   * Each tabulated phase's source mean cosine per channel, by term name: the mode's g, not the
   * table's own, so that the tracer's check of the table against it proves something (R08.T12.c's
   * finding).
   */
  readonly phaseAsymmetry: Readonly<Record<string, Rgb>>;
  /** The sun's illuminance normal to its beam at the top, per channel, lx. */
  readonly sunIlluminanceLx: Rgb;
  /** The seed of the final trace's draws. */
  readonly seed: number;
  readonly detectorHalfAngleDeg: number;
  readonly geometries: ReadonlyArray<ReferenceGeometry>;
  readonly aggregates: ReadonlyArray<ReferenceAggregate>;
}

/**
 * Each case's seed, its own, so that two cases' references draw independent noise: Design note
 * 10's false failures over ten cases assume it, and `earth` and `earth-ozone-slab` would otherwise
 * follow the same paths, their shells alike but at the slab's edges. The pilots took seeds from
 * 1000 (ruling 8).
 */
export const REFERENCE_SEEDS: Readonly<Record<string, number>> = {
  earth: 0,
  "earth-ozone-slab": 1,
  mars: 2,
  "saturn-oblate": 3,
};

/** A case's seed, from {@link REFERENCE_SEEDS}. */
function seedOf(name: string): number {
  const seed = REFERENCE_SEEDS[name];
  if (seed === undefined) {
    throw new Error(`case ${name} has no seed`);
  }
  return seed;
}

/** Where the disc and the limb are seen from on a sphere's case, m above the ground: 1,000 km. */
export const SPHERE_ORBIT_HEIGHT_M = 1_000_000;

// --- Earth -----------------------------------------------------------------------------------

/** Earth's sphere: R05's `tableRadiusM` of WGS 84, (2a + c) ÷ 3 = 6,371,008.77 m. */
export const EARTH_REFERENCE_RADIUS_M = tableRadiusM(WGS84_LEVEL_SPHEROID);

/** A sphere case's geometries: Design note 10's ground set, disc, limb and aerial perspective. */
function sphereGeometries(medium: AtmosphereMedium, radiusM: number): ReferenceGeometry[] {
  return [
    ...sphereGroundSet(),
    ...discSet(SPHERE_ORBIT_HEIGHT_M),
    ...sphereLimbSet(radiusM, localScaleHeightM(medium, 0), SPHERE_ORBIT_HEIGHT_M),
    ...aerialSet(),
  ];
}

/** A case on a sphere under the reference Sun, at its own seed. */
function sphereCase(
  name: string,
  medium: AtmosphereMedium,
  radiusM: number,
  curves: ReadonlyArray<AbsorberCurve>,
  phaseAsymmetry: Readonly<Record<string, Rgb>>,
  extra: ReadonlyArray<ReferenceGeometry> = [],
): ReferenceCase {
  return {
    name,
    medium,
    figure: { kind: "sphere", radiusM },
    curves,
    phaseAsymmetry,
    sunIlluminanceLx: referenceSunIlluminanceLx(),
    seed: seedOf(name),
    detectorHalfAngleDeg: REFERENCE_DETECTOR_HALF_ANGLE_DEG,
    geometries: [...sphereGeometries(medium, radiusM), ...extra],
    aggregates: sphereAggregates(),
  };
}

/** The light slot 0 curves of a medium's tables. */
function slotZero(
  slots: ReadonlyArray<ReadonlyArray<AbsorberCurve>>,
): ReadonlyArray<AbsorberCurve> {
  const [curves] = slots;
  if (curves === undefined) {
    throw new Error("a medium with absorbers has its first light slot's curves");
  }
  return curves;
}

// --- Mars ------------------------------------------------------------------------------------

/**
 * Mars's ground albedo, grey: 0.250, Mars's Bond albedo on NASA's Mars Fact Sheet (D. R. Williams,
 * NSSDCA; read by R08.T12.b.1's science check, 2026-10-21): a hand value
 * (`decision-r08-t12b-scope.md`, ruling 3). A Bond albedo is the whole planet's, its air and dust
 * included, not the ground's, so it stands here for a ground of no measured albedo.
 */
export const MARS_GROUND_ALBEDO = 0.25;

/** Mars's sphere: {@link MARS_FIT_COLUMN}'s volumetric mean radius, 3,389.5 km. */
export const MARS_REFERENCE_RADIUS_M = MARS_FIT_COLUMN.referenceRadiusM;

/**
 * Mars's dust as a medium term: `aerosol.ts`'s `aerosolTerm` of {@link MARS_DUST_MODE} at
 * τ(550) = {@link MARS_DUST_OPTICAL_DEPTH_550}, well mixed (f = 0, the gas's density, as
 * `marsFitAir` takes it), from {@link MARS_FIT_COLUMN}'s base to its top, at the fitted channels;
 * its phase the TAMUdust2020 hexahedra's table with its matrix and its forward delta, which the
 * reference keeps as scattering (`decision-r08-aerosol-labels.md` item 3).
 */
const MARS_DUST = aerosolTerm(
  {
    name: "dust",
    particles: { kind: "distribution", mode: MARS_DUST_MODE },
    opticalDepth550: MARS_DUST_OPTICAL_DEPTH_550,
    basePa: MARS_FIT_COLUMN.pressuresPa[0] ?? Number.NaN,
    topPa: MARS_FIT_COLUMN.pressuresPa.at(-1) ?? Number.NaN,
    mixingExponent: 0,
  },
  MARS_FIT_COLUMN,
);

/**
 * Mars's reference atmosphere (`decision-r08-t12b-scope.md`, ruling 3): R08.T3.c's molecular term
 * of {@link MARS_AIR} on {@link MARS_FIT_COLUMN}, the dust ({@link MARS_DUST}), and a grey
 * Lambertian ground of {@link MARS_GROUND_ALBEDO}, its top the column's, where the density is
 * 10⁻⁷ of the datum's. No absorber: Mars's CO₂, CO and O₂ bands are negligible in the visible
 * (R08.T10.a's negligible absorbers), and its ozone is not modelled, a stated omission: its
 * vertical Chappuis depth stays under about 10⁻³ at 6 DU, Mariner 9's 60 µm-atm, the most on
 * record, and the equator the cases stand at has at most 0.3 DU (Perrier et al. 2006, JGR 111,
 * E09S06), about 2 × 10⁻³ along the limb (R08.T12.b.1's science check's estimate). The first Mars
 * `AtmosphereMedium`.
 */
export const MARS_REFERENCE: AtmosphereMedium = {
  name: "mars",
  topHeightM: MARS_FIT_COLUMN.topHeightM,
  groundAlbedo: [MARS_GROUND_ALBEDO, MARS_GROUND_ALBEDO, MARS_GROUND_ALBEDO],
  terms: [molecularTerm(MARS_FIT_COLUMN, MARS_AIR).term, MARS_DUST.term],
};

/** The dust's mean cosine per channel, from its optics (the mode's g). */
const MARS_DUST_ASYMMETRY: Rgb = [
  MARS_DUST.channels[0].asymmetry,
  MARS_DUST.channels[1].asymmetry,
  MARS_DUST.channels[2].asymmetry,
];

/**
 * R08.T13's Mars points: 3°, 5°, 8° and 12° from a sun 5° and 30° up, in the sun's vertical plane
 * above it (`science-r08-mmf.md` §3, "points added explicitly"), with the case's 0.5° cone. Those
 * under the sun 30° up (zenith 60°) join Design note 10's image of that sun; those under the sun 5°
 * up are an image of their own.
 */
export const MARS_SUN_OFFSETS_DEG: ReadonlyArray<number> = [3, 5, 8, 12];

/** The suns of {@link MARS_SUN_OFFSETS_DEG}, as zenith angles, degrees. */
export const MARS_AUREOLE_SUN_ZENITHS_DEG: ReadonlyArray<number> = [85, 60];

function marsAureole(): ReferenceGeometry[] {
  return MARS_AUREOLE_SUN_ZENITHS_DEG.flatMap((sunZenith) =>
    MARS_SUN_OFFSETS_DEG.map((offset) => ({
      name: `aureole-s${sunZenith}-d${offset}`,
      image: `ground-s${sunZenith}`,
      observer: { heightM: GROUND_OBSERVER_HEIGHT_M, latitudeDeg: 0 },
      view: { zenithDeg: sunZenith - offset, azimuthDeg: 0 },
      sun: { zenithDeg: sunZenith, azimuthDeg: 0 },
    })),
  );
}

// --- Saturn ----------------------------------------------------------------------------------

/**
 * Saturn at 1 bar (`decision-r08-spheroid.md` §2): a and c of Archinal et al. 2018 (Celest. Mech.
 * Dyn. Astron. 130, 22), 60,268 and 54,364 km; GM as the NSSDCA Saturn fact sheet prints it,
 * 3.7931 × 10¹⁶ m³ s⁻², which JPL's SAT441L solution gives to the same five figures (its credit to
 * Jacobson et al. 2006, AJ 132, 2520, from memory: the abstract prints no figure); and the fact
 * sheet's period, 10.656 h: the constants of R08.T3.d's tests and R08.T6.e's fixture, rebuilt
 * here.
 */
export const SATURN_OBLATE_SPHEROID: LevelSpheroid = {
  equatorialRadiusM: 60_268e3,
  polarRadiusM: 54_364e3,
  gmM3S2: 3.793_1e16,
  angularVelocityRadS: (2 * Math.PI) / (10.656 * 3_600),
};

/**
 * Saturn's gravity: R08.T3.d's `bodyGravity` of {@link SATURN_OBLATE_SPHEROID} with no figure law
 * (read as `rotational`) on R05's `tableRadiusM` of its figure, as the twin tests take it.
 */
export const SATURN_OBLATE_GRAVITY: BodyGravity & { readonly spheroid: LevelSpheroid } = (() => {
  const gravity = bodyGravity(
    {
      figure: SATURN_OBLATE_SPHEROID,
      massKg: SATURN_OBLATE_SPHEROID.gmM3S2 / GRAVITATIONAL_CONSTANT_M3_PER_KG_S2,
      angularVelocityRadS: SATURN_OBLATE_SPHEROID.angularVelocityRadS,
      bulkGravityMS2: SATURN_OBLATE_SPHEROID.gmM3S2 / SATURN_OBLATE_SPHEROID.equatorialRadiusM ** 2,
      figureLaw: null,
    },
    tableRadiusM(SATURN_OBLATE_SPHEROID),
  );
  const { spheroid } = gravity;
  if (spheroid === null) {
    throw new Error("a spinning figure has its level spheroid");
  }
  return { ...gravity, spheroid };
})();

/**
 * The Saturn-class air by volume: the NSSDCA Saturn fact sheet's 96.3% H₂, 3.25% He and 4,500 ppm
 * CH₄ (Voyager-era), as `column.test.ts` takes it. CH₄ enters its Rayleigh scattering; its bands
 * are R08.T12.b.2's.
 */
export const SATURN_OBLATE_AIR = [
  { species: "H2", moleFraction: 0.963 },
  { species: "He", moleFraction: 0.0325 },
  { species: "CH4", moleFraction: 0.0045 },
] as const satisfies GasFractions;

/** The Saturn-class air's temperature at 1 bar, K: 134, the NSSDCA fact sheet's. */
export const SATURN_OBLATE_TEMPERATURE_K = 134;

/** The Saturn-class medium's top, in its scale heights: 16, about 10⁻⁷ of the datum's density. */
export const SATURN_OBLATE_TOP_SCALE_HEIGHTS = 16;

/** The Saturn-class air's grey Lambertian ground at 1 bar: 0.3, a hand value (R08.T6.e's). */
export const SATURN_OBLATE_GROUND_ALBEDO = 0.3;

/** The Saturn-class air's mean molar mass, g mol⁻¹, from its gases' (`gasProperties`). */
const SATURN_OBLATE_MOLAR_MASS_G_PER_MOL = meanMolarMassGPerMol(
  SATURN_OBLATE_AIR.map(({ species, moleFraction }) => ({
    properties: gasProperties(species),
    moleFraction,
  })),
);

/**
 * The Saturn-class air's isothermal scale height at g_ref, k T ÷ (μ m_u g_ref), m: 49.72 km (the
 * SI's exact k and CODATA 2022's m_u, the client's shared constants).
 */
export const SATURN_OBLATE_SCALE_HEIGHT_M =
  (BOLTZMANN_J_PER_K * SATURN_OBLATE_TEMPERATURE_K) /
  (SATURN_OBLATE_MOLAR_MASS_G_PER_MOL *
    ATOMIC_MASS_CONSTANT_KG *
    SATURN_OBLATE_GRAVITY.referenceGravityMS2);

/**
 * The Saturn-class medium (`decision-r08-t12b-scope.md`, ruling 3): the test fixture's
 * `saturnClassMedium()`, which R08.T6.e's and T6.f's twin tests run on, rebuilt term for term from
 * the same constants. The H₂–He–CH₄ air's molecular term (R08.T3.c) at its 1-bar number density on
 * an isothermal column at g_ref, its density e^(−h\* ÷ H) at {@link SATURN_OBLATE_SCALE_HEIGHT_M},
 * its top {@link SATURN_OBLATE_TOP_SCALE_HEIGHTS} H up, a grey ground of
 * {@link SATURN_OBLATE_GROUND_ALBEDO}, and no absorber. It differs from the spheroid ruling's own
 * model (μ 2.25 g mol⁻¹ and T3.a's column, τ₅₅₀ = 0.223), which the ruling names as the
 * alternative; the beam check holds on either (R08.T12.d's record).
 */
export const SATURN_OBLATE_REFERENCE: AtmosphereMedium = {
  name: "saturn-oblate",
  topHeightM: SATURN_OBLATE_TOP_SCALE_HEIGHTS * SATURN_OBLATE_SCALE_HEIGHT_M,
  groundAlbedo: [
    SATURN_OBLATE_GROUND_ALBEDO,
    SATURN_OBLATE_GROUND_ALBEDO,
    SATURN_OBLATE_GROUND_ALBEDO,
  ],
  terms: [
    {
      ...molecularTerm(
        hydrostaticColumn({
          surfacePa: 1e5,
          temperature: { kind: "isothermal", temperatureK: SATURN_OBLATE_TEMPERATURE_K },
          meanMolarMassGPerMol: SATURN_OBLATE_MOLAR_MASS_G_PER_MOL,
          referenceGravityMS2: SATURN_OBLATE_GRAVITY.referenceGravityMS2,
          referenceRadiusM: tableRadiusM(SATURN_OBLATE_SPHEROID),
        }),
        SATURN_OBLATE_AIR,
      ).term,
      density: { kind: "exponential", scaleHeightM: SATURN_OBLATE_SCALE_HEIGHT_M },
    } satisfies MediumTerm,
  ],
};

/** One of the spheroid ruling's ground sites. */
interface SaturnSite {
  readonly site: string;
  readonly latitudeDeg: number;
  /** The views' azimuths, degrees: north and east. */
  readonly viewAzimuthsDeg: ReadonlyArray<number>;
  readonly sunAzimuthDeg: number;
}

/** The spheroid ruling's ground sites: name, geodetic latitude, view azimuths and sun's azimuth. */
const SATURN_SITES: ReadonlyArray<SaturnSite> = [
  // The equator, looking north and east, the sun in the east.
  { site: "equator", latitudeDeg: 0, viewAzimuthsDeg: [0, 90], sunAzimuthDeg: 90 },
  // 60°, looking north and east, the sun along the meridian, poleward.
  { site: "lat60", latitudeDeg: 60, viewAzimuthsDeg: [0, 90], sunAzimuthDeg: 0 },
  // The pole, looking along the meridians of longitude 0 and 90°, the sun along the first.
  { site: "pole", latitudeDeg: 90, viewAzimuthsDeg: [0, 90], sunAzimuthDeg: 0 },
];

/** The spheroid ruling's limb heights, in scale heights. */
export const SATURN_LIMB_SCALE_HEIGHTS: ReadonlyArray<number> = [0.3, 1, 3];

/** The spheroid ruling's limb latitudes, degrees: the equator, 45° and the pole. */
export const SATURN_LIMB_LATITUDES_DEG: ReadonlyArray<number> = [0, 45, 90];

/**
 * The limb's observer, m back along the line of sight from the tangent point: 24,000 km, some
 * 5,000 km above the datum, outside the top.
 */
export const SATURN_LIMB_DISTANCE_M = 24_000_000;

/**
 * The limb's sun at the tangent point: on its horizon, along the meridian, poleward (north; at the
 * pole along the meridian of longitude 0), at the terminator, where the ray's own gradient of s
 * matters most at 45° (Design note 17); the line of sight runs east there, at right angles to it,
 * so that no view lies within 3° of the sun.
 */
export const SATURN_LIMB_SUN: LocalDirection = { zenithDeg: 90, azimuthDeg: 0 };

/** The line of sight's azimuth at a Saturn limb's tangent point, degrees: east. */
export const SATURN_LIMB_LINE_OF_SIGHT_AZIMUTH_DEG = 90;

/** `saturn-oblate`'s geometries: the spheroid ruling's ground sites and limb. */
function saturnGeometries(): ReferenceGeometry[] {
  const ground = SATURN_SITES.flatMap(({ site, latitudeDeg, viewAzimuthsDeg, sunAzimuthDeg }) =>
    groundSet(
      site,
      { heightM: GROUND_OBSERVER_HEIGHT_M, latitudeDeg },
      viewAzimuthsDeg,
      sunAzimuthDeg,
      latitudeDeg === 90,
    ),
  );
  const limb = SATURN_LIMB_LATITUDES_DEG.flatMap((latitudeDeg) =>
    SATURN_LIMB_SCALE_HEIGHTS.map((n) => {
      // A limb at n H stands at the geodetic height n H ÷ s(φ) (R08.T12.d's record).
      const s = gravityRatio(SATURN_OBLATE_GRAVITY, (latitudeDeg * Math.PI) / 180);
      const placed = limbGeometry(
        SATURN_OBLATE_SPHEROID,
        latitudeDeg,
        (n * SATURN_OBLATE_SCALE_HEIGHT_M) / s,
        SATURN_LIMB_LINE_OF_SIGHT_AZIMUTH_DEG,
        SATURN_LIMB_SUN,
        SATURN_LIMB_DISTANCE_M,
      );
      return {
        name: `limb-lat${latitudeDeg}-${n}H`,
        image: `limb-lat${latitudeDeg}`,
        observer: placed.observer,
        view: placed.view,
        sun: placed.sun,
      };
    }),
  );
  return [...ground, ...limb];
}

// --- The cases -------------------------------------------------------------------------------

/** `earth`: {@link EARTH_REFERENCE} on {@link EARTH_REFERENCE_RADIUS_M}. */
export const EARTH_CASE: ReferenceCase = sphereCase(
  "earth",
  EARTH_REFERENCE,
  EARTH_REFERENCE_RADIUS_M,
  slotZero(EARTH_REFERENCE_CURVES),
  {},
);

/** `earth-ozone-slab`: {@link EARTH_SLAB_REFERENCE} on {@link EARTH_REFERENCE_RADIUS_M}. */
export const EARTH_OZONE_SLAB_CASE: ReferenceCase = sphereCase(
  "earth-ozone-slab",
  EARTH_SLAB_REFERENCE,
  EARTH_REFERENCE_RADIUS_M,
  slotZero(EARTH_SLAB_REFERENCE_CURVES),
  {},
);

/** `mars`: {@link MARS_REFERENCE} on {@link MARS_REFERENCE_RADIUS_M}, with R08.T13's points. */
export const MARS_CASE: ReferenceCase = sphereCase(
  "mars",
  MARS_REFERENCE,
  MARS_REFERENCE_RADIUS_M,
  [],
  { [MARS_DUST.term.name]: MARS_DUST_ASYMMETRY },
  marsAureole(),
);

/** `saturn-oblate`: {@link SATURN_OBLATE_REFERENCE} on Saturn's level spheroid, no aggregates. */
export const SATURN_OBLATE_CASE: ReferenceCase = {
  name: "saturn-oblate",
  medium: SATURN_OBLATE_REFERENCE,
  figure: { kind: "spheroid", gravity: SATURN_OBLATE_GRAVITY },
  curves: [],
  phaseAsymmetry: {},
  sunIlluminanceLx: referenceSunIlluminanceLx(),
  seed: seedOf("saturn-oblate"),
  detectorHalfAngleDeg: REFERENCE_DETECTOR_HALF_ANGLE_DEG,
  geometries: saturnGeometries(),
  aggregates: [],
};

/** The cloud-free cases, in the order their files are listed (R08.T12.b.1). */
export const REFERENCE_CASES: ReadonlyArray<ReferenceCase> = [
  EARTH_CASE,
  EARTH_OZONE_SLAB_CASE,
  MARS_CASE,
  SATURN_OBLATE_CASE,
];
