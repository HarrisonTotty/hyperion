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
  TexelRect,
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
  marchAreaPixels,
  marchAreaTexels,
  type MarchShell,
  wholeMarchArea,
} from "./marchArea";
import { DRAWN_SURFACE_BAND_M, type MarchDepthSteps } from "./marchSteps";
import { type AtmosphereMedium, localScaleHeightM } from "./medium";
import {
  type BodyGravity,
  bodyGravity,
  geodeticOf,
  gravityRatio,
  principalRadiiM,
  tableRadiusM,
} from "./oblate";
import commonWgsl from "./shaders/common.wgsl?raw";
import aerialPerspectiveWgsl from "./shaders/aerialPerspective.wgsl?raw";
import aerialPerspectiveBelowDeckWgsl from "./shaders/aerialPerspectiveBelowDeck.wgsl?raw";
import aerialReadWgsl from "./shaders/aerialRead.wgsl?raw";
import belowDeckWgsl from "./shaders/belowDeck.wgsl?raw";
import belowDeckViewWgsl from "./shaders/belowDeckView.wgsl?raw";
import compositeWgsl from "./shaders/composite.wgsl?raw";
import compositeBehindWgsl from "./shaders/compositeBehind.wgsl?raw";
import compositeTransmittanceWgsl from "./shaders/compositeTransmittance.wgsl?raw";
import farLimbWgsl from "./shaders/farLimb.wgsl?raw";
import limbDispatchWgsl from "./shaders/limbDispatch.wgsl?raw";
import limbListWgsl from "./shaders/limbList.wgsl?raw";
import limbNodesWgsl from "./shaders/limbNodes.wgsl?raw";
import limbResolveWgsl from "./shaders/limbResolve.wgsl?raw";
import marchAreaWgsl from "./shaders/marchArea.wgsl?raw";
import marchFillWgsl from "./shaders/marchFill.wgsl?raw";
import marchSourceTableWgsl from "./shaders/marchSourceTable.wgsl?raw";
import mediumWgsl from "./shaders/medium.wgsl?raw";
import oblateWgsl from "./shaders/oblate.wgsl?raw";
import rayMarchWgsl from "./shaders/rayMarch.wgsl?raw";
import rayMarchBelowDeckWgsl from "./shaders/rayMarchBelowDeck.wgsl?raw";
import rayMarchCentreWgsl from "./shaders/rayMarchCentre.wgsl?raw";
import rayMarchCurvesWgsl from "./shaders/rayMarchCurves.wgsl?raw";
import rayMarchFootprintWgsl from "./shaders/rayMarchFootprint.wgsl?raw";
import rayMarchPackedWgsl from "./shaders/rayMarchPacked.wgsl?raw";
import rayMarchSlotsWgsl from "./shaders/rayMarchSlots.wgsl?raw";
import scatteringWgsl from "./shaders/scattering.wgsl?raw";
import skyViewWgsl from "./shaders/skyView.wgsl?raw";
import skyViewBelowDeckWgsl from "./shaders/skyViewBelowDeck.wgsl?raw";
import sourceWgsl from "./shaders/source.wgsl?raw";
import thermalSourceWgsl from "./shaders/thermalSource.wgsl?raw";
import deckReadWgsl from "./shaders/deckRead.wgsl?raw";
import thickReadWgsl from "./shaders/thickRead.wgsl?raw";
import thickReadBandsWgsl from "./shaders/thickReadBands.wgsl?raw";
import thickSampleRowWgsl from "./shaders/thickSampleRow.wgsl?raw";
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
  THICK_CORRECTION_MODES,
  THICK_MODES,
  THICK_SUN_COSINES,
  THICK_TRAVEL_COSINES,
} from "./thick/read";
import { belowDeckOpticalDepthInto, belowDeckReadScratch, belowDeckSplitM } from "./thick/deckRead";
import { tableOnlyKernel } from "./surfaceLighting";
import { atan2, log, sin, tan } from "../../lib/pinnedMath";

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
  /**
   * The polarisation correction's grid, a band and light slot (plan R08, Design notes 10 and 11;
   * R08.T14.f): the thick table's, which it shares, read on both settings, and held in it on a
   * `thickScattering` world (`thick/read.ts`'s `thick` kind) and alone on a thin one (its
   * `correction` kind).
   */
  readonly polarisation: ThickTableSize;
}

/**
 * The thick table's grid (plan R08, Design note 9; R08.T14.c; `thick/read.ts`): its altitudes, sun
 * cosines, travel cosines and azimuthal modes, the bake's `THICK_BAKE_GRID`, and the modes it
 * stores, the polarisation correction's three (R08.T14.f).
 */
export interface ThickTableSize {
  readonly altitudes: number;
  readonly sunCosines: number;
  readonly travelCosines: number;
  readonly modes: number;
}

/**
 * The thick table's grid as built (R08.T14.c, R08.T14.f, R08.T14.d.1 and R08.T14.d.2): 33
 * altitudes, 46 μ₀ (14 below the horizontal and 32 above it) and 16 μ_v, J per unit of the kernels'
 * scattering σ_s,k at m = 0, 1 and 2, the bake's and the correction's summed into one texel a cell,
 * 388,608 B a band and light slot (`thick/read.ts`'s `THICK_TABLE_BYTES_A_SLOT`).
 */
export const THICK_TABLE_SIZE: ThickTableSize = {
  altitudes: THICK_ALTITUDES,
  sunCosines: THICK_SUN_COSINES,
  travelCosines: THICK_TRAVEL_COSINES,
  modes: THICK_MODES,
};

/**
 * The polarisation correction's grid as built (R08.T14.f; Design note 11's correction row): the
 * thick table's, m = 0, 1 and 2, 388,608 B a band and light slot in the same texels (270,336 B at
 * R08.T14.f's 32 μ₀, against the note's 393 KB for three modes in `rgba16float`).
 */
export const POLARISATION_TABLE_SIZE: ThickTableSize = {
  altitudes: THICK_ALTITUDES,
  sunCosines: THICK_SUN_COSINES,
  travelCosines: THICK_TRAVEL_COSINES,
  modes: THICK_CORRECTION_MODES,
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
    polarisation: POLARISATION_TABLE_SIZE,
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
    polarisation: POLARISATION_TABLE_SIZE,
  },
};

/**
 * Where the composite forms what lies behind the sky from above the top (the owner's priority,
 * 2026-11-01): `march`, at the ray march's resolution where it runs below the output's, the
 * transmittance across from above formed once a march texel as each channel's ln τ
 * (`compositeBehind.wgsl`) and read bilinearly from a pixel's four candidates (`composite.wgsl`'s
 * `behindAtMarch`) where all are sky and their τ is at most 2; or `per-pixel`, formed in the
 * composite for each pixel, as before, the way back. Within a drawn edge's texel, past that depth
 * and within a source's disc, it is per pixel either way.
 */
export type CompositeTransmittance = "march" | "per-pixel";

/**
 * What the composite dims a drawn surface seen through the air by (plan R08, R08.T9.c): `channels`,
 * its transmittance per channel, which the aerial-perspective volume and the ray march store in a
 * second texture beside their light (`aerialRead.wgsl`'s `aerialTransmittanceAt`, `rayMarch.wgsl`'s
 * `storeMarched`); or `grey`, the channels' mean, R05's choice (Hillaire 2020, §5.4, "the mean of the
 * wavelength dependent RGB transmittance"), the way back. Through 5 km from 2 m on the measured
 * Earth the mean left the ground 13% too dim in red and 23% too bright in blue, and through the limb
 * from orbit grey where the air reddens it. Both store the second textures, so `grey` draws the
 * pixels it drew before, to the bit.
 */
export type SurfaceTransmittance = "channels" | "grey";

/**
 * What a drawn surface seen through the air keeps of its light scattered within the medium's
 * forward lobes (plan R08, fix-r08-decked-air-opacity; R08.T7's lobes, `forwardLobe.ts`):
 * `transmitted`, the ray march's second target with each drawn lobe's in-lobe scattering dropped
 * from its extinction, e^(−∫(σ_t − Σ P σ_s) ds) per channel (`rayMarch.wgsl`), R08.T7's δ-M
 * (Wiscombe 1977, J. Atmos. Sci. 34, 1408), as the sun's beam and Ψ_ms already take it; or
 * `extinguished`, all of it as extinction, the way back. From orbit a surface's light scattered
 * within a lobe (30°) lands within its own pixel, as the tracer counts it: on the served Earth-like
 * `42002cae00000004.0300` (27), whose water deck the client draws uniform at τ(550) 1.15, the drawn
 * ground stood at 0.30–0.41 of the tracer's independent-pixel truth (`hyperion-fit
 * atmosphere-reference`, disc p0–p30 from 2,000 km). The march's first target, the
 * aerial-perspective volume, the `grey` way back's mean and a medium with no drawn lobe are the same
 * either way, to the bit. A march that reads a bake's table keeps none either way: the table's J
 * holds that light ({@link withThickRead}).
 */
export type SurfaceLobes = "transmitted" | "extinguished";

/**
 * How a view's ray march covers its target (plan R08, R08.T8.d; {@link MarchCover}): `area`, the
 * air's area by the footprint's entry point alone, the way back; or `tiles`, R08.T11.d's exact tile
 * cull over it, the centre ray's entry point marching the tiles no far-limb footprint can blend in
 * and the footprint's the rest. Since R08.T8.d both of the cull's entry points take the area's
 * form, specialised for one band where the tables fit it ({@link marchSpecialisedFor},
 * {@link marchOneBandFor}), so that the cull drops no specialisation; a thick or self-luminous
 * body's march takes no cull, as before.
 */
export type ViewMarchCover = Exclude<MarchCover, "whole frame">;

/**
 * How a frame's march takes the node rays of its far-limb footprints (plan R08, R08.T8.d; the
 * owner's Mac, 2026-11-01): `per-texel`, each texel marching its centre ray and its nodes in turn,
 * the way back, so that a tile of 8 × 8 texels waits on its slowest texel's up to ten nodes; or
 * `packed`, each blended texel's nodes written to a list by the march (`rayMarchPacked.wgsl`) and
 * marched one an invocation (`limbNodes.wgsl`), then summed into the texel in node order
 * (`limbResolve.wgsl`), so that each texel's bits are the per-texel march's. A texel whose nodes
 * find no room in the lists is marched per texel. The thin read's march, on the area and the tile
 * cull, and since perf-thick-march the thick read's but over a deck, on the area; the deck and
 * self-luminous marches, and the whole frame, march per texel.
 */
export type LimbRays = "per-texel" | "packed";

/**
 * Where a lit pass takes each source's transmittance through the air to a lit point (plan R08,
 * R08.T9.b's lever): `table`, from a table formed once after each build of the tables by height
 * and zenith cosine for each latitude band and light slot, each texel the sliced lookup itself
 * (`AtmosphereTables.sourceTransmittance`, `sourceTransmittance.wgsl`), read bilinearly in its
 * optical depth; or `per-fragment`, the sliced lookup at each lit fragment, a star's once and a
 * planetshine source's at each of its three nodes, the way back. The table takes no azimuth: its
 * rays stand at π ÷ 4 from north, and the ground's shadow is the reference sphere's.
 */
export type SourceTransmittance = "table" | "per-fragment";

/**
 * Where the ray march takes each step's sources' light (plan R08, R08.T8.e's table; the owner's Mac):
 * `table`, from R08.T9.b's sources' table by the step's h* and the source's zenith cosine, its
 * lobes' layers for a medium with forward lobes (`marchSourceTable.wgsl`), a body of several bands
 * taking a source within 0.1 of the horizon's cosine by the sliced lookup, and a body of one band
 * drawn by the march's table-only twins (`sourceTableEngine.ts`); or `per-step`, the sliced lookup
 * a source a step, the way back. The thin read's marches, and since perf-thick-march the thick
 * read's shared read but over a deck on a body of one band, from the table alone
 * ({@link RAY_MARCH_THICK_TABLE_KERNEL}); the thick read's march by slot, whose lookup is one read a
 * source a step already, and its march over several bands, and a deck or self-luminous body's
 * march, take each step's sources per step under either.
 */
export type MarchSourceTransmittance = "table" | "per-step";

/**
 * The atmosphere's levers on a setting, `SETTINGS[s].atmosphereLevers` (the owner's priority,
 * 2026-11-01), each with the way back beside it, as R14's `GeometrySettings` keeps its own.
 */
export interface AtmosphereSettings {
  /** {@link CompositeTransmittance}: `march` on both settings; `per-pixel` is the way back. */
  readonly compositeTransmittance: CompositeTransmittance;
  /** {@link SurfaceTransmittance} (R08.T9.c): `channels` on both settings; `grey` is the way back. */
  readonly surfaceTransmittance: SurfaceTransmittance;
  /** {@link SurfaceLobes}: `transmitted` on both settings; `extinguished` is the way back. */
  readonly surfaceLobes: SurfaceLobes;
  /** {@link ViewMarchCover} (R08.T8.d): `area` on both settings, the way back; `tiles` the lever. */
  readonly marchCover: ViewMarchCover;
  /** {@link LimbRays} (R08.T8.d): `packed` on both settings; `per-texel` is the way back. */
  readonly limbRays: LimbRays;
  /**
   * {@link SourceTransmittance} (R08.T9.b's lever): `table` on both settings; `per-fragment` is the
   * way back. The lit passes take it (`photoreal/renderer.ts`); the composite reads none.
   */
  readonly sourceTransmittance: SourceTransmittance;
  /**
   * {@link MarchSourceTransmittance} (R08.T8.e's table): the march's step's sources from the
   * sources' table, or `per-step`, the way back.
   */
  readonly marchSourceTransmittance: MarchSourceTransmittance;
}

/**
 * The composite with its transmittance behind the sky per pixel, as before the cost levers, its
 * surfaces dimmed per channel (R08.T9.c) and keeping their forward lobes' light
 * (fix-r08-decked-air-opacity), a view's march over the air's area per texel, each source's
 * transmittance to a lit point per fragment, and the march's sources per step: what an atmosphere
 * drawn outside a view takes (the smoke's checks, the descent spike), unless given its view's.
 */
