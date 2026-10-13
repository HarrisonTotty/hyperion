/**
 * The atmosphere's view settings (plan R08, "Settings"): the field R08 adds to R05's
 * `ViewSettings`, `atmosphereView`, with its values on each setting (`view/quality/qualitySetting.ts`,
 * R05 Design note 26), and named projections of the settings, as R05's `TERRAIN_SETTINGS` is of
 * `SETTINGS[s].terrain`, for R12's ladder audit.
 *
 * @remarks
 * R08.T9.a projects the aerial perspective's scope here; R08.T7 adds the view's sources
 * ({@link AtmosphereViewSettings}, {@link ATMOSPHERE_VIEW_SETTINGS}, {@link SKY_SUN_CAP},
 * {@link MAX_SKY_SOURCES}) and {@link ATMOSPHERE_QUALITY_LIMITS}. A projection is read from the
 * field it projects, never held twice; this module takes `TABLE_SIZES` from `hillaire.ts`, which
 * is `SETTINGS[s].atmosphere`, and `qualitySetting.ts` takes {@link ATMOSPHERE_VIEW_SETTINGS} from
 * here, so neither imports the other's importer. The sky's stars are the surface's:
 * {@link AtmosphereViewSettings.skySunCap} is R07's `MAX_BODY_LIGHTS` and
 * {@link AtmosphereViewSettings.skyPlanetshineSources} R07's `PLANETSHINE_SOURCES_HIGH` and `_LOW`,
 * the values of `SETTINGS[s].photoreal.planetshineSources`, never second literals (Design note 7;
 * `decision-r08-design.md` item 3).
 */

import { MAX_BODY_LIGHTS } from "../lighting/hostLights";
import { PLANETSHINE_SOURCES_HIGH, PLANETSHINE_SOURCES_LOW } from "../lighting/planetshine";
import type { QualitySetting } from "../quality/qualitySetting";
import { TABLE_SIZES, type TableSizes } from "./hillaire";

/**
 * The sources the sky's kernels hold, and the stars among them, `hillaire.ts`'s (where the kernels'
 * uniform is packed): exported here for R12's ladder.
 */
export { MAX_SKY_LIGHT_SLOTS, MAX_SKY_SOURCES } from "./hillaire";

/** Where aerial perspective applies: R05's `TableSizes.aerialPerspectiveScope`. */
export type AerialPerspectiveScope = TableSizes["aerialPerspectiveScope"];

/**
 * Where aerial perspective applies on each setting (Design note 11; R08.T9.a): R05's
 * `SETTINGS[s].atmosphere.aerialPerspectiveScope`, projected here rather than held twice.
 *
 * @remarks
 * `scene` (high) applies it to everything opaque: the composite lays it over every opaque pass drawn
 * into the scene before it, and `HillaireAtmosphere.aerialPerspective` publishes the volume and the
 * transmittance tables to the passes it does not cover (R11's). `terrain` (low) applies it to the
 * terrain alone, in R05's one deferred composite, and publishes nothing: the budget's "aerial
 * perspective on terrain only". Beyond the volume's 32 km reach, on both, the composite's ray march
 * takes over.
 */
export const AERIAL_PERSPECTIVE_SCOPE: Readonly<Record<QualitySetting, AerialPerspectiveScope>> = {
  high: TABLE_SIZES.high.aerialPerspectiveScope,
  low: TABLE_SIZES.low.aerialPerspectiveScope,
};

/** What the atmosphere's view takes of a setting: the sky's sources and its stated limits. */
export interface AtmosphereViewSettings {
  /**
   * The stars whose light the sky takes: the body's own, R07's `MAX_BODY_LIGHTS` (2 on both
   * settings), so that the sky's stars are the surface's (Design note 7).
   */
  readonly skySunCap: number;
  /** The planetshine sources the sky takes: `SETTINGS[s].photoreal.planetshineSources`, 2 or 1. */
  readonly skyPlanetshineSources: number;
  /** The quality limits this plan lists for R12's audit (R12.T7.b), one statement each. */
  readonly qualityLimits: ReadonlyArray<string>;
}

/**
 * Design note 7's limits on both settings: a star outside the body's list, and planetshine's
 * approximations.
 */
const SHARED_LIMITS: ReadonlyArray<string> = [
  `The sky and the surface take a body's ${MAX_BODY_LIGHTS} brightest stars past R07's 10⁻⁴ cut; a third star of a triple or higher system lights neither (from 10⁻⁴ to roughly 10⁻¹ of the brightest; while the two brightest are below the horizon it would be the only light, about 13 to 13,000 lx at Earth's insolation, and is drawn as a disc over a night).`,
  "Inside the air, the sky loop skips a source whose upper limb lies below the twilight limit −[acos(R ÷ r_c) + 2 acos(R ÷ R_top)] (20.2° below the horizon on Earth's ground with a 100 km top; single scattering, refraction not drawn), and one under 10⁻⁴ of the brightest source above the horizon (the Moon by day); the surfaces keep both.",
  "Planetshine in the sky is a point source at the neighbour's centre: the band depletion of its reflected spectrum is not modelled; its disc is not taken in the sky's phase function (under 0.8% for Rayleigh at 9.8° of radius, 3% at 20°); a partly set source's sky is lit or shadowed by its centre alone; it takes the absorber curves of the body's brightest star (light slot 0).",
  "Planetshine's illuminance in the sky is the camera's, carried from the body's centre by the inverse square, at every sample of the view's rays (Design note 7 states ±0.9% across an Io-class moon for the body's centre).",
];

/**
 * The atmosphere's view settings per quality setting, `SETTINGS[s].atmosphereView` (R05 Design note
 * 26). The low setting takes one planetshine source, as R07's style does (R07 Design note 18).
 */
export const ATMOSPHERE_VIEW_SETTINGS: Readonly<Record<QualitySetting, AtmosphereViewSettings>> = {
  high: {
    skySunCap: MAX_BODY_LIGHTS,
    skyPlanetshineSources: PLANETSHINE_SOURCES_HIGH,
    qualityLimits: SHARED_LIMITS,
  },
  low: {
    skySunCap: MAX_BODY_LIGHTS,
    skyPlanetshineSources: PLANETSHINE_SOURCES_LOW,
    qualityLimits: [
      ...SHARED_LIMITS,
      `The low setting's sky takes ${PLANETSHINE_SOURCES_LOW} planetshine source, against ${PLANETSHINE_SOURCES_HIGH} on high (R07 Design note 18).`,
    ],
  },
};

/** The sky's stars per setting: `SETTINGS[s].atmosphereView.skySunCap`. */
export const SKY_SUN_CAP: Readonly<Record<QualitySetting, number>> = {
  high: ATMOSPHERE_VIEW_SETTINGS.high.skySunCap,
  low: ATMOSPHERE_VIEW_SETTINGS.low.skySunCap,
};

/** The atmosphere's quality limits per setting: `SETTINGS[s].atmosphereView.qualityLimits`. */
export const ATMOSPHERE_QUALITY_LIMITS: Readonly<Record<QualitySetting, ReadonlyArray<string>>> = {
  high: ATMOSPHERE_VIEW_SETTINGS.high.qualityLimits,
  low: ATMOSPHERE_VIEW_SETTINGS.low.qualityLimits,
};
