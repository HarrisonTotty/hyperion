/**
 * The disc's reflectance table against the full passes from 10⁹ m (plan R08, R08.T16.b; Design note
 * 13): a body's table baked as the optics worker bakes it, and the integrated radiance of its disc
 * drawn by the table (R07's disc's twin, `discShading.ts`, with its air) against the full passes'
 * (the march's twin, `tablesCpu.ts`, over the mesh's twin, its ground the same disc's plain shading),
 * for `thick/discBake.test.ts` and `thick/discBakeSaturn.test.ts`.
 *
 * @remarks
 * - **The view.** The camera stands 10⁹ m from the centre over the equator (or as far as
 *   {@link DiscAirView.distanceM} puts it), the pole up the image, the sun at the phase angle in the
 *   equatorial plane at 1 au, of illuminance 1 lx in each channel.
 * - **The ground** is a Lambertian law of the medium's ground albedo on both sides, its surface
 *   lighting through the air (`surfaceLightingTwin` over the twin's tables).
 * - **The quadrature** runs over the image plane in the coordinates that make the datum's outline a
 *   unit circle, 200 radii (most within a twentieth of the limb, where the air's light rises) by 48
 *   azimuths, out past the air's shell, so that the full passes' limb is counted; the table's disc
 *   has none there.
 * - **The full passes** are the march's twin at the high setting's counts along each ray, ending at
 *   the datum where it meets it, as the mesh's depth ends it (`composite.wgsl`: the march's light,
 *   and the ground times its mean transmittance), routed (its tables holding the same thick table
 *   the bake read, as T14.d's routing would draw the body, and over a deck its table, as T15's
 *   route would) and unrouted (Ψ_ms, as the product draws it today).
 */

import { normalise, type Vec3, vec3 } from "../geometry/vec3";
import type { PhotometricLaw } from "../view/appearance/law";
import { geometricAlbedo } from "../view/appearance/phase";
import type { DiscAir } from "../view/atmosphere/discReflectance";
import type { DiscReflectanceTable } from "../view/atmosphere/discTable";
import { tableRadiusM, TABLE_SIZES } from "../view/atmosphere/hillaire";
import type { AtmosphereMedium } from "../view/atmosphere/medium";
import { gravityRatio, type SpheroidFigure } from "../view/atmosphere/oblate";
import { keptBakeLightsOf } from "../view/atmosphere/opticsMessages";
import { R06_SOLAR_BAKE_SPECTRUM } from "../view/atmosphere/earth";
import {
  type AbsorberCurveSlots,
  type AtmosphereBody,
  kernelMedium,
  tablesPlanOf,
} from "../view/atmosphere/tables";
import {
  IRRADIANCE_KERNEL_OPTIONS,
  oblateIrradianceTwin,
  type OblatePlanetTwin,
  oblatePlanetTwin,
  oblateRayMarchRayAlongTwin,
  oblateRayMarchTexelTwin,
  surfaceLightingTwin,
  twinView,
} from "../view/atmosphere/tablesCpu";
import { thickBakeBands, type ThickBakeBody } from "../view/atmosphere/thick/bake";
import type { DeckTables } from "../view/atmosphere/thick/deck";
import { deckAboveTables, packDeckTable } from "../view/atmosphere/thick/deckRead";
import { bakeDiscReflectance } from "../view/atmosphere/thick/discBake";
import { bakeKeptCase } from "../view/atmosphere/thick/keptBake";
import { type PackedThickTable, packThickTable } from "../view/atmosphere/thick/read";
import type { SpectralSource } from "../view/atmosphere/thick/spectralColumn";
import type { SurfaceLightingReader } from "../view/atmosphere/surfaceLighting";
import { type DiscRecord, shadeDiscPoint } from "../view/bodies/discShading";
import { NEAR_PLANE_M, type ProjectionCamera } from "../view/camera/projection";
import { IDENTITY_QUATERNION, quaternionFromRows } from "../view/camera/quaternion";
import { annulusEdges } from "../view/lighting/annuli";
import type { Rgb } from "../view/photometry/toneCurve";
import { AU_M } from "../view/scenes/kept";
import { asin, cos, log, sin } from "../lib/pinnedMath";

/** The camera's distance from the centre, m: the boundary's (Design note 13). */
export const DISC_AIR_DISTANCE_M = 1e9;

/** The Sun's nominal radius, m (IAU 2015 B3). */
const SUN_RADIUS_M = 6.957e8;