export const PER_PIXEL_LEVERS: AtmosphereSettings = {
  compositeTransmittance: "per-pixel",
  surfaceTransmittance: "channels",
  surfaceLobes: "transmitted",
  marchCover: "area",
  limbRays: "per-texel",
  sourceTransmittance: "per-fragment",
  marchSourceTransmittance: "per-step",
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
 * A body-fixed point's geodetic coordinates and the per-planet tables' ground (plan R08,
 * R08.T12.b.2): `oblate.ts`'s, so that the reference cases' import graph, which the thick cases'
 * worker takes, stays free of the engine.
 */
export { type Geodetic, geodeticOf, tableRadiusM } from "./oblate";

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
    const solidAngle = 4 * Math.PI * sin(sun.angularRadiusRad / 2) ** 2;
    const disc = drawsDiscOf(sun) && solidAngle > 0 ? frameScale / solidAngle : 0;
    return {
      direction: [s.x, s.y, s.z, sin(sun.angularRadiusRad)],
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

/**
 * The pass a frame forms what lies behind its sky from above the top in, at the march's resolution,
 * before its composite (the owner's priority, 2026-11-01; {@link COMPOSITE_BEHIND_KERNEL}).
 */
export const ATMOSPHERE_BEHIND_PASS = "atmosphere behind";

/** The sky-view kernel. */
export const SKY_VIEW_KERNEL = kernel("atmosphere sky view", skyViewWgsl);
/** The aerial-perspective kernel. */
export const AERIAL_PERSPECTIVE_KERNEL = kernel(
  "atmosphere aerial perspective",
  aerialPerspectiveWgsl,
);
/**
 * The thin march's step's sources from the sources' table (plan R08, R08.T8.e's table;
 * {@link MarchSourceTransmittance}; `marchSourceTable.wgsl`): the shared read's line that takes a
 * source's light per step (`source.wgsl`'s `sourceInScatteringAt`, which `rayMarchCurves.wgsl`'s
 * step takes), with what stands in its place: the light from the bound table where it holds the
 * source, and the read's own lookup elsewhere, each a loop of at most one pass, not an `if`, as the
 * composite's reads are (`composite.wgsl`), since SwiftShader runs both sides of a branch under its
 * lanes' mask. The arithmetic of either way is the read's own.
 */
const TABLED_SHARED_READ: ReadonlyArray<readonly [string, string]> = [
  [
    "let light = sourceLightAt(body, start, bodyDir, slot);",
    [
      "let tabled = marchTableTakes(body, start.scaledHeightM, muSun);",
      "  var light = SourceLight(0.0, vec3f(0.0), vec3f(0.0));",
      "  for (var fromTable = tabled; fromTable; fromTable = false) {",
      "    light = marchTabledLightAt(body, start, muSun, slot, lobes);",
      "  }",
      "  for (var direct = !tabled; direct; direct = false) {",
      "    light = sourceLightAt(body, start, bodyDir, slot);",
      "  }",
    ].join("\n"),
  ],
];

/**
 * {@link TABLED_SHARED_READ}'s for the read by slot (`rayMarchSlots.wgsl`'s `rayMarchSourcesAt`):
 * its sliced ray and slot read inside the loop of the read's own way, so that each stays at one call
 * site (`rayMarchSlots.wgsl`'s head), the source's cosine formed for the table as the step forms it.
 */
const TABLED_SLOT_READ: ReadonlyArray<readonly [string, string]> = [
  [
    "let sliced = slicedRayOf(body, oblateBody, start, dir);",
    [
      "let stepMu = dot(dir, up);",
      "    let tabled = marchTableTakes(body, start.scaledHeightM, stepMu);",
      "    var light = SourceLight(0.0, vec3f(0.0), vec3f(0.0));",
      "    for (var fromTable = tabled; fromTable; fromTable = false) {",
      "      light = marchTabledLightAt(body, start, stepMu, slot, lobes);",
      "    }",
      "    for (var direct = !tabled; direct; direct = false) {",
      "      let sliced = slicedRayOf(body, oblateBody, start, dir);",
    ].join("\n"),
  ],
  [
    "let read = slicedSlotDepthsOf(sliced, body, slot, lobes);",
    "  let read = slicedSlotDepthsOf(sliced, body, slot, lobes);",
  ],
  [
    "let light = SourceLight(select(1.0, 0.0, sliced.shadowed), exp(-read[0]), read[1]);",
    "  light = SourceLight(select(1.0, 0.0, sliced.shadowed), exp(-read[0]), read[1]);\n    }",
  ],
];

/**
 * `pair`, a thin march's (the shared read's or the read by slot's), with its step's sources from the
 * sources' table where one is bound ({@link TABLED_SHARED_READ}, {@link TABLED_SLOT_READ}), and
 * `marchSourceTable.wgsl` appended, which declares the table at `atmosphere_source_table`: through
 * the table's one-texel stub every step takes the read's own lookup, as before the lever. Its name
 * is kept. The thick, deck and thermal marches are made from the march without it
 * ({@link RAY_MARCH_PER_STEP_KERNEL}), so that their kernels keep their bytes; the thick read's
 * marches from the table are made from the march with it ({@link RAY_MARCH_THICK_TABLE_KERNEL}).
 *
 * @throws Error naming the kernel when its sources hold a rewritten line nowhere, or twice.
 */
function withMarchSourceTable(pair: KernelPair): KernelPair {
  const bySlot = pair.reference.includes("fn slicedSlotDepthsOf(");
  const tabled = withMarchRewrites(
    pair,
    bySlot ? TABLED_SLOT_READ : TABLED_SHARED_READ,
    "the sources' table",
  );
  return {
    ...tabled,
    name: pair.name,
    reference: `${tabled.reference}\n${marchSourceTableWgsl}`,
    subgroup: tabled.subgroup === null ? null : `${tabled.subgroup}\n${marchSourceTableWgsl}`,
  };
}

/**
 * The per-pixel ray-march kernel over the far limb's footprint (`farLimb.wgsl`, R08.T8.c), each
 * source's light at each step read through the shared tables with its slot's curves applied on
 * read (`rayMarchCurves.wgsl`), as before R08.T8.e's table: the base of the thick read's, the deck
 * read's and the thermal forms' marches ({@link withThickRead}, {@link withThermalSource}), which
 * take each step's sources per step. No frame dispatches it.
 */
export const RAY_MARCH_PER_STEP_KERNEL = kernel(
  "atmosphere ray march",
  `${farLimbWgsl}\n${marchAreaWgsl}\n${rayMarchWgsl}\n${rayMarchCurvesWgsl}\n${rayMarchFootprintWgsl}`,
);
/**
 * The per-pixel ray-march kernel over the far limb's footprint (`farLimb.wgsl`, R08.T8.c), each
 * source's light read through the shared tables with its slot's curves applied on read
 * (`rayMarchCurves.wgsl`), or since R08.T8.e's table from the sources' table where one is bound
 * ({@link withMarchSourceTable}): the march of every body whose tables hold no slot layers
 * (`tables.ts`'s `slotDepthsRefusal`; R08.T8.e). Since R08.T11.d's tile cull it marches the tiles
 * of the air's area whose footprints may blend (`rayMarchFootprint.wgsl`), after
 * {@link RAY_MARCH_CENTRE_KERNEL} took the rest, and under the reference path every tile.
 */
export const RAY_MARCH_KERNEL = withMarchSourceTable(RAY_MARCH_PER_STEP_KERNEL);
/**
 * {@link RAY_MARCH_SLOTS_KERNEL}'s march as before R08.T8.e's table, each step's sources per step,
 * as {@link RAY_MARCH_PER_STEP_KERNEL} is the shared read's: the base of the thick, deck and
 * thermal marches by slot. No frame dispatches it.
 */
export const RAY_MARCH_SLOTS_PER_STEP_KERNEL = kernel(
  "atmosphere ray march by slot",
  `${farLimbWgsl}\n${marchAreaWgsl}\n${rayMarchWgsl}\n${rayMarchSlotsWgsl}\n${rayMarchFootprintWgsl}`,
);
/**
 * The per-pixel ray-march kernel of a body whose tables hold the slot layers (plan R08, R08.T8.e,
 * lever 5; `decision-r08-t8-leans.md` §10 and §13; `AtmosphereTables.slotDepths`): the same march,
 * each source's light at each step and the bare ground's one read of its light slot's layer
 * (`rayMarchSlots.wgsl`), which only this kernel and {@link RAY_MARCH_SLOTS_CENTRE_KERNEL}
 * declare, each step's from the sources' table where one is bound (R08.T8.e's table). Chosen when
 * the body's pipeline is made, so that neither form pays for the other's registers.
 */
export const RAY_MARCH_SLOTS_KERNEL = withMarchSourceTable(RAY_MARCH_SLOTS_PER_STEP_KERNEL);
/**
 * The ray march's first entry point under the tile cull (plan R08, R08.T11.d;
 * `decision-r08-t8-leans.md` §11, Ruling 4): {@link RAY_MARCH_KERNEL}'s march at the centre ray
 * alone, which classes each tile of the air's area, writes (0, 0, 0, 1) where every ray misses the
 * air and marches the tiles no footprint can blend in (`rayMarchCentre.wgsl`), holding none of the
 * footprint across its march.
 */
export const RAY_MARCH_CENTRE_KERNEL = withMarchSourceTable(
  kernel(
    "atmosphere ray march at the centre ray",
    `${farLimbWgsl}\n${marchAreaWgsl}\n${rayMarchWgsl}\n${rayMarchCurvesWgsl}\n${rayMarchCentreWgsl}`,
  ),
);
/** {@link RAY_MARCH_CENTRE_KERNEL} by slot, {@link RAY_MARCH_SLOTS_KERNEL}'s read (R08.T11.d). */
export const RAY_MARCH_SLOTS_CENTRE_KERNEL = withMarchSourceTable(
  kernel(
    "atmosphere ray march by slot at the centre ray",
    `${farLimbWgsl}\n${marchAreaWgsl}\n${rayMarchWgsl}\n${rayMarchSlotsWgsl}\n${rayMarchCentreWgsl}`,
  ),
);
/**
 * The one-band march's two rewrites of the general one's sources (R08.T11.g), each made once and
 * each within a line, so that a checkout's line ends do not matter: `oblateSliceCount`'s and
 * `oblateBandCount`'s bodies (`oblate.wgsl`). The specialised march's first two.
 */
const ONE_BAND_MARCH: ReadonlyArray<readonly [string, string]> = [
  ["return clamp(u32(body.counts.x), 1u, MAX_SLICES);", "return 1u;"],
  ["return clamp(u32(body.counts.y), 1u, MAX_BANDS);", "return 1u;"],
];

/**
 * The specialised march's three rewrites of the general one's sources (R08.T11.d), each made once
 * and each within a line: {@link ONE_BAND_MARCH}'s two, and the march's edges' count
 * (`rayMarch.wgsl`).
 */
const SPECIALISED_MARCH: ReadonlyArray<readonly [string, string]> = [
  ...ONE_BAND_MARCH,
  [
    "let edgeCount = min(u32(layerEdges.count.x), MARCH_EDGES_MAX);",
    "let edgeCount = min(u32(layerEdges.count.x), 0u);",
  ],
];

/**
 * `pair` with each of `rewrites` made once, named `<its name> for <form>`.
 *
 * @throws Error naming the kernel when its sources hold a place to rewrite nowhere, or twice.
 */
function withMarchRewrites(
  pair: KernelPair,
  rewrites: ReadonlyArray<readonly [string, string]>,
  form: string,
): KernelPair {
  const rewrite = (wgsl: string): string =>
    rewrites.reduce((code, [general, special]) => {
      const at = code.indexOf(general);
      if (at < 0 || code.indexOf(general, at + 1) >= 0) {
        throw new Error(`kernel ${pair.name} holds no single place for: ${general}`);
      }
      return code.replace(general, special);
    }, wgsl);
  return {
    ...pair,
    name: `${pair.name} for ${form}`,
    reference: rewrite(pair.reference),
    subgroup: pair.subgroup === null ? null : rewrite(pair.subgroup),
  };
}

/**
 * `pair`, a ray march's, specialised for a body of one slice and one band under a medium with no
 * layer edge (plan R08, R08.T11.d; `decision-r08-approximate-plate.md` item 3): the slices' and the
 * bands' counts constant 1 (`oblate.wgsl`) and the march's edges' count 0 (`rayMarch.wgsl`), the
 * values its uniforms hold for such a body, so that the compiler drops T6.f's sliced and banded
 * code and T6.i's private array. Exact for those bodies; chosen from their tables
 * ({@link marchSpecialisedFor}). The edges' uniform stays read, so the bindings are the general
 * kernel's.
 *
 * @throws Error naming the kernel when its sources hold a place to rewrite nowhere, or twice.
 */
export function withMarchSpecialised(pair: KernelPair): KernelPair {
  return withMarchRewrites(pair, SPECIALISED_MARCH, "one band, edge-free");
}

/**
 * `pair`, a ray march's, for a body of one slice and one band under a medium with layer edges
 * (plan R08, R08.T11.g; main's pre-ruling of 2026-10-28): {@link withMarchSpecialised}'s slices'
 * and bands' counts constant 1 (`oblate.wgsl`), the march's edges left on their uniform, so that
 * the compiler drops T6.f's sliced and banded code and keeps T6.i's cut at the edges. Exact for
 * those bodies, whose uniforms hold 1 and 1; chosen from their tables ({@link marchOneBandFor}).
 * The bindings are the general kernel's.
 *
 * @throws Error naming the kernel when its sources hold a place to rewrite nowhere, or twice.
 */
export function withMarchOneBand(pair: KernelPair): KernelPair {
  return withMarchRewrites(pair, ONE_BAND_MARCH, "one band, edged");
}

/**
 * {@link RAY_MARCH_KERNEL} specialised for one band and no layer edge (R08.T11.d;
 * {@link withMarchSpecialised}): the march of a view's frame over such a body's air.
 */
export const RAY_MARCH_SPECIAL_KERNEL = withMarchSpecialised(RAY_MARCH_KERNEL);

/** {@link RAY_MARCH_SLOTS_KERNEL} specialised likewise (R08.T11.d). */
export const RAY_MARCH_SLOTS_SPECIAL_KERNEL = withMarchSpecialised(RAY_MARCH_SLOTS_KERNEL);

/**
 * {@link RAY_MARCH_KERNEL} for one band, edged (R08.T11.g; {@link withMarchOneBand}): the march of
 * a view's frame over the air of a body of one slice and one band under a medium with layer edges.
 */
export const RAY_MARCH_ONE_BAND_KERNEL = withMarchOneBand(RAY_MARCH_KERNEL);

/**
 * {@link RAY_MARCH_SLOTS_KERNEL} for one band, edged (R08.T11.g): the slab Earth's, whose tables
 * hold the slot layers.
 */
export const RAY_MARCH_SLOTS_ONE_BAND_KERNEL = withMarchOneBand(RAY_MARCH_SLOTS_KERNEL);

/**
 * {@link RAY_MARCH_CENTRE_KERNEL} specialised for one band and no layer edge (plan R08, R08.T8.d;
 * {@link withMarchSpecialised}): the tile cull's centre ray over such a body, so that the cull, a
 * view's {@link ViewMarchCover} `tiles`, marches it as the area's specialised march does.
 */
export const RAY_MARCH_CENTRE_SPECIAL_KERNEL = withMarchSpecialised(RAY_MARCH_CENTRE_KERNEL);
/** {@link RAY_MARCH_SLOTS_CENTRE_KERNEL} specialised likewise (R08.T8.d). */
export const RAY_MARCH_SLOTS_CENTRE_SPECIAL_KERNEL = withMarchSpecialised(
  RAY_MARCH_SLOTS_CENTRE_KERNEL,
);
/** {@link RAY_MARCH_CENTRE_KERNEL} for one band, edged (R08.T8.d; {@link withMarchOneBand}). */
export const RAY_MARCH_CENTRE_ONE_BAND_KERNEL = withMarchOneBand(RAY_MARCH_CENTRE_KERNEL);
/** {@link RAY_MARCH_SLOTS_CENTRE_KERNEL} for one band, edged (R08.T8.d). */
export const RAY_MARCH_SLOTS_CENTRE_ONE_BAND_KERNEL = withMarchOneBand(
  RAY_MARCH_SLOTS_CENTRE_KERNEL,
);

/** The march's form a body's tables take (R08.T11.d, R08.T11.g): the general or a specialised. */
export type MarchForm = "general" | "special" | "one band";

/** Every form of the march, the general first. */
const MARCH_FORMS: ReadonlyArray<MarchForm> = ["general", "special", "one band"];

/** One of the march's two reads of the sources' light (R08.T8.e, lever 5). */
type MarchRead = "shared read" | "by slot";

/** Each read's tile cull's centre ray, by form (R08.T11.d; R08.T8.d's forms). */
const CENTRE_PAIRS: Readonly<Record<MarchRead, Readonly<Record<MarchForm, KernelPair>>>> = {
  "shared read": {
    general: RAY_MARCH_CENTRE_KERNEL,
    special: RAY_MARCH_CENTRE_SPECIAL_KERNEL,
    "one band": RAY_MARCH_CENTRE_ONE_BAND_KERNEL,
  },
  "by slot": {
    general: RAY_MARCH_SLOTS_CENTRE_KERNEL,
    special: RAY_MARCH_SLOTS_CENTRE_SPECIAL_KERNEL,
    "one band": RAY_MARCH_SLOTS_CENTRE_ONE_BAND_KERNEL,
  },
};

/**
 * {@link RAY_MARCH_PACKED_KERNEL}'s march as before R08.T8.e's table, each step's sources per step,
 * as {@link RAY_MARCH_PER_STEP_KERNEL} is the footprint's: the base of the thick read's packed
 * entry point per step (perf-thick-march). No frame dispatches it.
 */
const RAY_MARCH_PACKED_PER_STEP_KERNEL = kernel(
  "atmosphere ray march, its limb packed",
  `${farLimbWgsl}\n${limbListWgsl}\n${marchAreaWgsl}\n${rayMarchWgsl}\n${rayMarchCurvesWgsl}\n${rayMarchPackedWgsl}`,
);
/** {@link RAY_MARCH_PACKED_PER_STEP_KERNEL} by slot. No frame dispatches it. */
const RAY_MARCH_SLOTS_PACKED_PER_STEP_KERNEL = kernel(
  "atmosphere ray march by slot, its limb packed",
  `${farLimbWgsl}\n${limbListWgsl}\n${marchAreaWgsl}\n${rayMarchWgsl}\n${rayMarchSlotsWgsl}\n${rayMarchPackedWgsl}`,
);

/**
 * The ray march's entry point over the far limb's footprint with its node rays packed (plan R08,
 * R08.T8.d; {@link LimbRays} `packed`; `rayMarchPacked.wgsl`): each texel blended in writes its
 * nodes to the limb's lists and marches its centre ray alone, for the node pass
 * ({@link RAY_MARCH_LIMB_KERNEL}) and the resolve ({@link LIMB_RESOLVE_KERNEL}); every other texel
 * as {@link RAY_MARCH_KERNEL} marches it.
 */
export const RAY_MARCH_PACKED_KERNEL = withMarchSourceTable(RAY_MARCH_PACKED_PER_STEP_KERNEL);
/** {@link RAY_MARCH_PACKED_KERNEL} by slot, {@link RAY_MARCH_SLOTS_KERNEL}'s read (R08.T8.d). */
export const RAY_MARCH_SLOTS_PACKED_KERNEL = withMarchSourceTable(
  RAY_MARCH_SLOTS_PACKED_PER_STEP_KERNEL,
);

/**
 * The lines of `rayMarch.wgsl` the limb's node pass leaves out (R08.T8.d): its two targets'
 * declarations and stores, since the pass writes its marches to the limb's list and a kernel binds
 * every binding it declares (`compute.ts`'s kernelBindings). Each stands once in a march's sources.
 */
const WITHOUT_MARCH_TARGETS: ReadonlyArray<readonly [string, string]> = [
  ["@group(0) @binding(8) var rayMarchOut : texture_storage_2d<rgba16float, write>;", ""],
  [
    "@group(0) @binding(19) var rayMarchTransmittanceOut : texture_storage_2d<rgba16float, write>;",
    "",
  ],
  ["  textureStore(rayMarchOut, texel, marched.light);", ""],
  [
    "  textureStore(rayMarchTransmittanceOut, texel, vec4f(marched.transmittance, marched.light.a));",
    "",
  ],
];

/** `pair`, a march's, without its two targets ({@link WITHOUT_MARCH_TARGETS}), its name kept. */
function withoutMarchTargets(pair: KernelPair): KernelPair {
  return { ...withMarchRewrites(pair, WITHOUT_MARCH_TARGETS, "the limb"), name: pair.name };
}

/**
 * {@link RAY_MARCH_LIMB_KERNEL}'s node pass as before R08.T8.e's table, each step's sources per
 * step: the base of the thick read's node pass per step (perf-thick-march). No frame dispatches it.
 */
const RAY_MARCH_LIMB_PER_STEP_KERNEL = withoutMarchTargets(
  kernel(
    "atmosphere ray march's limb rays",
    `${limbListWgsl}\n${rayMarchWgsl}\n${rayMarchCurvesWgsl}\n${limbNodesWgsl}`,
  ),
);
/** {@link RAY_MARCH_LIMB_PER_STEP_KERNEL} by slot. No frame dispatches it. */
const RAY_MARCH_SLOTS_LIMB_PER_STEP_KERNEL = withoutMarchTargets(
  kernel(
    "atmosphere ray march's limb rays by slot",
    `${limbListWgsl}\n${rayMarchWgsl}\n${rayMarchSlotsWgsl}\n${limbNodesWgsl}`,
  ),
);

/**
 * The far limb's node pass (plan R08, R08.T8.d; `limbNodes.wgsl`): each node ray the packed entry
 * point listed, one an invocation, by the one `marchRay` call site, over the shared read.
 */
export const RAY_MARCH_LIMB_KERNEL = withMarchSourceTable(RAY_MARCH_LIMB_PER_STEP_KERNEL);
/** {@link RAY_MARCH_LIMB_KERNEL} by slot (R08.T8.d). */
export const RAY_MARCH_SLOTS_LIMB_KERNEL = withMarchSourceTable(
  RAY_MARCH_SLOTS_LIMB_PER_STEP_KERNEL,
);

/** {@link RAY_MARCH_PACKED_KERNEL} specialised for one band and no layer edge (R08.T8.d). */
export const RAY_MARCH_PACKED_SPECIAL_KERNEL = withMarchSpecialised(RAY_MARCH_PACKED_KERNEL);
/** {@link RAY_MARCH_SLOTS_PACKED_KERNEL} specialised likewise (R08.T8.d). */
export const RAY_MARCH_SLOTS_PACKED_SPECIAL_KERNEL = withMarchSpecialised(
  RAY_MARCH_SLOTS_PACKED_KERNEL,
);
/** {@link RAY_MARCH_PACKED_KERNEL} for one band, edged (R08.T8.d). */
export const RAY_MARCH_PACKED_ONE_BAND_KERNEL = withMarchOneBand(RAY_MARCH_PACKED_KERNEL);
/** {@link RAY_MARCH_SLOTS_PACKED_KERNEL} for one band, edged (R08.T8.d). */
export const RAY_MARCH_SLOTS_PACKED_ONE_BAND_KERNEL = withMarchOneBand(
  RAY_MARCH_SLOTS_PACKED_KERNEL,
);
/** {@link RAY_MARCH_LIMB_KERNEL} specialised for one band and no layer edge (R08.T8.d). */
export const RAY_MARCH_LIMB_SPECIAL_KERNEL = withMarchSpecialised(RAY_MARCH_LIMB_KERNEL);
/** {@link RAY_MARCH_SLOTS_LIMB_KERNEL} specialised likewise (R08.T8.d). */
export const RAY_MARCH_SLOTS_LIMB_SPECIAL_KERNEL = withMarchSpecialised(
  RAY_MARCH_SLOTS_LIMB_KERNEL,
);
/** {@link RAY_MARCH_LIMB_KERNEL} for one band, edged (R08.T8.d). */
export const RAY_MARCH_LIMB_ONE_BAND_KERNEL = withMarchOneBand(RAY_MARCH_LIMB_KERNEL);
/** {@link RAY_MARCH_SLOTS_LIMB_KERNEL} for one band, edged (R08.T8.d). */
export const RAY_MARCH_SLOTS_LIMB_ONE_BAND_KERNEL = withMarchOneBand(RAY_MARCH_SLOTS_LIMB_KERNEL);

/**
 * The far limb's resolve (plan R08, R08.T8.d; `limbResolve.wgsl`): each listed texel's node rays
 * summed in node order and blended with its centre ray into both of the march's targets.
 */
export const LIMB_RESOLVE_KERNEL: KernelPair = {
  name: "atmosphere ray march's limb resolve",
  reference: `${limbListWgsl}\n${limbResolveWgsl}`,
  subgroup: null,
  readback: "presentation-only",
};

/**
 * The far limb's dispatches (plan R08, R08.T8.d; `limbDispatch.wgsl`): the lists' counts held to
 * their room, and the node pass's and the resolve's workgroups, whole numbers.
 */
export const LIMB_DISPATCH_KERNEL: KernelPair = {
  name: "atmosphere ray march's limb dispatch",
  reference: `${limbListWgsl}\n${limbDispatchWgsl}`,
  subgroup: null,
  readback: "bit-exact",
};

/** Each read's packed entry point and node pass, by form (R08.T8.d). */
const LIMB_PAIRS: Readonly<
  Record<
    MarchRead,
    Readonly<Record<MarchForm, { readonly footprint: KernelPair; readonly nodes: KernelPair }>>
  >
> = {
  "shared read": {
    general: { footprint: RAY_MARCH_PACKED_KERNEL, nodes: RAY_MARCH_LIMB_KERNEL },
    special: { footprint: RAY_MARCH_PACKED_SPECIAL_KERNEL, nodes: RAY_MARCH_LIMB_SPECIAL_KERNEL },
    "one band": {
      footprint: RAY_MARCH_PACKED_ONE_BAND_KERNEL,
      nodes: RAY_MARCH_LIMB_ONE_BAND_KERNEL,
    },
  },
  "by slot": {
    general: { footprint: RAY_MARCH_SLOTS_PACKED_KERNEL, nodes: RAY_MARCH_SLOTS_LIMB_KERNEL },
    special: {
      footprint: RAY_MARCH_SLOTS_PACKED_SPECIAL_KERNEL,
      nodes: RAY_MARCH_SLOTS_LIMB_SPECIAL_KERNEL,
    },
    "one band": {
      footprint: RAY_MARCH_SLOTS_PACKED_ONE_BAND_KERNEL,
      nodes: RAY_MARCH_SLOTS_LIMB_ONE_BAND_KERNEL,
    },
  },
};

/** Every packed kernel (R08.T8.d): each read's forms, then the resolve and the dispatches. */
export const LIMB_KERNELS: ReadonlyArray<KernelPair> = [
  ...Object.values(LIMB_PAIRS).flatMap((forms) =>
    Object.values(forms).flatMap(({ footprint, nodes }) => [footprint, nodes]),
  ),
  LIMB_RESOLVE_KERNEL,
  LIMB_DISPATCH_KERNEL,
];

/**
 * The thin read's marches' table-only twins (plan R08, R08.T8.e's table;
 * {@link MarchSourceTransmittance} `table`; `surfaceLighting.ts`' `tableOnlyKernel`): every march a
 * view's frame dispatches with a body of one band's sources' table bound, of both reads, in every
 * form and entry point (the footprint's, the tile cull's centre ray's, the packed limb's footprint
 * and its nodes), with `marchSourceTable.wgsl`'s SOURCE_TABLE_ONLY set, so that the sliced lookup a
 * source a step is compiled out. The app's engine makes each at its first such dispatch
 * (`sourceTableEngine.ts`); a deck or self-luminous body's march is never given a table, and the
 * thick read's marches from the table are table-only themselves ({@link RAY_MARCH_THICK_TABLE_KERNEL}).
 */
export const MARCH_TABLE_ONLY_KERNELS: ReadonlyArray<KernelPair> = [
  RAY_MARCH_KERNEL,
  RAY_MARCH_SLOTS_KERNEL,
  RAY_MARCH_SPECIAL_KERNEL,
  RAY_MARCH_SLOTS_SPECIAL_KERNEL,
  RAY_MARCH_ONE_BAND_KERNEL,
  RAY_MARCH_SLOTS_ONE_BAND_KERNEL,
  ...Object.values(CENTRE_PAIRS).flatMap((forms) => Object.values(forms)),
  ...Object.values(LIMB_PAIRS).flatMap((forms) =>
    Object.values(forms).flatMap(({ footprint, nodes }) => [footprint, nodes]),
  ),
].flatMap((pair) => tableOnlyKernel(pair) ?? []);

/**
 * The thick read's rewrites of a per-frame kernel's sources (plan R08, R08.T14.c): each source's
 * multiple scattering taken by `thickRead.wgsl`'s `thickSourceAt` in place of `source.wgsl`'s
 * `sourceAt`, and the forward lobes' higher orders at the analytic term's share. Each is one line;
 * the second stands once in every kernel (`source.wgsl`'s `sourceInScatteringAt`), the third only
 * in a march by slot (`rayMarchSlots.wgsl`), and the first in each of them. The last, in a ray march
 * alone (fix-r08-decked-air-opacity): a drawn surface keeps its forward lobes' light
 * ({@link SurfaceLobes}) beside the correction alone, never on a bake's table, whose J re-emits the
 * truncated lobe's diffuse field, the ground's light among it, into each direction (`thick/bake.ts`'s
 * E, its first term), so that the lobes' light is the table's there.
 */
const THICK_READ: ReadonlyArray<{
  readonly general: string;
  readonly thick: string;
  readonly least: number;
  readonly most: number;
}> = [
  {
    general: "phased += lobeOrdersOf(local, angle, light.depth);",
    thick: "phased += thickAnalyticWeight(muSun) * lobeOrdersOf(local, angle, light.depth);",
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
  {
    general: "let surfaceLobes = lobes && view.forward.w > 0.5;",
    thick: "let surfaceLobes = lobes && view.forward.w > 0.5 && thickCorrectionOnly();",
    least: 0,
    most: 1,
  },
];

/**
 * The thick table's read a per-frame kernel takes ({@link withThickRead}): `thickRead.wgsl`, the
 * table and a layer's read, then `thickReadBands.wgsl`, the read over the body's bands.
 */
export const THICK_READ_WGSL = `${thickReadWgsl}\n${thickReadBandsWgsl}`;

/**
 * `pair`, a per-frame kernel (the sky view, the aerial perspective or a ray march), reading the
 * thick multiple-scattering table (plan R08, R08.T14.c; Design note 9; `thick/read.ts`): its
 * sources' multiple scattering is {@link THICK_READ_WGSL}'s, the bake's J_ms from the table's lowest sun
 * cosine cell up and Hillaire's Ψ_ms below the rows' edge, −7/32, blended across that cell, and the
 * forward lobes' higher orders take Ψ_ms's share ({@link THICK_READ}). Every other line is the
 * kernel's own, so that the thin kernel stays as it was to the byte and the thick one differs from it
 * only there; the table binds at `thickTable` beside the kernel's own bindings.
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
    return `${code}\n${THICK_READ_WGSL}`;
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
/**
 * {@link RAY_MARCH_KERNEL} reading the thick table (R08.T14.c), each step's sources per step: made
 * from {@link RAY_MARCH_PER_STEP_KERNEL} (R08.T8.e's table). The base of the deck's, the thermal
 * and the row-once forms ({@link RAY_MARCH_THICK_ROW_KERNEL}, which a frame dispatches since
 * perf-thick-march); no frame dispatches it.
 */
export const RAY_MARCH_THICK_KERNEL = withThickRead(RAY_MARCH_PER_STEP_KERNEL);
/** {@link RAY_MARCH_SLOTS_KERNEL} reading the thick table (R08.T14.c), from its march per step. */
export const RAY_MARCH_SLOTS_THICK_KERNEL = withThickRead(RAY_MARCH_SLOTS_PER_STEP_KERNEL);
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
 * The thick read's altitude row once a sample (plan R08, perf-thick-march's step 4;
 * `thickSampleRow.wgsl`): the march's step clears the row held (`rayMarch.wgsl`'s rayMarchStep),
 * and `thickReadBands.wgsl`'s thickMultipleAt takes it from `thickSampleRowAt`, which finds it at
 * the step's first source, in place of a search a source. Each line stands once in a thick march.
 */
const THICK_ROW_ONCE: ReadonlyArray<readonly [string, string]> = [
  [
    "let shares = rayMarchSourcesAt(body, start, datum.normal, ray.dir, lobes, local);",
    [
      "thickSampleRowFound = false;",
      "  let shares = rayMarchSourcesAt(body, start, datum.normal, ray.dir, lobes, local);",
    ].join("\n"),
  ],
  [
    "let row = thickRowAt(max(start.scaledHeightM, 0.0));",
    "let row = thickSampleRowAt(start.scaledHeightM);",
  ],
];

/**
 * `pair`, a thick read's march ({@link withThickRead}), with its altitude row found once a sample and
 * shared by the sample's sources ({@link THICK_ROW_ONCE}), `thickSampleRow.wgsl` appended: exact,
 * since every source of a step reads the row at the step's h*. Its name is kept.
 *
 * @throws Error naming the kernel when its sources hold a rewritten line nowhere, or twice.
 */
function withThickRowOnce(pair: KernelPair): KernelPair {
  const shared = withMarchRewrites(pair, THICK_ROW_ONCE, "the row once a sample");
  return {
    ...shared,
    name: pair.name,
    reference: `${shared.reference}\n${thickSampleRowWgsl}`,
    subgroup: shared.subgroup === null ? null : `${shared.subgroup}\n${thickSampleRowWgsl}`,
  };
}

/**
 * The thick read's march of `pair`, a thin march with the sources' table ({@link
 * withMarchSourceTable}), from the table alone, named `name` (plan R08, perf-thick-march):
 * {@link withThickRead}'s read with its row once a sample ({@link withThickRowOnce}) and
 * `marchSourceTable.wgsl`'s SOURCE_TABLE_ONLY set (`surfaceLighting.ts`' `tableOnlyKernel`), so
 * that every step's sources are the bound table's and the sliced lookup a source a step is compiled
 * out: the march of a thick body of one band over the shared read, whose table holds every source a
 * step takes. The thick read has no form with both ways: SwiftShader's GPU process crashed
 * compiling the shared read's general one (exit 139), so a thick body of several bands marches per
 * step; and its march by slot keeps its lookup, one read a source a step already, which the table
 * did not beat on the pod.
 *
 * @throws Error naming the kernel where `pair` declares no sources' table.
 */
function thickFromTable(pair: KernelPair, name: string): KernelPair {
  const tabled = tableOnlyKernel(withThickRowOnce(withThickRead(pair)));
  if (tabled === null) {
    throw new Error(`kernel ${pair.name} declares no sources' table`);
  }
  return { ...tabled, name };
}

/**
 * {@link RAY_MARCH_THICK_KERNEL} with its altitude row found once a sample (plan R08, R08.T14.c's
 * read; perf-thick-march's step 4; {@link withThickRowOnce}), its name kept: the thick read's march
 * per step a frame dispatches, exact, the base keeping its bytes for the deck and thermal forms.
 */
export const RAY_MARCH_THICK_ROW_KERNEL = withThickRowOnce(RAY_MARCH_THICK_KERNEL);
/** {@link RAY_MARCH_SLOTS_THICK_KERNEL} with its row once a sample likewise. */
export const RAY_MARCH_SLOTS_THICK_ROW_KERNEL = withThickRowOnce(RAY_MARCH_SLOTS_THICK_KERNEL);
/** {@link RAY_MARCH_THICK_SPECIAL_KERNEL} with its row once a sample likewise. */
export const RAY_MARCH_THICK_ROW_SPECIAL_KERNEL = withThickRowOnce(RAY_MARCH_THICK_SPECIAL_KERNEL);
/** {@link RAY_MARCH_SLOTS_THICK_SPECIAL_KERNEL} with its row once a sample likewise. */
export const RAY_MARCH_SLOTS_THICK_ROW_SPECIAL_KERNEL = withThickRowOnce(
  RAY_MARCH_SLOTS_THICK_SPECIAL_KERNEL,
);

/**
 * The thick read's march of `pair`, a thin march per step, with its row once a sample, named
 * `name` (perf-thick-march): {@link RAY_MARCH_THICK_KERNEL}'s arithmetic to the bit.
 */
function thickPerStep(pair: KernelPair, name: string): KernelPair {
  return { ...withThickRowOnce(withThickRead(pair)), name };
}

/**
 * {@link RAY_MARCH_THICK_KERNEL} from the sources' table alone (plan R08, perf-thick-march;
 * {@link MarchSourceTransmittance} `table` on the thick read): made from {@link RAY_MARCH_KERNEL}
 * ({@link thickFromTable}), so that the thick read's march per step keeps its bytes; for a thick
 * body of one band over the shared read.
 */
export const RAY_MARCH_THICK_TABLE_KERNEL = thickFromTable(
  RAY_MARCH_KERNEL,
  "atmosphere ray march, thick, from the sources' table",
);
/** {@link RAY_MARCH_THICK_TABLE_KERNEL} specialised for one band and no layer edge. */
export const RAY_MARCH_THICK_TABLE_SPECIAL_KERNEL = withMarchSpecialised(
  RAY_MARCH_THICK_TABLE_KERNEL,
);

/** The thick read's forms of the march (R08.T14.c): it has no one-band, edged form. */
export type ThickMarchForm = Exclude<MarchForm, "one band">;

/** One read's packed entry point and node pass over the thick read, by form (perf-thick-march). */
type ThickLimbForms = Readonly<
  Record<ThickMarchForm, { readonly footprint: KernelPair; readonly nodes: KernelPair }>
>;

/**
 * The packed entry points and node passes over the thick read (plan R08, perf-thick-march;
 * {@link LimbRays} `packed` on the thick read): per step, each read's, {@link LIMB_PAIRS}' thin forms
 * through {@link thickPerStep}, which march as the thick read's march per texel does, to the bit;
 * and from the table, the shared read's through {@link thickFromTable}, for a thick body of one band
 * over the shared read under {@link MarchSourceTransmittance} `table`.
 */
const THICK_LIMB_PAIRS: {
  readonly "per step": Readonly<Record<MarchRead, ThickLimbForms>>;
  readonly "from the table": ThickLimbForms;
} = (() => {
  const thickFormsOf = (
    footprint: KernelPair,
    nodes: KernelPair,
    made: (pair: KernelPair, name: string) => KernelPair,
    suffix: string,
  ): ThickLimbForms => {
    const general = {
      footprint: made(footprint, `${footprint.name}, thick${suffix}`),
      nodes: made(nodes, `${nodes.name}, thick${suffix}`),
    };
    return {
      general,
      special: {
        footprint: withMarchSpecialised(general.footprint),
        nodes: withMarchSpecialised(general.nodes),
      },
    };
  };
  return {
    "per step": {
      "shared read": thickFormsOf(
        RAY_MARCH_PACKED_PER_STEP_KERNEL,
        RAY_MARCH_LIMB_PER_STEP_KERNEL,
        thickPerStep,
        "",
      ),
      "by slot": thickFormsOf(
        RAY_MARCH_SLOTS_PACKED_PER_STEP_KERNEL,
        RAY_MARCH_SLOTS_LIMB_PER_STEP_KERNEL,
        thickPerStep,
        "",
      ),
    },
    "from the table": thickFormsOf(
      RAY_MARCH_PACKED_KERNEL,
      RAY_MARCH_LIMB_KERNEL,
      thickFromTable,
      ", from the sources' table",
    ),
  };
})();

/**
 * The thick read's packed kernels (perf-thick-march): each read's packed entry point and node pass
 * per step in each form, then the shared read's from the table; the resolve and the dispatches are
 * {@link LIMB_KERNELS}'.
 */
export const THICK_LIMB_KERNELS: ReadonlyArray<KernelPair> = [
  ...Object.values(THICK_LIMB_PAIRS["per step"]),
  THICK_LIMB_PAIRS["from the table"],
].flatMap((forms) => Object.values(forms).flatMap(({ footprint, nodes }) => [footprint, nodes]));

/**
 * The thermal source's rewrites of a per-frame kernel's sources (plan R08, R08.T18.b;
 * `thermalSource.ts`): each sample's medium gains its black body B̃ at its h* times the frame's
 * scale and its terms' absorption, summed as its extinction is (not σ_ext − σ_s, which cancels in
 * `f32` where the air scatters far more than it absorbs), `scattering.wgsl`'s struct, its loop and
 * its one constructor; `source.wgsl`'s step adds slot 0's emission, B̃ times the depth the step
 * absorbs, its terms' and its absorbers' curves',
 * through the step factor it scatters through; and each kernel's step adds the scattered thermal
 * light to slot 0's share (`thermalSharesOf`), the light travelling to the eye at −(ray · zenith),
 * one of the last three a kernel.
 */
const THERMAL_SOURCE: ReadonlyArray<{
  readonly general: string;
  readonly thermal: string;
  readonly least: number;
  readonly most: number;
}> = [
  {
    general: "  pressureColumns : AbsorberColumns,\n  densities : TermDensities,\n}",
    thermal:
      "  pressureColumns : AbsorberColumns,\n  densities : TermDensities,\n  planck : vec3f,\n" +
      "  absorbing : vec3f,\n}",
    least: 1,
    most: 1,
  },
  {
    general: "fn sampleMediumAt(heightM : f32) -> SampleMedium {\n  var scattering = vec3f(0.0);",
    thermal:
      "fn sampleMediumAt(heightM : f32) -> SampleMedium {\n  var scattering = vec3f(0.0);\n" +
      "  var absorbing = vec3f(0.0);",
    least: 1,
    most: 1,
  },
  {
    general:
      "    extinction += (term.scattering.rgb + term.absorption.rgb) * d;\n" +
      "    columns = addColumns(columns, termColumns(term, d), 1.0);",
    thermal:
      "    extinction += (term.scattering.rgb + term.absorption.rgb) * d;\n" +
      "    absorbing += term.absorption.rgb * d;\n" +
      "    columns = addColumns(columns, termColumns(term, d), 1.0);",
    least: 1,
    most: 1,
  },
  {
    general: "return SampleMedium(scattering, extinction, columns, pressureColumns, densities);",
    thermal:
      "return SampleMedium(scattering, extinction, columns, pressureColumns, densities, " +
      "thermalPlanckAt(heightM) * view.marchSky.w, absorbing);",
    least: 1,
    most: 1,
  },
  {
    general: "(*march).luminance += throughput0 * shares.slot0 * dtM * stepFactor(stepDepth0);",
    thermal:
      "(*march).luminance += throughput0 * (shares.slot0 * dtM + local.planck * " +
      "(local.absorbing * dtM + (next0.depth - path0.depth))) * stepFactor(stepDepth0);",
    least: 1,
    most: 1,
  },
  {
    general: "let shares = rayMarchSourcesAt(body, start, datum.normal, ray.dir, lobes, local);",
    thermal:
      "let shares = thermalSharesOf(rayMarchSourcesAt(body, start, datum.normal, ray.dir, lobes, " +
      "local), start, local, -dot(ray.dir, datum.normal));",
    least: 0,
    most: 1,
  },
  {
    general: "let shares = viewSourcesAt(body, start, up, dir, lobes, local);",
    thermal:
      "let shares = thermalSharesOf(viewSourcesAt(body, start, up, dir, lobes, local), start, " +
      "local, -dot(dir, up));",
    least: 0,
    most: 1,
  },
  {
    general: "let shares = skySourcesAt(body, start, up, ray.dir, frame, lobes, local);",
    thermal:
      "let shares = thermalSharesOf(skySourcesAt(body, start, up, ray.dir, frame, lobes, local), " +
      "start, local, -dot(ray.dir, up));",
    least: 0,
    most: 1,
  },
];

/** The last three of {@link THERMAL_SOURCE}: each kernel's step holds one of them. */
const THERMAL_STEP_REWRITES = 3;

/** The binding `thermalSource.wgsl` declares its buffer at. */
const THERMAL_SOURCE_BINDING = "@group(0) @binding(25) var<storage, read> thermalSource";

/**
 * `thermalSource.wgsl` with its buffer at binding 26, for a kernel whose own sources hold binding 25
 * (a deck's march, its table; R08.T15.b.2), so that every other thermal form keeps its bytes.
 */
const THERMAL_SOURCE_OVER_DECK_WGSL = thermalSourceWgsl.replace(
  THERMAL_SOURCE_BINDING,
  "@group(0) @binding(26) var<storage, read> thermalSource",
);

/**
 * The thermal light a kernel's thermal form draws ({@link withThermalSource}): the medium's emission
 * and the scattered thermal light a thick bake holds, or its emission alone (R08.T15.b.2: a deck's
 * march, whose bake holds none).
 */
export type ThermalLight = "emitted and scattered" | "emitted";

/**
 * `pair`, a per-frame kernel (the sky view, the aerial perspective or a ray march, of the thin read
 * or the thick, or a thick march over a deck), drawing the medium's own light (plan R08, R08.T18.b;
 * `thermalSource.ts`): the rewrites of {@link THERMAL_SOURCE} and `thermalSource.wgsl` appended,
 * its buffer at `thermalSource` beside the kernel's own bindings, at binding 25, or 26 where the
 * kernel's own hold 25 (R08.T15.b.2: a deck's march, its table). Every other line is the kernel's
 * own, so that a body below the thermal cut draws by the kernel as it was to the byte.
 *
 * @param light - `emitted` leaves the step's shares as they are, reading no scattered thermal light
 *   (R08.T15.b.2): a deck's bake holds none, and on SwiftShader the deck's march with the read as
 *   well crashed its GPU process as it compiled (`exit_code=139` after some 300 s, pod, 2026-11-02).
 * @throws Error naming the kernel when its sources hold a rewritten line more or fewer times than
 *   {@link THERMAL_SOURCE} allows, or no step's or more than one.
 */
export function withThermalSource(
  pair: KernelPair,
  light: ThermalLight = "emitted and scattered",
): KernelPair {
  const rewrite = (wgsl: string): string => {
    let steps = 0;
    const code = THERMAL_SOURCE.reduce((text, { general, thermal, least, most }, i) => {
      const count = text.split(general).length - 1;
      if (count < least || count > most) {
        throw new Error(
          `kernel ${pair.name} holds ${count} places for: ${general}, not ${least} to ${most}`,
        );
      }
      if (i >= THERMAL_SOURCE.length - THERMAL_STEP_REWRITES) {
        steps += count;
        if (light === "emitted") {
          return text;
        }
      }
      return text.replaceAll(general, thermal);
    }, wgsl);
    if (steps !== 1) {
      throw new Error(`kernel ${pair.name} holds ${steps} steps' shares, not 1`);
    }
    const source = wgsl.includes("@group(0) @binding(25) ")
      ? THERMAL_SOURCE_OVER_DECK_WGSL
      : thermalSourceWgsl;
    return `${code}\n${source}`;
  };
  return {
    ...pair,
    name: `${pair.name}, thermal`,
    reference: rewrite(pair.reference),
    subgroup: pair.subgroup === null ? null : rewrite(pair.subgroup),
  };
}

/**
 * The deck read's rewrites of a thick ray march's bare ground (plan R08, R08.T15.a.2;
 * `thick/deckRead.ts`): each source's light on the bare ground, where the ray meets the kernels'
 * datum before any drawn surface ({@link DECK_GROUND}), is `deckRead.wgsl`'s `deckGroundAt` in place of the Lambertian
 * max(μ₀, 0) A ÷ π, its own transmittance to the ground passed to it for a source the bake did not
 * carry. Each is one line and stands once in each march. Since R08.T16.a.2's shares by the light
 * each piece returns (`rayMarch.wgsl`'s `rayMarchDepthShares`) a segment onto an envelope's floor
 * keeps R05.T12.e's placement, as one onto a surface's deck does.
 */
const DECK_READ: ReadonlyArray<readonly [string, string]> = [
  [
    "luminance += viewThroughputOf(&march, slot) * source.weight.rgb * toSun",
    "luminance += viewThroughputOf(&march, slot) * source.weight.rgb",
  ],
  [
    "* max(muSun, 0.0) * medium.groundAlbedo.rgb / PI_VIEW;",
    "* deckGroundAt(body, s, datum.normal, dir, source.direction.xyz, slot, toSun);",
  ],
];

/**
 * The deck read's rewrite of where a thick ray march meets its bare ground (plan R08, R08.T16.b.2;
 * research-rm4-rm5-close-out §1.3): over a deck, wherever the ray reaches the kernels' datum, the
 * deck's, before any drawn surface, not only where nothing is drawn, so that a disc drawn below the
 * deck's datum (a routed envelope's on its column's base, a surface beneath its deck) never hides
 * the deck the march reads there. It stands twice in each march: the deck's light, and the
 * transmittance to what lies beyond, 0 there. A drawn surface at or beyond the datum's hit takes
 * the bare ground's count already (R08.T11.b), so from above the air the march to it is the one to
 * the bare ground.
 */
const DECK_GROUND: readonly [string, string] = [
  "if (depth == 0.0 && hitsGround && ground.x <= tEnd) {",
  "if (hitsGround && ground.x <= tEnd) {",
];

/**
 * The deck read's rewrite of which texels a thick ray march draws (plan R08, R08.T15.b.2's route;
 * research-leans-33 §4(c)): below the march band, besides the surfaces beyond the
 * aerial-perspective volume's reach, those within it whose ray meets the deck's datum first
 * (`deckRead.wgsl`'s `deckBeforeSurface`), which the composite takes from the march under the deck's
 * route (`composite.wgsl`'s `levers.z`), since the volume reads no deck. It stands once in each
 * march, at its footprint's entry point.
 */
const DECK_NEEDED: readonly [string, string] = [
  "let needed = view.exact.w > 0.0 || (depth > 0.0 && tSurface > reach);",
  "let needed = view.exact.w > 0.0 || (depth > 0.0 && (tSurface > reach || deckBeforeSurface(uv, tSurface)));",
];

/**
 * `pair`, a thick ray march's ({@link withThickRead}), reading a cloud deck as its ground (plan
 * R08, R08.T15.a.2; Design note 9; `thick/deckRead.ts`): the bare ground's light is the deck's
 * ({@link DECK_READ}), read wherever the ray reaches the deck's datum before a drawn surface
 * ({@link DECK_GROUND}, R08.T16.b.2), and such a surface within the volume's reach marched
 * ({@link DECK_NEEDED}), its table bound at `deckTable` beside the thick table. Every other line is
 * the thick kernel's own, so that a body with no deck keeps its kernels to the byte.
 *
 * @throws Error naming the kernel when its sources hold a rewritten line other than once (the bare
 *   ground's test other than twice), or it does not read the thick table.
 */
export function withDeckRead(pair: KernelPair): KernelPair {
  const rewrite = (wgsl: string): string => {
    if (!wgsl.includes("fn thickSourceAt(")) {
      throw new Error(`kernel ${pair.name} reads no thick table, which a deck's march stands on`);
    }
    const replaced = (text: string, [general, deck]: readonly [string, string], places: number) => {
      const count = text.split(general).length - 1;
      if (count !== places) {
        throw new Error(`kernel ${pair.name} holds ${count} places for: ${general}, not ${places}`);
      }
      return text.replaceAll(general, deck);
    };
    const code = [...DECK_READ, DECK_NEEDED].reduce(
      (text, rewritten) => replaced(text, rewritten, 1),
      wgsl,
    );
    return `${replaced(code, DECK_GROUND, 2)}\n${deckReadWgsl}`;
  };
  return {
    ...pair,
    name: `${pair.name}, over a deck`,
    reference: rewrite(pair.reference),
    subgroup: pair.subgroup === null ? null : rewrite(pair.subgroup),
  };
}

/**
 * A kernel beneath a cloud deck (plan R08, R08.T15.b.2; `thick/deckRead.ts`, "Below the deck"):
 * `view/shaders/preciseTrig.wgsl`, `common.wgsl`, `view.wgsl`, `oblate.wgsl`, `belowDeck.wgsl` and
 * `belowDeckView.wgsl`, then the kernel, which reads the table below the deck in place of every
 * table of the upper medium and declares no medium. Each is a kernel of its own beside the thick
 * read's, so that every other kernel keeps its bytes.
 */
function belowDeckKernel(name: string, wgsl: string): KernelPair {
  return {
    name,
    reference: `${preciseTrigWgsl}\n${commonWgsl}\n${viewWgsl}\n${oblateWgsl}\n${belowDeckWgsl}\n${belowDeckViewWgsl}\n${wgsl}`,
    subgroup: null,
    readback: "presentation-only",
  };
}

/** The sky view beneath a cloud deck ({@link belowDeckKernel}; `skyViewBelowDeck.wgsl`). */
export const SKY_VIEW_BELOW_DECK_KERNEL = belowDeckKernel(
  "atmosphere sky view beneath a deck",
  skyViewBelowDeckWgsl,
);
/** The aerial perspective beneath a cloud deck (`aerialPerspectiveBelowDeck.wgsl`). */
export const AERIAL_PERSPECTIVE_BELOW_DECK_KERNEL = belowDeckKernel(
  "atmosphere aerial perspective beneath a deck",
  aerialPerspectiveBelowDeckWgsl,
);
/** The ray march beneath a cloud deck, over the air's area's tiles (`rayMarchBelowDeck.wgsl`). */
export const RAY_MARCH_BELOW_DECK_KERNEL = belowDeckKernel(
  "atmosphere ray march beneath a deck",
  `${marchAreaWgsl}\n${rayMarchBelowDeckWgsl}`,
);

/** {@link SKY_VIEW_KERNEL} drawing the medium's own light ({@link withThermalSource}; R08.T18.b). */
export const SKY_VIEW_THERMAL_KERNEL = withThermalSource(SKY_VIEW_KERNEL);
/** {@link AERIAL_PERSPECTIVE_KERNEL} drawing the medium's own light (R08.T18.b). */
export const AERIAL_PERSPECTIVE_THERMAL_KERNEL = withThermalSource(AERIAL_PERSPECTIVE_KERNEL);
/**
 * {@link RAY_MARCH_KERNEL} drawing the medium's own light (R08.T18.b), each step's sources per step:
 * made from {@link RAY_MARCH_PER_STEP_KERNEL} (R08.T8.e's table; SwiftShader's GPU process hung
 * compiling the thermal march with the table's read as well, pod, 2026-11-04).
 */
export const RAY_MARCH_THERMAL_KERNEL = withThermalSource(RAY_MARCH_PER_STEP_KERNEL);
/** {@link RAY_MARCH_SLOTS_KERNEL} drawing the medium's own light (R08.T18.b), from its march per step. */
export const RAY_MARCH_SLOTS_THERMAL_KERNEL = withThermalSource(RAY_MARCH_SLOTS_PER_STEP_KERNEL);
/** {@link SKY_VIEW_THICK_KERNEL} drawing the medium's own light and its thick bake's (R08.T18.b). */
export const SKY_VIEW_THICK_THERMAL_KERNEL = withThermalSource(SKY_VIEW_THICK_KERNEL);
/** {@link AERIAL_PERSPECTIVE_THICK_KERNEL} likewise (R08.T18.b). */
export const AERIAL_PERSPECTIVE_THICK_THERMAL_KERNEL = withThermalSource(
  AERIAL_PERSPECTIVE_THICK_KERNEL,
);
/** {@link RAY_MARCH_THICK_KERNEL} likewise (R08.T18.b). */
export const RAY_MARCH_THICK_THERMAL_KERNEL = withThermalSource(RAY_MARCH_THICK_KERNEL);
/** {@link RAY_MARCH_SLOTS_THICK_KERNEL} likewise (R08.T18.b). */
export const RAY_MARCH_SLOTS_THICK_THERMAL_KERNEL = withThermalSource(RAY_MARCH_SLOTS_THICK_KERNEL);
/** {@link RAY_MARCH_THICK_KERNEL} over a cloud deck ({@link withDeckRead}; R08.T15.a.2). */
export const RAY_MARCH_DECK_KERNEL = withDeckRead(RAY_MARCH_THICK_KERNEL);
/** {@link RAY_MARCH_SLOTS_THICK_KERNEL} over a cloud deck (R08.T15.a.2). */
export const RAY_MARCH_SLOTS_DECK_KERNEL = withDeckRead(RAY_MARCH_SLOTS_THICK_KERNEL);
/**
 * {@link RAY_MARCH_DECK_KERNEL} drawing the medium's emission ({@link withThermalSource}, `emitted`;
 * R08.T18.b over R08.T15.b.2's deck route), so that a self-luminous body over a deck reads its deck.
 */
export const RAY_MARCH_DECK_THERMAL_KERNEL = withThermalSource(RAY_MARCH_DECK_KERNEL, "emitted");
/** {@link RAY_MARCH_SLOTS_DECK_KERNEL} likewise (R08.T15.b.2). */
export const RAY_MARCH_SLOTS_DECK_THERMAL_KERNEL = withThermalSource(
  RAY_MARCH_SLOTS_DECK_KERNEL,
  "emitted",
);
/** {@link RAY_MARCH_DECK_KERNEL} specialised for one band and no layer edge (R08.T15.a.2). */
export const RAY_MARCH_DECK_SPECIAL_KERNEL = withMarchSpecialised(RAY_MARCH_DECK_KERNEL);
/** {@link RAY_MARCH_SLOTS_DECK_KERNEL} specialised likewise (R08.T15.a.2). */
export const RAY_MARCH_SLOTS_DECK_SPECIAL_KERNEL = withMarchSpecialised(
  RAY_MARCH_SLOTS_DECK_KERNEL,
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
 * Whether a body's tables take the one-band march ({@link withMarchOneBand}; plan R08, R08.T11.g):
 * one slice and one band in their plan, and layer edges in their medium's, from the tables alone,
 * never the body's class.
 */
export function marchOneBandFor(tables: AtmosphereTables): boolean {
  return (
    tables.plan.kappa.length === 1 &&
    tables.plan.bands.length === 1 &&
    marchEdgeCountOf(tables.layerEdges) > 0
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
 *   ({@link RAY_MARCH_SPECIAL_KERNEL}, {@link marchSpecialisedFor}) and for one of one band and
 *   layer edges ({@link RAY_MARCH_ONE_BAND_KERNEL}, {@link marchOneBandFor}; R08.T11.g);
 * - `tiles`: the area under the tile cull (`decision-r08-t8-leans.md` §11, Ruling 4), the centre
 *   ray's entry point ({@link RAY_MARCH_CENTRE_KERNEL}) taking the tiles no far-limb footprint can
 *   blend in and the footprint's the rest, each since R08.T8.d in the area's form, specialised for
 *   one band where the tables fit it; a view's under {@link ViewMarchCover} `tiles`. Before the
 *   specialised forms, on the RTX 3080 it cost 0.4–1.0 ms against the whole frame from 400 km and
 *   10⁷ m, where the area alone gained 0.4–0.6 ms, and gained 0.4–1.3 ms farther (plan R08's
 *   "Deviations in R08.T11.d, as built");
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
 * The composite's `Draw` members after the view's, in its order: the shell, the march's area
 * (R08.T11.d) and the composite's levers (the owner's priority, 2026-11-01).
 */
const COMPOSITE_MEMBERS = [...VIEW_MEMBERS, "shell", "marchArea", "levers"] as const;

/**
 * The composite's WGSL: `view/shaders/preciseTrig.wgsl`, `common.wgsl`, `view.wgsl`, `oblate.wgsl`
 * (R08.T6.f), the volume's read (`aerialRead.wgsl`, R08.T9.a), the far limb's footprint
 * (`farLimb.wgsl`, R08.T8.c), the transmittance to space (`compositeTransmittance.wgsl`), then its
 * own.
 */
const COMPOSITE_WGSL = `${preciseTrigWgsl}\n${commonWgsl}\n${viewWgsl}\n${oblateWgsl}\n${aerialReadWgsl}\n${farLimbWgsl}\n${compositeTransmittanceWgsl}\n${compositeWgsl}`;

/**
 * What lies behind the sky from above the top at the march's resolution (the owner's priority,
 * 2026-11-01; `compositeBehind.wgsl`): the composite's transmittance across from above, light slot
 * 0's, per channel, once a sky texel of the march's area rather than once a pixel, as ln τ, which
 * the composite reads from a pixel's four candidates ({@link CompositeTransmittance}). A frame
 * dispatches it where its
 * {@link AtmosphereSettings.compositeTransmittance} is `march`, the march runs below the output's
 * resolution and the camera is above the top ({@link HillaireAtmosphere.drawFrame}).
 */
export const COMPOSITE_BEHIND_KERNEL: KernelPair = {
  name: "atmosphere behind",
  reference: `${preciseTrigWgsl}\n${commonWgsl}\n${viewWgsl}\n${oblateWgsl}\n${farLimbWgsl}\n${marchAreaWgsl}\n${compositeTransmittanceWgsl}\n${compositeBehindWgsl}`,
  subgroup: null,
  readback: "presentation-only",
};

/** The composite: a full-screen draw of pre-exposed luminance over the terrain target. */
export const COMPOSITE_MATERIAL: WgslMaterialSpec = {
  name: "atmosphere composite",
  displayName: "ATMOSPHERE",
  vertexWgsl: COMPOSITE_WGSL,
  fragmentWgsl: COMPOSITE_WGSL,
  uniforms: COMPOSITE_MEMBERS.map((name) => ({ name, type: "vec4f" as const })),
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
    // What lies behind the sky from above at the march's resolution (the owner's priority).
    { name: "behindMarch", binding: 12 },
    // The transmittance per channel beside the volume and the march (R08.T9.c).
    { name: "aerialTransmittance", binding: 13, viewDimension: "3d" },
    { name: "rayMarchTransmittance", binding: 14 },
  ],
  cullMode: "none",
  depthWrite: false,
  colourWrites: true,
  blend: "none",
};

/**
 * A views check's probe of the composite's cost (the owner's priority, 2026-11-01): `sky-off`, every
 * sky pixel keeping the scene's light; `transmittance-off`, the sky drawn with no transmittance
 * across from above (`composite.wgsl`'s `transmittanceAcrossFromAbove`, 1 in its place);
 * `per-pixel`, that transmittance formed per pixel whatever the setting's
 * {@link AtmosphereSettings.compositeTransmittance}, the way back's composite, the frame forming its
 * target at the march's resolution all the same, so that one build's profiles tell the composite's
 * saving apart from that pass's cost; or `surface-grey`, every drawn surface dimmed by the channels'
 * mean whatever {@link AtmosphereSettings.surfaceTransmittance} (R08.T9.c), the grey way back's
 * composite, the frame storing the transmittance per channel all the same. It is check-only: an
 * inspection profile's `--composite-probe`, never drawn on a player's launch.
 */
export type CompositeProbe = "sky-off" | "transmittance-off" | "per-pixel" | "surface-grey";

/** The line of `composite.wgsl` a probe's material replaces. */
const COMPOSITE_PROBE_LINE = "const COMPOSITE_PROBE : u32 = 0u;";

/**
 * {@link COMPOSITE_MATERIAL} under `probe`: the same draw with `composite.wgsl`'s `COMPOSITE_PROBE`
 * set, 1 for `sky-off`, 2 for `transmittance-off`, 3 for `per-pixel` and 4 for `surface-grey`.
 *
 * @throws Error if the composite's WGSL no longer declares the probe's constant.
 */
export function compositeProbedMaterial(probe: CompositeProbe): WgslMaterialSpec {
  let value: number;
  switch (probe) {
    case "sky-off":
      value = 1;
      break;
    case "transmittance-off":
      value = 2;
      break;
    case "per-pixel":
      value = 3;
      break;
    case "surface-grey":
      value = 4;
      break;
  }
  if (!COMPOSITE_WGSL.includes(COMPOSITE_PROBE_LINE)) {
    throw new Error(`the composite's WGSL declares no ${COMPOSITE_PROBE_LINE}`);
  }
  const wgsl = COMPOSITE_WGSL.replace(
    COMPOSITE_PROBE_LINE,
    `const COMPOSITE_PROBE : u32 = ${String(value)}u;`,
  );
  return { ...COMPOSITE_MATERIAL, vertexWgsl: wgsl, fragmentWgsl: wgsl };
}

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
  /**
   * The centre ray's in a form (R08.T8.d), made at its first culled frame in it where the kernels
   * are made synchronously or were not made ahead ({@link FrameKernelsAhead.tiles}).
   */
  readonly centre: (form: MarchForm) => ComputeHandle;
  readonly footprint: ComputeHandle;
  /**
   * The footprint's entry point specialised for one band and no layer edge (R08.T11.d,
   * {@link withMarchSpecialised}), made up front with the footprint's by
   * {@link createAtmosphereFrameKernels} and at its first frame over the area where the kernels
   * are made synchronously.
   */
  readonly special: () => ComputeHandle;
  /**
   * The footprint's entry point for one band, edged (R08.T11.g, {@link withMarchOneBand}), made as
   * {@link MarchKernels.special} is.
   */
  readonly oneBand: () => ComputeHandle;
}

/**
 * A read's packed entry point and node pass in one form, with the resolve and the dispatches
 * (R08.T8.d).
 */
interface LimbPair {
  readonly footprint: ComputeHandle;
  readonly nodes: ComputeHandle;
  readonly resolve: ComputeHandle;
  readonly dispatch: ComputeHandle;
}

/**
 * The march whose far limb's node rays a frame packs (plan R08, R08.T8.d; perf-thick-march): the
 * thin read's ({@link LIMB_PAIRS}), or the thick read's but over a deck, per step or, over the shared
 * read, from the sources' table ({@link THICK_LIMB_PAIRS}).
 */
type LimbMarch = "thin" | "thick" | "thick from the table";

/**
 * `march`'s packed entry point and node pass of `read` in `form`, or `null` where it has none: the
 * thick read has no one-band, edged form, and none by slot from the table.
 */
function limbPairsOf(
  march: LimbMarch,
  read: MarchRead,
  form: MarchForm,
): { readonly footprint: KernelPair; readonly nodes: KernelPair } | null {
  let pairs: { readonly footprint: KernelPair; readonly nodes: KernelPair } | null;
  switch (march) {
    case "thin":
      pairs = LIMB_PAIRS[read][form];
      break;
    case "thick":
      pairs = form === "one band" ? null : THICK_LIMB_PAIRS["per step"][read][form];
      break;
    case "thick from the table":
      pairs =
        form === "one band" || read === "by slot" ? null : THICK_LIMB_PAIRS["from the table"][form];
      break;
  }
  return pairs;
}

/** The far limb's packed kernels (R08.T8.d; {@link LIMB_KERNELS}, {@link THICK_LIMB_KERNELS}). */
interface LimbKernels {
  /**
   * Each march's packed kernels of each read in `form`, or `null` while they are made, or where the
   * march has none in that form: at the first ask, at once or in the background
   * ({@link limbKernelsOver}'s `later`), a frame marching its limb per texel, which gives the same
   * bits, until they are made, and for good where their making failed.
   */
  readonly pair: (march: LimbMarch, read: MarchRead, form: MarchForm) => LimbPair | null;
}

/**
 * The far limb's packed kernels on `engine` (R08.T8.d): the resolve, the dispatches and, of `reads`,
 * each form of `forms` made up front, asynchronously, the thin read's, and the thick read's both ways
 * in each of `thickForms` (perf-thick-march); every other made at its first ask, in the background.
 */
async function limbKernelsOf(
  engine: RenderEngine,
  reads: MarchReads,
  forms: ReadonlyArray<MarchForm>,
  thickForms: ReadonlyArray<ThickMarchForm>,
): Promise<LimbKernels> {
  const asked = (["shared read", "by slot"] as const).filter(
    (read) => reads === "both" || reads === read,
  );
  const keys: Array<readonly [LimbMarch, MarchRead, MarchForm]> = [];
  for (const read of asked) {
    for (const form of forms) {
      keys.push(["thin", read, form]);
    }
    for (const form of thickForms) {
      keys.push(["thick", read, form]);
      if (read === "shared read") {
        keys.push(["thick from the table", read, form]);
      }
    }
  }
  const [pairs, resolve, dispatch] = await Promise.all([
    Promise.all(
      keys.map(async ([march, read, form]) => {
        const kernels = limbPairsOf(march, read, form);
        if (kernels === null) {
          throw new Error(`the ${march} read has no packed kernels in the ${form} form`);
        }
        const [footprint, nodes] = await Promise.all([
          engine.createComputeAsync(kernels.footprint),
          engine.createComputeAsync(kernels.nodes),
        ]);
        return [`${march}/${read}/${form}`, { footprint, nodes }] as const;
      }),
    ),
    engine.createComputeAsync(LIMB_RESOLVE_KERNEL),
    engine.createComputeAsync(LIMB_DISPATCH_KERNEL),
  ]);
  const made = new Map(pairs.map(([key, pair]) => [key, { ...pair, resolve, dispatch }] as const));
  return limbKernelsOver(engine, true, made, { resolve, dispatch });
}

/**
 * The far limb's packed kernels on `engine` over those `made`, each other read and form made at its
 * first ask: at once where not `later` (an atmosphere's own kernels), else in the background, by
 * `createComputeAsync`, so that no frame waits on a compile (the device's shared kernels, a view's;
 * on SwiftShader a synchronous compile beside the march's was ended as hung, R08.T11.d).
 */
function limbKernelsOver(
  engine: RenderEngine,
  later: boolean,
  made = new Map<string, LimbPair>(),
  shared: Pick<LimbPair, "resolve" | "dispatch"> | null = null,
): LimbKernels {
  let common = shared;
  let commonMaking: Promise<Pick<LimbPair, "resolve" | "dispatch">> | null = null;
  const commonLater = async (): Promise<Pick<LimbPair, "resolve" | "dispatch">> => {
    const [resolve, dispatch] = await Promise.all([
      engine.createComputeAsync(LIMB_RESOLVE_KERNEL),
      engine.createComputeAsync(LIMB_DISPATCH_KERNEL),
    ]);
    common = { resolve, dispatch };
    return common;
  };
  const asked = new Set<string>();
  return {
    pair: (march, read, form) => {
      const key = `${march}/${read}/${form}`;
      const held = made.get(key);
      if (held !== undefined) {
        return held;
      }
      const pairs = limbPairsOf(march, read, form);
      if (pairs === null) {
        return null;
      }
      if (!later) {
        common ??= {
          resolve: engine.createCompute(LIMB_RESOLVE_KERNEL),
          dispatch: engine.createCompute(LIMB_DISPATCH_KERNEL),
        };
        const pair: LimbPair = {
          footprint: engine.createCompute(pairs.footprint),
          nodes: engine.createCompute(pairs.nodes),
          ...common,
        };
        made.set(key, pair);
        return pair;
      }
      if (!asked.has(key)) {
        asked.add(key);
        const making = async (): Promise<void> => {
          const [footprint, nodes, both] = await Promise.all([
            engine.createComputeAsync(pairs.footprint),
            engine.createComputeAsync(pairs.nodes),
            common === null ? (commonMaking ??= commonLater()) : Promise.resolve(common),
          ]);
          made.set(key, { footprint, nodes, ...both });
        };
        void making().catch((cause: unknown) => {
          console.warn(
            `the far limb's packed kernels (${march}, ${read}, ${form}) could not be made; its frames march per texel:`,
            cause,
          );
        });
      }
      return null;
    },
  };
}

/** The per-frame kernels, each with its output, as one frame dispatches them. */
interface FrameKernels {
  readonly skyView: ComputeHandle;
  readonly aerial: ComputeHandle;
  /**
   * The march over the shared read ({@link RAY_MARCH_CENTRE_KERNEL}, {@link RAY_MARCH_KERNEL},
   * {@link RAY_MARCH_SPECIAL_KERNEL}, {@link RAY_MARCH_ONE_BAND_KERNEL}):
   * an atmosphere's own kernels make it at the first frame of tables that take it (R08.T11.d), so
   * that a body with slot layers never compiles the pair; the device's shared kernels
   * ({@link createAtmosphereFrameKernels}) make every kernel up front, off the frame path.
   */
  readonly rayMarch: () => MarchKernels;
  /**
   * The ray march by slot ({@link RAY_MARCH_SLOTS_CENTRE_KERNEL}, {@link RAY_MARCH_SLOTS_KERNEL},
   * {@link RAY_MARCH_SLOTS_SPECIAL_KERNEL}, {@link RAY_MARCH_SLOTS_ONE_BAND_KERNEL}), made at the
   * first frame of tables that hold the slot layers (R08.T8.e).
   */
  readonly rayMarchSlots: () => MarchKernels;
  /**
   * The fill of the march's target outside the air's area ({@link RAY_MARCH_FILL_KERNEL}), made at
   * the first reader's ask (R08.T11.d).
   */
  readonly rayMarchFill: () => ComputeHandle;
  /**
   * The far limb's packed kernels ({@link LimbRays} `packed`; R08.T8.d): made at the first frame
   * that takes them where they were not made ahead ({@link FrameKernelsAhead.packed}).
   */
  readonly limb: () => LimbKernels;
  /**
   * The kernels that read the thick table ({@link withThickRead}; plan R08, R08.T14.c), for a body
   * whose tables hold one ({@link AtmosphereTables.thickTable}), a thick bake or a polarisation
   * correction alone (R08.T14.f): made at the first frame of such tables where they were not made
   * ahead ({@link FrameKernelsAhead.thick}), so that a view of bodies with neither compiles none of
   * them.
   */
  readonly thick: () => ThickKernels;
  /**
   * The kernels' thermal forms ({@link withThermalSource}; plan R08, R08.T18.b), of `read`, for a
   * body whose tables hold a thermal source ({@link AtmosphereTables.thermalSource}): made at the
   * first frame of such tables where they were not made ahead ({@link FrameKernelsAhead.thermal}),
   * as the thick kernels are, so that a view of bodies under the thermal cut compiles none of them.
   */
  readonly thermal: (read: ThermalRead) => ThermalKernels;
  /**
   * What lies behind the sky from above at the march's resolution ({@link COMPOSITE_BEHIND_KERNEL};
   * the owner's priority, 2026-11-01): made at the first frame that takes it, where it was not made
   * ahead ({@link FrameKernelsAhead.behind}).
   */
  readonly behind: () => ComputeHandle;
}

/**
 * The per-frame kernels' thermal forms (R08.T18.b; {@link FrameKernels.thermal}): the sky view, the
 * aerial perspective and each read's march, its footprint's entry point alone, which a
 * self-luminous body takes under every cover (no specialised march and no tile cull).
 */
interface ThermalKernels {
  readonly skyView: ComputeHandle;
  readonly aerial: ComputeHandle;
  readonly rayMarch: () => ComputeHandle;
  readonly rayMarchSlots: () => ComputeHandle;
}

/**
 * The read whose thermal forms a self-luminous body's frame takes (R08.T18.b): the thin, the thick,
 * or the thick over a cloud deck, whose marches read the deck at their ground (R08.T15.b.2).
 */
type ThermalRead = "thin" | "thick" | "deck";

/** The thermal forms of one read's kernels. */
interface ThermalPairs {
  readonly skyView: KernelPair;
  readonly aerial: KernelPair;
  readonly rayMarch: KernelPair;
  readonly rayMarchSlots: KernelPair;
}

/**
 * Each read's thermal forms: over a deck the thick read's sky view and aerial perspective, which read
 * no deck, beside the deck's marches (R08.T15.b.2).
 */
const THERMAL_PAIRS: Readonly<Record<ThermalRead, ThermalPairs>> = {
  thin: {
    skyView: SKY_VIEW_THERMAL_KERNEL,
    aerial: AERIAL_PERSPECTIVE_THERMAL_KERNEL,
    rayMarch: RAY_MARCH_THERMAL_KERNEL,
    rayMarchSlots: RAY_MARCH_SLOTS_THERMAL_KERNEL,
  },
  thick: {
    skyView: SKY_VIEW_THICK_THERMAL_KERNEL,
    aerial: AERIAL_PERSPECTIVE_THICK_THERMAL_KERNEL,
    rayMarch: RAY_MARCH_THICK_THERMAL_KERNEL,
    rayMarchSlots: RAY_MARCH_SLOTS_THICK_THERMAL_KERNEL,
  },
  deck: {
    skyView: SKY_VIEW_THICK_THERMAL_KERNEL,
    aerial: AERIAL_PERSPECTIVE_THICK_THERMAL_KERNEL,
    rayMarch: RAY_MARCH_DECK_THERMAL_KERNEL,
    rayMarchSlots: RAY_MARCH_SLOTS_DECK_THERMAL_KERNEL,
  },
};

/**
 * The thermal forms made synchronously on `engine` at their first ask, each pair through `as`, each
 * march's at the first frame that takes it.
 */
function thermalKernelsOf(
  engine: RenderEngine,
  as: (pair: KernelPair) => KernelPair,
): (read: ThermalRead) => ThermalKernels {
  const made = new Map<ThermalRead, ThermalKernels>();
  return (read) => {
    const held = made.get(read);
    if (held !== undefined) {
      return held;
    }
    const pairs = THERMAL_PAIRS[read];
    let shared: ComputeHandle | null = null;
    let slots: ComputeHandle | null = null;
    const kernels: ThermalKernels = {
      skyView: engine.createCompute(as(pairs.skyView)),
      aerial: engine.createCompute(as(pairs.aerial)),
      rayMarch: () => (shared ??= engine.createCompute(as(pairs.rayMarch))),
      rayMarchSlots: () => (slots ??= engine.createCompute(as(pairs.rayMarchSlots))),
    };
    made.set(read, kernels);
    return kernels;
  };
}

/**
 * The thermal forms made up front by `createComputeAsync` ({@link FrameKernelsAhead.thermal}): the
 * thin read's and the thick read's, each march as `reads` asks; a read not asked is made at its
 * first frame, and the marches over a deck at theirs, beside the thick read's sky view and aerial
 * perspective (R08.T15.b.2).
 */
async function thermalKernelsAhead(
  engine: RenderEngine,
  reads: MarchReads,
): Promise<(read: ThermalRead) => ThermalKernels> {
  const ahead = async (pairs: ThermalPairs): Promise<ThermalKernels> => {
    const [skyView, aerial, shared, slots] = await Promise.all([
      engine.createComputeAsync(pairs.skyView),
      engine.createComputeAsync(pairs.aerial),
      reads === "by slot" ? Promise.resolve(null) : engine.createComputeAsync(pairs.rayMarch),
      reads === "shared read"
        ? Promise.resolve(null)
        : engine.createComputeAsync(pairs.rayMarchSlots),
    ]);
    let rayMarch = shared;
    let rayMarchSlots = slots;
    return {
      skyView,
      aerial,
      rayMarch: () => (rayMarch ??= engine.createCompute(pairs.rayMarch)),
      rayMarchSlots: () => (rayMarchSlots ??= engine.createCompute(pairs.rayMarchSlots)),
    };
  };
  const [thin, thick] = await Promise.all([ahead(THERMAL_PAIRS.thin), ahead(THERMAL_PAIRS.thick)]);
  let deckShared: ComputeHandle | null = null;
  let deckSlots: ComputeHandle | null = null;
  const deck: ThermalKernels = {
    skyView: thick.skyView,
    aerial: thick.aerial,
    rayMarch: () => (deckShared ??= engine.createCompute(THERMAL_PAIRS.deck.rayMarch)),
    rayMarchSlots: () => (deckSlots ??= engine.createCompute(THERMAL_PAIRS.deck.rayMarchSlots)),
  };
  return (read) => {
    let kernels: ThermalKernels;
    switch (read) {
      case "thin":
        kernels = thin;
        break;
      case "thick":
        kernels = thick;
        break;
      case "deck":
        kernels = deck;
        break;
    }
    return kernels;
  };
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
  /**
   * The march over the shared read ({@link RAY_MARCH_THICK_ROW_KERNEL}: {@link RAY_MARCH_THICK_KERNEL}
   * with its row once a sample).
   */
  readonly rayMarch: () => ThickMarchKernels;
  /** The march by slot ({@link RAY_MARCH_SLOTS_THICK_ROW_KERNEL}). */
  readonly rayMarchSlots: () => ThickMarchKernels;
  /**
   * The march over the shared read from the sources' table alone ({@link RAY_MARCH_THICK_TABLE_KERNEL};
   * perf-thick-march), for a body of one band over the shared read in a frame whose lever takes the
   * table ({@link MarchSourceTransmittance}).
   */
  readonly tabledRayMarch: () => ThickMarchKernels;
  /**
   * The march over a cloud deck ({@link RAY_MARCH_DECK_KERNEL}; R08.T15.a.2), for tables that hold
   * a deck table, made at its first frame.
   */
  readonly deckRayMarch: () => ThickMarchKernels;
  /** The march by slot over a cloud deck ({@link RAY_MARCH_SLOTS_DECK_KERNEL}). */
  readonly deckRayMarchSlots: () => ThickMarchKernels;
  /**
   * The kernels beneath a cloud deck over a surface (R08.T15.b.2; {@link belowDeckKernel}), for a
   * camera under the deck of tables that hold a table below it, made at its first frame.
   */
  readonly belowDeck: () => BelowDeckKernels;
}

/** The per-frame kernels beneath a cloud deck (R08.T15.b.2; {@link ThickKernels.belowDeck}). */
interface BelowDeckKernels {
  readonly skyView: ComputeHandle;
  readonly aerial: ComputeHandle;
  readonly rayMarch: ComputeHandle;
}

/** The kernels beneath a cloud deck, made synchronously on `engine` at their first ask. */
function belowDeckKernelsOf(
  engine: RenderEngine,
  as: (pair: KernelPair) => KernelPair,
): () => BelowDeckKernels {
  let made: BelowDeckKernels | null = null;
  return () =>
    (made ??= {
      skyView: engine.createCompute(as(SKY_VIEW_BELOW_DECK_KERNEL)),
      aerial: engine.createCompute(as(AERIAL_PERSPECTIVE_BELOW_DECK_KERNEL)),
      rayMarch: engine.createCompute(as(RAY_MARCH_BELOW_DECK_KERNEL)),
    });
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
      let tabledShared: ThickMarchKernels | null = null;
      let deckShared: ThickMarchKernels | null = null;
      let deckSlots: ThickMarchKernels | null = null;
      return {
        skyView: engine.createCompute(as(SKY_VIEW_THICK_KERNEL)),
        aerial: engine.createCompute(as(AERIAL_PERSPECTIVE_THICK_KERNEL)),
        rayMarch: () =>
          (shared ??= marchOf(RAY_MARCH_THICK_ROW_KERNEL, RAY_MARCH_THICK_ROW_SPECIAL_KERNEL)),
        rayMarchSlots: () =>
          (slots ??= marchOf(
            RAY_MARCH_SLOTS_THICK_ROW_KERNEL,
            RAY_MARCH_SLOTS_THICK_ROW_SPECIAL_KERNEL,
          )),
        tabledRayMarch: () =>
          (tabledShared ??= marchOf(
            RAY_MARCH_THICK_TABLE_KERNEL,
            RAY_MARCH_THICK_TABLE_SPECIAL_KERNEL,
          )),
        deckRayMarch: () =>
          (deckShared ??= marchOf(RAY_MARCH_DECK_KERNEL, RAY_MARCH_DECK_SPECIAL_KERNEL)),
        deckRayMarchSlots: () =>
          (deckSlots ??= marchOf(RAY_MARCH_SLOTS_DECK_KERNEL, RAY_MARCH_SLOTS_DECK_SPECIAL_KERNEL)),
        belowDeck: belowDeckKernelsOf(engine, as),
      };
    })());
}

