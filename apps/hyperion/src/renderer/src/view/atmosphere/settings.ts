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
import { type AtmosphereSettings, type MarchBand, TABLE_SIZES, type TableSizes } from "./hillaire";
import { GIANT_ROUTE, type GiantRoute } from "./thick/routing";

/**
 * The sources the sky's kernels hold, and the stars among them, `hillaire.ts`'s (where the kernels'
 * uniform is packed): exported here for R12's ladder.
 */
export { MAX_SKY_LIGHT_SLOTS, MAX_SKY_SOURCES } from "./hillaire";

/**
 * The atmosphere's levers' types and the composite as drawn before them, `hillaire.ts`'s (where the
 * frame takes them), exported here beside their values per setting.
 */
export {
  type AtmosphereSettings,
  type CompositeTransmittance,
  PER_PIXEL_LEVERS,
  type SurfaceLobes,
  type SurfaceTransmittance,
} from "./hillaire";

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
  /**
   * How a cool giant's envelope is drawn inside 10⁹ m (R08.T16.a.2; `thick/routing.ts`'s
   * {@link GiantRoute}): `envelope` on both settings, through its deck's route, a lean
   * (2026-11-05); `closed` is the way back. The atmosphere cache reads it as it is made.
   */
  readonly giantRoute: GiantRoute;
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
    giantRoute: GIANT_ROUTE,
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
    giantRoute: GIANT_ROUTE,
  },
};

/** The sky's stars per setting: `SETTINGS[s].atmosphereView.skySunCap`. */
export const SKY_SUN_CAP: Readonly<Record<QualitySetting, number>> = {
  high: ATMOSPHERE_VIEW_SETTINGS.high.skySunCap,
  low: ATMOSPHERE_VIEW_SETTINGS.low.skySunCap,
};

/**
 * The atmosphere's levers per setting (the owner's priority, 2026-11-01).
 *
 * @remarks
 * `march` reaches a frame only where the march runs at half the output's resolution: on low, and on
 * high once its frame-rate floor draws the air at low's sizes; at full resolution the composite's
 * pixels are the march's texels, and it forms the term per pixel as before. On SwiftShader at low
 * from orbit (pod, 16 vCPU) the term took 480 of the composite's 607 ms.
 *
 * `channels` (plan R08, R08.T9.c) dims a drawn surface seen through the air per channel on both
 * settings, its cost the second textures' memory (the volume's 262 KB on high, 131 KB on low, and
 * 8 bytes a march texel) and one more store a froxel and a march texel; `grey` is the way back.
 *
 * `transmitted` (plan R08, fix-r08-decked-air-opacity) keeps a drawn surface's light scattered
 * within the medium's forward lobes in the march's second target on both settings, its cost one sum
 * over the medium's terms and one `exp` a march step where the medium has a drawn lobe, none
 * elsewhere; `extinguished` is the way back. Since fix-r08-decked-air-route the term is formed only
 * on a ray that ends on a drawn surface, in a loop of at most one pass, its optical depth summed
 * with one `exp` a ray, and a deck's march compiles it out. On SwiftShader its code costs the
 * march's compile (test-render's venus-92bar row, part 2: 341 s against BASE's 252 s, side by side
 * on a pod, from 374–381 s before the deck's march compiled it out), not its frames.
 *
 * `packed` (plan R08, R08.T8.d) takes the far limb's node rays one an invocation on both settings:
 * byte for byte the per-texel march on SwiftShader, and from orbit in the inspection profile (pod,
 * 16 vCPU, from 22 Mm and 28.9 Mm, each against the composite's time) 29% less of the march on high
 * at the floor's low sizes and 16% less on low; `per-texel` is the way back. `area` stays the
 * march's cover on both: under the packed limb the tile cull was within 5% of it either way there,
 * so the owner's Mac decides it.
 *
 * `table` (plan R08, R08.T9.b's lever) takes each source's transmittance to a lit point from the
 * sources' table on both settings, a body of several bands per fragment within 0.1 of the
 * horizon's cosine; `per-fragment` is the way back, which an inspection profile's `--lit-probe
 * per-fragment` draws in the same build. On SwiftShader at low from orbit (pod, 16 vCPU) the
 * sliced lookups were 57–66% of the bodies pass and 72–74% of the disc cells (every source's
 * transmittance 1 against as drawn). The smoke holds the table's reads within 1/3 stop of the
 * lookups per fragment at 99.96% of Earth's lit points and all of the Saturn-class figure's, and
 * every lit pixel of R05's Earth's air bodies drawn both ways. A body of one band is drawn by the
 * lit modules' table-only twins, the sliced lookup compiled out (`SourceTableEngine`), which draw
 * the modules' bytes.
 *
 * `table` (plan R08, R08.T8.e's table, `marchSourceTransmittance`) takes the thin march's step's
 * sources from the same table on both settings; `per-step` is the way back, which an inspection
 * profile's `--march-sources per-step` draws in the same build. On SwiftShader from orbit (pod, 32
 * vCPU, 28.9 Mm, the general march over several bands) the step's lookups were about 40% of the
 * march (`--march-probe depth-zero` against as drawn), and the table took 33% off the march at low
 * and 34% at high against `per-step`, 20% and 16% against the march before the lever. The smoke
 * holds the march from the table within 1/3 stop of per step on 99.5% of its frames' sky texels
 * or more, and a body of one band's table-only march twins to the module's bytes.
 *
 * Since perf-thick-march both levers reach the thick read's march but over a deck, which every world
 * with air takes once its correction lands. `packed` takes it byte for byte, within −6% to +4% of
 * per texel on SwiftShader from orbit and from 400 km (pod, 32 vCPU; the measured Earth, R05's
 * Earth and TEST TITAN over thick tables), though the limb's node rays are 33–51% of the march
 * where they are a share (`--march-probe centre`): the pod's lanes wait little on a tile's slowest
 * texel, so the owner's Mac decides it. `table` takes the thick read's shared read on a body of one
 * band from the table alone, 16% off the march at high there; its march by slot, whose lookup is one
 * read a source a step already, took 3–14% more from the table, and stays per step, as a body of
 * several bands does.
 */
export const ATMOSPHERE_SETTINGS: Readonly<Record<QualitySetting, AtmosphereSettings>> = {
  high: {
    compositeTransmittance: "march",
    surfaceTransmittance: "channels",
    surfaceLobes: "transmitted",
    marchCover: "area",
    limbRays: "packed",
    sourceTransmittance: "table",
    marchSourceTransmittance: "table",
  },
  low: {
    compositeTransmittance: "march",
    surfaceTransmittance: "channels",
    surfaceLobes: "transmitted",
    marchCover: "area",
    limbRays: "packed",
    sourceTransmittance: "table",
    marchSourceTransmittance: "table",
  },
};

/** The atmosphere's quality limits per setting: `SETTINGS[s].atmosphereView.qualityLimits`. */
export const ATMOSPHERE_QUALITY_LIMITS: Readonly<Record<QualitySetting, ReadonlyArray<string>>> = {
  high: ATMOSPHERE_VIEW_SETTINGS.high.qualityLimits,
  low: ATMOSPHERE_VIEW_SETTINGS.low.qualityLimits,
};
