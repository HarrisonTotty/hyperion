/**
 * Hillaire's atmosphere drawn every frame (plan R05, R05.T12.c, Design note 16): the sky-view
 * table, the aerial-perspective volume and the per-pixel ray march, built from the per-planet tables
 * of `tables.ts`, and the composite that lays them over the terrain target.
 *
 * @remarks
 * The per-planet tables are built once on a sphere whose ground is the figure's mean radius R₁ =
 * (2a + c) ÷ 3, and rebuilt only when the medium changes, never when the sun moves. Every lookup
 * reads them at a point's height above the datum spheroid, with its sun and view cosines taken
 * against the spheroid's normal (Design note 16): the per-frame tables are built on the camera's
 * own sphere, of its Gaussian radius √(MN), and the ray march clips against the spheroid's shells.
 * The per-planet tables hold radiance per unit of illuminance in each channel, the smooth terms
 * evaluated at the fitted channels' wavelengths (plan R08, Design note 5, R08.T6.d). Since R08.T7
 * the per-frame tables are summed over the frame's sky sources in one march (Design note 7): up to
 * {@link MAX_SKY_SOURCES}, each weighted by its per-channel illuminance (R07's `starIlluminance`
 * through R06's colour, {@link SunState.illuminanceLx}) times the frame's pre-exposure, formed in
 * `f64` ({@link frameSourcesOf}). So they hold pre-exposed Rec. 709 luminance, as the composite's
 * output does, and the half float's range follows the exposure into twilight and moonlight
 * (`decision-r08-t6b-floor.md` §6). Each source takes its own light slot's absorber curves: its
 * transmittance, its multiple-scattering layer, its absorber depth along every view path and its
 * disc (`decision-r08-t6b-floor.md` §8). The sky view is indexed by the local azimuth about the
 * first source's horizontal ({@link skyAxisOf}). The composite multiplies what the scene holds
 * behind the sky by its transmittance to space, a disc's own light's inside it, draws the discs of
 * the sources that draw one, and keeps the scene's alpha, R07's `METER_CLASS`.
 *
 * The kernels are `f32`. What they cannot form there without losing it, they are given formed in
 * `f64` and narrowed once (plan R08, Design note 18, R08.T6.g): the camera's height above the
 * datum and the datum's normal, the ray march's quadric constants, the sky view's horizon angle
 * and each source's direction with the sine of its angular radius ({@link viewExactOf},
 * {@link frameSourcesOf}). No kernel forms a height as a difference of radii, and none takes WGSL's
 * builtin trigonometry (`view/shaders/preciseTrig.wgsl`).
 *
 * The sky view and the ray march cut each side of their rays' split where the ray crosses one of
 * the medium's layer edges, the tables' `layerEdges` (plan R08, R08.T6.i; `tables.ts`'s
 * `packLayerEdges`), at the same counts; on a medium with none their steps are R05.T12.e's (since
 * R08.T8 the ray march's two-sided sky segments, inside the air or outside it, take the sun-aware
 * warp, even where the Sun is ahead or across the ray and R05's one-sided placement from the
 * segment's start where it is behind; `marchSteps.ts`'s `marchEvenStep` and `marchEvenSteps`). The
 * aerial perspective keeps its even steps.
 *
 * Aerial perspective (plan R08, R08.T9.a). The composite lays it over every opaque surface of the
 * scene it is given, through the volume within its 32 km reach and the ray march beyond it. Along
 * the froxels' own rays the march's in-scattering agrees with the volume's to 2% at the volume's
 * far edge (`aerialPerspective.test.ts`); between froxels the volume's bilinear read near the
 * horizon misses it by up to 3.0% at 60° across, and 10.4% at 127° (plan R08's Risks, deferred).
 * Where it applies is the setting's `aerialPerspectiveScope` (`settings.ts`'s
 * `AERIAL_PERSPECTIVE_SCOPE`): on `terrain` (low) the scene is the terrain alone and nothing else
 * takes it; on `scene` (high) every opaque pass drawn into the scene before the composite takes it
 * there, and {@link HillaireAtmosphere.aerialPerspective} publishes the volume and the
 * transmittance tables for the passes the composite does not cover. {@link HillaireAtmosphere.skyView}
 * publishes the frame's sky-view table on both.
 */

import { dot, normalise, scale as scaled, sub, type Vec3 } from "../../geometry/vec3";
import type { Quaternion } from "../camera/pose";
import { rotate } from "../camera/quaternion";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../engine/gpuFlags";
import type { KernelPair } from "../engine/kernels";
import type {
  BufferHandle,
  ComputeHandle,
  DrawItem,
  MaterialHandle,
  MeshHandle,
  RenderEngine,
  TextureHandle,
  ViewSize,
  WgslMaterialSpec,
} from "../engine/types";
import { photopicIlluminance } from "../lighting/illuminance";
import type { Rgb } from "../photometry/toneCurve";
import type { QualitySetting } from "../quality/qualitySetting";
import preciseTrigWgsl from "../shaders/preciseTrig.wgsl?raw";
import {
  MARCH_TILE_TEXELS,
  type MarchArea,
  marchAreaOf,
  marchAreaTexels,
  type MarchShell,
  wholeMarchArea,
} from "./marchArea";
import { DRAWN_SURFACE_BAND_M, type MarchDepthSteps } from "./marchSteps";
import { type AtmosphereMedium, localScaleHeightM } from "./medium";
import { type BodyGravity, bodyGravity, gravityRatio, principalRadiiM } from "./oblate";
import commonWgsl from "./shaders/common.wgsl?raw";
import aerialPerspectiveWgsl from "./shaders/aerialPerspective.wgsl?raw";
import aerialReadWgsl from "./shaders/aerialRead.wgsl?raw";
import compositeWgsl from "./shaders/composite.wgsl?raw";
import farLimbWgsl from "./shaders/farLimb.wgsl?raw";
import marchAreaWgsl from "./shaders/marchArea.wgsl?raw";
import marchFillWgsl from "./shaders/marchFill.wgsl?raw";
import mediumWgsl from "./shaders/medium.wgsl?raw";
import oblateWgsl from "./shaders/oblate.wgsl?raw";
import rayMarchWgsl from "./shaders/rayMarch.wgsl?raw";
import rayMarchCentreWgsl from "./shaders/rayMarchCentre.wgsl?raw";
import rayMarchCurvesWgsl from "./shaders/rayMarchCurves.wgsl?raw";
import rayMarchFootprintWgsl from "./shaders/rayMarchFootprint.wgsl?raw";
import rayMarchSlotsWgsl from "./shaders/rayMarchSlots.wgsl?raw";
import scatteringWgsl from "./shaders/scattering.wgsl?raw";
import skyViewWgsl from "./shaders/skyView.wgsl?raw";
import sourceWgsl from "./shaders/source.wgsl?raw";
import thickReadWgsl from "./shaders/thickRead.wgsl?raw";
import viewWgsl from "./shaders/view.wgsl?raw";
import {
  absorberCount,
  lineSlotMask,
  type AbsorberCurveSlots,
  type AtmosphereBody,
  AtmosphereTables,
  KERNEL_ARM_FORMAT,
  marchEdgeCountOf,
  MULTI_SCATTERING_SIZE,
  NO_ABSORBER_CURVES,
  packMediumUniform,
  type TableSize,
  TRANSMITTANCE_SIZE,
  withKernelArmFormat,
} from "./tables";
import {
  THICK_ALTITUDES,
  THICK_MODES,
  THICK_SUN_COSINES,
  THICK_TRAVEL_COSINES,
} from "./thick/read";

/** A 3D table's size, texels. */
export interface VolumeSize {
  readonly widthTexels: number;
  readonly heightTexels: number;
  readonly slices: number;
}

/**
 * The atmosphere's table sizes and sample counts for one quality setting (Design note 16): the
 * field `SETTINGS[s].atmosphere`, which R08 generalises.
 */
export interface TableSizes {
  /** The per-planet transmittance table, the same on both settings. */
  readonly transmittance: TableSize;
  /** The per-planet multiple-scattering table, the same on both settings. */
  readonly multiScattering: TableSize;
  readonly skyView: TableSize;
  /** Steps along each sky-view ray. */
  readonly skyViewSamples: number;
  readonly aerialPerspective: VolumeSize;
  /** Steps through each slice of the aerial-perspective volume. */
  readonly aerialPerspectiveSamplesPerSlice: number;
  /** The aerial-perspective volume's reach, m. */
  readonly aerialPerspectiveReachM: number;
  /** The ray march's resolution as a fraction of the output's: 1, or 0.5 with a depth-aware upsample. */
  readonly rayMarchScale: 1 | 0.5;
  /** Steps along each ray of the per-pixel ray march. */
  readonly rayMarchSamples: number;
  /**
   * Where aerial perspective applies (plan R08, Design note 11, R08.T9.a; projected as `settings.ts`'s
   * `AERIAL_PERSPECTIVE_SCOPE`): `scene`, the high setting's, over everything opaque, the composite
   * over the opaque passes drawn before it and the volume published to the rest
   * ({@link HillaireAtmosphere.aerialPerspective}); or `terrain`, the low setting's, applied in the
   * one deferred composite to the terrain alone (the budget's "aerial perspective on terrain only").
   */
  readonly aerialPerspectiveScope: "scene" | "terrain";
  /**
   * Where the ray march takes over the sky from the sky-view table, and the surfaces from the
   * aerial-perspective volume, as the camera climbs (plan R08, R08.T8; projected as `settings.ts`'s
   * `MARCH_BAND`): {@link marchWeightOf}'s band.
   */
  readonly marchBand: MarchBand;
  /**
   * The most sources the per-pixel ray march takes, the brightest of the frame's by photopic
   * illuminance at the camera ({@link marchSourcesOf}; plan R08, R08.T8.e,
   * `decision-r08-t8-leans.md` §9; projected as `settings.ts`'s `atmosphereView.marchSourceCap`):
   * the sky view, the aerial perspective and the composite take every source drawn. At least 1.
   */
  readonly marchSourceCap: number;
  /**
   * The ray march's count scaled by each segment's optical depth (plan R08, R08.T8.e; lever 3 of
   * `decision-r08-t8-leans.md` §8, §9's Ruling 2, item 3; `marchSteps.ts`'s
   * {@link MarchDepthSteps} and `marchSamplesOf`): at most {@link TableSizes.rayMarchSamples}, the
   * bare ground down to the floors R05.T12.e's gate sets and a thin sky segment at its own count
   * (§10, Ruling 4, item 2), a drawn surface on the datum at the bare ground's (R08.T11.b), other
   * sky segments and drawn surfaces at the setting's count; `null` where every segment takes the
   * setting's count.
   */
  readonly marchDepthSteps: MarchDepthSteps | null;
  /**
   * The thick multiple-scattering table's grid, a band and light slot (plan R08, Design note 9;
   * R08.T14.c): the bake's, the same on both settings, since the bake costs CPU at arrival and not
   * frame time (Design note 11).
   */
  readonly thick: ThickTableSize;
}

/**
 * The thick table's grid (plan R08, Design note 9; R08.T14.c; `thick/read.ts`): its altitudes, sun
 * cosines, travel cosines and azimuthal modes, the bake's `THICK_BAKE_GRID` and `THICK_BAKE_MODES`.
 */
export interface ThickTableSize {
  readonly altitudes: number;
  readonly sunCosines: number;
  readonly travelCosines: number;
  readonly modes: number;
}

/**
 * The thick table's grid as built (R08.T14.c): 33 altitudes, 32 μ₀ and 16 μ_v, m = 0 and 1,
 * 270,336 B a band and light slot as `rgba16float` (`thick/read.ts`'s `THICK_TABLE_BYTES_A_SLOT`).
 */
export const THICK_TABLE_SIZE: ThickTableSize = {
  altitudes: THICK_ALTITUDES,
  sunCosines: THICK_SUN_COSINES,
  travelCosines: THICK_TRAVEL_COSINES,
  modes: THICK_MODES,
};

/**
 * The band of camera heights over which the per-pixel ray march takes over from the sky-view table
 * and the aerial-perspective volume (plan R08, R08.T8: "the switch to the sky-view table is by
 * altitude, with a blend band"), each edge a share of the medium's top in h\*, the height the
 * medium is read at (Design note 17).
 *
 * @remarks
 * Below the lower edge the tables serve as R05 drew them, and from the upper edge up the march
 * draws everything, as it does from above the top; between them the composite blends the two
 * linearly in h\*. The band stands well below the top because the sky view's bilinear read across
 * the limb errs more the higher the camera, where the march does not: measured in the twin on
 * R05's Earth (plan R08's Risks, "Deviations in T8.a, as built").
 */
export interface MarchBand {
  /** The band's lower edge, a share of the medium's top, in [0, 1). */
  readonly lowerShareOfTop: number;
  /** The band's upper edge, a share of the medium's top, in (`lowerShareOfTop`, 1]. */
  readonly upperShareOfTop: number;
}

/**
 * The ray march's share of the frame at a camera whose h\* is `heightStarM` under a medium
 * `topHeightM` deep (plan R08, R08.T8): 0 below `band`'s lower edge, where the sky-view table and
 * the aerial-perspective volume serve; 1 from its upper edge up, the top and above included, where
 * the march draws everything; linear in h\* between them, the composite's blend. Formed in `f64`
 * for the kernels (`view.wgsl`'s `AtmosphereView.exact.w`) and the twins alike.
 *
 * @throws RangeError for a band whose edges are not 0 ≤ lower < upper ≤ 1, a top that is not
 *   finite and above 0, or a height that is not a number.
 */
export function marchWeightOf(heightStarM: number, topHeightM: number, band: MarchBand): number {
  const { lowerShareOfTop: lower, upperShareOfTop: upper } = band;
  if (Number.isNaN(heightStarM)) {
    throw new RangeError("the camera's height must be a number");
  }
  if (!(lower >= 0 && lower < upper && upper <= 1)) {
    throw new RangeError(`a march band runs from 0 ≤ lower < upper ≤ 1, not ${lower} to ${upper}`);
  }
  if (!(Number.isFinite(topHeightM) && topHeightM > 0)) {
    throw new RangeError(`the medium's top must be finite and above 0, not ${topHeightM} m`);
  }
  const share = (heightStarM / topHeightM - lower) / (upper - lower);
  return Math.min(Math.max(share, 0), 1);
}

/**
 * The atmosphere's sizes per quality setting (Design note 16).
 *
 * @remarks
 * High, Hillaire's code: sky-view 192 × 108 (sebh's table) with 75 steps, the aerial-perspective volume
 * 32³ reaching 32 km (Hillaire 2020, Table 2 and §5.4, and Bevy), the ray march at full resolution
 * with 36 steps. Low: sky-view 128 × 64 with 16 steps, the volume 32 × 32 × 16, the ray march at
 * half resolution with 16 steps, aerial perspective on the terrain alone. The per-planet tables are
 * full size on both (128 KB, rarely built).
 *
 * The sky view and the ray march place their steps toward each ray's lowest point, not evenly
 * (`marchSteps.ts`; since R08.T8 the ray march's two-sided sky segments, inside the air or outside
 * it, take the sun-aware warp, even where the Sun is ahead or across the ray and R05's one-sided
 * placement from the segment's start where it is behind, `marchEvenStep` and `marchEvenSteps`), and
 * their counts are R05.T12.e's: its
 * quadrature gate (`marchSteps.test.ts`)
 * holds high's within 2% of a converged march (5% for twilight rays), where even steps were up to
 * 33% off, and low's 16 and 16 no worse over the gate than even steps at the same counts. The high
 * sky view takes 75 steps, not Hillaire 2020's 30 (Table 2; addendum B of
 * `decision-r05-high-atmosphere.md`): from cameras at 60–100 km a limb ray's narrow source at its
 * tangent point needs them, where 30 erred by up to 9.0% at twilight (the band seen from them; 8.2%
 * on their limb) and 75 by 3.2%, in the twin. A count changes only through that gate, T18's
 * included. Two
 * steps a slice of the aerial-perspective volume is this module's choice; T18 revisits it with the
 * spike's measurements.
 *
 * The high ray march takes 36 steps since plan R08's R08.T7, where R05.T12.e set 32: on a medium with
 * layer edges the twilight limb ray "limb, 0.1 H, sun 90°/0°" of the slab Earth missed its 5% at 32
 * (5.63%), from the optically thick air about its lowest point, and 36 is the measured fallback that
 * holds every ray of the slab Earth's gate (`decision-r08-t6d-findings.md` §11.4). The cure that
 * adds no step, a floor on the piece next to t*, cured that ray (2.76%) but put five others over
 * (the disc at 2.80%, against 2%), and was not adopted (plan R08's Risks, "Deviations in T7, as
 * built"). It costs 12.5% more on every high ray-march pixel.
 *
 * The march takes over the sky from the sky view between 0.3 and 0.5 of the medium's top on high,
 * and between 0.2 and 0.3 on low (plan R08, R08.T8; {@link MarchBand}). Measured in the twin on
 * R05's Earth (a 100 km top), against a march of 1,024 steps, at the 5% metric's floor (6 × 10⁻³ of
 * the frame's brightest): the sky view's read across the upper limb is 2.31% off from 30 km, 4.30%
 * from 50 km, 6.21% from 60 km and 15.8% from 1 m under the top on high, and 4.26% from 20 km,
 * 5.73% from 30 km and 18.2% from 60 km on low, where each setting's march, every two-sided sky segment in
 * even steps (`decision-r08-t8-leans.md` §4), stays within 0.40–1.55% on high from 20 km to 1 m under
 * the top and 2.0–3.5% on low from 20 to 30 km (`marchBand.test.ts`). Through high's band the
 * composite's blend stays within 2.31%, inside Design note 10's 5%.
 *
 * The march takes at most two sources on both settings, the brightest left by Design note 7's
 * skips (plan R08, R08.T8.e; `decision-r08-t8-leans.md` §9, Ruling 2): each source costs it about
 * 3.5 ms on R05's Earth and 5.5 ms on `EARTH_REFERENCE` from 400 km on high on the RTX 3080 (plan
 * R08's Risks, "Deviations in T8.d, as built"), and two lets a binary's second star light its air.
 *
 * High's march scales the bare ground's count by its optical depth since R08.T8.e (lever 3 of
 * `decision-r08-t8-leans.md` §8, §9's Ruling 2, item 3; {@link TableSizes.marchDepthSteps}): the
 * bare ground takes 20 steps, 4 more for each of the medium's march edges, climbing back to 36 in
 * even steps from a green optical depth of 2 to 8; a sky segment whose green optical depth, summed
 * from its lowest point along each side, is under 0.05 takes 28 (§10's floor, searched from 16 up:
 * under 28 the band's last-light window broke, and 12 broke the far limb's flicker and the band's
 * seam; `marchSteps.ts`'s `MarchDepthSteps`). Since R08.T11.b (lever A of
 * `decision-r08-t11-frame.md` §2) a drawn surface on a ray that meets the datum, at or beyond its
 * hit or within 25 km above it before the hit, takes the bare ground's count, R07's face and
 * terrain among them; anything else drawn keeps 36. R05.T12.e's gate holds every ray within its
 * tolerance at one source and at six on R05's Earth, the measured Earth and the slab Earth, the
 * slab's named misses unmoved, its drawn surfaces on the datum and at the band's edge among them.
 * Low keeps its 16.
 */