/**
 * The thick kernels made up front by `createComputeAsync` (R08.T14.c; {@link FrameKernelsAhead.thick}):
 * the sky view, the aerial perspective and each read's march with its specialised form, as `reads`
 * asks, and where `tabled` its marches from the sources' table too (perf-thick-march); a read or a
 * way not asked is made at its first frame.
 */
async function thickKernelsAhead(
  engine: RenderEngine,
  reads: MarchReads,
  tabled: boolean,
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
  const [skyView, aerial, rayMarch, rayMarchSlots, tabledRayMarch] = await Promise.all([
    engine.createComputeAsync(SKY_VIEW_THICK_KERNEL),
    engine.createComputeAsync(AERIAL_PERSPECTIVE_THICK_KERNEL),
    marchOf(reads !== "by slot", RAY_MARCH_THICK_ROW_KERNEL, RAY_MARCH_THICK_ROW_SPECIAL_KERNEL),
    marchOf(
      reads !== "shared read",
      RAY_MARCH_SLOTS_THICK_ROW_KERNEL,
      RAY_MARCH_SLOTS_THICK_ROW_SPECIAL_KERNEL,
    ),
    marchOf(
      tabled && reads !== "by slot",
      RAY_MARCH_THICK_TABLE_KERNEL,
      RAY_MARCH_THICK_TABLE_SPECIAL_KERNEL,
    ),
  ]);
  // The deck's marches are made at their first frame: no view routes a deck before R08.T15.b.2.
  const [deckRayMarch, deckRayMarchSlots] = await Promise.all([
    marchOf(false, RAY_MARCH_DECK_KERNEL, RAY_MARCH_DECK_SPECIAL_KERNEL),
    marchOf(false, RAY_MARCH_SLOTS_DECK_KERNEL, RAY_MARCH_SLOTS_DECK_SPECIAL_KERNEL),
  ]);
  const kernels: ThickKernels = {
    skyView,
    aerial,
    rayMarch,
    rayMarchSlots,
    tabledRayMarch,
    deckRayMarch,
    deckRayMarchSlots,
    // The kernels beneath a deck are made at their first frame, as the deck's marches are.
    belowDeck: belowDeckKernelsOf(engine, (pair) => pair),
  };
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
   * Whether the tile cull's centre ray's kernels are made too (R08.T11.d's `tiles` cover), in every
   * form (`true`) or in those listed (R08.T8.d): for a view whose setting takes
   * {@link ViewMarchCover} `tiles`, and the smoke's check of that cover. Not where absent; then each
   * made at the first `tiles` frame in its form.
   */
  readonly tiles?: boolean | ReadonlyArray<MarchForm>;
  /**
   * Whether the kernels that read the thick table are made too (plan R08, R08.T14.c): for a caller
   * that draws a body with one, as the smoke's frame timing with the thick read forced does, and a
   * view once the polarisation correction is routed (R08.T14.f, `thick/routing.ts`'s
   * `CORRECTION_ROUTED`), since every world with air then draws by them; `with the table`, with the
   * thick read's marches from the sources' table too (perf-thick-march;
   * {@link RAY_MARCH_THICK_TABLE_KERNEL}), for a view whose setting takes
   * {@link MarchSourceTransmittance} `table`. Not where absent; then made at the first frame of
   * tables that hold one.
   */
  readonly thick?: boolean | "with the table";
  /**
   * Whether the kernels' thermal forms are made too (plan R08, R08.T18.b): for a caller that draws a
   * self-luminous body, as the smoke's thermal group does. Not where absent; then made at the first
   * frame of tables that hold a thermal source.
   */
  readonly thermal?: boolean;
  /**
   * Whether the kernel that forms what lies behind the sky from above at the march's resolution is
   * made too ({@link COMPOSITE_BEHIND_KERNEL}; the owner's priority, 2026-11-01): for a view, whose
   * atmospheres take its setting's `AtmosphereSettings`. Not where absent; then made at the first
   * frame that takes it.
   */
  readonly behind?: boolean;
  /**
   * Whether the far limb's packed kernels are made too ({@link LIMB_KERNELS}; R08.T8.d), of the
   * reads asked, in every form (`true`) or in those listed: for a view whose setting takes
   * {@link LimbRays} `packed`, and the smoke's check of it. Not where absent; then each made at the
   * first frame that takes it.
   */
  readonly packed?: boolean | ReadonlyArray<MarchForm>;
  /**
   * Whether the thick read's packed kernels are made too ({@link THICK_LIMB_KERNELS};
   * perf-thick-march), of the reads asked, both ways, in its every form (`true`) or in those listed:
   * for the smoke's check of them. Not where absent; then each made at the first frame that takes
   * it.
   */
  readonly thickPacked?: boolean | ReadonlyArray<ThickMarchForm>;
}

/** The forms a {@link FrameKernelsAhead} member asks for: every one for `true`, none where absent. */
function formsOf(asked: boolean | ReadonlyArray<MarchForm> | undefined): ReadonlyArray<MarchForm> {
  if (asked === undefined || asked === false) {
    return [];
  }
  return asked === true ? MARCH_FORMS : asked;
}

/**
 * Makes a device's {@link AtmosphereFrameKernels}: each kernel by `createComputeAsync` and the
 * composite's pipeline into an `rgba16float` target, as a photorealistic frame's atmosphere targets
 * are, by `createMaterialAsync`, so that no frame waits on a compile (plan R08, R08.T10.b). Each
 * read's march is made with its footprint's entry point three times, the general, the one
 * specialised for one band and no layer edge (R08.T11.d, {@link RAY_MARCH_SPECIAL_KERNEL}) and the
 * one for one band, edged (R08.T11.g, {@link RAY_MARCH_ONE_BAND_KERNEL}).
 *
 * @throws Error, as a rejection, as the engine's async creations reject, `EngineUnavailable`
 *   among them during a loss.
 */
export async function createAtmosphereFrameKernels(
  engine: RenderEngine,
  ahead: FrameKernelsAhead = {},
): Promise<AtmosphereFrameKernels> {
  const reads = ahead.reads ?? "both";
  const tiles = formsOf(ahead.tiles);
  const triangle = compositeTriangle(engine);
  const pairOf = async (
    made: boolean,
    read: MarchRead,
    footprintPair: KernelPair,
    specialPair: KernelPair,
    oneBandPair: KernelPair,
  ): Promise<() => MarchKernels> => {
    const centres = new Map<MarchForm, ComputeHandle>();
    const centre = (form: MarchForm): ComputeHandle => {
      let cull = centres.get(form);
      if (cull === undefined) {
        cull = engine.createCompute(CENTRE_PAIRS[read][form]);
        centres.set(form, cull);
      }
      return cull;
    };
    if (!made) {
      let late: MarchKernels | null = null;
      return () =>
        (late ??= (() => {
          let special: ComputeHandle | null = null;
          let oneBand: ComputeHandle | null = null;
          return {
            centre,
            footprint: engine.createCompute(footprintPair),
            special: () => (special ??= engine.createCompute(specialPair)),
            oneBand: () => (oneBand ??= engine.createCompute(oneBandPair)),
          };
        })());
    }
    const [footprint, special, oneBand, ...culled] = await Promise.all([
      engine.createComputeAsync(footprintPair),
      engine.createComputeAsync(specialPair),
      engine.createComputeAsync(oneBandPair),
      ...tiles.map((form) => engine.createComputeAsync(CENTRE_PAIRS[read][form])),
    ]);
    for (const [i, cull] of culled.entries()) {
      const form = tiles[i];
      if (form !== undefined) {
        centres.set(form, cull);
      }
    }
    const kernels: MarchKernels = {
      centre,
      footprint,
      special: () => special,
      oneBand: () => oneBand,
    };
    return () => kernels;
  };
  const [skyView, aerial, rayMarch, rayMarchSlots, composite] = await Promise.all([
    engine.createComputeAsync(SKY_VIEW_KERNEL),
    engine.createComputeAsync(AERIAL_PERSPECTIVE_KERNEL),
    pairOf(
      reads !== "by slot",
      "shared read",
      RAY_MARCH_KERNEL,
      RAY_MARCH_SPECIAL_KERNEL,
      RAY_MARCH_ONE_BAND_KERNEL,
    ),
    pairOf(
      reads !== "shared read",
      "by slot",
      RAY_MARCH_SLOTS_KERNEL,
      RAY_MARCH_SLOTS_SPECIAL_KERNEL,
      RAY_MARCH_SLOTS_ONE_BAND_KERNEL,
    ),
    engine.createMaterialAsync(COMPOSITE_MATERIAL, ["rgba16float"], [triangle]),
  ]);
  const thick =
    ahead.thick === undefined || ahead.thick === false
      ? thickKernelsOf(engine, (pair) => pair)
      : await thickKernelsAhead(engine, reads, ahead.thick === "with the table");
  const thermal =
    ahead.thermal === true
      ? await thermalKernelsAhead(engine, reads)
      : thermalKernelsOf(engine, (pair) => pair);
  let behind: ComputeHandle | null =
    ahead.behind === true ? await engine.createComputeAsync(COMPOSITE_BEHIND_KERNEL) : null;
  const packedForms = formsOf(ahead.packed);
  const thickPackedForms = formsOf(ahead.thickPacked).filter(
    (form): form is ThickMarchForm => form !== "one band",
  );
  let limb: LimbKernels | null =
    packedForms.length > 0 || thickPackedForms.length > 0
      ? await limbKernelsOf(engine, reads, packedForms, thickPackedForms)
      : null;
  // The fill is a reader's (the smoke's), which a view's frame never asks: made at the first ask.
  let fill: ComputeHandle | null = null;
  return {
    skyView,
    aerial,
    rayMarch,
    rayMarchSlots,
    rayMarchFill: () => (fill ??= engine.createCompute(RAY_MARCH_FILL_KERNEL)),
    limb: () => (limb ??= limbKernelsOver(engine, true)),
    thick,
    thermal,
    behind: () => (behind ??= engine.createCompute(COMPOSITE_BEHIND_KERNEL)),
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

/**
 * The per-frame tables a frame writes: the sky view, the aerial-perspective volume, the march, and
 * beside the volume and the march their transmittance per channel (R08.T9.c), rgb with the mean in
 * a.
 */
export interface FrameOutputs {
  readonly skyView: TextureHandle;
  readonly aerial: TextureHandle;
  readonly rayMarch: TextureHandle;
  readonly aerialTransmittance: TextureHandle;
  readonly rayMarchTransmittance: TextureHandle;
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
  /**
   * The volume's transmittance per channel (plan R08, R08.T9.c), `rgba16float`, 3D, beside
   * {@link AerialPerspective.volume} froxel for froxel: each channel's transmittance from the camera
   * to the slice's far face (rgb), and their mean (a). Read through `aerialRead.wgsl`'s
   * `aerialTransmittanceAt(volume, sampler, uv, distanceM, reachM)`, so that a pass dims its colour
   * per channel as the composite does under `AtmosphereSettings`' `channels`.
   */
  readonly transmittanceVolume: TextureHandle;
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
    horizonAngleRad: atan2(bottomRadiusM, Math.sqrt(h * (2 * bottomRadiusM + h))),
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
    const cosTilt = Math.sqrt(cos2);
    across = (d.y * n.x - d.x * n.y) / cosTilt;
    along = (d.z * cos2 - n.z * (d.x * n.x + d.y * n.y)) / cosTilt;
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
  /** The volume's transmittance per channel (R08.T9.c). */
  readonly aerialTransmittance: TextureHandle;
  readonly kernels: FrameKernels;
  readonly composite: MaterialHandle;
  readonly triangle: MeshHandle;
  rayMarch: TextureHandle | null;
  /** The march's transmittance per channel (R08.T9.c), made and grown with its target. */
  rayMarchTransmittance: TextureHandle | null;
  rayMarchSize: ViewSize | null;
  /**
   * What lies behind the sky from above at the march's resolution (the owner's priority,
   * 2026-11-01), the ray march's size and grown with it, made at the first frame that forms it.
   */
  behindMarch: TextureHandle | null;
  behindMarchSize: ViewSize | null;
  /**
   * The tile cull's kinds, one `u32` a tile of the air's area (R08.T11.d), grown with the area's
   * largest and never made smaller; the kernel arm's frames take it too.
   */
  marchTiles: BufferHandle | null;
  marchTilesBytes: number;
  /** The far limb's lists (R08.T8.d), made at the first packed frame and grown, never shrunk. */
  limb: LimbLists | null;
  /** The lists' room the counts read back last asked for, texels and node rays (R08.T8.d). */
  limbWanted: { texels: number; rays: number };
  /** Whether a read-back of the lists' counts is in flight, and the packed frames since the last. */
  limbReading: boolean;
  limbFramesSinceRead: number;
}

/**
 * The far limb's lists on the GPU (plan R08, R08.T8.d; `limbList.wgsl`): their counts, the node
 * pass's and the resolve's indirect workgroups, the texel entries and the node rays, with the room
 * each list was made with.
 */
interface LimbLists {
  readonly counts: BufferHandle;
  readonly dispatch: BufferHandle;
  readonly texels: BufferHandle;
  readonly rays: BufferHandle;
  readonly texelRoom: number;
  readonly rayRoom: number;
}

/** Releases `lists`' buffers, where there are any. */
function releaseLimbLists(engine: RenderEngine, lists: LimbLists | null): void {
  if (lists !== null) {
    for (const buffer of [lists.counts, lists.dispatch, lists.texels, lists.rays]) {
      engine.releaseBuffer(buffer);
    }
  }
}

/** A limb texel's entry and a node ray, bytes (`limbList.wgsl`'s LimbTexel and LimbRay). */
const LIMB_TEXEL_BYTES = 48;
const LIMB_RAY_BYTES = 48;

/**
 * The lists' least room, texels, and the room a frame's area asks for: texels, `LIMB_ROOM_PER_EDGE`
 * of them for each texel of the area's width and height, against its limb's length, at most the
 * area's perimeter, and a band of the limb a few texels deep (from 22 Mm on the floor's low sizes
 * the inspection's 2,528 limb texels in an area of 32 × 31 tiles, 2 to 3 a texel of its edges;
 * from 400 km on high the smoke's limb past 1,024 in 20 × 8); node rays, `LIMB_RAYS_PER_TEXEL` a
 * texel (8.9 the mean there, 10 the most). A frame past either marches the texels that find no room
 * per texel, exactly; the counts, read back every `LIMB_READ_FRAMES` packed frames, grow the lists
 * to `LIMB_ROOM_GROWTH` times the counts for the frames after.
 */
const LIMB_LEAST_TEXELS = 2048;
const LIMB_ROOM_PER_EDGE = 6;
const LIMB_RAYS_PER_TEXEL = 8;
const LIMB_READ_FRAMES = 30;
const LIMB_ROOM_GROWTH = 1.25;

/** Where the node pass's and the resolve's indirect workgroups stand in the dispatch buffer, bytes. */
const LIMB_NODES_ARGS_OFFSET_BYTES = 0;
const LIMB_RESOLVE_ARGS_OFFSET_BYTES = 12;

/**
 * The node pass's and the resolve's workgroup, invocations (`limbList.wgsl`'s LIMB_WORKGROUP), and
 * the most workgroups a dispatch takes along x (WebGPU's `maxComputeWorkgroupsPerDimension`'s
 * default, which every adapter meets).
 */
const LIMB_WORKGROUP = 64;
const LIMB_MOST_WORKGROUPS = 65_535;

/** The lists' counts cleared before a frame's packed entry point (`LimbCounts.reserved`). */
const LIMB_COUNTS_CLEARED = new Uint32Array(2);

/** The kernel arm's per-frame GPU objects (R08.T6.g), made at its first frame. */
interface KernelArmResources {
  readonly kernels: FrameKernels;
  readonly skyView: TextureHandle;
  readonly aerial: TextureHandle;
  readonly aerialTransmittance: TextureHandle;
  rayMarch: TextureHandle | null;
  rayMarchTransmittance: TextureHandle | null;
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
    aerialTransmittance: frameTexture(
      engine,
      "atmosphere aerial transmittance",
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
    rayMarchTransmittance: null,
    rayMarchSize: null,
    behindMarch: null,
    behindMarchSize: null,
    marchTiles: null,
    marchTilesBytes: 0,
    limb: null,
    limbWanted: { texels: 0, rays: 0 },
    limbReading: false,
    limbFramesSinceRead: 0,
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
  let behind: ComputeHandle | null = null;
  let limb: LimbKernels | null = null;
  const pairOf = (
    read: MarchRead,
    footprint: KernelPair,
    specialPair: KernelPair,
    oneBandPair: KernelPair,
  ): MarchKernels => {
    const centres = new Map<MarchForm, ComputeHandle>();
    let special: ComputeHandle | null = null;
    let oneBand: ComputeHandle | null = null;
    return {
      centre: (form) => {
        let made = centres.get(form);
        if (made === undefined) {
          made = engine.createCompute(as(CENTRE_PAIRS[read][form]));
          centres.set(form, made);
        }
        return made;
      },
      footprint: engine.createCompute(as(footprint)),
      special: () => (special ??= engine.createCompute(as(specialPair))),
      oneBand: () => (oneBand ??= engine.createCompute(as(oneBandPair))),
    };
  };
  return {
    skyView: engine.createCompute(as(SKY_VIEW_KERNEL)),
    aerial: engine.createCompute(as(AERIAL_PERSPECTIVE_KERNEL)),
    rayMarch: () =>
      (shared ??= pairOf(
        "shared read",
        RAY_MARCH_KERNEL,
        RAY_MARCH_SPECIAL_KERNEL,
        RAY_MARCH_ONE_BAND_KERNEL,
      )),
    rayMarchSlots: () =>
      (slots ??= pairOf(
        "by slot",
        RAY_MARCH_SLOTS_KERNEL,
        RAY_MARCH_SLOTS_SPECIAL_KERNEL,
        RAY_MARCH_SLOTS_ONE_BAND_KERNEL,
      )),
    rayMarchFill: () => (fill ??= engine.createCompute(as(RAY_MARCH_FILL_KERNEL))),
    // The kernel arm marches the whole frame, per texel: the packed kernels are the frame's alone.
    limb: () => (limb ??= limbKernelsOver(engine, false)),
    thick: thickKernelsOf(engine, as),
    thermal: thermalKernelsOf(engine, as),
    behind: () => (behind ??= engine.createCompute(as(COMPOSITE_BEHIND_KERNEL))),
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
    aerialTransmittance: frameTexture(
      engine,
      "atmosphere aerial transmittance (kernel arm)",
      [
        sizes.aerialPerspective.widthTexels,
        sizes.aerialPerspective.heightTexels,
        sizes.aerialPerspective.slices,
      ],
      KERNEL_ARM_FORMAT,
    ),
    rayMarch: null,
    rayMarchTransmittance: null,
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
 * The share of the air's top height by which the camera's own, as the composite forms it in `f32`,
 * must pass it before {@link compositeAreaOf} holds the camera above the top: over that product's
 * rounding, 2⁻²⁴ of it.
 */
const ABOVE_TOP_MARGIN = 1e-6;

/**
 * The pixels of a frame its composite draws by its full path (R08.T16.a.2's composite over the
 * air's area), from the composite's `Draw` as the frame filled it: a rectangle that holds every
 * pixel `composite.wgsl` does not return early at ({@link marchAreaPixels}), or `null` where any
 * may be one, the whole frame among them.
 *
 * @remarks
 * Outside the rectangle the composite writes each pixel the scene's light, `colour × 1 + 0 ×
 * factor` clamped to the half float's largest, its alpha the scene's: from above the air's top
 * (`camera.w × curvature.z`, by {@link ABOVE_TOP_MARGIN}), from the march band's upper edge up
 * (`exact.w` 1), with no source's disc drawn by the composite (each with no radius or no light),
 * outside the march's area. A frame that meets the three draws the rectangle alone over a copy of
 * the scene (`FrameSubmission.colourFrom`), the same bits wherever the scene's light is finite and
 * not −0.
 */
export function compositeAreaOf(
  uniforms: Readonly<Record<string, Float32Array>>,
): TexelRect | null {
  const member = (name: string): Float32Array => {
    const values = uniforms[name];
    if (values === undefined) {
      throw new Error(`the composite's Draw has no ${name}`);
    }
    return values;
  };
  const camera = member("camera");
  const curvature = member("curvature");
  const topM = member("shell")[0] ?? 0;
  const aboveTop =
    (camera[3] ?? 0) * (curvature[2] ?? 0) >= topM * (1 + ABOVE_TOP_MARGIN) && topM > 0;
  if (!aboveTop || (member("exact")[3] ?? 0) < 1) {
    return null;
  }
  for (let i = 0; i < MAX_SKY_SOURCES; i += 1) {
    const sinRadius = member(`sourceDirection${i}`)[3] ?? 0;
    const disc = member(`sourceDisc${i}`);
    if (sinRadius > 0 && (disc[0] !== 0 || disc[1] !== 0 || disc[2] !== 0)) {
      return null;
    }
  }
  const [widthPx = 0, heightPx = 0, scale = 1] = member("output");
  const [x0 = 0, y0 = 0, x1 = 0, y1 = 0] = member("marchArea");
  const area = marchAreaPixels({ x0, y0, x1, y1 }, scale, widthPx, heightPx);
  return area.width === widthPx && area.height === heightPx ? null : area;
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
   * How the march covers its target (R08.T11.d): over the device's shared kernels its levers' cover
   * (R08.T8.d), the area or the tile cull; over its own the whole frame, until asked
   * ({@link HillaireAtmosphere.setMarchCover}).
   */
  #cover: MarchCover;
  /**
   * The levers it draws with (the owner's priority, 2026-11-01): its view's setting's
   * ({@link HillaireAtmosphere.over}), or {@link PER_PIXEL_LEVERS}, as drawn before them.
   */
  readonly #levers: AtmosphereSettings;
  /**
   * Whether the latest frame formed what lies behind its sky at the march's resolution (the
   * owner's priority), the composite's `levers.x`.
   */
  #behindFormed = false;
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
   * @param levers - The frame-cost levers (the owner's priority, 2026-11-01): a view's setting's
   *   `SETTINGS[s].atmosphereLevers`, or the composite as drawn before them.
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
    levers: AtmosphereSettings = PER_PIXEL_LEVERS,
  ) {
    // The cap is checked where the sizes are given, not first at a frame (R08.T8.e).
    marchSourceIndices([], tables.marchSourceCap);
    this.#engine = engine;
    this.#sizes = tables;
    this.#levers = levers;
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
    // A view's atmosphere, over the device's shared kernels, marches the air's area alone, by its
    // levers' cover (R08.T8.d); one with its own kernels (the smoke's other checks, the descent
    // spike) the whole frame as before R08.T11.d, unless asked, so that their frames are those they
    // were checked by.
    this.#cover = this.#shared === null ? "whole frame" : levers.marchCover;
    this.#uniforms = Object.fromEntries(
      COMPOSITE_MEMBERS.map((name) => [name, new Float32Array(4)]),
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
   * ({@link tablesFigureOf}), its kernels the device's shared ones, and its levers its view's
   * setting's, `SETTINGS[s].atmosphereLevers` (the owner's priority, 2026-11-01), or as drawn
   * before them where not given.
   *
   * @throws RangeError for a march source cap that is not an integer of at least 1.
   */
  static over(
    engine: RenderEngine,
    tables: AtmosphereTables,
    sizes: TableSizes,
    kernels: AtmosphereFrameKernels | null,
    levers: AtmosphereSettings = PER_PIXEL_LEVERS,
  ): HillaireAtmosphere {
    return new HillaireAtmosphere(
      engine,
      { kind: "held", tables, kernels },
      sizes,
      tablesFigureOf(tables),
      tables.curves,
      tables.body?.gravity ?? null,
      levers,
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
    /** The volume's transmittance per channel (R08.T9.c), rgb with the mean in a. */
    readonly aerialTransmittance: TextureHandle;
    /**
     * The march's transmittance per channel (R08.T9.c), rgb with the mean in a, (1, 1, 1, 1) where
     * the first target reads (0, 0, 0, 1); `null` before the first frame.
     */
    readonly rayMarchTransmittance: TextureHandle | null;
    /**
     * The tile cull's kinds the latest frame wrote, one `u32` a tile of its area, row by row
     * (`marchArea.wgsl`'s `MARCH_TILE_*`; R08.T11.d), `null` before the first frame that marched.
     */
    readonly marchTiles: BufferHandle | null;
    /** The latest frame's march area (R08.T11.d), `null` before the first frame. */
    readonly marchArea: MarchArea | null;
    /**
     * The far limb's counts (R08.T8.d; `limbList.wgsl`'s LimbCounts, four `u32`): the texels and
     * node rays the packed entry point reserved, then those taken; `null` before the first packed
     * frame.
     */
    readonly limbCounts: BufferHandle | null;
  } {
    this.#read("skyView");
    this.#read("aerial");
    this.#read("rayMarch");
    return {
      skyView: this.#frame.skyView,
      aerial: this.#frame.aerial,
      rayMarch: this.#frame.rayMarch,
      aerialTransmittance: this.#frame.aerialTransmittance,
      rayMarchTransmittance: this.#frame.rayMarchTransmittance,
      marchTiles: this.#frame.marchTiles,
      marchArea: this.#area,
      limbCounts: this.#frame.limb?.counts ?? null,
    };
  }

  /**
   * The pixels the latest {@link HillaireAtmosphere.drawFrame}'s composite draws by its full path,
   * as its `Draw` now holds them ({@link compositeAreaOf}; R08.T16.a.2): `null` where any may be.
   */
  get compositeArea(): TexelRect | null {
    return compositeAreaOf(this.#uniforms);
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
   * band and no layer edge where the tables fit it ({@link marchSpecialisedFor}), and by the one-band
   * march where they hold one band and layer edges and no thick table ({@link marchOneBandFor};
   * R08.T11.g). Over its own kernels (the smoke's other checks, the descent spike, the kernel arm)
   * it marches the whole frame by the general march as before, unless asked
   * ({@link HillaireAtmosphere.setMarchCover}); a view's takes its levers' cover, the area or the
   * tile cull, in the area's form (R08.T8.d), and its levers' limb rays. Every
   * cover is exact, each texel what the general march over the whole frame gives: the smoke's
   * `checkMarchArea`, `checkMarchSpecialised` and `checkMarchOneBand` hold the area's, specialised
   * or for one band, and the tile cull's to the whole frame's, byte for byte on SwiftShader, and so
   * bind them.
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
    const { rayMarch, rayMarchTransmittance } = this.#rayMarchTextures(view.viewport);
    const { inputs, curvature } = this.#dispatchFrame(
      frame.kernels,
      {
        skyView: frame.skyView,
        aerial: frame.aerial,
        rayMarch,
        aerialTransmittance: frame.aerialTransmittance,
        rayMarchTransmittance,
      },
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
        // The march's own target where the frame formed none, which the composite then never reads.
        behindMarch: this.#behindFormed ? (frame.behindMarch ?? rayMarch) : rayMarch,
        aerialTransmittance: frame.aerialTransmittance,
        rayMarchTransmittance,
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
   * @returns The arm's `rgba32float` targets, the three tables and the volume's and the march's
   *   transmittance per channel (R08.T9.c), for a tolerance read-back.
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
      arm.rayMarchTransmittance === null ||
      size === null ||
      size.widthPx !== view.viewport.widthPx ||
      size.heightPx !== view.viewport.heightPx
    ) {
      for (const texture of [arm.rayMarch, arm.rayMarchTransmittance]) {
        if (texture !== null) {
          engine.releaseTexture(texture);
        }
      }
      const scale = this.#sizes.rayMarchScale;
      const marchSize: [number, number] = [
        Math.max(1, Math.ceil(view.viewport.widthPx * scale)),
        Math.max(1, Math.ceil(view.viewport.heightPx * scale)),
      ];
      arm.rayMarch = frameTexture(
        engine,
        "atmosphere ray march (kernel arm)",
        marchSize,
        KERNEL_ARM_FORMAT,
      );
      arm.rayMarchTransmittance = frameTexture(
        engine,
        "atmosphere ray march's transmittance (kernel arm)",
        marchSize,
        KERNEL_ARM_FORMAT,
      );
      arm.rayMarchSize = view.viewport;
    }
    const outputs: FrameOutputs = {
      skyView: arm.skyView,
      aerial: arm.aerial,
      rayMarch: arm.rayMarch,
      aerialTransmittance: arm.aerialTransmittance,
      rayMarchTransmittance: arm.rayMarchTransmittance,
    };
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
    for (const texture of [
      frame.skyView,
      frame.aerial,
      frame.aerialTransmittance,
      frame.rayMarch,
      frame.rayMarchTransmittance,
      frame.behindMarch,
    ]) {
      if (texture !== null) {
        this.#engine.releaseTexture(texture);
      }
    }
    frame.rayMarch = null;
    frame.rayMarchTransmittance = null;
    frame.rayMarchSize = null;
    frame.behindMarch = null;
    frame.behindMarchSize = null;
    if (frame.marchTiles !== null) {
      this.#engine.releaseBuffer(frame.marchTiles);
    }
    frame.marchTiles = null;
    frame.marchTilesBytes = 0;
    releaseLimbLists(this.#engine, frame.limb);
    frame.limb = null;
    const arm = this.#arm;
    if (arm !== null) {
      for (const texture of [
        arm.skyView,
        arm.aerial,
        arm.aerialTransmittance,
        arm.rayMarch,
        arm.rayMarchTransmittance,
      ]) {
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
        transmittanceVolume: frame.aerialTransmittance,
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
                horizonAngleRad: atan2(radiusM, Math.sqrt(heightM * (2 * radiusM + heightM))),
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
    // A body whose tables hold a deck table marches by the kernels that read it at the ground
    // (R08.T15.a.2), beside the thick table that holds its J above the deck.
    const deckTable = tables.deckTable;
    // A self-luminous body draws by the kernels' thermal forms of its read (R08.T18.b), whose marches
    // over a deck read it (R08.T15.b.2), and makes none of the others.
    const thermalSource = tables.thermalSource;
    let read: ThermalRead = "thin";
    if (thickTable !== null) {
      read = deckTable === null ? "thick" : "deck";
    }
    const thermal = thermalSource === null ? null : kernels.thermal(read);
    const thick = thickTable === null || thermal !== null ? null : kernels.thick();
    // A camera beneath a deck over a surface, under the tables' datum, the deck's, draws by the
    // kernels that read the table below it in place of every other (R08.T15.b.2).
    const belowDeckTable = tables.belowDeckTable;
    const beneath =
      belowDeckTable !== null && inputs.heightM < 0 ? kernels.thick().belowDeck() : null;
    const belowSampled =
      belowDeckTable === null ? null : { oblateBody: tables.oblateBody, belowDeckTable };
    const common = {
      buffers: { terms: tables.terms, ...(thermalSource === null ? {} : { thermalSource }) },
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
      if (beneath !== null && belowSampled !== null) {
        engine.dispatch(
          beneath.skyView,
          {
            buffers: {},
            sampled: belowSampled,
            uniforms: { view: viewUniforms },
            storage: { skyViewOut: { texture: outputs.skyView, level: 0 } },
          },
          groups8(sizes.skyView.widthTexels, sizes.skyView.heightTexels),
          ATMOSPHERE_VIEW_PASS,
        );
        return;
      }
      engine.dispatch(
        thermal?.skyView ?? thick?.skyView ?? kernels.skyView,
        {
          ...common,
          uniforms: { medium: perFrameMedium, view: viewUniforms, layerEdges: tables.layerEdges },
          storage: { skyViewOut: { texture: outputs.skyView, level: 0 } },
        },
        groups8(sizes.skyView.widthTexels, sizes.skyView.heightTexels),
        ATMOSPHERE_VIEW_PASS,
      );
    };
    // The volume and its transmittance per channel (R08.T9.c), written together.
    const aerialStorage = {
      aerialOut: { texture: outputs.aerial, level: 0 },
      aerialTransmittanceOut: { texture: outputs.aerialTransmittance, level: 0 },
    };
    // The march's target and its transmittance per channel (R08.T9.c), written together.
    const marchStorage = {
      rayMarchOut: { texture: outputs.rayMarch, level: 0 },
      rayMarchTransmittanceOut: { texture: outputs.rayMarchTransmittance, level: 0 },
    };
    const aerial = (): void => {
      if (beneath !== null && belowSampled !== null) {
        engine.dispatch(
          beneath.aerial,
          {
            buffers: {},
            sampled: belowSampled,
            uniforms: { view: viewUniforms },
            storage: aerialStorage,
          },
          groups8(sizes.aerialPerspective.widthTexels, sizes.aerialPerspective.heightTexels),
          ATMOSPHERE_VIEW_PASS,
        );
        return;
      }
      engine.dispatch(
        thermal?.aerial ?? thick?.aerial ?? kernels.aerial,
        {
          ...common,
          uniforms: { medium: perFrameMedium, view: viewUniforms },
          storage: aerialStorage,
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
    // Nor does a self-luminous body's (R08.T18.b), which takes its read's thermal footprint alone.
    const tileCull = this.#cover === "tiles" && thick === null && thermal === null;
    // The march's form on every cover but the whole frame: specialised for one band where the
    // tables fit it (R08.T11.d, R08.T11.g); the thick read has no one-band form.
    let form: MarchForm = "general";
    if (this.#cover !== "whole frame" && marchSpecialisedFor(tables)) {
      form = "special";
    } else if (this.#cover !== "whole frame" && thick === null && marchOneBandFor(tables)) {
      form = "one band";
    }
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
    // Each step's sources from the sources' table (R08.T8.e's table, `MarchSourceTransmittance`):
    // the thin read's march under its lever's `table`, a body of one band's by the march's
    // table-only twins where the engine has them (`sourceTableEngine.ts`), and through the table's
    // one-texel stub per step under `per-step`; since perf-thick-march the thick read's march but
    // over a deck under `table` over the shared read on a body of one band, by its marches from the
    // table alone (`thickFromTable`), and per step by slot or on a body of several bands. The deck
    // and thermal marches declare no table.
    const thickLevers = thick !== null && deckTable === null;
    const thickTabled =
      thickLevers &&
      this.#levers.marchSourceTransmittance === "table" &&
      slotDepths === null &&
      tables.plan.bands.length === 1;
    let sourceTable: TextureHandle | null = null;
    if (thick === null && thermal === null) {
      sourceTable =
        this.#levers.marchSourceTransmittance === "table"
          ? tables.sourceTransmittance()
          : tables.perFragmentSourceTransmittance;
    } else if (thickTabled) {
      sourceTable = tables.sourceTransmittance();
    }
    const sourceTableBound = sourceTable === null ? {} : { atmosphere_source_table: sourceTable };
    // The march's kernels: the thin read's, with the tile cull's centre ray, or the thick read's,
    // from the sources' table where its lever takes it.
    const march = (): FrameMarch =>
      thick === null
        ? {
            kind: "thin",
            kernels: slotDepths === null ? kernels.rayMarch() : kernels.rayMarchSlots(),
          }
        : {
            kind: "thick",
            kernels:
              deckTable === null
                ? slotDepths === null
                  ? thickTabled
                    ? thick.tabledRayMarch()
                    : thick.rayMarch()
                  : thick.rayMarchSlots()
                : slotDepths === null
                  ? thick.deckRayMarch()
                  : thick.deckRayMarchSlots(),
          };
    if (area.tilesAcross > 0 && area.tilesDown > 0 && beneath !== null && belowSampled !== null) {
      // Beneath a deck, its march: the surfaces beyond the volume's reach (R08.T15.b.2).
      engine.dispatch(
        beneath.rayMarch,
        {
          buffers: {},
          sampled: { ...belowSampled, sceneDepth: scene.depth },
          uniforms: { view: viewUniforms, marchArea: areaUniform },
          storage: marchStorage,
        },
        [area.tilesAcross, area.tilesDown, 1],
        ATMOSPHERE_VIEW_PASS,
      );
    } else if (area.tilesAcross > 0 && area.tilesDown > 0) {
      const bindings = {
        buffers: { ...common.buffers, marchTiles: this.#marchTiles(area) },
        sampled: {
          ...common.sampled,
          sceneDepth: scene.depth,
          ...sourceTableBound,
          ...(slotDepths === null ? {} : { slotDepths }),
          ...(deckTable === null ? {} : { deckTable }),
        },
        uniforms: {
          medium: perFrameMedium,
          view: marchUniforms,
          layerEdges: tables.layerEdges,
          marchArea: areaUniform,
        },
        storage: marchStorage,
      };
      const tiles = [area.tilesAcross, area.tilesDown, 1] as const;
      const marched = thermal === null ? march() : null;
      if (tileCull && marched?.kind === "thin") {
        engine.dispatch(marched.kernels.centre(form), bindings, tiles, ATMOSPHERE_VIEW_PASS);
      }
      // A view's area over a body of one band with no layer edge takes the specialised march
      // (R08.T11.d), and over a thin one of one band with layer edges the one-band march
      // (R08.T11.g), since R08.T8.d under the tile cull too: each exact there, and the general one
      // on the reference path. A self-luminous body takes its read's thermal footprint under every
      // cover (R08.T18.b).
      let footprint: ComputeHandle;
      if (thermal !== null) {
        footprint = slotDepths === null ? thermal.rayMarch() : thermal.rayMarchSlots();
      } else if (marched === null) {
        throw new Error("a frame without the thermal forms marches by its read's kernels");
      } else if (form === "special") {
        footprint = marched.kernels.special();
      } else if (form === "one band" && marched.kind === "thin") {
        footprint = marched.kernels.oneBand();
      } else {
        footprint = marched.kernels.footprint;
      }
      // The far limb's node rays packed (R08.T8.d, `LimbRays`): the frame's thin march over an
      // area from above the top, where alone a footprint blends, by its packed entry point in the
      // march's form, then the lists' dispatches, the node pass and the resolve, each texel's bits
      // the per-texel march's; per texel while the packed kernels are made. Where the camera's f32
      // h* stands on the top, either march gives the same bits. Since perf-thick-march the thick
      // read's march but over a deck likewise, per step or from the sources' table as it marches.
      const cameraAt = this.#uniforms["camera"];
      const curvatureAt = this.#uniforms["curvature"];
      let limbMarch: LimbMarch | null = null;
      if (marched?.kind === "thin") {
        limbMarch = "thin";
      } else if (marched?.kind === "thick" && thickLevers) {
        limbMarch = thickTabled ? "thick from the table" : "thick";
      }
      const pair =
        deferUnread &&
        limbMarch !== null &&
        this.#cover !== "whole frame" &&
        this.#levers.limbRays === "packed" &&
        (cameraAt?.[3] ?? 0) * (curvatureAt?.[2] ?? 0) >= medium.topHeightM
          ? kernels.limb().pair(limbMarch, slotDepths === null ? "shared read" : "by slot", form)
          : null;
      if (pair !== null) {
        const lists = this.#limbLists(area);
        const listed = {
          limbCounts: lists.counts,
          limbTexels: lists.texels,
          limbRays: lists.rays,
        };
        engine.writeBuffer(lists.counts, 0, LIMB_COUNTS_CLEARED);
        engine.dispatch(
          pair.footprint,
          { ...bindings, buffers: { ...bindings.buffers, ...listed } },
          tiles,
          ATMOSPHERE_VIEW_PASS,
        );
        engine.dispatch(
          pair.dispatch,
          {
            uniforms: {},
            buffers: { ...listed, limbDispatch: lists.dispatch },
            sampled: {},
            storage: {},
          },
          [1, 1, 1],
          ATMOSPHERE_VIEW_PASS,
        );
        engine.dispatch(
          pair.nodes,
          {
            buffers: { ...common.buffers, limbCounts: lists.counts, limbRays: lists.rays },
            sampled: {
              ...common.sampled,
              sceneDepth: scene.depth,
              ...sourceTableBound,
              ...(slotDepths === null ? {} : { slotDepths }),
            },
            uniforms: {
              medium: perFrameMedium,
              view: marchUniforms,
              layerEdges: tables.layerEdges,
            },
            storage: {},
          },
          { buffer: lists.dispatch, offsetBytes: LIMB_NODES_ARGS_OFFSET_BYTES },
          ATMOSPHERE_VIEW_PASS,
        );
        engine.dispatch(
          pair.resolve,
          { uniforms: {}, buffers: listed, sampled: {}, storage: marchStorage },
          { buffer: lists.dispatch, offsetBytes: LIMB_RESOLVE_ARGS_OFFSET_BYTES },
          ATMOSPHERE_VIEW_PASS,
        );
        this.#readLimbCounts(lists);
      } else {
        engine.dispatch(footprint, bindings, tiles, ATMOSPHERE_VIEW_PASS);
      }
    }
    // What lies behind the sky from above the top at the march's resolution (the owner's priority,
    // 2026-11-01; `compositeBehind.wgsl`), over the march's tiles: where the march runs below the
    // output's resolution, whose texels it saves, and the camera is above the top with the march's
    // share 1, as the composite tests it (`aboveTop` and `exact.w`), so that it reads the target only
    // where it was formed. Never beneath a deck, nor for the kernel arm, which draws no composite.
    const camera = this.#uniforms["camera"];
    const curvatureMember = this.#uniforms["curvature"];
    const shellMember = this.#uniforms["shell"];
    const formsBehind =
      shellMember !== undefined &&
      marchesAll &&
      this.#levers.compositeTransmittance === "march" &&
      sizes.rayMarchScale < 1 &&
      beneath === null &&
      (camera?.[3] ?? 0) * (curvatureMember?.[2] ?? 0) >= (shellMember[0] ?? Infinity) &&
      area.tilesAcross > 0 &&
      area.tilesDown > 0;
    if (formsBehind) {
      engine.dispatch(
        kernels.behind(),
        {
          buffers: {},
          sampled: {
            sceneDepth: scene.depth,
            transmittance: tables.transmittance,
            absorberColumns: tables.absorberColumns,
            absorberCurves: tables.absorberCurves,
            absorberPressureColumns: tables.absorberPressureColumns,
            lastAbsorberPressureColumn: tables.lastAbsorberPressureColumn,
            oblateBody: tables.oblateBody,
          },
          uniforms: {
            draw: shellMember,
            view: viewUniforms,
            marchArea: areaUniform,
          },
          storage: {
            behindOut: { texture: this.#behindMarchTexture(view.viewport), level: 0 },
          },
        },
        [area.tilesAcross, area.tilesDown, 1],
        ATMOSPHERE_BEHIND_PASS,
      );
    }
    if (deferUnread) {
      this.#behindFormed = formsBehind;
      // Under a deck's route from above its split the march's deck forms draw the surfaces whose
      // rays meet the deck first, and the composite takes them from the march (R08.T15.b.2's route;
      // research-leans-33 §4(c)): its `levers.z`, 0 for every other body.
      this.#uniforms["levers"]?.set([
        formsBehind ? 1 : 0,
        this.#levers.surfaceTransmittance === "channels" ? 1 : 0,
        read === "deck" && inputs.heightM >= 0 ? 1 : 0,
        0,
      ]);
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
              storage: marchStorage,
            },
            groups8(area.widthTexels, area.heightTexels),
            ATMOSPHERE_VIEW_PASS,
          );
        };
    // What lies beyond the air from beneath a deck takes the camera's vertical depth (R08.T15.b.2;
    // `composite.wgsl`'s belowDeckBehind): the composite's `shell.w`, 0 elsewhere.
    const compositeShell = this.#uniforms["shell"];
    if (compositeShell !== undefined) {
      compositeShell[3] = beneath === null ? 0 : this.#belowDeckDepthGreen(inputs);
    }
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
   * The kernels' vertical optical depth from the top to the camera beneath a deck, its green channel
   * (R08.T15.b.2; `thick/deckRead.ts`'s `belowDeckOpticalDepthInto`), at h* = split + s h, h the
   * camera's height above the deck's datum, held to [0, split]; 0 with no table below.
   */
  #belowDeckDepthGreen(inputs: AtmosphereInputs): number {
    const packed = this.#tables.belowDeckPacked;
    if (packed === null) {
      return 0;
    }
    const splitM = belowDeckSplitM(packed);
    const s = inputs.gravityRatio;
    const heightStarM = Math.min(Math.max(splitM + s * inputs.heightM, 0), splitM);
    const depth: [number, number, number] = [0, 0, 0];
    belowDeckOpticalDepthInto(
      packed,
      this.#tables.plan.bands.map((band) => log(band.gravityRatio)),
      s,
      heightStarM,
      belowDeckReadScratch(),
      depth,
    );
    return depth[1];
  }

  /**
   * The far limb's lists for a frame over `area` (R08.T8.d): their room the larger of what its whole
   * target asks ({@link LIMB_ROOM_PER_EDGE}, {@link LIMB_RAYS_PER_TEXEL}) and what the counts read
   * back last asked, made anew where the held lists are smaller, never smaller.
   */
  #limbLists(area: MarchArea): LimbLists {
    const frame = this.#frame;
    // The whole target's edges, not the area's, so that an approach whose area grows frame by frame
    // remakes no list (as the tiles' kinds are made for the whole target, `#marchTiles`).
    const edges =
      (Math.ceil(area.widthTexels / MARCH_TILE_TEXELS) +
        Math.ceil(area.heightTexels / MARCH_TILE_TEXELS)) *
      MARCH_TILE_TEXELS;
    const texels = Math.max(LIMB_LEAST_TEXELS, LIMB_ROOM_PER_EDGE * edges, frame.limbWanted.texels);
    const rays = Math.max(LIMB_RAYS_PER_TEXEL * texels, frame.limbWanted.rays);
    const held = frame.limb;
    if (held !== null && held.texelRoom >= texels && held.rayRoom >= rays) {
      return held;
    }
    // Each list within a storage binding, and its pass's workgroups within a dispatch's.
    const { maxStorageBufferBindingSize, maxBufferSize } = this.#engine.capabilities;
    const most = (bytes: number): number =>
      Math.min(
        Math.floor(Math.min(maxStorageBufferBindingSize, maxBufferSize) / bytes),
        LIMB_MOST_WORKGROUPS * LIMB_WORKGROUP,
      );
    const texelRoom = Math.min(Math.max(texels, held?.texelRoom ?? 0), most(LIMB_TEXEL_BYTES));
    const rayRoom = Math.min(Math.max(rays, held?.rayRoom ?? 0), most(LIMB_RAY_BYTES));
    if (held !== null && held.texelRoom >= texelRoom && held.rayRoom >= rayRoom) {
      return held;
    }
    releaseLimbLists(this.#engine, held);
    const buffer = (name: string, bytes: number, usage: number): BufferHandle =>
      this.#engine.createBuffer({ name, bytes, usage, category: "atmosphere-view" });
    const made: LimbLists = {
      counts: buffer(
        "atmosphere far limb's counts",
        16,
        BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST | BUFFER_USAGE.COPY_SRC,
      ),
      dispatch: buffer(
        "atmosphere far limb's dispatches",
        24,
        BUFFER_USAGE.STORAGE | BUFFER_USAGE.INDIRECT,
      ),
      texels: buffer(
        "atmosphere far limb's texels",
        texelRoom * LIMB_TEXEL_BYTES,
        BUFFER_USAGE.STORAGE,
      ),
      rays: buffer("atmosphere far limb's rays", rayRoom * LIMB_RAY_BYTES, BUFFER_USAGE.STORAGE),
      texelRoom,
      rayRoom,
    };
    frame.limb = made;
    return made;
  }

  /**
   * Reads `lists`' counts back at the first packed frame and every {@link LIMB_READ_FRAMES} after,
   * one read at a time (R08.T8.d): where a frame reserved more than their room, the frames after
   * ask {@link LIMB_ROOM_GROWTH} times its counts. A read that fails asks nothing, and is said.
   */
  #readLimbCounts(lists: LimbLists): void {
    const frame = this.#frame;
    if (
      frame.limbReading ||
      (frame.limbFramesSinceRead > 0 && frame.limbFramesSinceRead < LIMB_READ_FRAMES)
    ) {
      frame.limbFramesSinceRead += 1;
      return;
    }
    frame.limbReading = true;
    frame.limbFramesSinceRead = 1;
    const engine = this.#engine;
    const grown = async (): Promise<void> => {
      // The dispatches' kernel, whose counts are whole numbers, writes them last.
      const bytes = await engine.readBuffer(lists.counts);
      const words = new Uint32Array(bytes, 0, Math.min(2, Math.floor(bytes.byteLength / 4)));
      const [texels = 0, rays = 0] = words;
      if (texels > lists.texelRoom || rays > lists.rayRoom) {
        frame.limbWanted = {
          texels: Math.max(frame.limbWanted.texels, Math.ceil(texels * LIMB_ROOM_GROWTH)),
          rays: Math.max(frame.limbWanted.rays, Math.ceil(rays * LIMB_ROOM_GROWTH)),
        };
      }
    };
    void grown()
      .catch((cause: unknown) => {
        console.warn("the far limb's counts could not be read back:", cause);
      })
      .finally(() => {
        frame.limbReading = false;
      });
  }

  /**
   * The tile cull's kinds' buffer for `area` (R08.T11.d), one `u32` a tile: made for the whole
   * target's tiles, its largest, and grown with the target, never made smaller, as the ray-march
   * target is ({@link HillaireAtmosphere.drawFrame}'s `#rayMarchTextures`).
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
   * The ray-march target and its transmittance per channel (R08.T9.c) at the setting's scale of the
   * largest output so far: they grow on a resize, releasing the smaller, and are never remade
   * smaller, so that a window resized back and forth makes nothing new; the kernel and the composite
   * address them by the current output's size.
   */
  #rayMarchTextures(viewport: ViewSize): {
    readonly rayMarch: TextureHandle;
    readonly rayMarchTransmittance: TextureHandle;
  } {
    const frame = this.#frame;
    const fits =
      frame.rayMarchSize !== null &&
      frame.rayMarchSize.widthPx >= viewport.widthPx &&
      frame.rayMarchSize.heightPx >= viewport.heightPx;
    if (frame.rayMarch !== null && frame.rayMarchTransmittance !== null && fits) {
      return { rayMarch: frame.rayMarch, rayMarchTransmittance: frame.rayMarchTransmittance };
    }
    for (const texture of [frame.rayMarch, frame.rayMarchTransmittance]) {
      if (texture !== null) {
        this.#engine.releaseTexture(texture);
      }
    }
    const grown: ViewSize = {
      widthPx: Math.max(viewport.widthPx, frame.rayMarchSize?.widthPx ?? 0),
      heightPx: Math.max(viewport.heightPx, frame.rayMarchSize?.heightPx ?? 0),
    };
    const scale = this.#sizes.rayMarchScale;
    const size: [number, number] = [
      Math.max(1, Math.ceil(grown.widthPx * scale)),
      Math.max(1, Math.ceil(grown.heightPx * scale)),
    ];
    const rayMarch = frameTexture(this.#engine, "atmosphere ray march", size, "rgba16float");
    const rayMarchTransmittance = frameTexture(
      this.#engine,
      "atmosphere ray march's transmittance",
      size,
      "rgba16float",
    );
    frame.rayMarch = rayMarch;
    frame.rayMarchTransmittance = rayMarchTransmittance;
    frame.rayMarchSize = grown;
    return { rayMarch, rayMarchTransmittance };
  }

  /**
   * The target of what lies behind the sky at the march's resolution (the owner's priority,
   * 2026-11-01): the ray march's size, grown as it is and never made smaller.
   */
  #behindMarchTexture(viewport: ViewSize): TextureHandle {
    const frame = this.#frame;
    const fits =
      frame.behindMarchSize !== null &&
      frame.behindMarchSize.widthPx >= viewport.widthPx &&
      frame.behindMarchSize.heightPx >= viewport.heightPx;
    if (frame.behindMarch !== null && fits) {
      return frame.behindMarch;
    }
    if (frame.behindMarch !== null) {
      this.#engine.releaseTexture(frame.behindMarch);
    }
    const grown: ViewSize = {
      widthPx: Math.max(viewport.widthPx, frame.behindMarchSize?.widthPx ?? 0),
      heightPx: Math.max(viewport.heightPx, frame.behindMarchSize?.heightPx ?? 0),
    };
    const scale = this.#sizes.rayMarchScale;
    frame.behindMarch = frameTexture(
      this.#engine,
      "atmosphere behind",
      [
        Math.max(1, Math.ceil(grown.widthPx * scale)),
        Math.max(1, Math.ceil(grown.heightPx * scale)),
      ],
      "rgba16float",
    );
    frame.behindMarchSize = grown;
    return frame.behindMarch;
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
    const tanX = tan(view.fovXRad / 2);
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
    // Its w, whether a drawn surface keeps its forward lobes' light ({@link SurfaceLobes}), the
    // ray march's alone.
    set("forward", [
      forward.x,
      forward.y,
      forward.z,
      this.#levers.surfaceLobes === "transmitted" ? 1 : 0,
    ]);
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
    // Its w, the frame's pre-exposure scale, is the thermal forms' factor for the medium's own light,
    // which no star weights (R08.T18.b); the other kernels read none of it.
    set("marchSky", [
      thinSky?.samples ?? 0,
      thinSky?.depthG ?? 0,
      ground?.drawnBandM ?? 0,
      scene.exposureScale,
    ]);
    // The composite's own: the atmosphere's height, formed in f64 (R08.T6.g), how many of the
    // medium's terms are absorbers, whose columns its transmittance to space reads (R08.T6.c), and
    // its line slots, whose pressure channels the read takes (R08.T4.d).
    set("shell", [topHeightM, absorberCount(medium), lineSlotMask(medium), 0]);
    return curvature;
  }
}