/** A body the table is judged on. */
export interface DiscAirCase {
  readonly name: string;
  readonly medium: AtmosphereMedium;
  readonly sources: ReadonlyArray<SpectralSource>;
  readonly curves: AbsorberCurveSlots;
  readonly body: AtmosphereBody;
  /**
   * The cloud deck the medium stands on (R08.T16.b.2), its bake's tables a band, the medium its
   * upper medium and the body on the deck's datum: J its own, read by the table's bake and the
   * routed passes, and its table at the datum; absent for a ground, J then the thick bake's.
   */
  readonly deck?: ReadonlyArray<DeckTables>;
}

/** A case's table and what it was baked from. */
export interface BakedDiscAir {
  readonly table: DiscReflectanceTable;
  /** The thick table the bake read J from, which the routed full passes read too. */
  readonly source: PackedThickTable;
  readonly bakeBody: ThickBakeBody;
}

/** A case's table, J baked by the thick bake's solver under the reference Sun, as the worker bakes it. */
export function bakedDiscAir(item: DiscAirCase): BakedDiscAir {
  const figure = item.body.figure;
  const bakeBody: ThickBakeBody = {
    figure,
    gravity: item.body.gravity,
    bottomRadiusM: tableRadiusM(figure),
  };
  const bands = thickBakeBands(bakeBody);
  const { deck } = item;
  const solved =
    deck === undefined
      ? bakeKeptCase(
          { medium: item.medium, sources: item.sources },
          keptBakeLightsOf(item.medium, [R06_SOLAR_BAKE_SPECTRUM]),
          bakeBody,
        )
      : deckAboveTables(deck);
  const source = packThickTable(solved, kernelMedium(item.medium));
  const table = bakeDiscReflectance({
    medium: kernelMedium(item.medium),
    curves: item.curves,
    bands,
    source,
    ...(deck === undefined
      ? {}
      : {
          deck: {
            table: packDeckTable(deck),
            lnBands: Float64Array.from(bands, (band) => log(band.gravityRatio)),
          },
        }),
  });
  return { table, source, bakeBody };
}

/** A case's full passes' twins: its planet's tables, routed and not, and its surface lighting. */
export interface FullPassTwins {
  readonly unrouted: OblatePlanetTwin;
  readonly routed: OblatePlanetTwin;
  readonly lighting: SurfaceLightingReader;
}

/** The twins of a case's full passes at the kernels' sizes (some seconds to tens of seconds). */
export function fullPassTwins(item: DiscAirCase, baked: BakedDiscAir): FullPassTwins {
  const unrouted = oblatePlanetTwin(kernelMedium(item.medium), item.body, item.curves);
  // The irradiance at the datum's row alone, which the disc reads at altitude 0.
  const rows = Array.from(
    { length: IRRADIANCE_KERNEL_OPTIONS.size.widthTexels },
    (_, x) => [x, 0] as const,
  );
  const lighting = surfaceLightingTwin(
    unrouted,
    oblateIrradianceTwin(unrouted, IRRADIANCE_KERNEL_OPTIONS, rows),
  );
  const deck = item.deck === undefined ? {} : { deck: packDeckTable(item.deck) };
  return { unrouted, routed: { ...unrouted, thick: baked.source, ...deck }, lighting };
}

/** The integrated radiance of each side at one phase, per channel, per unit of the sun's lx. */
export interface IntegratedRadiance {
  /** The disc drawn by its table. */
  readonly disc: Rgb;
  /** The full passes, routed. */
  readonly routed: Rgb;
  /** The full passes, unrouted (Ψ_ms). */
  readonly unrouted: Rgb;
  /** The routed passes' light from rays that miss the datum: the air's limb beyond it. */
  readonly limb: Rgb;
  /** The disc's solid angle, sr. */
  readonly solidAngleSr: number;
}

/** The body-fixed axes' coordinates of a vector of the view's: (z, x, y). */
function bodyFixed(v: Vec3): Vec3 {
  return vec3(v.z, v.x, v.y);
}

/** The near distance along the unit `d` from `o` to the spheroid (a, a, c), or `null`. */
function datumHit(o: Vec3, d: Vec3, a: number, c: number): number | null {
  const ox = o.x / a;
  const oy = o.y / a;
  const oz = o.z / c;
  const dx = d.x / a;
  const dy = d.y / a;
  const dz = d.z / c;
  const qa = dx * dx + dy * dy + dz * dz;
  const qb = ox * dx + oy * dy + oz * dz;
  const qc = ox * ox + oy * oy + oz * oz - 1;
  const disc = qb * qb - qa * qc;
  if (disc < 0) {
    return null;
  }
  const t = (-qb - Math.sqrt(disc)) / qa;
  return t > 0 ? t : null;
}

/** A case's Lambertian ground of its medium's albedo. */
export function groundLawOf(item: DiscAirCase): PhotometricLaw {
  const [r, g, b] = item.medium.groundAlbedo;
  return { a: [r, g, b], lommelSeeligerShare: 0, template: "lambert", phaseExponent: [1, 1, 1] };
}