export const TABLE_SIZES: Readonly<Record<QualitySetting, TableSizes>> = {
  high: {
    transmittance: TRANSMITTANCE_SIZE,
    multiScattering: MULTI_SCATTERING_SIZE,
    skyView: { widthTexels: 192, heightTexels: 108 },
    skyViewSamples: 75,
    aerialPerspective: { widthTexels: 32, heightTexels: 32, slices: 32 },
    aerialPerspectiveSamplesPerSlice: 2,
    aerialPerspectiveReachM: 32_000,
    rayMarchScale: 1,
    rayMarchSamples: 36,
    aerialPerspectiveScope: "scene",
    marchBand: { lowerShareOfTop: 0.3, upperShareOfTop: 0.5 },
    marchSourceCap: 2,
    marchDepthSteps: {
      groundFloor: 20,
      groundFloorPerEdge: 4,
      groundRampDepths: [2, 8],
      thinSky: { samples: 28, depthG: 0.05 },
      drawnBandM: DRAWN_SURFACE_BAND_M,
    },
    thick: THICK_TABLE_SIZE,
  },
  low: {
    transmittance: TRANSMITTANCE_SIZE,
    multiScattering: MULTI_SCATTERING_SIZE,
    skyView: { widthTexels: 128, heightTexels: 64 },
    skyViewSamples: 16,
    aerialPerspective: { widthTexels: 32, heightTexels: 32, slices: 16 },
    aerialPerspectiveSamplesPerSlice: 2,
    aerialPerspectiveReachM: 32_000,
    rayMarchScale: 0.5,
    rayMarchSamples: 16,
    aerialPerspectiveScope: "terrain",
    marchBand: { lowerShareOfTop: 0.2, upperShareOfTop: 0.3 },
    marchSourceCap: 2,
    marchDepthSteps: null,
    thick: THICK_TABLE_SIZE,
  },
};

/**
 * The datum spheroid the atmosphere stands on: R05's `BodyFigure` (planet.ts, T7.a) has these
 * fields among others, and is accepted wherever this is.
 */
export interface SpheroidFigure {
  readonly equatorialRadiusM: number;
  readonly polarRadiusM: number;
}

/**
 * The camera in the body-fixed axes (z along the pole), as selection's `ViewSelectionInput.camera`,
 * with its field of view and presented size.
 */
export interface AtmosphereCamera {
  /** From the body's centre, m. */
  readonly positionM: Vec3;
  /** Rotates camera axes (−z forward, +y up) into the body-fixed axes. */
  readonly orientation: Quaternion;
  /** The horizontal field of view, rad. */
  readonly fovXRad: number;
  /** The presented size, device pixels. */
  readonly viewport: ViewSize;
}

/**
 * The sources the sky's kernels hold (plan R08, Design note 7; R08.T7): four stars, this plan's
 * first high figure, kept for R07's deferred raise of `MAX_BODY_LIGHTS`, and two planetshine
 * sources. The per-frame bound is 2 + 2 sources on high and 2 + 1 on low, so a raise of R07's count
 * changes no kernel. `settings.ts` exports it, which takes `TABLE_SIZES` from here; `view.wgsl`
 * holds the same number.
 */
export const MAX_SKY_SOURCES = 6;

/**
 * The stars among {@link MAX_SKY_SOURCES}, and so the light slots a march tracks a path for: each
 * star's own absorber curves, a planetshine source taking slot 0's (Design note 7).
 * `settings.ts` exports it; `view.wgsl` holds the same number.
 */
export const MAX_SKY_LIGHT_SLOTS = 4;

/**
 * One of the sky's sources seen from the body: a star of the body's list or a planetshine source
 * (plan R08, Design note 7; R08.T7's `sources.ts` makes them from R07's arrays).
 */
export interface SunState {
  /** The unit direction to the source in the body-fixed axes. */
  readonly directionBodyFixed: Vec3;
  /**
   * The source's illuminance face-on, per linear Rec. 709 channel, lx: for a star R07's
   * `starIlluminance(disc, distance)` (`view/lighting/illuminance.ts`), the star's photopic
   * illuminance spread over the channels by R06's colour (plan R08, Design note 5, R08.T6.d); for a
   * planetshine source R07's `SecondarySource.illuminance`, carried to the camera by the inverse
   * square. The per-view tables hold each source's light per unit of the frame's scale
   * ({@link HillaireAtmosphere.drawFrame}), weighted by it channel by channel.
   */
  readonly illuminanceLx: Rgb;
  /**
   * The disc's angular radius, rad: asin(R ÷ d), which the caller keeps consistent with
   * `illuminanceLx`, the two from one disc at one distance, so that the disc's luminance is the
   * source's mean luminance whatever the distance. Inside it the composite takes the source's own
   * transmittance to space for what lies behind (R08.T7). 0 for none (the comparison with Hillaire's
   * images, which have none).
   */
  readonly angularRadiusRad: number;
  /**
   * The light slot whose absorber curves of growth its light takes (Design note 5;
   * `decision-r08-t6b-floor.md` §8): a star's rank in the body's list, 0 the brightest, and 0 for a
   * planetshine source, which takes the body's brightest star's. Absent: 0.
   */
  readonly curveSlot?: number;
  /**
   * Whether the composite draws the disc itself, a uniform disc of the source's mean luminance.
   * `false` where R06's host-disc layer draws it into the scene beneath (Design note 7), and for a
   * planetshine source, which R07 draws. Absent: drawn when `angularRadiusRad` is above 0.
   */
  readonly drawsDisc?: boolean;
}

/** A source's light slot ({@link SunState.curveSlot}), 0 when absent. */
export function curveSlotOf(sun: SunState): number {
  return sun.curveSlot ?? 0;
}

/** Whether the composite draws a source's disc ({@link SunState.drawsDisc}). */
export function drawsDiscOf(sun: SunState): boolean {
  return sun.drawsDisc ?? sun.angularRadiusRad > 0;
}

/**
 * DN 16's lookup inputs at the camera, widened by R08.T6.f with its latitude and gravity scale
 * (plan R08, Design note 17).
 */
export interface AtmosphereInputs {
  /** √(MN) + h, m: R05's radius the per-frame tables put the camera at. */
  readonly radiusM: number;
  /** The geodetic height above the datum, m. */
  readonly heightM: number;
  /** The datum's outward normal under the camera, which μ is measured against. */
  readonly normal: Vec3;
  /** The camera's geodetic latitude, rad. */
  readonly latitudeRad: number;
  /**
   * s = g(φ) ÷ g_ref at the camera (R08.T3.d's `gravityRatio`, `BodyGravity.gravityOffsetMS2`
   * included): the factor its height is scaled by, h\* = s h; 1 on a body with no spheroid.
   */
  readonly gravityRatio: number;
}

/** A point's geodetic latitude and height on a spheroid, with its Gaussian radius. */
export interface Geodetic {
  readonly latitudeRad: number;
  readonly longitudeRad: number;
  readonly heightM: number;
  readonly normal: Vec3;
  /** √(MN), m: the Gaussian radius of curvature at the latitude. */
  readonly gaussianRadiusM: number;
}

/**
 * Vermeille 2002's k of a body-fixed point on the spheroid of first eccentricity squared `e2` and
 * equatorial radius `a`, with k ÷ (k + e²), which takes the point's distance from the axis ρ to D
 * ({@link geodeticOf}), in Karney 2011's forms: `hyperion-fit`'s `LevelSpheroid::vermeille`
 * (`atmosphere/spheroid.rs`, R08.T12.d), operation for operation.
 */
function vermeille(p: Vec3, a: number, e2: number): readonly [number, number] {
  const e4 = e2 * e2;
  const inverseA2 = 1 / (a * a);
  const pp = (p.x * p.x + p.y * p.y) * inverseA2;
  const q = (1 - e2) * p.z * p.z * inverseA2;
  const r = (pp + q - e4) / 6;
  const e4pq = e4 * pp * q;
  const evolute = 8 * r * r * r + e4pq;
  let u: number;
  if (evolute > 0) {
    const sum = Math.sqrt(e4pq) + Math.sqrt(evolute);
    const bigR = Math.cbrt(sum * sum);
    u = r + 0.5 * bigR + (2 * r * r) / bigR;
  } else if (r < 0) {
    const magnitude = -r;
    const cosTheta = Math.min(Math.max(e4pq / (4 * magnitude * magnitude * magnitude) - 1, -1), 1);
    u = magnitude * (2 * Math.cos(Math.acos(cosTheta) / 3) - 1);
  } else {
    // On the axis at the evolute's cusp, z = ±(a² − c²) ÷ c, where u = 0 is the root.
    u = 0;
  }
  const v = Math.sqrt(u * u + e4 * q);
  const w = (e2 * (u + v - q)) / (2 * v);
  const uv = u + v;
  const root = Math.sqrt(uv + w * w);
  const k = w > 0 ? uv / (root + w) : root - w;
  return [k, k / (k + e2)];
}

/**
 * A body-fixed point's geodetic coordinates on the spheroid (a, a, c), by Vermeille 2002's closed
 * form (plan R08, R08.T6.e), for any flattening.
 *
 * @remarks
 * Vermeille 2002 (J. Geodesy 76, 451–454, doi:10.1007/s00190-002-0273-6) in the forms Karney 2011
 * restates it in (arXiv:1102.1215, App. B), as R08.T12.d's `hyperion-fit` takes it
 * (`atmosphere/spheroid.rs`'s `LevelSpheroid::geodetic`): p = ρ² ÷ a², q = (1 − e²) z² ÷ a²,
 * r = (p + q − e⁴) ÷ 6; u, the largest root of u²(u − 3r) = e⁴pq ÷ 2, outside the evolute
 * (8r³ + e⁴pq > 0) as r + ½R + 2r² ÷ R with R = ∛((√(e⁴pq) + √(8r³ + e⁴pq))²), which is
 * Karney's u = r + T + r² ÷ T with no division by r and none of Vermeille's own form's cancellation
 * where r < 0, and inside it (only above the poles of a body with c ÷ a < 1 ÷ √2) the cubic's
 * largest root by Viète's trigonometric form, |r|(2 cos(θ ÷ 3) − 1), where Karney takes the smallest,
 * r(1 + 2 cos(ψ ÷ 3)): any real root gives the same k; v = √(u² + e⁴q), w = e²(u + v − q) ÷ 2v, k = √(u + v + w²) − w, taken as
 * (u + v) ÷ (√(u + v + w²) + w) where w > 0, D = kρ ÷ (k + e²), and then h = (k + e² − 1) √(D² + z²)
 * ÷ k and φ = atan2(z, D). It replaces R05's fixed-point iteration on φ and h, which did not
 * converge above a flattening of about 0.04 (4.7 mm at f = 0.05, metres at 0.1), where Saturn's
 * is 0.098; on WGS 84 the two agree to under 10⁻⁸ m. Valid at every point on or outside the datum
 * and just below it, not inside the body's equatorial disc ρ ≤ ae², which no camera reaches.
 * N = a ÷ √(1 − e² sin²φ) (NIMA TR8350.2, eq. 4-15) and M = a(1 − e²) ÷ (1 − e² sin²φ)^(3/2)
 * (TR8350.2, §7) give the Gaussian radius.
 */
export function geodeticOf(p: Vec3, figure: SpheroidFigure): Geodetic {
  const a = figure.equatorialRadiusM;
  const c = figure.polarRadiusM;
  const e2 = ((a - c) * (a + c)) / (a * a);
  const rho = Math.hypot(p.x, p.y);
  const longitudeRad = Math.atan2(p.y, p.x);
  const [k, along] = vermeille(p, a, e2);
  const d = along * rho;
  const length = Math.hypot(d, p.z);
  const latitudeRad = Math.atan2(p.z, d);
  const sin = p.z / length;
  const w2 = 1 - e2 * sin * sin;
  const n = a / Math.sqrt(w2);
  const m = (a * (1 - e2)) / (w2 * Math.sqrt(w2));
  return {
    latitudeRad,
    longitudeRad,
    heightM: ((k + e2 - 1) / k) * length,
    normal: { x: (p.x * along) / length, y: (p.y * along) / length, z: sin },
    gaussianRadiusM: Math.sqrt(m * n),
  };
}

/**
 * DN 16's lookup inputs at the camera: r = √(MN) + h, h the geodetic height, the spheroid normal μ
 * is measured against, and, since R08.T6.f, the geodetic latitude and s = g(φ) ÷ g_ref there.
 *
 * @param gravity - The body's gravity (R08.T3.d's `bodyGravity`); none for s ≡ 1, as on a body
 *   with no spheroid.
 * @throws RangeError as `oblate.ts`'s `gravityRatio`.
 */
export function atmosphereInputs(
  camera: Pick<AtmosphereCamera, "positionM">,
  figure: SpheroidFigure,
  gravity: BodyGravity | null = null,
): AtmosphereInputs {
  const g = geodeticOf(camera.positionM, figure);
  return {
    radiusM: g.gaussianRadiusM + g.heightM,
    heightM: g.heightM,
    normal: g.normal,
    latitudeRad: g.latitudeRad,
    gravityRatio: gravity === null ? 1 : gravityRatio(gravity, g.latitudeRad),
  };
}

/**
 * The body an atmosphere's tables take on `figure` with no gravity given (R08.T6.f): the figure
 * with R08.T3.d's gravity for a body with no rotation section, s ≡ 1, one slice and one band, so
 * that `oblate.wgsl`'s datum, and the ray march's heights and normals on it, are the figure's own
 * (R05 Design note 16); `null` on a sphere, whose tables are R05's on R_ref = a.
 */
export function bodyOfFigure(figure: SpheroidFigure): AtmosphereBody | null {
  if (figure.equatorialRadiusM === figure.polarRadiusM) {
    return null;
  }
  return {
    figure,
    gravity: bodyGravity(
      // The bulk gravity is read nowhere with no spheroid: s is 1 whatever it is.
      { figure, massKg: 1, angularVelocityRadS: null, bulkGravityMS2: 9.806_65, figureLaw: null },
      tableRadiusM(figure),
    ),
  };
}

/**
 * The figure's mean radius R₁ = (2a + c) ÷ 3, m: the per-planet tables' ground; the "mean
 * radius of semi-axes" of NIMA TR8350.2, Table 3.3 (6,371,008.7714 m on WGS 84).
 */
export function tableRadiusM(figure: SpheroidFigure): number {
  return (2 * figure.equatorialRadiusM + figure.polarRadiusM) / 3;
}

/**
 * The camera's height on the sky view's sphere, m: `view.wgsl`'s `skyViewHeight`, which keeps it
 * 1 m clear of the ground and of the top, where the table's horizon split degenerates.
 *
 * @param atmosphereM - The atmosphere's height above the ground, m.
 */
export function skyViewHeight(heightM: number, atmosphereM: number): number {
  return Math.min(Math.max(heightM, 1), atmosphereM - 1);
}

/**
 * What the kernels cannot form in `f32` without losing it, formed in `f64` (plan R08, Design note
 * 18): `view.wgsl`'s `AtmosphereView.exact`, which the uniform narrows once.
 */
