/**
 * The atmosphere's view settings (plan R08, "Settings"): the field R08 adds to R05's
 * `ViewSettings`, `atmosphereView`, with its values on each setting (`view/quality/qualitySetting.ts`,
 * R05 Design note 26), and named projections of the settings, as R05's `TERRAIN_SETTINGS` is of
 * `SETTINGS[s].terrain`, for R12's ladder audit.
 *
 * @remarks
 * R08.T9.a projects the aerial perspective's scope here, and R08.T8 the march band
 * ({@link MARCH_BAND}); R08.T7 adds the view's sources
 * ({@link AtmosphereViewSettings}, {@link ATMOSPHERE_VIEW_SETTINGS}, {@link SKY_SUN_CAP},
 * {@link MAX_SKY_SOURCES}) and {@link ATMOSPHERE_QUALITY_LIMITS}, and R08.T8.e the march's source
 * cap ({@link AtmosphereViewSettings.marchSourceCap}). A projection is read from the
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
import { type MarchBand, TABLE_SIZES, type TableSizes } from "./hillaire";

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

/**
 * Where the ray march takes over from the sky-view table and the aerial-perspective volume as the
 * camera climbs, on each setting (plan R08, R08.T8: "the switch to the sky-view table is by
 * altitude, with a blend band"): R05's `SETTINGS[s].atmosphere.marchBand`, projected here rather
 * than held twice, each edge a share of the medium's top in h\*.
 *
 * @remarks
 * 0.3 to 0.5 of the top on high and 0.2 to 0.3 on low: 30–50 km and 20–30 km under Earth's
 * 100 km. Above the band the march draws every pixel, as from above the top, at the cost it has
 * there. `hillaire.ts`'s `TABLE_SIZES` gives the measurement it was set from.
 */
export const MARCH_BAND: Readonly<Record<QualitySetting, MarchBand>> = {
  high: TABLE_SIZES.high.marchBand,
  low: TABLE_SIZES.low.marchBand,
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
  /**
   * The most sources the per-pixel ray march takes, the brightest left by Design note 7's skips (2
   * on both settings; R08.T8.e, `decision-r08-t8-leans.md` §9): `SETTINGS[s].atmosphere`'s
   * `marchSourceCap`, which the kernels read (`hillaire.ts`'s `marchSourcesOf`), projected here.
   * The sky view, the aerial perspective and the surfaces take every source the skips leave.
   */
  readonly marchSourceCap: number;
  /** The quality limits this plan lists for R12's audit (R12.T7.b), one statement each. */
  readonly qualityLimits: ReadonlyArray<string>;
}

/**
 * Design note 7's limits on both settings: a star outside the body's list, and planetshine's
 * approximations.
 */
const SHARED_LIMITS: ReadonlyArray<string> = [
  `The sky and the surface take a body's ${MAX_BODY_LIGHTS} brightest stars past R07's 10⁻⁴ cut; a third star of a triple or higher system lights neither (from 10⁻⁴ to roughly 10⁻¹ of the brightest; while the two brightest are below the horizon it would be the only light, about 13 to 13,000 lx at Earth's insolation, and is drawn as a disc over a night).`,
  "Inside the air and outside it, the sky loop skips a source whose upper limb lies below the twilight limit −[acos(R ÷ r_c) + 2 acos(R ÷ R_top)] (20.2° below the horizon on Earth's ground with a 100 km top, 40.1° from 400 km; single scattering, refraction not drawn), and one under 10⁻⁴ of the brightest source standing at least acos(R ÷ r_c) above the horizontal (on the ground the horizon; the Moon by day); the surfaces keep both.",
  "Planetshine in the sky is a point source at the neighbour's centre: the band depletion of its reflected spectrum is not modelled; its disc is not taken in the sky's phase function (under 0.8% for Rayleigh at 9.8° of radius, 3% at 20°); a partly set source's sky is lit or shadowed by its centre alone; it takes the absorber curves of the body's brightest star (light slot 0).",
  "Planetshine's illuminance in the sky is the camera's, carried from the body's centre by the inverse square, at every sample of the view's rays (Design note 7 states ±0.9% across an Io-class moon for the body's centre).",
];

/**
 * The march's source cap as a quality limit on `setting` (R08.T8.e; `decision-r08-t8-leans.md` §9):
 * from the march band up, and on surfaces beyond the aerial volume, the air takes at most the cap's
 * sources' light; inside the band, where the composite blends the march with the sky view, and at
 * the volume's reach, a source past the cap fades.
 */
function marchSourceLimit(setting: QualitySetting): string {
  const { marchSourceCap, aerialPerspectiveReachM } = TABLE_SIZES[setting];
  return `From the march band up, and on terrain beyond the aerial volume, a body's air takes at most ${marchSourceCap} sources' light, the brightest left by Design note 7's skips; inside the band, and at the aerial volume's ${aerialPerspectiveReachM / 1_000} km reach, a source past them fades.`;
}

/**
 * The atmosphere's view settings per quality setting, `SETTINGS[s].atmosphereView` (R05 Design note
 * 26). The low setting takes one planetshine source, as R07's style does (R07 Design note 18).
 */
export const ATMOSPHERE_VIEW_SETTINGS: Readonly<Record<QualitySetting, AtmosphereViewSettings>> = {
  high: {
    skySunCap: MAX_BODY_LIGHTS,
    skyPlanetshineSources: PLANETSHINE_SOURCES_HIGH,
    marchSourceCap: TABLE_SIZES.high.marchSourceCap,
    qualityLimits: [...SHARED_LIMITS, marchSourceLimit("high")],
  },
  low: {
    skySunCap: MAX_BODY_LIGHTS,
    skyPlanetshineSources: PLANETSHINE_SOURCES_LOW,
    marchSourceCap: TABLE_SIZES.low.marchSourceCap,
    qualityLimits: [
      ...SHARED_LIMITS,
      marchSourceLimit("low"),
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