/** A sum's three channels. */
function rgbOf(v: ReadonlyArray<number>): Rgb {
  return [v[0] ?? 0, v[1] ?? 0, v[2] ?? 0];
}

/**
 * How {@link integratedRadianceAt} counts: its march's samples a multiple of the high setting's
 * (1, the product's; more for a converged reference, R08.T16.b.2), and its quadrature's radii and
 * azimuths (200 and 48).
 */
export interface IntegrationCounts {
  readonly marchScale?: number;
  readonly radii?: number;
  readonly azimuths?: number;
}

/** The high setting's sizes with its march's counts `scale` times as many. */
function marchSizesOf(scale: number): typeof TABLE_SIZES.high {
  const high = TABLE_SIZES.high;
  if (scale === 1) {
    return high;
  }
  const steps = high.marchDepthSteps;
  return {
    ...high,
    rayMarchSamples: high.rayMarchSamples * scale,
    marchDepthSteps:
      steps === null
        ? null
        : {
            ...steps,
            groundFloor: steps.groundFloor * scale,
            groundFloorPerEdge: steps.groundFloorPerEdge * scale,
          },
  };
}

/**
 * Where {@link integratedRadianceAt} looks from and what its routed full passes draw (plan R08,
 * R08.T16.b.2; research-rm4-rm5-close-out §1.3).
 */
export interface DiscAirView {
  /** The camera's distance from the centre, m: {@link DISC_AIR_DISTANCE_M} if absent. */
  readonly distanceM?: number;
  /**
   * The figure the routed full passes' black disc stands on beneath a deck, its depth ending their
   * march where the ray meets it (a routed envelope's on its medium's datum, below the deck's);
   * absent, nothing is drawn there, as T16.b.2 marched it.
   */
  readonly routedDisc?: SpheroidFigure;
}

/**
 * The integrated radiance of a case's disc drawn by its table and of its full passes from 10⁹ m at
 * the phase angle `phaseDeg` (the module's remarks), or from where `view` puts the camera.
 */