export interface ViewExact {
  /**
   * The ray march's ground constant q_g = |S P|² − a², m², P the camera's body-fixed position and
   * S = diag(1, 1, a ÷ c). Near the ground it is a small difference of large squares, which `f32`
   * would leave without digits (0.25 m of an Earth radius); formed here it keeps about 10⁻⁹ m.
   */
  readonly groundQuadricM2: number;
  /**
   * Its top shell's q_t = |S_t P|² − (a + H ÷ s_e)², m², S_t = diag(1, 1, (a + H ÷ s_e) ÷
   * (c + H ÷ s_p)): the quadric through h\* = H at the equator and the poles (R08.T6.f; Design note
   * 17), (a + H, c + H) where s ≡ 1.
   */
  readonly topQuadricM2: number;
  /**
   * The camera's local scale height in h\*, m (R08.T6.f): the medium's vertical optical depth above
   * the camera's h\* in green over its extinction there, held between 1 m and the room to the top,
   * as `tablesCpu.ts`'s sliced lookup takes it from a start, here from the medium's own column
   * ({@link localScaleHeightM}) for the composite's lookup, which reads no medium.
   */
  readonly localScaleHeightM: number;
}

/**
 * The fitted lookup's least local scale height and the medium's local scale height (plan R08,
 * R08.T6.f): `medium.ts`'s since R08.T9.b, where `tables.ts` tabulates the latter into the body's
 * table for the lit passes without a cycle of imports.
 */
export { FIT_SCALE_FLOOR_M, localScaleHeightM } from "./medium";

/**
 * The view's {@link ViewExact} for a camera at `positionM` with its {@link atmosphereInputs}. Each
 * source's sin ρ, which the composite's disc test |d × s| ≤ sin ρ takes since `f32` cannot resolve
 * 1 − cos ρ for ρ under 3.5 × 10⁻⁴ rad, is formed beside it ({@link frameSourcesOf}, R08.T7).
 *
 * @param gravity - The body's gravity, whose s at the equator and the pole places the top's shell
 *   (R08.T6.f); none for s ≡ 1.
 * @throws RangeError as `oblate.ts`'s `gravityRatio`.
 */
export function viewExactOf(
  positionM: Vec3,
  figure: SpheroidFigure,
  medium: AtmosphereMedium,
  inputs: AtmosphereInputs,
  gravity: BodyGravity | null = null,
): ViewExact {
  const a = figure.equatorialRadiusM;
  const c = figure.polarRadiusM;
  const quadric = (equatorialM: number, polarM: number): number => {
    const z = (positionM.z * equatorialM) / polarM;
    return positionM.x ** 2 + positionM.y ** 2 + z * z - equatorialM * equatorialM;
  };
  const top = topShellOf(figure, medium.topHeightM, gravity);
  return {
    groundQuadricM2: quadric(a, c),
    topQuadricM2: quadric(top.equatorialM, top.polarM),
    localScaleHeightM: localScaleHeightM(medium, inputs.gravityRatio * inputs.heightM),
  };
}

/**
 * The air's top shell, m: the quadric (a + H ÷ s_e, c + H ÷ s_p) through h\* = H at the equator
 * and the poles (R08.T6.f; Design note 17), s_e and s_p the body's gravity's ratios there,
 * (a + H, c + H) where s ≡ 1: the shell the ray march clips its air by ({@link viewExactOf}'s
 * `topQuadricM2`) and the march's area bounds (R08.T11.d).
 *
 * @throws RangeError as `oblate.ts`'s `gravityRatio`.
 */
export function topShellOf(
  figure: SpheroidFigure,
  topHeightM: number,
  gravity: BodyGravity | null,
): MarchShell {
  const atEquator = gravity === null ? 1 : gravityRatio(gravity, 0);
  const atPole = gravity === null ? 1 : gravityRatio(gravity, Math.PI / 2);
  return {
    equatorialM: figure.equatorialRadiusM + topHeightM / atEquator,
    polarM: figure.polarRadiusM + topHeightM / atPole,
  };
}

/**
 * The camera on an oblate body as the per-view kernels take it (plan R08, R08.T6.f; `view.wgsl`'s
 * `AtmosphereView.curvature`), formed in `f64`: its principal radii M and N, from which each
 * sky-view column's sphere R_α is taken, s at its latitude, and the sky view's atmosphere, the
 * medium's top over that s (`tablesCpu.ts`'s `oblateCameraOf`).
 */
export interface ViewCurvature {
  readonly meridionalM: number;
  readonly primeVerticalM: number;
  readonly gravityRatio: number;
  readonly atmosphereM: number;
}

/** The camera's {@link ViewCurvature} from its {@link atmosphereInputs}. */
export function viewCurvatureOf(
  figure: SpheroidFigure,
  medium: AtmosphereMedium,
  inputs: AtmosphereInputs,
): ViewCurvature {
  const { meridionalM, primeVerticalM } = principalRadiiM(figure, inputs.latitudeRad);
  return {
    meridionalM,
    primeVerticalM,
    gravityRatio: inputs.gravityRatio,
    atmosphereM: medium.topHeightM / inputs.gravityRatio,
  };
}

/**
 * The sky view's x axis at a camera (plan R08, R08.T7): its azimuth origin
 * ({@link skyAzimuthOrigin}) at the first source, about which the table's local azimuth is
 * measured, so that the brightest star's vertical falls on the table's first and last columns,
 * formed in `f64`; with no source, at the zenith's own fallback.
 *
 * @param normal - The datum's unit outward normal under the camera.
 */
export function skyAxisOf(normal: Vec3, suns: ReadonlyArray<SunState>): Vec3 {
  return skyAzimuthOrigin(normal, suns[0]?.directionBodyFixed ?? normal);
}

/**
 * One of the frame's sources as the per-frame kernels and the composite take it (`view.wgsl`'s
 * `SkySource`), formed in `f64` and narrowed once (plan R08, R08.T7).
 */
export interface FrameSource {
  /** xyz its unit direction, body-fixed; w sin ρ of its angular radius, 0 for none. */
  readonly direction: readonly [number, number, number, number];
  /**
   * rgb its illuminance per channel times the frame's scale (`drawFrame`'s), the weight of its light
   * in the per-view tables; w its light slot, the path, absorber curves and multiple-scattering
   * layer its light takes (0 for every source of a medium with no absorber).
   */
  readonly weight: readonly [number, number, number, number];
  /** rgb its disc's luminance at the top of the air times the frame's scale, 0 where not drawn. */
  readonly disc: readonly [number, number, number, number];
}

/**
 * The frame's sources as the kernels take them, each light weighted by `frameScale`, the
 * per-view tables' unit (plan R08, R08.T7; `decision-r08-t6b-floor.md` §6): the frame's
 * pre-exposure, so that the tables hold pre-exposed luminance, as the composite's output is, and a
 * twilight or moonlit sky stays in the half float's normal range wherever the exposure follows it.
 *
 * @param absorbers - Whether the medium has an absorber: without one every source takes path 0,
 *   since without one the tables are the same for every light.
 * @throws RangeError for more sources than {@link MAX_SKY_SOURCES}, a light slot outside
 *   [0, {@link MAX_SKY_LIGHT_SLOTS}) or, under an absorber, one past the curves' `lightSlots`, or a
 *   frame scale that is not finite and above 0.
 */
export function frameSourcesOf(
  suns: ReadonlyArray<SunState>,
  frameScale: number,
  absorbers: boolean,
  lightSlots: number,
): FrameSource[] {
  if (suns.length > MAX_SKY_SOURCES) {
    throw new RangeError(`the sky's kernels hold ${MAX_SKY_SOURCES} sources, not ${suns.length}`);
  }
  if (!(Number.isFinite(frameScale) && frameScale > 0)) {
    throw new RangeError(`a frame's scale must be finite and above 0, not ${frameScale}`);
  }
  return suns.map((sun) => {
    const slot = curveSlotOf(sun);
    if (!(Number.isInteger(slot) && slot >= 0 && slot < MAX_SKY_LIGHT_SLOTS)) {
      throw new RangeError(
        `a source's light slot must be an integer in [0, ${MAX_SKY_LIGHT_SLOTS}), not ${slot}`,
      );
    }
    if (absorbers && slot >= lightSlots) {
      throw new RangeError(
        `a source takes light slot ${slot}, but the medium's absorbers have curves for ${lightSlots} lights`,
      );
    }
    const s = sun.directionBodyFixed;
    const e = sun.illuminanceLx;
    // The disc's solid angle 2π(1 − cos ρ), as 4π sin²(ρ ÷ 2), which keeps its digits for a small
    // disc where 1 − cos ρ would not even in f64 (a sun of 10⁻⁸ rad).
    const solidAngle = 4 * Math.PI * Math.sin(sun.angularRadiusRad / 2) ** 2;
    const disc = drawsDiscOf(sun) && solidAngle > 0 ? frameScale / solidAngle : 0;
    return {
      direction: [s.x, s.y, s.z, Math.sin(sun.angularRadiusRad)],
      weight: [e[0] * frameScale, e[1] * frameScale, e[2] * frameScale, absorbers ? slot : 0],
      disc: [e[0] * disc, e[1] * disc, e[2] * disc, 0],
    };
  });
}

/**
 * The indices of the sources the ray march takes among `suns` (plan R08, R08.T8.e;
 * `decision-r08-t8-leans.md` §9, Ruling 2): the `cap` brightest by photopic illuminance at the
 * camera, the earlier first of two alike, in their given order, so that a frame of at most `cap`
 * sources marches them all as given.
 *
 * @throws RangeError for a cap that is not an integer of at least 1.
 */
function marchSourceIndices(suns: ReadonlyArray<SunState>, cap: number): number[] {
  if (!(Number.isInteger(cap) && cap >= 1)) {
    throw new RangeError(`the march's source cap must be an integer of at least 1, not ${cap}`);
  }
  if (suns.length <= cap) {
    return suns.map((_, i) => i);
  }
  const brightness = suns.map((sun) => photopicIlluminance(sun.illuminanceLx));
  return suns
    .map((_, i) => i)
    .toSorted((i, j) => (brightness[j] ?? 0) - (brightness[i] ?? 0) || i - j)
    .slice(0, cap)
    .toSorted((i, j) => i - j);
}

/**
 * The sources the per-pixel ray march takes of a frame's `suns` (plan R08, R08.T8.e;
 * `decision-r08-t8-leans.md` §9, Ruling 2): at most `cap`, `TableSizes.marchSourceCap`, the
 * brightest by photopic illuminance at the camera, in their given order (the earlier first of two
 * alike). The sky view, the aerial perspective and the composite take every source; the twins of
 * the march take these, as {@link HillaireAtmosphere.drawFrame}'s march does.
 *
 * @remarks
 * Design note 7's skips (`sources.ts`) leave most views from orbit one source, and a binary's
 * second star past the skips is the case two serves. A third left by them, a terminator view with
 * a moon and a neighbour or a binary with a moon, lights the march's air no longer: from the march
 * band's upper edge up, and on surfaces beyond the aerial volume's reach, it is not drawn; inside
 * the band, where the composite blends the march with the sky view, it fades
 * (`ATMOSPHERE_QUALITY_LIMITS`).
 *
 * @throws RangeError for a cap that is not an integer of at least 1.
 */
export function marchSourcesOf(suns: ReadonlyArray<SunState>, cap: number): SunState[] {
  return marchSourceIndices(suns, cap).flatMap((i) => suns[i] ?? []);
}

/**
 * The light slots a frame's marches track a path for: one past the largest source's, at least 1
 * (slot 0, whose path gives the views' mean transmittance); 1 for a medium with no absorber.
 */
export function pathSlotsOf(sources: ReadonlyArray<FrameSource>): number {
  return Math.max(1, ...sources.map((source) => source.weight[3] + 1));
}

/**
 * The per-frame kernels: `view/shaders/preciseTrig.wgsl` (R08.T6.g), `common.wgsl`, `medium.wgsl`,
 * `view.wgsl`, `oblate.wgsl` (R08.T6.f), `scattering.wgsl` (R08.T9.b), `source.wgsl`, then each
 * kernel, which declares the `medium` uniform, the `terms` buffer and the `densityTables` and
 * `phaseTables` that `medium.wgsl` and `scattering.wgsl` read in place (R08.T6.b), and the body's
 * table `oblateBody` (R08.T6.f).
 */
function kernel(name: string, wgsl: string): KernelPair {
  return {
    name,
    reference: `${preciseTrigWgsl}\n${commonWgsl}\n${mediumWgsl}\n${viewWgsl}\n${oblateWgsl}\n${scatteringWgsl}\n${sourceWgsl}\n${wgsl}`,
    subgroup: null,
    readback: "presentation-only",
  };
}

/**
 * The pass a frame's sky view, aerial perspective and ray march dispatch under: the photorealistic
 * style's `atmosphere` slot names it among its passes' labels (plan R08, R08.T10.b), and the
 * spike's rows read it.
 */
export const ATMOSPHERE_VIEW_PASS = "atmosphere view";

/**
 * The pass a photorealistic frame draws each body's composite in, over the scene target into the
 * next of its atmosphere targets (plan R08, R08.T10.b), the spike's label for its own.
 */
export const ATMOSPHERE_COMPOSITE_PASS = "atmosphere composite";

/** The sky-view kernel. */
export const SKY_VIEW_KERNEL = kernel("atmosphere sky view", skyViewWgsl);
/** The aerial-perspective kernel. */
export const AERIAL_PERSPECTIVE_KERNEL = kernel(
  "atmosphere aerial perspective",
  aerialPerspectiveWgsl,
);
/**
 * The per-pixel ray-march kernel over the far limb's footprint (`farLimb.wgsl`, R08.T8.c), each
 * source's light read through the shared tables with its slot's curves applied on read
 * (`rayMarchCurves.wgsl`): the march of every body whose tables hold no slot layers
 * (`tables.ts`'s `slotDepthsRefusal`; R08.T8.e). Since R08.T11.d's tile cull it marches the tiles
 * of the air's area whose footprints may blend (`rayMarchFootprint.wgsl`), after
 * {@link RAY_MARCH_CENTRE_KERNEL} took the rest, and under the reference path every tile.
 */
export const RAY_MARCH_KERNEL = kernel(
  "atmosphere ray march",
  `${farLimbWgsl}\n${marchAreaWgsl}\n${rayMarchWgsl}\n${rayMarchCurvesWgsl}\n${rayMarchFootprintWgsl}`,
);
/**
 * The per-pixel ray-march kernel of a body whose tables hold the slot layers (plan R08, R08.T8.e,
 * lever 5; `decision-r08-t8-leans.md` §10 and §13; `AtmosphereTables.slotDepths`): the same march,
 * each source's light at each step and the bare ground's one read of its light slot's layer
 * (`rayMarchSlots.wgsl`), which only this kernel and {@link RAY_MARCH_SLOTS_CENTRE_KERNEL}
 * declare. Chosen when the body's pipeline is made, so that neither form pays for the other's
 * registers.
 */
export const RAY_MARCH_SLOTS_KERNEL = kernel(
  "atmosphere ray march by slot",
  `${farLimbWgsl}\n${marchAreaWgsl}\n${rayMarchWgsl}\n${rayMarchSlotsWgsl}\n${rayMarchFootprintWgsl}`,
);
/**
 * The ray march's first entry point under the tile cull (plan R08, R08.T11.d;
 * `decision-r08-t8-leans.md` §11, Ruling 4): {@link RAY_MARCH_KERNEL}'s march at the centre ray
 * alone, which classes each tile of the air's area, writes (0, 0, 0, 1) where every ray misses the
 * air and marches the tiles no footprint can blend in (`rayMarchCentre.wgsl`), holding none of the
 * footprint across its march.
 */
export const RAY_MARCH_CENTRE_KERNEL = kernel(
  "atmosphere ray march at the centre ray",
  `${farLimbWgsl}\n${marchAreaWgsl}\n${rayMarchWgsl}\n${rayMarchCurvesWgsl}\n${rayMarchCentreWgsl}`,
);
/** {@link RAY_MARCH_CENTRE_KERNEL} by slot, {@link RAY_MARCH_SLOTS_KERNEL}'s read (R08.T11.d). */
export const RAY_MARCH_SLOTS_CENTRE_KERNEL = kernel(
  "atmosphere ray march by slot at the centre ray",
  `${farLimbWgsl}\n${marchAreaWgsl}\n${rayMarchWgsl}\n${rayMarchSlotsWgsl}\n${rayMarchCentreWgsl}`,
);
/**
 * The specialised march's three rewrites of the general one's sources (R08.T11.d), each made once
 * and each within a line, so that a checkout's line ends do not matter: `oblateSliceCount`'s and
 * `oblateBandCount`'s bodies (`oblate.wgsl`) and the march's edges' count (`rayMarch.wgsl`).
 */
const SPECIALISED_MARCH: ReadonlyArray<readonly [string, string]> = [
  ["return clamp(u32(body.counts.x), 1u, MAX_SLICES);", "return 1u;"],
  ["return clamp(u32(body.counts.y), 1u, MAX_BANDS);", "return 1u;"],
  [
    "let edgeCount = min(u32(layerEdges.count.x), MARCH_EDGES_MAX);",
    "let edgeCount = min(u32(layerEdges.count.x), 0u);",
  ],
];

/**
 * `pair`, a ray march's, specialised for a body of one slice and one band under a medium with no
 * layer edge (plan R08, R08.T11.d; `decision-r08-approximate-plate.md` item 3): the slices' and the
 * bands' counts constant 1 (`oblate.wgsl`) and the march's edges' count 0 (`rayMarch.wgsl`), the
 * values its uniforms hold for such a body, so that the compiler drops T6.f's sliced and banded
 * code and T6.i's private array. Exact for those bodies; chosen from their tables
 * ({@link marchSpecialisedFor}). The edges' uniform stays read, so the bindings are the general
 * kernel's.
 *
 * @throws Error naming the kernel when its sources lack a place to rewrite.
 */
export function withMarchSpecialised(pair: KernelPair): KernelPair {
  const rewrite = (wgsl: string): string =>
    SPECIALISED_MARCH.reduce((code, [general, special]) => {
      const at = code.indexOf(general);
      if (at < 0 || code.indexOf(general, at + 1) >= 0) {
        throw new Error(`kernel ${pair.name} holds no single place for: ${general}`);
      }
      return code.replace(general, special);
    }, wgsl);
  return {
    ...pair,
    name: `${pair.name} for one band, edge-free`,
    reference: rewrite(pair.reference),
    subgroup: pair.subgroup === null ? null : rewrite(pair.subgroup),
  };
}

/**
 * {@link RAY_MARCH_KERNEL} specialised for one band and no layer edge (R08.T11.d;
 * {@link withMarchSpecialised}): the march of a view's frame over such a body's air.
 */
export const RAY_MARCH_SPECIAL_KERNEL = withMarchSpecialised(RAY_MARCH_KERNEL);

/** {@link RAY_MARCH_SLOTS_KERNEL} specialised likewise (R08.T11.d). */
export const RAY_MARCH_SLOTS_SPECIAL_KERNEL = withMarchSpecialised(RAY_MARCH_SLOTS_KERNEL);

/**
 * The thick read's rewrites of a per-frame kernel's sources (plan R08, R08.T14.c): each source's
 * multiple scattering taken by `thickRead.wgsl`'s `thickSourceAt` in place of `source.wgsl`'s
 * `sourceAt`, and the forward lobes' higher orders at the analytic term's share. Each is one line;
 * the second stands once in every kernel (`source.wgsl`'s `sourceInScatteringAt`), the third only
 * in a march by slot (`rayMarchSlots.wgsl`), and the first in each of them.
 */
const THICK_READ: ReadonlyArray<{
  readonly general: string;
  readonly thick: string;
  readonly least: number;
  readonly most: number;
}> = [
  {
    general: "phased += lobeOrdersOf(local, angle, light.depth);",
    thick: "phased += thickAnalyticShare(muSun) * lobeOrdersOf(local, angle, light.depth);",
    least: 1,
    most: 2,
  },
  {
    general: "return sourceAt(body, start, light, muSun, slot, local, phased);",
    thick: "return thickSourceAt(body, start, light, muSun, slot, local, phased, ray, dir, up);",
    least: 1,
    most: 1,
  },
  {
    general: "let one = sourceAt(body, start, light, muSun, slot, local, phased);",
    thick: "let one = thickSourceAt(body, start, light, muSun, slot, local, phased, ray, dir, up);",
    least: 0,
    most: 1,
  },
];

/**
 * `pair`, a per-frame kernel (the sky view, the aerial perspective or a ray march), reading the
 * thick multiple-scattering table (plan R08, R08.T14.c; Design note 9; `thick/read.ts`): its
 * sources' multiple scattering is `thickRead.wgsl`'s, the bake's J_ms from the table's first sun
 * cosine cell up and Hillaire's Ψ_ms below the horizon, blended across that cell, and the forward
 * lobes' higher orders take Ψ_ms's share ({@link THICK_READ}). Every other line is the kernel's own,
 * so that the thin kernel stays as it was to the byte and the thick one differs from it only there;
 * the table binds at `thickTable` beside the kernel's own bindings.
 *
 * @throws Error naming the kernel when its sources hold a rewritten line more or fewer times than
 *   {@link THICK_READ} allows, or still call `sourceAt` after them.
 */