export function integratedRadianceAt(
  item: DiscAirCase,
  baked: BakedDiscAir,
  twins: FullPassTwins,
  phaseDeg: number,
  counts: IntegrationCounts = {},
  view: DiscAirView = {},
): IntegratedRadiance {
  const sizes = marchSizesOf(counts.marchScale ?? 1);
  const figure = item.body.figure;
  const a = figure.equatorialRadiusM;
  const c = figure.polarRadiusM;
  const D = view.distanceM ?? DISC_AIR_DISTANCE_M;
  const alpha = (phaseDeg * Math.PI) / 180;
  // The view's axes are the galactic ones (the camera's identity), the camera at the origin looking
  // down −z at the centre, the pole along +y; the body-fixed axes (z, x, y) of them.
  const sun = vec3(sin(alpha), 0, cos(alpha));
  const camera: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
  const cameraM = vec3(D, 0, 0);
  const twin = twinView(
    {
      positionM: cameraM,
      orientation: quaternionFromRows([vec3(0, 0, 1), vec3(1, 0, 0), vec3(0, 1, 0)]),
      fovXRad: Math.PI / 3,
      viewport: { widthPx: 1920, heightPx: 1080 },
    },
    {
      directionBodyFixed: bodyFixed(sun),
      illuminanceLx: [1, 1, 1],
      angularRadiusRad: asin(SUN_RADIUS_M / AU_M),
    },
    figure,
    NEAR_PLANE_M,
  );
  const annuli = annulusEdges(0.6, 0.6, 4);
  const plain: DiscRecord = {
    body: "0200080020000000.0001",
    rect: { leftPx: 0, topPx: 0, rightPx: 1, bottomPx: 1 },
    direction: vec3(0, 0, -1),
    radiusOverDistance: a / D,
    pole: vec3(0, 1, 0),
    polarOverEquatorial: c / a,
    interiorSamples: 1,
    limbSamples: 4,
    surface: { kind: "uniform", law: groundLawOf(item) },
    tableRows: [0],
    exposureOverPi: 1 / Math.PI,
    lights: [
      {
        direction: sun,
        distance: AU_M / a,
        radius: SUN_RADIUS_M / a,
        illuminance: [1, 1, 1],
        annuli: [annuli, annuli, annuli],
      },
    ],
    occluders: [],
    secondaries: [],
  };
  const air: DiscAir = {
    table: baked.table,
    texture: { kind: "texture", name: "disc table" },
    lnBands: tablesPlanOf(item.body, tableRadiusM(figure)).bands.map((band) =>
      log(band.gravityRatio),
    ),
    gravity: item.body.gravity,
  };
  const aired: DiscRecord = { ...plain, air };
  const topM = item.medium.topHeightM;
  const shellScale = Math.max(
    1 / gravityRatio(item.body.gravity, 0),
    1 / gravityRatio(item.body.gravity, Math.PI / 2),
  );
  const rMax = 1 + ((topM * shellScale) / c) * 1.02;
  const sums = {
    disc: [0, 0, 0],
    routed: [0, 0, 0],
    unrouted: [0, 0, 0],
    limb: [0, 0, 0],
  };
  const radii = counts.radii ?? 200;
  const azimuths = counts.azimuths ?? 48;
  // Most of the radii within a twentieth of the limb, and on out over the shell.
  const radiusAt = (u: number): number =>
    u < 0.8 ? (u / 0.8) * 0.95 : 0.95 + ((u - 0.8) / 0.2) * (rMax - 0.95);
  const across = a / D;
  const up = c / D;
  for (let i = 0; i < radii; i += 1) {
    const r0 = radiusAt(i / radii);
    const r1 = radiusAt((i + 1) / radii);
    const r = 0.5 * (r0 + r1);
    for (let k = 0; k < azimuths; k += 1) {
      const phi = ((k + 0.5) / azimuths) * 2 * Math.PI;
      const xi = across * r * cos(phi);
      const eta = up * r * sin(phi);
      const weight = across * up * r * (r1 - r0) * ((2 * Math.PI) / azimuths);
      const ray = normalise(vec3(xi, eta, -1));
      const d = bodyFixed(ray);
      const hit = datumHit(cameraM, d, a, c);
      const tSurfaceM = hit ?? Number.POSITIVE_INFINITY;
      // Over a deck the march's ground is the deck, nothing drawn on it, or the black disc drawn
      // below it, so that it reads the deck's light there and its transmittance to it is 0
      // (R08.T16.b.2).
      const below = view.routedDisc;
      const routedSurfaceM =
        item.deck === undefined
          ? tSurfaceM
          : below === undefined
            ? Number.POSITIVE_INFINITY
            : (datumHit(cameraM, d, below.equatorialRadiusM, below.polarRadiusM) ??
              Number.POSITIVE_INFINITY);
      const routed = oblateRayMarchTexelTwin(
        twins.routed,
        twin,
        sizes,
        oblateRayMarchRayAlongTwin(twins.routed, twin, cameraM, d, routedSurfaceM),
      );
      const unrouted = oblateRayMarchTexelTwin(
        twins.unrouted,
        twin,
        sizes,
        oblateRayMarchRayAlongTwin(twins.unrouted, twin, cameraM, d, tSurfaceM),
      );
      let ground: Rgb = [0, 0, 0];
      let disc: Rgb = [0, 0, 0];
      if (hit !== null) {
        const p = vec3(cameraM.x + hit * d.x, cameraM.y + hit * d.y, cameraM.z + hit * d.z);
        // The hit from the centre ÷ a in the view's axes: body-fixed (x, y, z) is the view's (y, z, x).
        const q = vec3(p.y / a, p.z / a, p.x / a);
        ground = shadeDiscPoint(plain, camera, q, ray, twins.lighting).radiance;
        disc = shadeDiscPoint(aired, camera, q, ray, twins.lighting).radiance;
      }
      for (const ch of [0, 1, 2] as const) {
        sums.routed[ch] = (sums.routed[ch] ?? 0) + (routed[ch] + routed[3] * ground[ch]) * weight;
        sums.unrouted[ch] =
          (sums.unrouted[ch] ?? 0) + (unrouted[ch] + unrouted[3] * ground[ch]) * weight;
        sums.disc[ch] = (sums.disc[ch] ?? 0) + disc[ch] * weight;
        if (hit === null) {
          sums.limb[ch] = (sums.limb[ch] ?? 0) + routed[ch] * weight;
        }
      }
    }
  }
  return {
    disc: rgbOf(sums.disc),
    routed: rgbOf(sums.routed),
    unrouted: rgbOf(sums.unrouted),
    limb: rgbOf(sums.limb),
    solidAngleSr: Math.PI * across * up,
  };
}

/**
 * A disc's geometric albedo per channel from its integrated radiance at phase 0 under 1 lx: π times
 * its mean radiance over its solid angle, the I/F of a disc of that light.
 */
export function geometricAlbedoOf(sum: Rgb, solidAngleSr: number): Rgb {
  return [
    (Math.PI * sum[0]) / solidAngleSr,
    (Math.PI * sum[1]) / solidAngleSr,
    (Math.PI * sum[2]) / solidAngleSr,
  ];
}

/** The bare ground's geometric albedo, R07's `BodyPhotometry.geometricAlbedo` of its law. */
export function groundGeometricAlbedo(item: DiscAirCase): Rgb {
  return geometricAlbedo(groundLawOf(item));
}