export function withThickRead(pair: KernelPair): KernelPair {
  const rewrite = (wgsl: string): string => {
    const code = THICK_READ.reduce((text, { general, thick, least, most }) => {
      const count = text.split(general).length - 1;
      if (count < least || count > most) {
        throw new Error(
          `kernel ${pair.name} holds ${count} places for: ${general}, not ${least} to ${most}`,
        );
      }
      return text.replaceAll(general, thick);
    }, wgsl);
    if (/[^\w]sourceAt\(/u.test(code.replaceAll("fn sourceAt(", ""))) {
      throw new Error(`kernel ${pair.name} still calls sourceAt after the thick read's rewrites`);
    }
    return `${code}\n${thickReadWgsl}`;
  };
  return {
    ...pair,
    name: `${pair.name}, thick`,
    reference: rewrite(pair.reference),
    subgroup: pair.subgroup === null ? null : rewrite(pair.subgroup),
  };
}

/** {@link SKY_VIEW_KERNEL} reading the thick table ({@link withThickRead}; R08.T14.c). */
export const SKY_VIEW_THICK_KERNEL = withThickRead(SKY_VIEW_KERNEL);
/** {@link AERIAL_PERSPECTIVE_KERNEL} reading the thick table (R08.T14.c). */
export const AERIAL_PERSPECTIVE_THICK_KERNEL = withThickRead(AERIAL_PERSPECTIVE_KERNEL);
/** {@link RAY_MARCH_KERNEL} reading the thick table (R08.T14.c). */
export const RAY_MARCH_THICK_KERNEL = withThickRead(RAY_MARCH_KERNEL);
/** {@link RAY_MARCH_SLOTS_KERNEL} reading the thick table (R08.T14.c). */
export const RAY_MARCH_SLOTS_THICK_KERNEL = withThickRead(RAY_MARCH_SLOTS_KERNEL);
/**
 * {@link RAY_MARCH_THICK_KERNEL} specialised for one band and no layer edge, as
 * {@link RAY_MARCH_SPECIAL_KERNEL} is ({@link withMarchSpecialised}; R08.T14.c): the thick read
 * takes its bands' count through `oblateBandCount`, which the rewrite folds to 1.
 */
export const RAY_MARCH_THICK_SPECIAL_KERNEL = withMarchSpecialised(RAY_MARCH_THICK_KERNEL);
/** {@link RAY_MARCH_SLOTS_THICK_KERNEL} specialised likewise (R08.T14.c). */
export const RAY_MARCH_SLOTS_THICK_SPECIAL_KERNEL = withMarchSpecialised(
  RAY_MARCH_SLOTS_THICK_KERNEL,
);

/**
 * Whether a body's tables take the specialised march ({@link withMarchSpecialised}; plan R08,
 * R08.T11.d): one slice and one band in their plan, and no layer edge in their medium's, from the
 * tables alone, never the body's class.
 */
export function marchSpecialisedFor(tables: AtmosphereTables): boolean {
  return (
    tables.plan.kappa.length === 1 &&
    tables.plan.bands.length === 1 &&
    marchEdgeCountOf(tables.layerEdges) === 0
  );
}

/**
 * The ray march's target outside the air's area, (0, 0, 0, 1), for a reader of the whole target
 * (plan R08, R08.T11.d; `marchFill.wgsl`): {@link HillaireAtmosphere.frameTables} asks it, and a
 * frame never does, its composite reading no air there.
 */
export const RAY_MARCH_FILL_KERNEL: KernelPair = {
  name: "atmosphere ray march's fill",
  reference: `${marchAreaWgsl}\n${marchFillWgsl}`,
  subgroup: null,
  readback: "presentation-only",
};

/**
 * How a frame's ray march covers its target (plan R08, R08.T11.d), each exact, each texel what the
 * march over the whole frame gives:
 * - `area`, a view's: {@link RAY_MARCH_KERNEL} over the air's area alone (`marchArea.ts`), the
 *   composite reading no air outside it, specialised for a body of one band and no layer edge
 *   ({@link RAY_MARCH_SPECIAL_KERNEL}, {@link marchSpecialisedFor});
 * - `tiles`: the area under the tile cull (`decision-r08-t8-leans.md` §11, Ruling 4),
 *   {@link RAY_MARCH_CENTRE_KERNEL} taking the tiles no far-limb footprint can blend in and
 *   {@link RAY_MARCH_KERNEL} the rest. Off: on the RTX 3080 it cost 0.4–1.0 ms against the whole
 *   frame from 400 km and 10⁷ m, where the area alone gained 0.4–0.6 ms, and gained 0.4–1.3 ms
 *   farther (plan R08's "Deviations in R08.T11.d, as built");
 * - `whole frame`: {@link RAY_MARCH_KERNEL} over every texel, the march as it was before them, the
 *   smoke's reference path for its byte-for-byte check, and an atmosphere's own kernels' default.
 */
export type MarchCover = "area" | "tiles" | "whole frame";

/**
 * The sources' members, `view.wgsl`'s `AtmosphereView.sources`: each source's direction, weight
 * and disc ({@link FrameSource}), source by source, as the composite's `Draw` names them.
 */
const SOURCE_MEMBERS = Array.from({ length: MAX_SKY_SOURCES }, (_, i) => [
  `sourceDirection${i}`,
  `sourceWeight${i}`,
  `sourceDisc${i}`,
]).flat();

/** The view's members, in `view.wgsl`'s `AtmosphereView` and the composite's `Draw` order. */
const VIEW_MEMBERS = [
  "camera",
  "up",
  "sky",
  "skyAxis",
  "right",
  "upRay",
  "forward",
  "tables",
  "figure",
  "output",
  "exact",
  "curvature",
  "marchGround",
  "marchSky",
  ...SOURCE_MEMBERS,
] as const;

/**
 * The composite's WGSL: `view/shaders/preciseTrig.wgsl`, `common.wgsl`, `view.wgsl`, `oblate.wgsl`
 * (R08.T6.f), the volume's read (`aerialRead.wgsl`, R08.T9.a), the far limb's footprint
 * (`farLimb.wgsl`, R08.T8.c), then its own.
 */
const COMPOSITE_WGSL = `${preciseTrigWgsl}\n${commonWgsl}\n${viewWgsl}\n${oblateWgsl}\n${aerialReadWgsl}\n${farLimbWgsl}\n${compositeWgsl}`;

/** The composite: a full-screen draw of pre-exposed luminance over the terrain target. */
export const COMPOSITE_MATERIAL: WgslMaterialSpec = {
  name: "atmosphere composite",
  displayName: "ATMOSPHERE",
  vertexWgsl: COMPOSITE_WGSL,
  fragmentWgsl: COMPOSITE_WGSL,
  uniforms: [...VIEW_MEMBERS, "shell", "marchArea"].map((name) => ({
    name,
    type: "vec4f" as const,
  })),
  samplers: [{ name: "linearClamp", filter: "linear", address: "clamp-to-edge", binding: 6 }],
  textures: [
    { name: "sceneColour", binding: 0 },
    { name: "sceneDepth", binding: 1, sampleType: "depth" },
    { name: "skyView", binding: 2 },
    { name: "aerial", binding: 3, viewDimension: "3d" },
    { name: "rayMarch", binding: 4 },
    // The transmittance tables, one layer a κ slice (R08.T6.f).
    { name: "transmittance", binding: 5, viewDimension: "2d-array" },
    // The absorbers' columns beside the transmittance, and the lights' curves of growth, f32 and
    // read by `textureLoad` (R08.T6.c).
    { name: "absorberColumns", binding: 7, viewDimension: "2d-array" },
    { name: "absorberCurves", binding: 8, sampleType: "unfilterable-float" },
    // The pressure-weighted columns beside them (R08.T4.d).
    { name: "absorberPressureColumns", binding: 9, viewDimension: "2d-array" },
    { name: "lastAbsorberPressureColumn", binding: 10, viewDimension: "2d-array" },
    // The body's figure, gravity and slicing, f32 and read by `textureLoad` (R08.T6.f).
    { name: "oblateBody", binding: 11, sampleType: "unfilterable-float" },
  ],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "none",
};

/**
 * What the atmosphere is laid over: the colour and reversed-Z depth of the opaque surfaces aerial
 * perspective applies to (R08.T9.a). On the `terrain` scope that is the terrain alone, as R05's
 * spike draws it and R10's lit terrain is drawn (R10.T10.b); on `scene`, every opaque pass drawn
 * into the target before the composite, with its depth. Where the depth is 0 the composite draws
 * the sky.
 */
export interface AtmosphereScene {
  readonly colour: TextureHandle;
  readonly depth: TextureHandle;
  /** The projection's near plane, m, for distances from depth. */
  readonly nearM: number;
  /** The pre-exposure scale (R02's `exposureScale` of the frame's EV100). */
  readonly exposureScale: number;
}

const STORAGE_SAMPLED =
  TEXTURE_USAGE.STORAGE_BINDING | TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_SRC;

/**
 * The ray march's two entry points over one read of the sources' light (plan R08, R08.T11.d's tile
 * cull): the centre ray's, dispatched first, and the footprint's.
 */
interface MarchKernels {
  /** The centre ray's, made at its first culled frame where the kernels are made synchronously. */
  readonly centre: () => ComputeHandle;
  readonly footprint: ComputeHandle;
  /**
   * The footprint's entry point specialised for one band and no layer edge (R08.T11.d,
   * {@link withMarchSpecialised}), made up front with the footprint's by
   * {@link createAtmosphereFrameKernels} and at its first frame over the area where the kernels
   * are made synchronously.
   */
  readonly special: () => ComputeHandle;
}

/** The per-frame kernels, each with its output, as one frame dispatches them. */
interface FrameKernels {
  readonly skyView: ComputeHandle;
  readonly aerial: ComputeHandle;
  /**
   * The march over the shared read ({@link RAY_MARCH_CENTRE_KERNEL}, {@link RAY_MARCH_KERNEL},
   * {@link RAY_MARCH_SPECIAL_KERNEL}):
   * an atmosphere's own kernels make it at the first frame of tables that take it (R08.T11.d), so
   * that a body with slot layers never compiles the pair; the device's shared kernels
   * ({@link createAtmosphereFrameKernels}) make every kernel up front, off the frame path.
   */
  readonly rayMarch: () => MarchKernels;
  /**
   * The ray march by slot ({@link RAY_MARCH_SLOTS_CENTRE_KERNEL}, {@link RAY_MARCH_SLOTS_KERNEL},
   * {@link RAY_MARCH_SLOTS_SPECIAL_KERNEL}), made at the first frame of tables that hold the slot
   * layers (R08.T8.e).
   */
  readonly rayMarchSlots: () => MarchKernels;
  /**
   * The fill of the march's target outside the air's area ({@link RAY_MARCH_FILL_KERNEL}), made at
   * the first reader's ask (R08.T11.d).
   */
  readonly rayMarchFill: () => ComputeHandle;
  /**
   * The kernels that read the thick table ({@link withThickRead}; plan R08, R08.T14.c), for a body
   * whose tables hold one ({@link AtmosphereTables.thickTable}): made at the first frame of such
   * tables where they were not made ahead ({@link FrameKernelsAhead.thick}), so that a view of thin
   * bodies alone compiles none of them.
   */
  readonly thick: () => ThickKernels;
}

/**
 * A march's entry points over the thick table (R08.T14.c): the footprint's, and its form
 * specialised for one band and no layer edge. A thick body's march takes no tile cull: under the
 * `tiles` cover its area takes the footprint's every tile, which is exact as the cull is.
 */
interface ThickMarchKernels {
  readonly footprint: ComputeHandle;
  readonly special: () => ComputeHandle;
}

/** A frame's march kernels: the thin read's or the thick read's (R08.T14.c). */
type FrameMarch =
  | { readonly kind: "thin"; readonly kernels: MarchKernels }
  | { readonly kind: "thick"; readonly kernels: ThickMarchKernels };

/** The per-frame kernels over the thick table (R08.T14.c; {@link FrameKernels.thick}). */
interface ThickKernels {
  readonly skyView: ComputeHandle;
  readonly aerial: ComputeHandle;
  /** The march over the shared read ({@link RAY_MARCH_THICK_KERNEL}). */
  readonly rayMarch: () => ThickMarchKernels;
  /** The march by slot ({@link RAY_MARCH_SLOTS_THICK_KERNEL}). */
  readonly rayMarchSlots: () => ThickMarchKernels;
}

/**
 * The thick kernels made synchronously on `engine` at their first ask, each pair through `as`
 * (the kernel arm's format, or as shipped), each march's at the first frame that takes it.
 */
function thickKernelsOf(
  engine: RenderEngine,
  as: (pair: KernelPair) => KernelPair,
): () => ThickKernels {
  let made: ThickKernels | null = null;
  const marchOf = (footprint: KernelPair, specialPair: KernelPair): ThickMarchKernels => {
    let special: ComputeHandle | null = null;
    return {
      footprint: engine.createCompute(as(footprint)),
      special: () => (special ??= engine.createCompute(as(specialPair))),
    };
  };
  return () =>
    (made ??= (() => {
      let shared: ThickMarchKernels | null = null;
      let slots: ThickMarchKernels | null = null;
      return {
        skyView: engine.createCompute(as(SKY_VIEW_THICK_KERNEL)),
        aerial: engine.createCompute(as(AERIAL_PERSPECTIVE_THICK_KERNEL)),
        rayMarch: () =>
          (shared ??= marchOf(RAY_MARCH_THICK_KERNEL, RAY_MARCH_THICK_SPECIAL_KERNEL)),
        rayMarchSlots: () =>
          (slots ??= marchOf(RAY_MARCH_SLOTS_THICK_KERNEL, RAY_MARCH_SLOTS_THICK_SPECIAL_KERNEL)),
      };
    })());
}

/**
 * The thick kernels made up front by `createComputeAsync` (R08.T14.c; {@link FrameKernelsAhead.thick}):
 * the sky view, the aerial perspective and each read's march with its specialised form, as `reads`
 * asks; a read not asked is made at its first frame.
 */
async function thickKernelsAhead(
  engine: RenderEngine,
  reads: MarchReads,
): Promise<() => ThickKernels> {
  const marchOf = async (
    made: boolean,
    footprintPair: KernelPair,
    specialPair: KernelPair,
  ): Promise<() => ThickMarchKernels> => {
    if (!made) {
      let late: ThickMarchKernels | null = null;
      return () =>
        (late ??= (() => {
          let special: ComputeHandle | null = null;
          return {
            footprint: engine.createCompute(footprintPair),
            special: () => (special ??= engine.createCompute(specialPair)),
          };
        })());
    }
    const [footprint, special] = await Promise.all([
      engine.createComputeAsync(footprintPair),
      engine.createComputeAsync(specialPair),
    ]);
    const kernels: ThickMarchKernels = { footprint, special: () => special };
    return () => kernels;
  };
  const [skyView, aerial, rayMarch, rayMarchSlots] = await Promise.all([
    engine.createComputeAsync(SKY_VIEW_THICK_KERNEL),
    engine.createComputeAsync(AERIAL_PERSPECTIVE_THICK_KERNEL),
    marchOf(reads !== "by slot", RAY_MARCH_THICK_KERNEL, RAY_MARCH_THICK_SPECIAL_KERNEL),
    marchOf(
      reads !== "shared read",
      RAY_MARCH_SLOTS_THICK_KERNEL,
      RAY_MARCH_SLOTS_THICK_SPECIAL_KERNEL,
    ),
  ]);
  const kernels: ThickKernels = { skyView, aerial, rayMarch, rayMarchSlots };
  return () => kernels;
}

/**
 * A device's per-frame kernels and composite, shared by every {@link HillaireAtmosphere} a view
 * draws on it (plan R08, R08.T10.b): made once, asynchronously, with the view's other pipelines
 * ({@link createAtmosphereFrameKernels}), so that a body coming into the view compiles nothing on
 * its frame and no kernel is made a body, since kernels have no release. They belong to the device
 * they were made on: after a restore the owner makes them, and its atmospheres, again.
 */
export interface AtmosphereFrameKernels extends FrameKernels {
  /** The composite, its pipeline made for an `rgba16float` target without depth. */
  readonly composite: MaterialHandle;
  /** The composite's full-screen triangle. */
  readonly triangle: MeshHandle;
}

/** The composite's full-screen triangle on `engine`. */
function compositeTriangle(engine: RenderEngine): MeshHandle {
  return engine.createMesh({
    name: "atmosphere composite triangle",
    positions: new Float32Array([-1, -1, 0, 3, -1, 0, -1, 3, 0]),
    indices: null,
    topology: "triangle-list",
    attributes: {},
  });
}

/**
 * Which of the ray march's two reads of the sources' light (R08.T8.e, lever 5) a device's shared
 * kernels make up front: the shared tables' (`shared read`), the slot layers' (`by slot`), or both,
 * a view's; the other made synchronously at the first frame that takes it.
 */
export type MarchReads = "both" | "shared read" | "by slot";

/** What {@link createAtmosphereFrameKernels} makes up front, beyond the frame's own kernels. */
export interface FrameKernelsAhead {
  /**
   * The march's reads: both for a view, which draws bodies with and without slot layers; one for
   * a caller that draws only one kind (the smoke's march-area check, R08.T11.d, sparing
   * SwiftShader's GPU process the other pair). Both where absent.
   */
  readonly reads?: MarchReads;
  /**
   * Whether the tile cull's centre ray's kernels are made too (R08.T11.d's `tiles` cover, off for
   * a view): for the smoke's check of that cover. Not where absent; then made at the first
   * `tiles` frame.
   */
  readonly tiles?: boolean;
  /**
   * Whether the kernels that read the thick table are made too (plan R08, R08.T14.c): for a caller
   * that draws a body with one, as the smoke's frame timing with the thick read forced does. Not
   * where absent; then made at the first frame of tables that hold one.
   */
  readonly thick?: boolean;
}

/**
 * Makes a device's {@link AtmosphereFrameKernels}: each kernel by `createComputeAsync` and the
 * composite's pipeline into an `rgba16float` target, as a photorealistic frame's atmosphere targets
 * are, by `createMaterialAsync`, so that no frame waits on a compile (plan R08, R08.T10.b). Each
 * read's march is made with its footprint's entry point twice, the general and the one specialised
 * for one band and no layer edge (R08.T11.d, {@link RAY_MARCH_SPECIAL_KERNEL}).
 *
 * @throws Error, as a rejection, as the engine's async creations reject, `EngineUnavailable`
 *   among them during a loss.
 */
export async function createAtmosphereFrameKernels(
  engine: RenderEngine,
  ahead: FrameKernelsAhead = {},
): Promise<AtmosphereFrameKernels> {
  const reads = ahead.reads ?? "both";
  const tiles = ahead.tiles ?? false;
  const triangle = compositeTriangle(engine);
  const pairOf = async (
    made: boolean,
    centrePair: KernelPair,
    footprintPair: KernelPair,
    specialPair: KernelPair,
  ): Promise<() => MarchKernels> => {
    if (!made) {
      let late: MarchKernels | null = null;
      return () =>
        (late ??= (() => {
          let centre: ComputeHandle | null = null;
          let special: ComputeHandle | null = null;
          return {
            centre: () => (centre ??= engine.createCompute(centrePair)),
            footprint: engine.createCompute(footprintPair),
            special: () => (special ??= engine.createCompute(specialPair)),
          };
        })());
    }
    const [footprint, special, centre] = await Promise.all([
      engine.createComputeAsync(footprintPair),
      engine.createComputeAsync(specialPair),
      tiles ? engine.createComputeAsync(centrePair) : Promise.resolve(null),
    ]);
    let late: ComputeHandle | null = centre;
    const kernels: MarchKernels = {
      centre: () => (late ??= engine.createCompute(centrePair)),
      footprint,
      special: () => special,
    };
    return () => kernels;
  };
  const [skyView, aerial, rayMarch, rayMarchSlots, composite] = await Promise.all([
    engine.createComputeAsync(SKY_VIEW_KERNEL),
    engine.createComputeAsync(AERIAL_PERSPECTIVE_KERNEL),
    pairOf(
      reads !== "by slot",
      RAY_MARCH_CENTRE_KERNEL,
      RAY_MARCH_KERNEL,
      RAY_MARCH_SPECIAL_KERNEL,
    ),
    pairOf(
      reads !== "shared read",
      RAY_MARCH_SLOTS_CENTRE_KERNEL,
      RAY_MARCH_SLOTS_KERNEL,
      RAY_MARCH_SLOTS_SPECIAL_KERNEL,
    ),
    engine.createMaterialAsync(COMPOSITE_MATERIAL, ["rgba16float"], [triangle]),
  ]);
  const thick =
    ahead.thick === true
      ? await thickKernelsAhead(engine, reads)
      : thickKernelsOf(engine, (pair) => pair);
  // The fill is a reader's (the smoke's), which a view's frame never asks: made at the first ask.
  let fill: ComputeHandle | null = null;
  return {
    skyView,
    aerial,
    rayMarch,
    rayMarchSlots,
    rayMarchFill: () => (fill ??= engine.createCompute(RAY_MARCH_FILL_KERNEL)),
    thick,
    composite,
    triangle,
  };
}

/**
 * Thrown by {@link HillaireAtmosphere.drawFrame} of an atmosphere drawn with shared kernels after a
 * restore of the device they were made on (plan R08, R08.T10.b): a lost device's throw, as
 * `EngineUnavailable` is, which its owner answers by making the atmosphere again and which raises
 * no fault (`decision-r08-t10b-leans.md` §2).
 */
export class AtmosphereRetired extends Error {
  constructor() {
    super("an atmosphere drawn with a lost device's shared kernels is made again by its owner");
    this.name = "AtmosphereRetired";
  }
}

/**
 * Per-planet tables a {@link HillaireAtmosphere} draws with and does not own: the atmosphere
 * cache's (plan R08, R08.T10.b; `decision-r08-t10a-leans.md` item 3), which the cache builds,
 * holds and disposes, its body's figure and gravity their own ({@link HillaireAtmosphere.over}).
 */
export interface HeldTables {
  readonly kind: "held";
  readonly tables: AtmosphereTables;
  /** The device's shared kernels; the atmosphere makes its own where absent. */
  readonly kernels: AtmosphereFrameKernels | null;
}

/**
 * Whether a {@link HillaireAtmosphere}'s source is {@link HeldTables} rather than a medium, which has
 * no `kind`.
 */
function isHeld(source: AtmosphereMedium | HeldTables): source is HeldTables {
  return "kind" in source;
}

/**
 * The figure a body's per-planet tables were built on: their body's datum spheroid, or for tables
 * of one slice and one band, the sphere of their ground's radius.
 */
export function tablesFigureOf(tables: AtmosphereTables): SpheroidFigure {
  const radiusM = tables.bottomRadiusM;
  return tables.body?.figure ?? { equatorialRadiusM: radiusM, polarRadiusM: radiusM };
}

/** The per-frame tables a frame writes: the sky view, the aerial-perspective volume, the march. */
export interface FrameOutputs {
  readonly skyView: TextureHandle;
  readonly aerial: TextureHandle;
  readonly rayMarch: TextureHandle;
}

/**
 * A view's aerial perspective as a pass other than the composite reads it (plan R08, R08.T9.a): what
 * {@link HillaireAtmosphere.aerialPerspective} publishes on the `scene` scope.
 *
 * @remarks
 * The volume is read through `shaders/aerialRead.wgsl`'s `aerialPerspectiveAt(volume, sampler,
 * uv, distanceM, reachM)` at a surface within `reachM`. That gives the light in front of the surface
 * per unit of {@link AerialPerspective.scaleCdPerM2} and its mean transmittance, so that a pass lays
 * `colour × a + rgb × scaleCdPerM2 × exposure` over its own colour as the composite does; beyond the
 * reach the composite's ray march serves the scene's own depth. The transmittance tables are read
 * together at the light's curve slot (R08.T6.c) with the line slots' pressure columns (R08.T4.d),
 * every lookup at a height above the datum and a cosine against its normal (Design note 18), as the
 * composite's `transmittanceToSpace` reads them: since R08.T6.f 2D arrays of one layer a κ slice
 * (Design note 17), read through `oblate.wgsl`'s sliced lookup, `slicedRayOf` and `slicedStoredOf`,
 * from a `LookupStart` at the reader's point (its place, ground quadric, height, normal, latitude,
 * s, h\* and local scale height), with the body's table {@link AerialPerspective.oblateBody}; the
 * composite forms the camera's (`transmittanceToSpace`). A reader that forms its own needs the
 * medium's extinction there for the local scale height (`localScaleHeightOf`), or takes the camera's
 * ({@link ViewExact.localScaleHeightM}) near it: flagged for R11's re-validation.
 *
 * The volume serves a reader within its reach only. A pass drawn after the composite has no march
 * beyond it, so an opaque surface past the reach is drawn into the scene before the composite, or
 * a later task publishes more (plan R08's Risks, "For R11").
 */
export interface AerialPerspective {
  /**
   * The volume, `rgba16float`, 3D: at each froxel's slice the light scattered into the ray from the
   * camera to the slice's far face (rgb, per unit of {@link AerialPerspective.scaleCdPerM2}) and the
   * ray's mean transmittance over the channels (a), froxel (i, j) on the ray through the screen at
   * (i, j) ÷ (size − 1), slice k reaching (k + 1) reach ÷ slices.
   */
  readonly volume: TextureHandle;
  readonly size: VolumeSize;
  /** The volume's reach, m. */
  readonly reachM: number;
  /**
   * The factor per linear Rec. 709 channel that turns the volume's rgb into luminance, cd m⁻²,
   * before pre-exposure: the frame's scale, 1 ÷ its pre-exposure since R08.T7 (R08.T9.a's
   * `scaleLx`, renamed for its unit), the tables holding pre-exposed luminance summed over the drawn
   * sources.
   */
  readonly scaleCdPerM2: Rgb;
  /**
   * The per-planet transmittance table (`tables.ts`): the optical depth of every term but the
   * absorbers in rgb, and absorber slot 0's column u ÷ U in alpha (R08.T6.c); a 2D array of one
   * layer a κ slice since R08.T6.f, as are the three tables beside it.
   */
  readonly transmittance: TextureHandle;
  /** Absorber slots 1–4's columns u ÷ U, beside the transmittance table. */
  readonly absorberColumns: TextureHandle;
  /**
   * The line slots' pressure-weighted columns per (r, μ) to the top, pressure channels 0 to 3 in
   * rgba (R08.T4.d).
   */
  readonly absorberPressureColumns: TextureHandle;
  /** Pressure channel 4's pressure-weighted column in r, one texel for a medium with fewer slots. */
  readonly lastAbsorberPressureColumn: TextureHandle;
  /**
   * The lights' curves of growth, `rgba32float`, read by `textureLoad`; light slot 0 is the drawn
   * sun's.
   */
  readonly absorberCurves: TextureHandle;
  /** How many of the medium's terms are absorbers: `slicedStoredOf`'s `count`. */
  readonly absorberCount: number;
  /**
   * The medium's line slots, bit k for absorber slot k (`tables.ts`'s `lineSlotMask`):
   * `absorberDepth`'s `lines`, from which it takes each slot's pressure channel.
   */
  readonly lineSlots: number;
  /**
   * The per-planet tables' ground, R₁ = (2a + c) ÷ 3 ({@link tableRadiusM}), m: R_ref, slice k
   * standing on κ_k R₁ (R08.T6.f).
   */
  readonly bottomRadiusM: number;
  /** The atmosphere's height above the datum, m, in h\* (R08.T6.f). */
  readonly topHeightM: number;
  /**
   * The body's table (`tables.ts`'s `packOblateBody`, `rgba32float`, read by `textureLoad`;
   * R08.T6.f): its figure, gravity ratio and slicing, which `oblate.wgsl`'s lookup reads.
   */
  readonly oblateBody: TextureHandle;
}

/**
 * A view's sky-view table with what reads it (plan R08, R08.T9.a): what
 * {@link HillaireAtmosphere.skyView} publishes, for R11's sky reflected in the ocean.
 *
 * @remarks
 * The table holds the sky's radiance per unit of {@link SkyViewTable.scaleCdPerM2}, summed over the
 * frame's drawn sources (Design note 7, R08.T7), by sebh's zenith parameterisation and the local
 * azimuth about {@link SkyViewTable.azimuthOrigin} over the whole circle. Since R08.T6.f each
 * column stands on its own sphere, the osculating sphere R_α of its azimuth at the camera, with its
 * own horizon split β (Design note 17): on an oblate body both are per azimuth, and on a sphere
 * (M = N) the one sphere of {@link SkyViewTable.sphere}, as before. A direction d is read at
 * `view.wgsl`'s `skyViewParamsToUv(groundHitFromHeight(R_α, heightM, d · zenith),
 * preciseAtan2(|d × zenith|, d · zenith), skyViewAzimuthUnit(d · x, d · y), size, β)`, (R_α, β) =
 * `oblate.wgsl`'s `skyViewColumnOf(meridionalRadiusM, primeVerticalRadiusM, zenith, x, d, heightM)`
 * ({@link skyViewColumnOf} here), x the azimuth origin and y = zenith × x, through Bruneton's texel
 * mapping (Design note 19). R08.T7's check holds the one table within 2% of per-source tables near
 * each source, so the per-sun fallback is not built.
 */
export interface SkyViewTable {
  /** The table, `rgba16float`, 2D, rgb the radiance per unit of {@link SkyViewTable.scaleCdPerM2}. */
  readonly texture: TextureHandle;
  readonly size: TableSize;
  /**
   * The factor per linear Rec. 709 channel that turns the table's rgb into luminance, cd m⁻²,
   * before pre-exposure: 1 ÷ the frame's pre-exposure since R08.T7 (R08.T9.a's `scaleLx`, renamed
   * for its unit), the table holding pre-exposed luminance.
   */
  readonly scaleCdPerM2: Rgb;
  /** The table's zenith: the datum's outward normal under the camera, body-fixed. */
  readonly zenith: Vec3;
  /**
   * The unit horizontal direction, body-fixed, of azimuth 0: towards the first source's azimuth,
   * the view's sky axis, as `view.wgsl`'s `skyFrame` takes it ({@link skyAxisOf}).
   */
  readonly azimuthOrigin: Vec3;
  /**
   * The camera's principal radii of curvature M and N, m: each column's sphere is R_α of them
   * along its azimuth (Euler's theorem; R08.T6.f).
   */
  readonly meridionalRadiusM: number;
  readonly primeVerticalRadiusM: number;
  /**
   * The camera's height on each column's sphere, m, kept 1 m inside its shell
   * ({@link skyViewHeight}).
   */
  readonly heightM: number;
  /** The atmosphere's height on each column's sphere, m: the medium's top over the camera's s. */
  readonly atmosphereM: number;
  /**
   * The scalar reading on a sphere, where M = N: every column's one sphere, its radius and β, the
   * values R08.T9.a published as `bottomRadiusM` and `horizonAngleRad`; `null` on an oblate body,
   * whose columns each take their own ({@link skyViewColumnOf}; R08.T6.f).
   */
  readonly sphere: SkyViewColumn | null;
}

/** A sky-view column's sphere (R08.T6.f): its radius R_α, m, and β, rad, the nadir to the horizon. */
export interface SkyViewColumn {
  readonly bottomRadiusM: number;
  readonly horizonAngleRad: number;
}

/**
 * The sphere the sky view's column at a direction stands on (plan R08, R08.T6.f): `oblate.wgsl`'s
 * `skyViewColumnOf` in `f64`, R_α of the camera's M and N along the direction's azimuth, the
 * azimuth origin standing for a vertical direction's, and β = atan2(R_α, √(h (2 R_α + h))).
 *
 * @param direction - A unit direction, body-fixed.
 */
export function skyViewColumnOf(table: SkyViewTable, direction: Vec3): SkyViewColumn {
  const n = table.zenith;
  const mu = dot(direction, n);
  let flat = sub(direction, scaled(n, mu));
  if (!(dot(flat, flat) > 1e-24)) {
    flat = table.azimuthOrigin;
  }
  const bottomRadiusM = curvatureRadiusAlongM(
    table.meridionalRadiusM,
    table.primeVerticalRadiusM,
    n,
    flat,
  );
  const h = table.heightM;
  return {
    bottomRadiusM,
    horizonAngleRad: Math.atan2(bottomRadiusM, Math.sqrt(h * (2 * bottomRadiusM + h))),
  };
}

/**
 * The radius of curvature along `d` at a point of principal radii M and N under the unit normal `n`,
 * m: Euler's theorem, M ÷ (1 − (1 − M ÷ N) sin²α), α the azimuth from north (`tablesCpu.ts`'s
 * curvatureAlongM); √(MN) for a vertical direction.
 */
function curvatureRadiusAlongM(
  meridionalM: number,
  primeVerticalM: number,
  n: Vec3,
  d: Vec3,
): number {
  const cos2 = n.x * n.x + n.y * n.y;
  let along: number;
  let across: number;
  if (cos2 < 1e-24) {
    along = n.z >= 0 ? -d.x : d.x;
    across = d.y;
  } else {
    const cos = Math.sqrt(cos2);
    across = (d.y * n.x - d.x * n.y) / cos;
    along = (d.z * cos2 - n.z * (d.x * n.x + d.y * n.y)) / cos;
  }
  const horizontal = along * along + across * across;
  if (!(horizontal > 1e-24)) {
    return Math.sqrt(meridionalM * primeVerticalM);
  }
  const gap = (primeVerticalM - meridionalM) / primeVerticalM;
  return meridionalM / (1 - gap * ((across * across) / horizontal));
}

/**
 * What the latest frame published: its view, and the tables {@link HillaireAtmosphere.drawFrame}
 * built for it.
 */
interface DrawnFrame {
  readonly view: AtmosphereCamera;
  readonly aerial: AerialPerspective;
  readonly skyView: SkyViewTable;
}

/** Whether two cameras are one view: the same position, orientation, field of view and size. */
function sameView(a: AtmosphereCamera, b: AtmosphereCamera): boolean {
  const p = a.positionM;
  const q = b.positionM;
  const r = a.orientation;
  const s = b.orientation;
  return (
    p.x === q.x &&
    p.y === q.y &&
    p.z === q.z &&
    r.w === s.w &&
    r.x === s.x &&
    r.y === s.y &&
    r.z === s.z &&
    a.fovXRad === b.fovXRad &&
    a.viewport.widthPx === b.viewport.widthPx &&
    a.viewport.heightPx === b.viewport.heightPx
  );
}

/**
 * The sky view's azimuth 0, `view.wgsl`'s `skyFrame` x: the sun's direction made horizontal
 * against `zenith` and normalised, or, with the sun at the zenith or the nadir, the body-fixed x
 * axis (y where the zenith lies near x) made so.
 */
export function skyAzimuthOrigin(zenith: Vec3, sun: Vec3): Vec3 {
  let x = sub(sun, scaled(zenith, dot(sun, zenith)));
  if (dot(x, x) < 1e-12) {
    const axis: Vec3 = Math.abs(zenith.x) > 0.9 ? { x: 0, y: 1, z: 0 } : { x: 1, y: 0, z: 0 };
    x = sub(axis, scaled(zenith, dot(axis, zenith)));
  }
  return normalise(x);
}

/** The per-frame GPU objects of one device. */
interface FrameResources {
  readonly skyView: TextureHandle;
  readonly aerial: TextureHandle;
  readonly kernels: FrameKernels;
  readonly composite: MaterialHandle;
  readonly triangle: MeshHandle;
  rayMarch: TextureHandle | null;
  rayMarchSize: ViewSize | null;
  /**
   * The tile cull's kinds, one `u32` a tile of the air's area (R08.T11.d), grown with the area's
   * largest and never made smaller; the kernel arm's frames take it too.
   */
  marchTiles: BufferHandle | null;
  marchTilesBytes: number;
}

/** The kernel arm's per-frame GPU objects (R08.T6.g), made at its first frame. */
interface KernelArmResources {
  readonly kernels: FrameKernels;
  readonly skyView: TextureHandle;
  readonly aerial: TextureHandle;
  rayMarch: TextureHandle | null;
  rayMarchSize: ViewSize | null;
}

/** A per-frame table's texture: 2D or 3D, at `format`, under `atmosphere-view`. */
function frameTexture(
  engine: RenderEngine,
  name: string,
  size: [number, number] | [number, number, number],
  format: GPUTextureFormat,
): TextureHandle {
  return engine.createTexture({
    name,
    size,
    dimension: size.length === 3 ? "3d" : "2d",
    format,
    mips: 1,
    usage: STORAGE_SAMPLED,
    category: "atmosphere-view",
  });
}

/**
 * A frame's sky-view and aerial-perspective dispatches waiting for a reader (R08.T8.d), `null` for
 * one made already or not deferred.
 */
interface UnreadTables {
  readonly skyView: (() => void) | null;
  readonly aerial: (() => void) | null;
  /**
   * The ray march's target outside the air's area, (0, 0, 0, 1), which the frame's composite does
   * not read (R08.T11.d): a reader of the whole target asks it.
   */
  readonly rayMarch: (() => void) | null;
}

const NO_UNREAD_TABLES: UnreadTables = { skyView: null, aerial: null, rayMarch: null };

/**
 * A frame's GPU objects on `engine`: its own textures, and `shared`'s kernels and composite where
 * given, else its own, made synchronously.
 */
function createFrameResources(
  engine: RenderEngine,
  sizes: TableSizes,
  shared: AtmosphereFrameKernels | null = null,
): FrameResources {
  return {
    skyView: frameTexture(
      engine,
      "atmosphere sky view",
      [sizes.skyView.widthTexels, sizes.skyView.heightTexels],
      "rgba16float",
    ),
    aerial: frameTexture(
      engine,
      "atmosphere aerial perspective",
      [
        sizes.aerialPerspective.widthTexels,
        sizes.aerialPerspective.heightTexels,
        sizes.aerialPerspective.slices,
      ],
      "rgba16float",
    ),
    kernels: shared ?? marchKernelsOf(engine, (pair) => pair),
    composite: shared?.composite ?? engine.createMaterial(COMPOSITE_MATERIAL),
    triangle: shared?.triangle ?? compositeTriangle(engine),
    rayMarch: null,
    rayMarchSize: null,
    marchTiles: null,
    marchTilesBytes: 0,
  };
}

/**
 * The per-frame kernels made synchronously on `engine`, each pair through `as` (the kernel arm's
 * format, or as shipped): each march (the shared read's or by slot) and the fill at the first
 * frame that takes it.
 */
function marchKernelsOf(engine: RenderEngine, as: (pair: KernelPair) => KernelPair): FrameKernels {
  let shared: MarchKernels | null = null;
  let slots: MarchKernels | null = null;
  let fill: ComputeHandle | null = null;
  const pairOf = (
    centre: KernelPair,
    footprint: KernelPair,
    specialPair: KernelPair,
  ): MarchKernels => {
    let made: ComputeHandle | null = null;
    let special: ComputeHandle | null = null;
    return {
      centre: () => (made ??= engine.createCompute(as(centre))),
      footprint: engine.createCompute(as(footprint)),
      special: () => (special ??= engine.createCompute(as(specialPair))),
    };
  };
  return {
    skyView: engine.createCompute(as(SKY_VIEW_KERNEL)),
    aerial: engine.createCompute(as(AERIAL_PERSPECTIVE_KERNEL)),
    rayMarch: () =>
      (shared ??= pairOf(RAY_MARCH_CENTRE_KERNEL, RAY_MARCH_KERNEL, RAY_MARCH_SPECIAL_KERNEL)),
    rayMarchSlots: () =>
      (slots ??= pairOf(
        RAY_MARCH_SLOTS_CENTRE_KERNEL,
        RAY_MARCH_SLOTS_KERNEL,
        RAY_MARCH_SLOTS_SPECIAL_KERNEL,
      )),
    rayMarchFill: () => (fill ??= engine.createCompute(as(RAY_MARCH_FILL_KERNEL))),
    thick: thickKernelsOf(engine, as),
  };
}

function createKernelArmResources(engine: RenderEngine, sizes: TableSizes): KernelArmResources {
  return {
    kernels: marchKernelsOf(engine, withKernelArmFormat),
    skyView: frameTexture(
      engine,
      "atmosphere sky view (kernel arm)",
      [sizes.skyView.widthTexels, sizes.skyView.heightTexels],
      KERNEL_ARM_FORMAT,
    ),
    aerial: frameTexture(
      engine,
      "atmosphere aerial perspective (kernel arm)",
      [
        sizes.aerialPerspective.widthTexels,
        sizes.aerialPerspective.heightTexels,
        sizes.aerialPerspective.slices,
      ],
      KERNEL_ARM_FORMAT,
    ),
    rayMarch: null,
    rayMarchSize: null,
  };
}

/** Workgroups of 8 × 8 covering a 2D size. */
function groups8(width: number, height: number): readonly [number, number, number] {
  return [Math.ceil(width / 8), Math.ceil(height / 8), 1];
}

/**
 * The march's `marchArea` uniform (`marchArea.wgsl`'s `MarchArea`, R08.T11.d): the area's first
 * tile and its tiles across and down; whether the tile cull applies, the widening, m, and the
 * target's size, texels.
 */
function marchAreaUniform(area: MarchArea, culled: boolean): Float32Array {
  return new Float32Array([
    area.firstTileX,
    area.firstTileY,
    area.tilesAcross,
    area.tilesDown,
    culled ? 1 : 0,
    area.widenM,
    area.widthTexels,
    area.heightTexels,
  ]);
}

/**
 * Earth-like atmosphere drawn by Hillaire's four tables: the per-planet pair, rebuilt when the
 * medium changes, and the per-frame sky view, aerial perspective and ray march.
 */
export class HillaireAtmosphere {
  readonly #engine: RenderEngine;
  readonly #sizes: TableSizes;
  #figure: SpheroidFigure;
  /** The body's gravity (R08.T3.d), or `null` for s ≡ 1 and one slice and one band. */
  #gravity: BodyGravity | null;
  #tables: AtmosphereTables;
  /** Whether it built its tables and disposes them; `false` for {@link HeldTables}. */
  readonly #ownsTables: boolean;
  /** The device's shared kernels it draws with, or `null` where it made its own. */
  readonly #shared: AtmosphereFrameKernels | null;
  /** Set by a restore where it drew with shared kernels, which died with the device. */
  #retired = false;
  #disposed = false;
  readonly #offRestored: () => void;
  #frame: FrameResources;
  /** The smoke's kernel arm (R08.T6.g), made at its first frame. */
  #arm: KernelArmResources | null = null;
  /** Each view member's four floats, reused every frame. */
  readonly #uniforms: Record<string, Float32Array>;
  /**
   * What the latest frame published: `null` before it, and once a restore remakes the frame or a
   * medium is set, until the next.
   */
  #drawn: DrawnFrame | null = null;
  /**
   * The latest frame's sky-view and aerial-perspective dispatches, left undone where the march
   * draws every pixel (lever 2 of `decision-r08-t8-leans.md` §8, R08.T8.d): each is made at its
   * first reader's ask (`#read`), and dropped at the next frame, a new medium or a restore; and,
   * since R08.T11.d, the march's target outside the air's area.
   */
  #unread: UnreadTables = NO_UNREAD_TABLES;
  /**
   * How the march covers its target (R08.T11.d): the area over the device's shared kernels, the
   * whole frame over its own until asked ({@link HillaireAtmosphere.setMarchCover}).
   */
  #cover: MarchCover;
  /** The latest frame's march area (R08.T11.d), for the smoke's read ({@link HillaireAtmosphere.frameTables}). */
  #area: MarchArea | null = null;

  /**
   * Makes the tables and the per-frame resources for `medium` on `figure`, or the per-frame
   * resources alone over {@link HeldTables}, on their own figure and gravity (use
   * {@link HillaireAtmosphere.over}, which passes them).
   *
   * @param tables - The setting's sizes, `SETTINGS[s].atmosphere`.
   * @param curves - Each light's curves of growth for the medium's absorbers, light slot 0 the
   *   drawn sun's (R08.T6.c); none for a medium with no absorber. Held tables keep their own.
   * @param gravity - The body's gravity (R08.T3.d's `bodyGravity` on `figure`), whose slicing the
   *   tables are built in and whose s the lookups take (Design note 17; R08.T6.f); none for s ≡ 1,
   *   one slice and one band. R08.T10.a gives a body's.
   * @throws RangeError as `AtmosphereTables`, for a slicing the tables cannot hold among the rest,
   *   or for a march source cap that is not an integer of at least 1 (`TableSizes.marchSourceCap`).
   */
  constructor(
    engine: RenderEngine,
    medium: AtmosphereMedium | HeldTables,
    tables: TableSizes,
    figure: SpheroidFigure,
    curves: AbsorberCurveSlots = NO_ABSORBER_CURVES,
    gravity: BodyGravity | null = null,
  ) {
    // The cap is checked where the sizes are given, not first at a frame (R08.T8.e).
    marchSourceIndices([], tables.marchSourceCap);
    this.#engine = engine;
    this.#sizes = tables;
    if (isHeld(medium)) {
      this.#ownsTables = false;
      this.#shared = medium.kernels;
      this.#tables = medium.tables;
      this.#figure = tablesFigureOf(medium.tables);
      this.#gravity = medium.tables.body?.gravity ?? null;
    } else {
      this.#ownsTables = true;
      this.#shared = null;
      this.#figure = figure;
      this.#gravity = gravity;
      this.#tables = new AtmosphereTables(
        engine,
        medium,
        tableRadiusM(figure),
        curves,
        this.#body(),
      );
    }
    this.#frame = createFrameResources(engine, tables, this.#shared);
    // A view's atmosphere, over the device's shared kernels, marches the air's area alone; one with
    // its own kernels (the smoke's other checks, the descent spike) the whole frame as before
    // R08.T11.d, unless asked, so that their frames are those they were checked by.
    this.#cover = this.#shared === null ? "whole frame" : "area";
    this.#uniforms = Object.fromEntries(
      [...VIEW_MEMBERS, "shell", "marchArea"].map((name) => [name, new Float32Array(4)]),
    );
    this.#offRestored = engine.onRestored(() => {
      this.#arm = null;
      this.#drawn = null;
      this.#unread = NO_UNREAD_TABLES;
      if (this.#shared !== null) {
        // The shared kernels died with the device; their owner makes them, and this, again.
        this.#retired = true;
        return;
      }
      this.#frame = createFrameResources(engine, tables);
    });
  }

  /**
   * A view's atmosphere of one body over the atmosphere cache's per-planet tables, which it takes
   * in place of building its own and never disposes (plan R08, R08.T10.b;
   * `decision-r08-t10a-leans.md` item 3): its figure and gravity the tables' body's
   * ({@link tablesFigureOf}), its kernels the device's shared ones.
   *
   * @throws RangeError for a march source cap that is not an integer of at least 1.
   */
  static over(
    engine: RenderEngine,
    tables: AtmosphereTables,
    sizes: TableSizes,
    kernels: AtmosphereFrameKernels | null,
  ): HillaireAtmosphere {
    return new HillaireAtmosphere(
      engine,
      { kind: "held", tables, kernels },
      sizes,
      tablesFigureOf(tables),
      tables.curves,
      tables.body?.gravity ?? null,
    );
  }

  /** The per-planet tables, rebuilt when the medium changes, or the cache's where held. */
  get tables(): AtmosphereTables {
    return this.#tables;
  }

  /**
   * Draws from here on over another set of held tables, the cache's after it rebuilt or remade the
   * body's (plan R08, R08.T10.b), on their figure and gravity. The latest frame's publication is
   * withdrawn until the next frame, as {@link HillaireAtmosphere.setMedium}'s is.
   *
   * @throws Error for an atmosphere that owns its tables, whose medium {@link HillaireAtmosphere.setMedium}
   *   sets.
   */
  setTables(tables: AtmosphereTables): void {
    if (this.#ownsTables) {
      throw new Error("an atmosphere that built its own tables takes a medium, not tables");
    }
    if (tables === this.#tables) {
      return;
    }
    this.#tables = tables;
    this.#figure = tablesFigureOf(tables);
    this.#gravity = tables.body?.gravity ?? null;
    this.#drawn = null;
    this.#unread = NO_UNREAD_TABLES;
  }

  /**
   * The sky-view table, the aerial-perspective volume and the ray-march target the latest
   * {@link HillaireAtmosphere.drawFrame} built, the target `null` before the first frame, for the
   * smoke page's agreement checks (R05.T12.e, R08.T6.b). Their kernels are presentation-only, so
   * only a tolerance read-back reads them.
   *
   * @remarks
   * From the march band's upper edge up the frame leaves its sky view and aerial perspective
   * undone until asked (R08.T8.d; {@link HillaireAtmosphere.drawFrame}), and this read asks for
   * both: it may submit their dispatches, once a frame. Since R08.T11.d the march writes the air's
   * area alone, which its composite reads, and this read asks for the rest of the target as well,
   * (0, 0, 0, 1), what the march wrote there before ({@link RAY_MARCH_FILL_KERNEL}), so that the
   * whole target reads as the march over the whole frame.
   */
  get frameTables(): {
    readonly skyView: TextureHandle;
    readonly aerial: TextureHandle;
    readonly rayMarch: TextureHandle | null;
    /**
     * The tile cull's kinds the latest frame wrote, one `u32` a tile of its area, row by row
     * (`marchArea.wgsl`'s `MARCH_TILE_*`; R08.T11.d), `null` before the first frame that marched.
     */
    readonly marchTiles: BufferHandle | null;
    /** The latest frame's march area (R08.T11.d), `null` before the first frame. */
    readonly marchArea: MarchArea | null;
  } {
    this.#read("skyView");
    this.#read("aerial");
    this.#read("rayMarch");
    return {
      skyView: this.#frame.skyView,
      aerial: this.#frame.aerial,
      rayMarch: this.#frame.rayMarch,
      marchTiles: this.#frame.marchTiles,
      marchArea: this.#area,
    };
  }

  /**
   * Sets how the frames' ray march covers its target from the next frame on ({@link MarchCover};
   * plan R08, R08.T11.d). An atmosphere over the device's shared kernels (a view's) marches the
   * area from its making; one with its own marches the whole frame until asked. The tile cull's
   * centre ray's kernel is made at its first `tiles` frame, synchronously, unless the shared kernels
   * made it up front ({@link createAtmosphereFrameKernels}' `tiles`): SwiftShader's GPU process,
   * compiling it beside the footprint's in one task, was ended as hung under load (exit_code 512,
   * Chromium's `RESULT_CODE_HUNG`).
   */
  setMarchCover(cover: MarchCover): void {
    this.#cover = cover;
  }

  /**
   * The aerial perspective {@link HillaireAtmosphere.drawFrame} built for `view` this frame, for the
   * passes the composite does not cover (R08.T9.a): on the `scene` scope (high) the volume and the
   * transmittance tables; on `terrain` (low) `null`, the one deferred composite applying it to the
   * terrain alone. Widens R05's `aerialPerspectiveVolume()`.
   *
   * @remarks
   * From the march band's upper edge up the frame makes its volume only when asked (R08.T8.d): the
   * first call a frame submits its dispatch, so a caller asks each frame before it encodes the pass
   * that samples the volume.
   *
   * @throws Error, on the `scene` scope, if the latest frame was not drawn for `view` (the same
   *   position, orientation, field of view and size), or none has been since the frame's resources
   *   were made or the medium was set: a caller's bug, which would read another view's tables,
   *   released ones or none.
   */
  aerialPerspective(view: AtmosphereCamera): AerialPerspective | null {
    if (this.#sizes.aerialPerspectiveScope === "terrain") {
      return null;
    }
    const drawn = this.#drawnFor(view);
    this.#read("aerial");
    return drawn.aerial;
  }

  /**
   * The sky-view table {@link HillaireAtmosphere.drawFrame} built for `view` this frame, with what
   * reads it (R08.T9.a), on both settings: for R11's sky reflected in the ocean.
   *
   * @remarks
   * From the march band's upper edge up the frame makes its sky view only when asked (R08.T8.d): the
   * first call a frame submits its dispatch, so a caller asks each frame before it encodes the pass
   * that samples the table.
   *
   * @throws Error if the latest frame was not drawn for `view`, as
   *   {@link HillaireAtmosphere.aerialPerspective} does.
   */
  skyView(view: AtmosphereCamera): SkyViewTable {
    const drawn = this.#drawnFor(view);
    this.#read("skyView");
    return drawn.skyView;
  }

  /**
   * Makes the latest frame's `table` if its dispatch waits for a reader (`#unread`), once: each is
   * submitted when asked for, after the frame's ray march and before any pass that reads it.
   */
  #read(table: keyof UnreadTables): void {
    const dispatch = this.#unread[table];
    if (dispatch !== null) {
      this.#unread = { ...this.#unread, [table]: null };
      dispatch();
    }
  }

  /** The latest frame's publication, if it was drawn for `view`. */
  #drawnFor(view: AtmosphereCamera): DrawnFrame {
    const drawn = this.#drawn;
    if (drawn === null) {
      throw new Error(
        "no atmosphere frame has been drawn since its tables were made or its medium was set",
      );
    }
    if (!sameView(drawn.view, view)) {
      throw new Error("the latest atmosphere frame was drawn for another view");
    }
    return drawn;
  }

  /**
   * Rebuilds the per-planet tables for a new medium, or new curves of growth for its absorbers; the
   * sun's moving never does. The frames draw the medium the tables follow
   * ({@link AtmosphereTables.medium}), so a medium they refuse is not drawn. The latest frame's
   * publication ({@link HillaireAtmosphere.aerialPerspective}, {@link HillaireAtmosphere.skyView})
   * is withdrawn until the next frame, since its tables may have been remade.
   *
   * @param curves - Each light's curves for the medium's absorbers, light slot 0 the drawn sun's.
   * @throws Error, RangeError or `EngineUnavailable` as {@link AtmosphereTables.setMedium}; Error
   *   over held tables, whose medium their owner sets ({@link HillaireAtmosphere.setTables}).
   */
  setMedium(medium: AtmosphereMedium, curves: AbsorberCurveSlots = NO_ABSORBER_CURVES): void {
    if (!this.#ownsTables) {
      throw new Error("an atmosphere over held tables takes new tables, not a medium");
    }
    this.#drawn = null;
    this.#unread = NO_UNREAD_TABLES;
    this.#tables.setMedium(medium, tableRadiusM(this.#figure), curves, this.#body());
  }

  /** The body the tables are sliced for ({@link bodyOfFigure} with no gravity). */
  #body(): AtmosphereBody | null {
    const gravity = this.#gravity;
    return gravity === null ? bodyOfFigure(this.#figure) : { figure: this.#figure, gravity };
  }

  /**
   * Builds the frame's sky view, aerial perspective and ray march, summed over the drawn sources
   * in one march along each ray (Design note 7, R08.T7), and returns the composite draw for the
   * caller to submit, with no depth test, into a target other than the scene's.
   *
   * @remarks
   * The per-view tables hold each source's light weighted by its illuminance times the frame's
   * pre-exposure, `scene.exposureScale` ({@link frameSourcesOf}): pre-exposed luminance, as the
   * composite's output is (`decision-r08-t6b-floor.md` §6), so that a twilight or moonlit sky
   * stays in the half float's normal range wherever the exposure follows it. Their bytes do not
   * grow with the sources. Each source's light takes its own light slot's absorber curves along
   * its path to each scattering point and along the view's own path, and its own
   * multiple-scattering layer under a medium with an absorber (`decision-r08-t6b-floor.md` §8).
   *
   * From the march band's upper edge up (the march's share 1) the composite reads neither the sky
   * view nor the aerial perspective, so the frame dispatches the ray march alone and leaves the two
   * tables to a reader that asks this frame ({@link HillaireAtmosphere.skyView},
   * {@link HillaireAtmosphere.aerialPerspective}, {@link HillaireAtmosphere.frameTables}): lever 2
   * of `decision-r08-t8-leans.md` §8, R08.T8.d. The next frame, a new medium, a restore or
   * {@link HillaireAtmosphere.dispose} drops what is left unread.
   *
   * The ray march takes at most `TableSizes.marchSourceCap` of the sources, the brightest at the
   * camera ({@link marchSourcesOf}; plan R08, R08.T8.e): the sky view, the aerial perspective and
   * the composite take them all.
   *
   * Over the device's shared kernels ({@link createAtmosphereFrameKernels}: every photorealistic
   * view's, and so the frame-timing harness's and QW31's) the ray march is dispatched over the air's
   * area alone, the tiles of its target that the top shell can reach (plan R08, R08.T11.d;
   * `marchArea.ts`), the composite reading no air outside them, by the march specialised for one
   * band and no layer edge where the tables fit it ({@link marchSpecialisedFor}). Over its own
   * kernels (the smoke's other checks, the descent spike, the kernel arm) it marches the whole frame
   * by the general march as before, unless asked ({@link HillaireAtmosphere.setMarchCover}; the
   * tile cull is a cover of its own, off). Every cover is exact, each texel what the general march
   * over the whole frame gives: the smoke's `checkMarchArea` and `checkMarchSpecialised` hold the
   * area's, specialised, and the tile cull's to the whole frame's, byte for byte on SwiftShader,
   * and so bind them.
   *
   * @param suns - The drawn sources, at most {@link MAX_SKY_SOURCES}: `sources.ts`'s `skySources`
   *   for a body, or a caller's own; none draws a sky of nothing but what lies behind it.
   * @throws RangeError as {@link frameSourcesOf}: too many sources, a light slot the medium's
   *   curves do not hold, or a pre-exposure that is not finite and above 0;
   *   {@link AtmosphereRetired} over shared kernels after a restore.
   */
  drawFrame(
    view: AtmosphereCamera,
    suns: ReadonlyArray<SunState>,
    scene: AtmosphereScene,
  ): DrawItem {
    if (this.#retired) {
      throw new AtmosphereRetired();
    }
    const frame = this.#frame;
    const rayMarch = this.#rayMarchTexture(view.viewport);
    const { inputs, curvature } = this.#dispatchFrame(
      frame.kernels,
      { skyView: frame.skyView, aerial: frame.aerial, rayMarch },
      view,
      suns,
      scene,
      true,
    );
    this.#drawn = this.#published(view, suns, scene, inputs, curvature);
    return {
      mesh: frame.triangle,
      material: frame.composite,
      offsetFromCameraM: new Float32Array(3),
      uniforms: this.#uniforms,
      textures: {
        sceneColour: scene.colour,
        sceneDepth: scene.depth,
        skyView: frame.skyView,
        aerial: frame.aerial,
        rayMarch,
        transmittance: this.#tables.transmittance,
        absorberColumns: this.#tables.absorberColumns,
        absorberCurves: this.#tables.absorberCurves,
        absorberPressureColumns: this.#tables.absorberPressureColumns,
        lastAbsorberPressureColumn: this.#tables.lastAbsorberPressureColumn,
        oblateBody: this.#tables.oblateBody,
      },
    };
  }

  /**
   * The frame's sky view, aerial perspective and ray march built again by their kernels assembled
   * to write `rgba32float` ({@link withKernelArmFormat}), over the same tables, view and scene: the
   * smoke's kernel arm (plan R08, R08.T6.g; `decision-r08-t6b-floor.md` §4), which holds each
   * kernel's own `f32` output to its twin with no half-float floor. Nothing is drawn; the composite
   * reads only {@link HillaireAtmosphere.drawFrame}'s tables. The kernels and targets are made at
   * the first call, and the ray march's anew when the viewport changes.
   *
   * @returns The arm's three `rgba32float` targets, for a tolerance read-back.
   * @throws RangeError as {@link HillaireAtmosphere.drawFrame}.
   */
  drawKernelArm(
    view: AtmosphereCamera,
    suns: ReadonlyArray<SunState>,
    scene: AtmosphereScene,
  ): FrameOutputs {
    const engine = this.#engine;
    this.#arm ??= createKernelArmResources(engine, this.#sizes);
    const arm = this.#arm;
    const size = arm.rayMarchSize;
    if (
      arm.rayMarch === null ||
      size === null ||
      size.widthPx !== view.viewport.widthPx ||
      size.heightPx !== view.viewport.heightPx
    ) {
      if (arm.rayMarch !== null) {
        engine.releaseTexture(arm.rayMarch);
      }
      const scale = this.#sizes.rayMarchScale;
      arm.rayMarch = frameTexture(
        engine,
        "atmosphere ray march (kernel arm)",
        [
          Math.max(1, Math.ceil(view.viewport.widthPx * scale)),
          Math.max(1, Math.ceil(view.viewport.heightPx * scale)),
        ],
        KERNEL_ARM_FORMAT,
      );
      arm.rayMarchSize = view.viewport;
    }
    const outputs = { skyView: arm.skyView, aerial: arm.aerial, rayMarch: arm.rayMarch };
    this.#dispatchFrame(arm.kernels, outputs, view, suns, scene, false);
    return outputs;
  }

  /**
   * Stops following the engine's restores, drops the latest frame's publication and any table it
   * left unread (R08.T8.d), and releases the frame's own textures and the kernel arm's targets if
   * it drew one (the smoke's, R08.T6.g), and the per-planet tables it built (held tables stay their
   * owner's; plan R08, R08.T10.b). Called again, it does nothing.
   */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.#offRestored();
    this.#unread = NO_UNREAD_TABLES;
    this.#drawn = null;
    if (this.#ownsTables) {
      this.#tables.dispose();
    }
    // Its own frame's textures: a view makes one of these a body, and lets it go as the body leaves
    // its picture (R08.T10.b). Its kernels and composite stay, the shared ones their owner's.
    const frame = this.#frame;
    for (const texture of [frame.skyView, frame.aerial, frame.rayMarch]) {
      if (texture !== null) {
        this.#engine.releaseTexture(texture);
      }
    }
    frame.rayMarch = null;
    frame.rayMarchSize = null;
    if (frame.marchTiles !== null) {
      this.#engine.releaseBuffer(frame.marchTiles);
    }
    frame.marchTiles = null;
    frame.marchTilesBytes = 0;
    const arm = this.#arm;
    if (arm !== null) {
      for (const texture of [arm.skyView, arm.aerial, arm.rayMarch]) {
        if (texture !== null) {
          this.#engine.releaseTexture(texture);
        }
      }
      this.#arm = null;
    }
  }

  /**
   * What a frame publishes for `view` (R08.T9.a): its volume, its sky view and their tables, each
   * per unit of the frame's scale (R08.T7): the tables hold pre-exposed luminance, so the factor
   * to luminance is 1 ÷ the pre-exposure in each channel.
   */
  #published(
    view: AtmosphereCamera,
    suns: ReadonlyArray<SunState>,
    scene: AtmosphereScene,
    inputs: AtmosphereInputs,
    curvature: ViewCurvature,
  ): DrawnFrame {
    const frame = this.#frame;
    const sizes = this.#sizes;
    const tables = this.#tables;
    const medium = tables.medium;
    const unit = 1 / scene.exposureScale;
    const scaleCdPerM2: Rgb = [unit, unit, unit];
    const heightM = skyViewHeight(inputs.heightM, curvature.atmosphereM);
    const radiusM = curvature.meridionalM;
    return {
      view,
      aerial: {
        volume: frame.aerial,
        size: sizes.aerialPerspective,
        reachM: sizes.aerialPerspectiveReachM,
        scaleCdPerM2,
        transmittance: tables.transmittance,
        absorberColumns: tables.absorberColumns,
        absorberPressureColumns: tables.absorberPressureColumns,
        lastAbsorberPressureColumn: tables.lastAbsorberPressureColumn,
        absorberCurves: tables.absorberCurves,
        absorberCount: absorberCount(medium),
        lineSlots: lineSlotMask(medium),
        bottomRadiusM: tableRadiusM(this.#figure),
        topHeightM: medium.topHeightM,
        oblateBody: tables.oblateBody,
      },
      skyView: {
        texture: frame.skyView,
        size: sizes.skyView,
        scaleCdPerM2,
        zenith: inputs.normal,
        azimuthOrigin: skyAxisOf(inputs.normal, suns),
        meridionalRadiusM: curvature.meridionalM,
        primeVerticalRadiusM: curvature.primeVerticalM,
        heightM,
        atmosphereM: curvature.atmosphereM,
        sphere:
          curvature.meridionalM === curvature.primeVerticalM
            ? {
                bottomRadiusM: radiusM,
                horizonAngleRad: Math.atan2(radiusM, Math.sqrt(heightM * (2 * radiusM + heightM))),
              }
            : null,
      },
    };
  }

  /**
   * Fills the view's uniform for this frame and dispatches the three per-frame kernels.
   *
   * @returns The camera's lookup inputs, and its curvature, formed for it in `f64`.
   */
  #dispatchFrame(
    kernels: FrameKernels,
    outputs: FrameOutputs,
    view: AtmosphereCamera,
    suns: ReadonlyArray<SunState>,
    scene: AtmosphereScene,
    deferUnread: boolean,
  ): { readonly inputs: AtmosphereInputs; readonly curvature: ViewCurvature } {
    const engine = this.#engine;
    const sizes = this.#sizes;
    const inputs = atmosphereInputs(view, this.#figure, this.#gravity);
    const tables = this.#tables;
    const medium = tables.medium;
    const curvature = this.#fillView(view, suns, scene, inputs, medium);
    const perFrameMedium = packMediumUniform(medium, inputs.radiusM - inputs.heightM, 1);
    // A body whose tables hold the thick table draws by the kernels that read it (R08.T14.c).
    const thickTable = tables.thickTable;
    const thick = thickTable === null ? null : kernels.thick();
    const common = {
      buffers: { terms: tables.terms },
      sampled: {
        densityTables: tables.densityTables,
        phaseTables: tables.phaseTables,
        lobeTables: tables.lobeTables,
        transmittance: tables.transmittance,
        multiScattering: tables.multiScattering,
        absorberColumns: tables.absorberColumns,
        absorberCurves: tables.absorberCurves,
        absorberPressureColumns: tables.absorberPressureColumns,
        lastAbsorberPressureColumn: tables.lastAbsorberPressureColumn,
        oblateBody: tables.oblateBody,
        ...(thickTable === null ? {} : { thickTable }),
      },
    };
    const viewUniforms = this.#viewBlock();
    // The march takes at most `marchSourceCap` of the sources, the brightest (R08.T8.e); the sky
    // view, the aerial perspective and the composite take them all.
    const marchUniforms = this.#marchBlock(viewUniforms, suns, scene, medium);
    const skyView = (): void => {
      engine.dispatch(
        thick?.skyView ?? kernels.skyView,
        {
          ...common,
          uniforms: { medium: perFrameMedium, view: viewUniforms, layerEdges: tables.layerEdges },
          storage: { skyViewOut: { texture: outputs.skyView, level: 0 } },
        },
        groups8(sizes.skyView.widthTexels, sizes.skyView.heightTexels),
        ATMOSPHERE_VIEW_PASS,
      );
    };
    const aerial = (): void => {
      engine.dispatch(
        thick?.aerial ?? kernels.aerial,
        {
          ...common,
          uniforms: { medium: perFrameMedium, view: viewUniforms },
          storage: { aerialOut: { texture: outputs.aerial, level: 0 } },
        },
        groups8(sizes.aerialPerspective.widthTexels, sizes.aerialPerspective.heightTexels),
        ATMOSPHERE_VIEW_PASS,
      );
    };
    // From the march band's upper edge up the composite reads neither the sky view nor the aerial
    // perspective (`composite.wgsl`: both only where the march's share, `view.exact.w` in f32 as
    // read here, is under 1), so each waits for a reader that asks, and none moves a pixel (lever 2
    // of `decision-r08-t8-leans.md` §8, R08.T8.d). The kernel arm makes all three, for its
    // read-backs, and leaves the frame's own waiting.
    const marchesAll = deferUnread && (this.#uniforms["exact"]?.[3] ?? 0) >= 1;
    if (!marchesAll) {
      skyView();
      aerial();
    }
    // The march over the air's area alone (R08.T11.d, `marchArea.ts`): the tiles its top shell,
    // widened, can reach, which the composite reads (`marchArea`, its texels); outside them it reads
    // no air. Under the tile cull the centre ray's entry point, first, classes every tile and takes
    // those no footprint can blend in, and the footprint's the rest; on the area and the whole
    // frame the footprint's takes every tile, as the one kernel did.
    // A thick body's march takes no cull (R08.T14.c): its area's every tile, exact as the cull is.
    const tileCull = this.#cover === "tiles" && thick === null;
    const shell = topShellOf(this.#figure, medium.topHeightM, this.#gravity);
    const area =
      this.#cover === "whole frame"
        ? wholeMarchArea(view, shell, sizes.rayMarchScale)
        : marchAreaOf(view, shell, sizes.rayMarchScale);
    const areaUniform = marchAreaUniform(area, tileCull);
    const texels = marchAreaTexels(area);
    this.#uniforms["marchArea"]?.set([texels.x0, texels.y0, texels.x1, texels.y1]);
    // A body whose tables hold the slot layers marches by slot, each source's light one read of
    // its slot's layer (R08.T8.e, lever 5); every other the shared read.
    const slotDepths = tables.slotDepths;
    // The march's kernels: the thin read's, with the tile cull's centre ray, or the thick read's.
    const march: FrameMarch =
      thick === null
        ? {
            kind: "thin",
            kernels: slotDepths === null ? kernels.rayMarch() : kernels.rayMarchSlots(),
          }
        : {
            kind: "thick",
            kernels: slotDepths === null ? thick.rayMarch() : thick.rayMarchSlots(),
          };
    if (area.tilesAcross > 0 && area.tilesDown > 0) {
      const bindings = {
        buffers: { ...common.buffers, marchTiles: this.#marchTiles(area) },
        sampled: {
          ...common.sampled,
          sceneDepth: scene.depth,
          ...(slotDepths === null ? {} : { slotDepths }),
        },
        uniforms: {
          medium: perFrameMedium,
          view: marchUniforms,
          layerEdges: tables.layerEdges,
          marchArea: areaUniform,
        },
        storage: { rayMarchOut: { texture: outputs.rayMarch, level: 0 } },
      };
      const tiles = [area.tilesAcross, area.tilesDown, 1] as const;
      if (tileCull && march.kind === "thin") {
        engine.dispatch(march.kernels.centre(), bindings, tiles, ATMOSPHERE_VIEW_PASS);
      }
      // A view's area over a body of one band with no layer edge takes the specialised march
      // (R08.T11.d): exact there, and the general one on the reference path and under the cull.
      const specialised = this.#cover === "area" && marchSpecialisedFor(tables);
      engine.dispatch(
        specialised ? march.kernels.special() : march.kernels.footprint,
        bindings,
        tiles,
        ATMOSPHERE_VIEW_PASS,
      );
    }
    // The rest of the target, (0, 0, 0, 1), for a reader of the whole (R08.T11.d): the frame's
    // waits for one, and the kernel arm's read-backs take it at once.
    const fill = area.whole
      ? null
      : (): void => {
          engine.dispatch(
            kernels.rayMarchFill(),
            {
              buffers: {},
              sampled: {},
              uniforms: { marchArea: areaUniform },
              storage: { rayMarchOut: { texture: outputs.rayMarch, level: 0 } },
            },
            groups8(area.widthTexels, area.heightTexels),
            ATMOSPHERE_VIEW_PASS,
          );
        };
    if (deferUnread) {
      this.#area = area;
      this.#unread = {
        skyView: marchesAll ? skyView : null,
        aerial: marchesAll ? aerial : null,
        rayMarch: fill,
      };
    } else {
      fill?.();
    }
    return { inputs, curvature };
  }

  /**
   * The tile cull's kinds' buffer for `area` (R08.T11.d), one `u32` a tile: made for the whole
   * target's tiles, its largest, and grown with the target, never made smaller, as the ray-march
   * target is ({@link HillaireAtmosphere.drawFrame}'s `#rayMarchTexture`).
   */
  #marchTiles(area: MarchArea): BufferHandle {
    const frame = this.#frame;
    const tileBytes = 4;
    const bytes = Math.max(area.tilesAcross * area.tilesDown, 1) * tileBytes;
    if (frame.marchTiles !== null && frame.marchTilesBytes >= bytes) {
      return frame.marchTiles;
    }
    if (frame.marchTiles !== null) {
      this.#engine.releaseBuffer(frame.marchTiles);
    }
    const whole =
      Math.ceil(area.widthTexels / MARCH_TILE_TEXELS) *
      Math.ceil(area.heightTexels / MARCH_TILE_TEXELS) *
      tileBytes;
    const made = Math.max(bytes, whole, frame.marchTilesBytes);
    frame.marchTiles = this.#engine.createBuffer({
      name: "atmosphere ray march's tiles",
      bytes: made,
      // Read back by the smoke page's check alone (`frameTables`).
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
      category: "atmosphere-view",
    });
    frame.marchTilesBytes = made;
    return frame.marchTiles;
  }

  /**
   * The ray-march target at the setting's scale of the largest output so far: it grows on a
   * resize and is never remade smaller, so that a window resized back and forth makes nothing new;
   * the kernel and the composite address it by the current output's size.
   */
  #rayMarchTexture(viewport: ViewSize): TextureHandle {
    const frame = this.#frame;
    const fits =
      frame.rayMarchSize !== null &&
      frame.rayMarchSize.widthPx >= viewport.widthPx &&
      frame.rayMarchSize.heightPx >= viewport.heightPx;
    if (frame.rayMarch !== null && fits) {
      return frame.rayMarch;
    }
    const grown: ViewSize = {
      widthPx: Math.max(viewport.widthPx, frame.rayMarchSize?.widthPx ?? 0),
      heightPx: Math.max(viewport.heightPx, frame.rayMarchSize?.heightPx ?? 0),
    };
    const scale = this.#sizes.rayMarchScale;
    frame.rayMarch = frameTexture(
      this.#engine,
      "atmosphere ray march",
      [
        Math.max(1, Math.ceil(grown.widthPx * scale)),
        Math.max(1, Math.ceil(grown.heightPx * scale)),
      ],
      "rgba16float",
    );
    frame.rayMarchSize = grown;
    return frame.rayMarch;
  }

  /**
   * The view's members as one `AtmosphereView` block, for the kernels: the frame's own, or
   * `members` (the march's, `#marchBlock`).
   */
  #viewBlock(members: Readonly<Record<string, ArrayLike<number>>> = this.#uniforms): Float32Array {
    const block = new Float32Array(VIEW_MEMBERS.length * 4);
    for (const [i, name] of VIEW_MEMBERS.entries()) {
      const member = members[name];
      if (member !== undefined) {
        block.set(member, i * 4);
      }
    }
    return block;
  }

  /**
   * The ray march's `AtmosphereView` block (plan R08, R08.T8.e): the frame's `view`, its sources
   * the march's ({@link marchSourcesOf} at `TableSizes.marchSourceCap`), in their order, with
   * `sky.x` their count and `sky.y` the light slots their paths take; `view` itself where the
   * frame draws no more sources than the cap, so that its march is the same to the bit.
   */
  #marchBlock(
    view: Float32Array,
    suns: ReadonlyArray<SunState>,
    scene: AtmosphereScene,
    medium: AtmosphereMedium,
  ): Float32Array {
    const marched = marchSourcesOf(suns, this.#sizes.marchSourceCap);
    if (marched.length === suns.length) {
      return view;
    }
    const sources = frameSourcesOf(
      marched,
      scene.exposureScale,
      absorberCount(medium) > 0,
      this.#tables.curves.length,
    );
    const sky = this.#uniforms["sky"] ?? new Float32Array(4);
    const members: Record<string, ArrayLike<number>> = {
      ...this.#uniforms,
      sky: [sources.length, pathSlotsOf(sources), sky[2] ?? 0, sky[3] ?? 0],
    };
    for (let i = 0; i < MAX_SKY_SOURCES; i += 1) {
      const source = sources[i];
      members[`sourceDirection${i}`] = source?.direction ?? [0, 0, 1, 0];
      members[`sourceWeight${i}`] = source?.weight ?? [0, 0, 0, 0];
      members[`sourceDisc${i}`] = source?.disc ?? [0, 0, 0, 0];
    }
    return this.#viewBlock(members);
  }

  #fillView(
    view: AtmosphereCamera,
    suns: ReadonlyArray<SunState>,
    scene: AtmosphereScene,
    inputs: AtmosphereInputs,
    medium: AtmosphereMedium,
  ): ViewCurvature {
    const set = (name: string, values: readonly [number, number, number, number]): void => {
      this.#uniforms[name]?.set(values);
    };
    const topHeightM = medium.topHeightM;
    const p = view.positionM;
    const n = inputs.normal;
    const absorbers = absorberCount(medium) > 0;
    // The per-view tables' unit is the frame's pre-exposure (`decision-r08-t6b-floor.md` §6): each
    // source's weight is its illuminance times it, so the tables hold pre-exposed luminance and the
    // composite's factor from them, the scale F = 1 ÷ exposureScale times the exposure, is 1.
    const sources = frameSourcesOf(
      suns,
      scene.exposureScale,
      absorbers,
      this.#tables.curves.length,
    );
    const axis = skyAxisOf(n, suns);
    const aspect = view.viewport.widthPx / view.viewport.heightPx;
    const tanX = Math.tan(view.fovXRad / 2);
    const tanY = tanX / aspect;
    const right = rotate(view.orientation, { x: 1, y: 0, z: 0 });
    const up = rotate(view.orientation, { x: 0, y: 1, z: 0 });
    const forward = rotate(view.orientation, { x: 0, y: 0, z: -1 });
    const sizes = this.#sizes;
    const exact = viewExactOf(p, this.#figure, medium, inputs, this.#gravity);
    const curvature = viewCurvatureOf(this.#figure, medium, inputs);
    set("camera", [p.x, p.y, p.z, inputs.heightM]);
    set("up", [n.x, n.y, n.z, inputs.radiusM - inputs.heightM]);
    set("sky", [sources.length, pathSlotsOf(sources), this.#tables.multiScatteringLayers, 1]);
    set("skyAxis", [axis.x, axis.y, axis.z, 0]);
    for (let i = 0; i < MAX_SKY_SOURCES; i += 1) {
      const source = sources[i];
      set(`sourceDirection${i}`, source?.direction ?? [0, 0, 1, 0]);
      set(`sourceWeight${i}`, source?.weight ?? [0, 0, 0, 0]);
      set(`sourceDisc${i}`, source?.disc ?? [0, 0, 0, 0]);
    }
    set("right", [right.x * tanX, right.y * tanX, right.z * tanX, 0]);
    set("upRay", [up.x * tanY, up.y * tanY, up.z * tanY, 0]);
    set("forward", [forward.x, forward.y, forward.z, 0]);
    set("tables", [
      tableRadiusM(this.#figure),
      sizes.aerialPerspectiveReachM,
      sizes.aerialPerspective.slices,
      sizes.aerialPerspectiveSamplesPerSlice,
    ]);
    set("figure", [
      this.#figure.equatorialRadiusM,
      this.#figure.polarRadiusM,
      scene.nearM,
      sizes.rayMarchSamples,
    ]);
    set("output", [
      view.viewport.widthPx,
      view.viewport.heightPx,
      sizes.rayMarchScale,
      sizes.skyViewSamples,
    ]);
    // The march's share of the frame at the camera's h* (R08.T8), formed in f64.
    const marchWeight = marchWeightOf(
      inputs.heightM * curvature.gravityRatio,
      topHeightM,
      sizes.marchBand,
    );
    set("exact", [exact.groundQuadricM2, exact.topQuadricM2, exact.localScaleHeightM, marchWeight]);
    set("curvature", [
      curvature.meridionalM,
      curvature.primeVerticalM,
      curvature.gravityRatio,
      curvature.atmosphereM,
    ]);
    // The march's count by optical depth on the bare ground (R08.T8.e, lever 3): its floor, what
    // each layer edge adds, and the ramp's depths; a floor of 0 where every ray takes the setting's.
    const ground = sizes.marchDepthSteps;
    set(
      "marchGround",
      ground === null
        ? [0, 0, 0, 0]
        : [
            ground.groundFloor,
            ground.groundFloorPerEdge,
            ground.groundRampDepths[0],
            ground.groundRampDepths[1],
          ],
    );
    // A thin sky segment's count and the depth under which it takes it (R08.T8.e, lever 3), a
    // count of 0 where every sky segment takes the setting's.
    const thinSky = ground?.thinSky ?? null;
    // And the band over the datum within which a drawn surface takes the bare ground's count
    // (R08.T11.b), 0 where the bare ground keeps the setting's.
    set("marchSky", [thinSky?.samples ?? 0, thinSky?.depthG ?? 0, ground?.drawnBandM ?? 0, 0]);
    // The composite's own: the atmosphere's height, formed in f64 (R08.T6.g), how many of the
    // medium's terms are absorbers, whose columns its transmittance to space reads (R08.T6.c), and
    // its line slots, whose pressure channels the read takes (R08.T4.d).
    set("shell", [topHeightM, absorberCount(medium), lineSlotMask(medium), 0]);
    return curvature;
  }
}
