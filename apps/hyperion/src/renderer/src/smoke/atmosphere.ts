/**
 * The smoke page's atmosphere checks (plan R05, R05.T12.b): Earth's per-planet tables built on the
 * GPU and read back with the harness's tolerance access, since their kernels are
 * `presentation-only`.
 *
 * @remarks
 * The transmittance table agrees with the `f64` oracle (`view/atmosphere/opticalDepth.ts`) to 1%
 * at 20 fixed texels, and every texel of both tables is finite, the transmittance table's optical
 * depth and absorber columns non-negative (R08.T6.c) and multiple scattering non-negative. R05.T12.c adds whole frames of the atmosphere on both settings
 * (`checkAtmosphereFrames`) and, when the run is asked for them, the comparison frames of
 * Hillaire's reference medium that a person sets beside his published images
 * (`captureAtmosphere`). R05.T12.e holds the sky view and the ray march at their settings' step
 * counts to the same kernels at 1,024 steps (`checkAtmosphereSteps`). R08.T6.b holds every kernel to
 * the CPU twin of `view/atmosphere/tablesCpu.ts` to 10⁻³, over a medium of every density profile
 * and phase function the kernels read (`checkAtmosphereTwins`). R08.T6.g holds them there from 1 m
 * to 2 km and from orbit, on Earth, a 2 R⊕ sphere and a Saturn-radius sphere, in two arms: the
 * stored `rgba16float` tables and each kernel's own `rgba32float` output (`checkAtmosphereHeights`).
 * Since R08.T6.c the transmittance table stores optical depth and the absorbers' columns: the oracle
 * check compares optical depth, and R08.T6.b's check holds them, a medium of four absorbers (one a
 * molecule pair) under their curves of growth among its media, and reports the channels a stored
 * T had under 2⁻¹⁴ on Earth's table and a Venus-class one.
 */

import { add, normalise, scale, type Vec3 } from "../geometry/vec3";
import { quaternion, quaternionFromRows, rotate } from "../view/camera/quaternion";
import { hydrostaticColumn } from "../view/atmosphere/column";
import { EARTH_REFERENCE, HILLAIRE_REFERENCE } from "../view/atmosphere/earth";
import {
  type AtmosphereCamera,
  atmosphereInputs,
  type FrameOutputs,
  HillaireAtmosphere,
  skyViewHeight,
  type SpheroidFigure,
  type SunState,
  TABLE_SIZES,
  type TableSizes,
  tableRadiusM,
} from "../view/atmosphere/hillaire";
import {
  grazingTwilight,
  LOW_TWIN_WORST,
  MARCH_TOLERANCE,
  TWILIGHT_TOLERANCE,
} from "../view/atmosphere/marchSteps";
import { type AtmosphereMedium, phaseTable, tabulatedDensity } from "../view/atmosphere/medium";
import {
  maxDistanceM,
  opticalDepthToTop,
  transmittanceUvToRMu,
  unitFromTexelUv,
} from "../view/atmosphere/opticalDepth";
import {
  absorberCount,
  type AbsorberCurveSlots,
  AtmosphereTables,
  kernelMedium,
  MULTI_SCATTERING_SIZE,
  NO_ABSORBER_CURVES,
  PHASE_TABLE_ENTRIES,
  type TableSize,
  TRANSMITTANCE_SIZE,
} from "../view/atmosphere/tables";
import {
  aerialPerspectiveTwin,
  multiScatteringTwin,
  type PlanetTwin,
  planetTwin,
  marchTwin,
  placedSteps,
  rayMarchRayAlongTwin,
  rayMarchRayTwin,
  rayMarchTexelTwin,
  rayMarchTwin,
  skyViewRayTwin,
  skyViewTwin,
  tableSources,
  type TwinTable,
  type TwinTransmittance,
  transmittanceTwin,
  type TwinView,
  twinView,
} from "../view/atmosphere/tablesCpu";
import { fourAbsorberMedium, venusClassMedium } from "../test/atmosphereFixtures";
import {
  PresentationOnlyReadback,
  type RenderEngine,
  type RenderTarget,
  type TextureHandle,
  type ViewSize,
} from "../view/engine/types";
import { exposureScale } from "../view/photometry/exposure";
import type { QualitySetting } from "../view/quality/qualitySetting";
import { HALF_FLOAT_MAX, toneCurve } from "../view/photometry/toneCurve";
import {
  type Checks,
  drawOf,
  flatSpec,
  frameOf,
  fullScreenMesh,
  halfTexels,
  NEAR_M,
  show,
  texel,
} from "./harness";

/** The ground's radius the check builds Earth's tables on, m: WGS 84's mean radius R₁. */
const EARTH_MEAN_RADIUS_M = 6_371_008.8;

/**
 * The 20 texels compared with the oracle: five across μ by four across r, each at its own (r, μ)
 * through Bruneton's texel mapping since R08.T6.g (plan R08, Design note 19), so that column 255
 * is the ray tangent to the ground and row 0 the ground itself.
 */
const ORACLE_TEXELS: ReadonlyArray<readonly [number, number]> = [0, 64, 128, 192, 255].flatMap(
  (x) => [0, 16, 40, 63].map((y) => [x, y] as const),
);

/** Builds Earth's tables and checks them against the oracle and their ranges. */
export async function checkAtmosphereTables(engine: RenderEngine, checks: Checks): Promise<void> {
  const tables = new AtmosphereTables(engine, EARTH_REFERENCE, EARTH_MEAN_RADIUS_M);

  let refused = false;
  try {
    await engine.readTexture(tables.transmittance);
  } catch (error: unknown) {
    refused = error instanceof PresentationOnlyReadback;
  }
  checks.check(
    "R05.T12.b the transmittance table is refused to an ordinary readback",
    refused,
    `refused ${String(refused)}`,
  );

  const transmittance = halfTexels(
    await engine.readTexture(tables.transmittance, 0, undefined, "tolerance"),
  );
  const shell = {
    bottomRadiusM: EARTH_MEAN_RADIUS_M,
    topRadiusM: EARTH_MEAN_RADIUS_M + EARTH_REFERENCE.topHeightM,
  };
  const width = TRANSMITTANCE_SIZE.widthTexels;
  for (const [x, y] of ORACLE_TEXELS) {
    const height = TRANSMITTANCE_SIZE.heightTexels;
    const { rM, mu } = transmittanceUvToRMu(
      shell,
      unitFromTexelUv((x + 0.5) / width, width),
      unitFromTexelUv((y + 0.5) / height, height),
    );
    // The table holds every ray to the top, the tangent column's whole grazing path (R08.T6.g), as
    // optical depth since R08.T6.c: compared in optical depth, to 1% of it, or the half float's
    // step below its least normal, where the top row's τ near 0 stands.
    const expected = opticalDepthToTop(EARTH_REFERENCE, EARTH_MEAN_RADIUS_M, rM, mu);
    const actual = texel(transmittance, width, x, y).slice(0, 3);
    const agrees = expected.every((e, c) => {
      const a = actual[c] ?? Number.NaN;
      return Math.abs(a - e) <= Math.max(0.01 * e, HALF_SUBNORMAL_SPACING);
    });
    checks.check(
      `R05.T12.b transmittance texel (${x}, ${y}) agrees with the f64 oracle to 1% in optical depth`,
      agrees,
      `h ${((rM - EARTH_MEAN_RADIUS_M) / 1_000).toFixed(2)} km, mu ${mu.toFixed(4)}: GPU τ ${show(actual)}, oracle τ ${show(expected)}`,
    );
  }
  const inRange = transmittance.every((v) => Number.isFinite(v) && v >= 0);
  checks.check(
    "R05.T12.b every stored optical depth and absorber column is finite and non-negative",
    inRange,
    `${transmittance.length / 4} texels, first ${show(transmittance.subarray(0, 4))}`,
  );

  const multiScattering = halfTexels(
    await engine.readTexture(tables.multiScattering, 0, undefined, "tolerance"),
  );
  checks.check(
    "R05.T12.b every multiple-scattering texel is finite and non-negative",
    multiScattering.every((v) => Number.isFinite(v) && v >= 0),
    `${multiScattering.length / 4} texels`,
  );
  // The first row's last texel: the sun overhead, at the ground, where the sky is lit.
  const overhead = texel(
    multiScattering,
    MULTI_SCATTERING_SIZE.widthTexels,
    MULTI_SCATTERING_SIZE.widthTexels - 1,
    0,
  );
  checks.check(
    "R05.T12.b multiple scattering at the ground under an overhead sun is positive",
    overhead.slice(0, 3).every((v) => v > 0),
    `texel ${show(overhead)}`,
  );
  checks.check(
    "R05.T12.b the tables are not rebuilt for the same medium",
    !tables.setMedium(EARTH_REFERENCE, EARTH_MEAN_RADIUS_M) && tables.builds === 1,
    `builds ${tables.builds}`,
  );
}

// --- R05.T12.c: frames of the atmosphere -------------------------------------------------------

/** WGS 84 (NIMA TR8350.2): the test planet's figure. */
const WGS84: SpheroidFigure = {
  equatorialRadiusM: 6_378_137,
  polarRadiusM: 6_378_137 * (1 - 1 / 298.257223563),
};

/** The check frames' size, device pixels. */
const FRAME_SIZE = { widthPx: 64, heightPx: 32 } as const;

/**
 * The Sun's angular radius at 1 au, rad: asin(R⊙ᴺ ÷ 1 au) = 0.0046505 (959.23″), with R⊙ᴺ =
 * 6.957 × 10⁸ m (IAU 2015 Resolution B3) and 1 au = 149,597,870,700 m (IAU 2012 Resolution B2).
 */
const SUN_ANGULAR_RADIUS_RAD = 0.004_650_5;

/**
 * A camera `heightM` above the equator at longitude 0 of `figure` (WGS 84 unless given), looking
 * north along the horizon.
 */
function equatorCamera(
  heightM: number,
  size: ViewSize,
  figure: SpheroidFigure = WGS84,
): AtmosphereCamera {
  const aspect = size.widthPx / size.heightPx;
  return {
    positionM: { x: figure.equatorialRadiusM + heightM, y: 0, z: 0 },
    // Camera −z (forward) to body +z (north), camera +y (up) to body +x (the normal): a half turn
    // about (1, 1, 0) ÷ √2.
    orientation: quaternion(0, Math.SQRT1_2, Math.SQRT1_2, 0),
    // The harness's projection has tan(fovY ÷ 2) = 1, so tan(fovX ÷ 2) = the aspect.
    fovXRad: 2 * Math.atan(aspect),
    viewport: size,
  };
}

/** The sun at an elevation above the camera's horizon, towards the north (body +x up, +z north). */
function sunAt(elevationDeg: number): SunState {
  const e = (elevationDeg * Math.PI) / 180;
  return {
    directionBodyFixed: { x: Math.sin(e), y: 0, z: Math.cos(e) },
    distanceAu: 1,
    angularRadiusRad: SUN_ANGULAR_RADIUS_RAD,
  };
}

/** The grey a check surface is drawn in, pre-exposed. */
const SURFACE_GREY = 0.02;

/** Renders one composite frame over a scene whose surface, if any, stands `surfaceM` ahead. */
async function compositeFrame(
  engine: RenderEngine,
  atmosphere: HillaireAtmosphere,
  camera: AtmosphereCamera,
  sun: SunState,
  surfaceM: number | null,
): Promise<Float32Array> {
  const size = camera.viewport;
  const aspect = size.widthPx / size.heightPx;
  const target = (name: string): RenderTarget =>
    engine.createRenderTarget({
      name,
      size,
      format: "rgba16float",
      mips: 1,
      depth: true,
      category: "render-targets",
    });
  const scene = target("atmosphere scene");
  const output = target("atmosphere output");
  try {
    const draws =
      surfaceM === null
        ? []
        : [
            drawOf(
              fullScreenMesh(engine, "atmosphere surface", -surfaceM),
              engine.createMaterial(flatSpec("atmosphere surface")),
              [SURFACE_GREY, SURFACE_GREY, SURFACE_GREY, 1],
            ),
          ];
    scene.render(frameOf("atmosphere scene", draws, aspect));
    const composite = atmosphere.drawFrame(camera, sun, {
      colour: scene.colour,
      depth: requireDepth(scene.depth),
      nearM: NEAR_M,
      exposureScale: exposureScale(15),
    });
    output.render(frameOf("atmosphere composite", [composite], aspect));
    return halfTexels(await engine.readTexture(output.colour));
  } finally {
    scene.dispose();
    output.dispose();
  }
}

function requireDepth(depth: TextureHandle | null): TextureHandle {
  if (depth === null) {
    throw new Error("the scene target has no depth");
  }
  return depth;
}

/** Every channel finite and within rgba16float's range. */
function withinHalfRange(texels: Float32Array): boolean {
  return texels.every((v) => Number.isFinite(v) && v >= 0 && v <= HALF_FLOAT_MAX);
}

/** The mean of each colour channel over the frame. */
function meanRgb(texels: Float32Array): readonly [number, number, number] {
  const sum = [0, 0, 0];
  for (let i = 0; i < texels.length; i += 4) {
    sum[0] = (sum[0] ?? 0) + (texels[i] ?? 0);
    sum[1] = (sum[1] ?? 0) + (texels[i + 1] ?? 0);
    sum[2] = (sum[2] ?? 0) + (texels[i + 2] ?? 0);
  }
  const n = texels.length / 4;
  return [(sum[0] ?? 0) / n, (sum[1] ?? 0) / n, (sum[2] ?? 0) / n];
}

/**
 * R05.T12.c: frames from 400 km and from 2 m, at noon and at the terminator, with every texel
 * finite and none above rgba16float's largest value, on both settings. The noon sky from the ground
 * is bluer than it is red. A dark surface 500 m and 5 km ahead (inside the aerial-perspective
 * volume) and 60 km ahead (beyond it, so the ray march's) takes on haze: more at 5 km and at 60 km
 * than at 500 m. Between 5 and 60 km the haze has all but reached the horizon sky's brightness, so
 * the two paths agree to within 10% there rather than ordering.
 */
export async function checkAtmosphereFrames(engine: RenderEngine, checks: Checks): Promise<void> {
  for (const setting of ["high", "low"] as const) {
    const atmosphere = new HillaireAtmosphere(engine, EARTH_REFERENCE, TABLE_SIZES[setting], WGS84);
    try {
      const cases = [
        { name: "2 m, noon", heightM: 2, elevationDeg: 60, surfaceM: null },
        { name: "2 m, terminator", heightM: 2, elevationDeg: 0, surfaceM: null },
        { name: "400 km, noon", heightM: 400_000, elevationDeg: 60, surfaceM: null },
        { name: "400 km, terminator", heightM: 400_000, elevationDeg: 0, surfaceM: null },
        { name: "2 m, a surface 500 m ahead", heightM: 2, elevationDeg: 30, surfaceM: 500 },
        { name: "2 m, a surface 5 km ahead", heightM: 2, elevationDeg: 30, surfaceM: 5_000 },
        { name: "2 m, a surface 60 km ahead", heightM: 2, elevationDeg: 30, surfaceM: 60_000 },
      ] as const;
      const blue = new Map<string, number>();
      for (const c of cases) {
        // The harness's checks run in order: each reads the GPU back before the next draws.
        // oxlint-disable-next-line no-await-in-loop
        const texels = await compositeFrame(
          engine,
          atmosphere,
          equatorCamera(c.heightM, FRAME_SIZE),
          sunAt(c.elevationDeg),
          c.surfaceM,
        );
        const mean = meanRgb(texels);
        blue.set(c.name, mean[2]);
        checks.check(
          `R05.T12.c ${setting}: ${c.name}, every texel finite and within rgba16float`,
          withinHalfRange(texels),
          `mean ${show(mean)}`,
        );
        if (c.name === "2 m, noon") {
          checks.check(
            `R05.T12.c ${setting}: the noon sky from the ground is lit and bluer than red`,
            mean[2] > mean[0] && mean[1] > 0,
            `mean ${show(mean)}`,
          );
        }
      }
      const close = blue.get("2 m, a surface 500 m ahead") ?? Number.NaN;
      const near = blue.get("2 m, a surface 5 km ahead") ?? Number.NaN;
      const far = blue.get("2 m, a surface 60 km ahead") ?? Number.NaN;
      checks.check(
        `R05.T12.c ${setting}: a surface takes on haze with distance, the two paths agreeing`,
        close > SURFACE_GREY * 0.5 && near > close && far > close && Math.abs(far / near - 1) < 0.1,
        `blue at 500 m ${show([close])}, 5 km ${show([near])}, 60 km ${show([far])}; the surface ${SURFACE_GREY}`,
      );
    } finally {
      atmosphere.dispose();
    }
  }
}

// --- R05.T12.e: the marches' steps against 1,024 -------------------------------------------------

/** The steps the agreement check's converged kernels take (`decision-r05-high-atmosphere.md`). */
const CONVERGED_STEPS = 1_024;

/** The f32 and rgba16float margin the agreement check adds to the quadrature gate's tolerances. */
const F32_MARGIN = 0.01;

/** The floor of e's denominator, as a fraction of the converged output's brightest texel. */
const ERROR_FLOOR = 1e-3;

/** The least height of addendum B's high cameras, m, whose sky view low holds to its own bound. */
const HIGH_CAMERA_M = 60_000;

/** A ray of a kernel's output, its origin from the centre of the sphere it is marched about. */
interface CheckRay {
  readonly originM: Vec3;
  readonly direction: Vec3;
  readonly sun: Vec3;
  readonly tStartM: number;
  readonly tEndM: number;
}

/** A texel coordinate's unit range on one axis, `common.wgsl`'s `unitFromTexelUv`, clamped. */
function unitOfUv(uv: number, size: number): number {
  return Math.min(Math.max(unitFromTexelUv(uv, size), 0), 1);
}

/**
 * The ray of the sky-view texel (x, y), as `skyView.wgsl` marches it: `skyViewUvToParams` in
 * `view.wgsl`, on the camera's own sphere, in the frame whose x points to the sun's azimuth.
 */
function skyViewRay(
  x: number,
  y: number,
  size: { readonly widthTexels: number; readonly heightTexels: number },
  camera: AtmosphereCamera,
  sun: SunState,
): CheckRay {
  const inputs = atmosphereInputs(camera, WGS84);
  const bottomM = inputs.radiusM - inputs.heightM;
  const shell = { bottomRadiusM: bottomM, topRadiusM: bottomM + EARTH_REFERENCE.topHeightM };
  const heightM = skyViewHeight(inputs.heightM, EARTH_REFERENCE.topHeightM);
  const rM = bottomM + heightM;
  const u = unitOfUv((x + 0.5) / size.widthTexels, size.widthTexels);
  const v = unitOfUv((y + 0.5) / size.heightTexels, size.heightTexels);
  const beta = Math.acos(Math.sqrt(Math.max(heightM * (2 * bottomM + heightM), 0)) / rM);
  const zenithHorizon = Math.PI - beta;
  const zenith =
    v < 0.5 ? zenithHorizon * (1 - (1 - 2 * v) ** 2) : zenithHorizon + beta * (2 * v - 1) ** 2;
  const lightViewCos = -(u * u * 2 - 1);
  const sinZenith = Math.sin(zenith);
  const direction = {
    x: sinZenith * lightViewCos,
    y: sinZenith * Math.sqrt(Math.max(1 - lightViewCos * lightViewCos, 0)),
    z: Math.cos(zenith),
  };
  const n = inputs.normal;
  const s = sun.directionBodyFixed;
  const muSun = n.x * s.x + n.y * s.y + n.z * s.z;
  return {
    originM: { x: 0, y: 0, z: rM },
    direction,
    sun: { x: Math.sqrt(Math.max(1 - muSun * muSun, 0)), y: 0, z: muSun },
    tStartM: 0,
    tEndM: maxDistanceM(shell, rM, direction.z),
  };
}

/** The near and far distances along a ray to a sphere about the centre, or null if missed. */
function sphereHits(o: Vec3, d: Vec3, radiusM: number): readonly [number, number] | null {
  const b = o.x * d.x + o.y * d.y + o.z * d.z;
  const disc = b * b - (o.x * o.x + o.y * o.y + o.z * o.z - radiusM * radiusM);
  if (disc < 0) {
    return null;
  }
  return [-b - Math.sqrt(disc), -b + Math.sqrt(disc)];
}

/**
 * The ray of the ray-march texel (x, y) of a `widthTexels` × `heightTexels` target, as `rayMarch.wgsl` takes
 * it (`rayThrough`), ending at a surface `surfaceM` ahead if there is one, and clipped against
 * spheres of the datum's equatorial radius: close enough on WGS 84 to say which rays graze, which
 * is all the check asks of it. `null` where the ray misses the atmosphere, whose texel the kernel
 * leaves dark.
 */
function rayMarchRay(
  x: number,
  y: number,
  widthTexels: number,
  heightTexels: number,
  camera: AtmosphereCamera,
  sun: SunState,
  surfaceM: number | null,
): CheckRay | null {
  const tanX = Math.tan(camera.fovXRad / 2);
  const tanY = tanX / (camera.viewport.widthPx / camera.viewport.heightPx);
  const ndcX = ((x + 0.5) / widthTexels) * 2 - 1;
  const ndcY = 1 - ((y + 0.5) / heightTexels) * 2;
  const right = rotate(camera.orientation, { x: 1, y: 0, z: 0 });
  const up = rotate(camera.orientation, { x: 0, y: 1, z: 0 });
  const forward = rotate(camera.orientation, { x: 0, y: 0, z: -1 });
  const unnormalised = add(forward, add(scale(right, ndcX * tanX), scale(up, ndcY * tanY)));
  const direction = normalise(unnormalised);
  const a = WGS84.equatorialRadiusM;
  const shell = sphereHits(camera.positionM, direction, a + EARTH_REFERENCE.topHeightM);
  if (shell === null || shell[1] <= 0) {
    return null;
  }
  const ground = sphereHits(camera.positionM, direction, a);
  const lengthM = Math.hypot(unnormalised.x, unnormalised.y, unnormalised.z);
  const surfaceAtM = surfaceM === null ? Number.POSITIVE_INFINITY : surfaceM * lengthM;
  const groundAtM = ground !== null && ground[0] > 0 ? ground[0] : Number.POSITIVE_INFINITY;
  return {
    originM: camera.positionM,
    direction,
    sun: sun.directionBodyFixed,
    tStartM: Math.max(shell[0], 0),
    tEndM: Math.min(shell[1], groundAtM, surfaceAtM),
  };
}

/** One output's worst texel against the converged output. */
interface Agreement {
  /** The worst e as a fraction of its texel's tolerance: at most 1 where every texel agrees. */
  readonly worst: number;
  readonly at: string;
  readonly texels: number;
  readonly twilight: number;
  /** Whether the converged output has a lit texel, without which it would compare nothing. */
  readonly lit: boolean;
}

/** A texel's tolerance: its quadrature gate's for its ray, plus {@link F32_MARGIN}. */
type Tolerance = (twilight: boolean) => number;

/**
 * Each texel's e = max over channels of |ΔL| ÷ max(L, 10⁻³ L_max) against the converged output,
 * L_max its brightest texel, as a fraction of its tolerance.
 */
function agreement(
  shipped: Float32Array,
  converged: Float32Array,
  widthTexels: number,
  rayAt: (x: number, y: number) => CheckRay | null,
  toleranceOf: Tolerance,
): Agreement {
  const floor = [0, 1, 2].map((c) => {
    let max = 0;
    for (let i = c; i < converged.length; i += 4) {
      max = Math.max(max, converged[i] ?? 0);
    }
    return ERROR_FLOOR * max;
  });
  let worst = 0;
  let at = "none";
  let twilight = 0;
  const texels = converged.length / 4;
  for (let i = 0; i < texels; i += 1) {
    const x = i % widthTexels;
    const y = Math.floor(i / widthTexels);
    const ray = rayAt(x, y);
    const grazing =
      ray !== null && grazingTwilight(ray.originM, ray.direction, ray.sun, ray.tStartM, ray.tEndM);
    twilight += grazing ? 1 : 0;
    const tolerance = toleranceOf(grazing);
    for (let c = 0; c < 3; c += 1) {
      const reference = converged[i * 4 + c] ?? Number.NaN;
      const delta = Math.abs((shipped[i * 4 + c] ?? Number.NaN) - reference);
      const denominator = Math.max(reference, floor[c] ?? 0);
      let e = 0;
      if (denominator > 0) {
        e = delta / denominator;
      } else if (delta !== 0) {
        e = Number.POSITIVE_INFINITY;
      }
      const ratio = Number.isNaN(e) ? Number.POSITIVE_INFINITY : e / tolerance;
      if (ratio > worst) {
        worst = ratio;
        const kind = grazing ? " (twilight)" : "";
        at = `texel (${x}, ${y}) channel ${c}: e ${(e * 100).toFixed(2)}% against ${(tolerance * 100).toFixed(1)}%${kind}`;
      }
    }
  }
  return { worst, at, texels, twilight, lit: floor.some((f) => f > 0) };
}

/** The setting's sizes with the sky-view table at a quarter of its width and height. */
function reducedSizes(sizes: TableSizes): TableSizes {
  return {
    ...sizes,
    skyView: {
      widthTexels: sizes.skyView.widthTexels / 4,
      heightTexels: sizes.skyView.heightTexels / 4,
    },
  };
}

/** A frame's sky-view table and ray-march target from each of two atmospheres, read back. */
interface FrameTables {
  readonly skyView: Float32Array;
  readonly rayMarch: Float32Array;
}

/** Draws one frame with each atmosphere in turn and reads its two per-frame outputs back. */
async function readFrameTables(
  engine: RenderEngine,
  atmospheres: readonly HillaireAtmosphere[],
  camera: AtmosphereCamera,
  sun: SunState,
  surfaceM: number | null,
): Promise<FrameTables[]> {
  const tables: FrameTables[] = [];
  for (const atmosphere of atmospheres) {
    // Each atmosphere's frame is read back before the next draws: they share the device.
    // oxlint-disable-next-line no-await-in-loop
    await compositeFrame(engine, atmosphere, camera, sun, surfaceM);
    const { skyView, rayMarch } = atmosphere.frameTables;
    if (rayMarch === null) {
      throw new Error("the ray march drew no target");
    }
    tables.push({
      // oxlint-disable-next-line no-await-in-loop
      skyView: halfTexels(await engine.readTexture(skyView, 0, undefined, "tolerance")),
      // oxlint-disable-next-line no-await-in-loop
      rayMarch: halfTexels(await engine.readTexture(rayMarch, 0, undefined, "tolerance")),
    });
  }
  return tables;
}

/**
 * The agreement check's tolerance for a setting's kernel: on high, the quadrature gate's 2% (5% for
 * twilight rays); on low, the twin's own worst e at low's counts for the kernel, the camera's range
 * and the class (addendum A, 4(b)); each plus {@link F32_MARGIN}.
 */
function toleranceFor(setting: QualitySetting, bound: keyof typeof LOW_TWIN_WORST): Tolerance {
  return (twilight) => {
    let tolerance: number;
    switch (setting) {
      case "high":
        tolerance = twilight ? TWILIGHT_TOLERANCE : MARCH_TOLERANCE;
        break;
      case "low": {
        const worst = LOW_TWIN_WORST[bound];
        tolerance = twilight ? worst.twilight : worst.ordinary;
        break;
      }
    }
    return tolerance + F32_MARGIN;
  };
}

/**
 * The elevation of the sun that sits on the horizon of the limb's tangent point seen straight
 * ahead from a camera `heightM` above the equator, deg: minus the horizon's dip, on the camera's own
 * sphere, as the sky view has it.
 */
function limbSunElevationDeg(heightM: number): number {
  const inputs = atmosphereInputs(equatorCamera(heightM, FRAME_SIZE), WGS84);
  const bottomM = inputs.radiusM - inputs.heightM;
  return -(Math.acos(bottomM / (bottomM + heightM)) * 180) / Math.PI;
}

/** One frame of the agreement check and the kernel it holds. */
interface StepsFrame {
  readonly name: string;
  readonly heightM: number;
  readonly elevationDeg: number;
  readonly surfaceM: number | null;
  readonly kernel: "skyView" | "rayMarch";
}

/**
 * R05.T12.e: the sky view and the ray march at their settings' step counts against the same
 * kernels at 1,024 steps in the same run, on both settings, within the quadrature gate's
 * tolerances plus 1% for f32 (on low, the twin's own worst at low's counts plus 1%).
 *
 * @remarks
 * The sky view is held from 2 m at noon and at sunset, from 5 km at sunset, whose table spans the
 * band between the local and the visible horizon (addendum A), and from 80 km looking at the limb
 * with the sun on its horizon (addendum B); the ray march from 400 km at noon and at the
 * terminator, and from 2 m over a surface 60 km ahead, beyond the aerial-perspective volume. The
 * output is reduced: the sky-view table at a quarter of the setting's width and height, each texel
 * still one ray of the shipped kernel, and the ray march over the 64 × 32 check frame. A converged
 * output with no lit texel fails, since it would compare nothing.
 */
export async function checkAtmosphereSteps(engine: RenderEngine, checks: Checks): Promise<void> {
  const frames: readonly StepsFrame[] = [
    { name: "2 m, noon", heightM: 2, elevationDeg: 60, surfaceM: null, kernel: "skyView" },
    { name: "2 m, sunset", heightM: 2, elevationDeg: 0, surfaceM: null, kernel: "skyView" },
    { name: "5 km, sunset", heightM: 5_000, elevationDeg: 0, surfaceM: null, kernel: "skyView" },
    {
      name: "80 km, the limb with the sun on its horizon",
      heightM: 80_000,
      elevationDeg: limbSunElevationDeg(80_000),
      surfaceM: null,
      kernel: "skyView",
    },
    {
      name: "400 km, noon",
      heightM: 400_000,
      elevationDeg: 60,
      surfaceM: null,
      kernel: "rayMarch",
    },
    {
      name: "400 km, terminator",
      heightM: 400_000,
      elevationDeg: 0,
      surfaceM: null,
      kernel: "rayMarch",
    },
    {
      name: "2 m, a surface 60 km ahead",
      heightM: 2,
      elevationDeg: 30,
      surfaceM: 60_000,
      kernel: "rayMarch",
    },
  ];
  for (const setting of ["high", "low"] as const) {
    const sizes = TABLE_SIZES[setting];
    const reduced = reducedSizes(sizes);
    const shipped = new HillaireAtmosphere(engine, EARTH_REFERENCE, reduced, WGS84);
    const converged = new HillaireAtmosphere(
      engine,
      EARTH_REFERENCE,
      { ...reduced, skyViewSamples: CONVERGED_STEPS, rayMarchSamples: CONVERGED_STEPS },
      WGS84,
    );
    const marchWidthTexels = Math.ceil(FRAME_SIZE.widthPx * sizes.rayMarchScale);
    const marchHeightTexels = Math.ceil(FRAME_SIZE.heightPx * sizes.rayMarchScale);
    try {
      for (const frame of frames) {
        const camera = equatorCamera(frame.heightM, FRAME_SIZE);
        const sun = sunAt(frame.elevationDeg);
        // The harness's checks run in order: each frame is read back before the next draws.
        // oxlint-disable-next-line no-await-in-loop
        const [fast, slow] = await readFrameTables(
          engine,
          [shipped, converged],
          camera,
          sun,
          frame.surfaceM,
        );
        if (fast === undefined || slow === undefined) {
          throw new Error("a frame's tables were not read back");
        }
        // The sky view's bound on low depends on the camera's range (addendum B's high cameras).
        const highCamera = frame.heightM >= HIGH_CAMERA_M;
        const tolerance = toleranceFor(
          setting,
          frame.kernel === "rayMarch" ? "rayMarch" : highCamera ? "skyViewHigh" : "skyView",
        );
        let result: Agreement;
        let kernel: string;
        let steps: number;
        switch (frame.kernel) {
          case "skyView":
            result = agreement(
              fast.skyView,
              slow.skyView,
              reduced.skyView.widthTexels,
              (x, y) => skyViewRay(x, y, reduced.skyView, camera, sun),
              tolerance,
            );
            kernel = "sky view";
            steps = sizes.skyViewSamples;
            break;
          case "rayMarch":
            result = agreement(
              fast.rayMarch,
              slow.rayMarch,
              marchWidthTexels,
              (x, y) =>
                rayMarchRay(x, y, marchWidthTexels, marchHeightTexels, camera, sun, frame.surfaceM),
              tolerance,
            );
            kernel = "ray march";
            steps = sizes.rayMarchSamples;
            break;
        }
        checks.check(
          `R05.T12.e ${setting}: ${frame.name}, the ${kernel} at ${steps} steps agrees with ${CONVERGED_STEPS}`,
          result.lit && result.worst <= 1,
          `worst ${result.at}; ${result.texels} texels, ${result.twilight} twilight; lit ${String(result.lit)}`,
        );
      }
    } finally {
      shipped.dispose();
      converged.dispose();
    }
  }
}

// --- R08.T6.b: the kernels against the CPU twin ---------------------------------------------------

/** The agreement R08.T6.b holds each kernel's readback to: 10⁻³ of its twin's value. */
const TWIN_TOLERANCE = 1e-3;

/**
 * The fraction of a table's largest value below which its values are held to 10⁻³ of that floor
 * rather than of themselves: 10⁻⁶ (R08.T6.b).
 */
const TWIN_FLOOR = 1e-6;

/**
 * rgba16float's spacing below its least normal, 2⁻¹⁴: 2⁻²⁴, so that a stored value there errs by up
 * to half of it whatever the kernel computed. A value is held to at least this, which is 10⁻³ of the
 * least normal: above 2⁻¹⁴ the bound is 10⁻³ alone.
 */
const HALF_SUBNORMAL_SPACING = 2 ** -24;

/**
 * The absorber amount u ÷ U below which a stored amount is held to 10⁻³ of this floor, 10⁻⁵ of U,
 * rather than of itself (R08.T6.c): 10⁻².
 *
 * @remarks
 * A layer's density ramps to 0 over one of its column's levels, about 115 m, so a slant path that
 * clips a ramp takes its whole small amount from a few steps whose f32 heights stand some 0.1 m
 * from the twin's: 4 × 10⁻⁴ of U was 1.2 × 10⁻³ off in the first run, and 1.2 × 10⁻³ of U
 * 3.9 × 10⁻³. A curve of growth is concave, so an amount's error moves its optical depth by at most
 * the linear regime's: 10⁻⁵ of U moves it by at most 10⁻⁵ of the absorber's linear vertical depth.
 */
const ABSORBER_AMOUNT_FLOOR = 1e-2;

/** One kernel's readback against its twin. */
interface TwinAgreement {
  /** The worst |ΔL| as a fraction of what its texel's channel is allowed: at most 1 to agree. */
  readonly worst: number;
  readonly at: string;
  /** The texels' channels held above the floor. */
  readonly above: number;
  /** The texels' channels over their bound and outside any envelope. */
  readonly over: number;
  /** The twin's largest colour value. */
  readonly largest: number;
  /** The channels the half float's floor of 2⁻²⁴ carries: over 10⁻³ of themselves, within it. */
  readonly floorCarried: number;
  /** The channels compared. */
  readonly channels: number;
  /** The channels that passed by the twin's envelope (`decision-r08-f32.md` §10.3), recorded. */
  readonly envelope: readonly string[];
  /** The worst envelope channel's distance from its envelope's middle, of its half-width. */
  readonly worstInEnvelope: number;
}

/** How a readback is held (R08.T6.g, `decision-r08-t6b-floor.md` §4, `decision-r08-f32.md` §10). */
interface TwinOptions {
  /**
   * `stored`: an `rgba16float` table, with the format's floor of 2⁻²⁴; `kernel`: a kernel's own
   * `rgba32float` output (the kernel arm), with none.
   */
  readonly arm: "stored" | "kernel";
  /** The channels that hold absorber amounts u ÷ U, floored at {@link ABSORBER_AMOUNT_FLOOR}. */
  readonly amountChannels?: ReadonlyArray<number>;
  /**
   * A texel's twin envelope, asked only of a texel with a channel over its bound, where the view
   * is from above the atmosphere's top; absent elsewhere, so that the bound holds outright.
   */
  readonly envelope?: (texel: number) => TexelEnvelope | null;
}

/** One channel's comparison: the twin's value, the GPU's, their distance, the floor, the bound. */
interface ChannelCheck {
  readonly c: number;
  readonly expected: number;
  readonly actual: number;
  readonly delta: number;
  readonly floor: number;
  readonly ratio: number;
}

/**
 * A readback against its twin, texel for texel: each channel's |GPU − twin| against
 * max(10⁻³ · max(|twin|, 10⁻⁶ · largest), 2⁻²⁴) for a stored table, and without the 2⁻²⁴ for a
 * kernel's own output, the largest taken over the colour channels, and over alpha apart when
 * `alpha` holds a quantity of its own (a mean transmittance, or an absorber amount). The channels
 * `options.amountChannels` names hold absorber amounts, floored at {@link ABSORBER_AMOUNT_FLOOR}
 * of their own unit as well.
 *
 * @remarks
 * Where `options.envelope` gives a texel's envelope, a channel over its bound passes by it when the
 * twin itself moves by more than 10⁻³ over the kernel's resolution there and the GPU's value lies
 * within the twin's range widened by 10⁻³ (`decision-r08-f32.md` §10.3); it is recorded with its
 * row, column, tangent height and spread.
 */
function twinAgreement(
  gpu: Float32Array,
  twin: TwinTable,
  alpha: boolean,
  options: TwinOptions = { arm: "stored" },
): TwinAgreement {
  const texels = twin.texels;
  const channels = alpha ? [0, 1, 2, 3] : [0, 1, 2];
  const largest = (group: readonly number[]): number => {
    let most = 0;
    for (let i = 0; i < texels.length; i += 4) {
      for (const c of group) {
        most = Math.max(most, Math.abs(texels[i + c] ?? 0));
      }
    }
    return most;
  };
  const colourLargest = largest([0, 1, 2]);
  const colourFloor = TWIN_FLOOR * colourLargest;
  const alphaFloor = TWIN_FLOOR * largest([3]);
  const halfFloor = options.arm === "stored" ? HALF_SUBNORMAL_SPACING : 0;
  const amountChannels = options.amountChannels ?? [];
  const perSlice = twin.widthTexels * twin.heightTexels;
  const count = texels.length / 4;
  let worst = gpu.length === texels.length ? 0 : Number.POSITIVE_INFINITY;
  let at = worst === 0 ? "none" : `${gpu.length / 4} texels read back, ${count} twinned`;
  let above = 0;
  let over = 0;
  let floorCarried = 0;
  let worstInEnvelope = 0;
  const envelope: string[] = [];
  const where = (i: number): string => {
    const x = i % twin.widthTexels;
    const y = Math.floor(i / twin.widthTexels) % twin.heightTexels;
    const z = Math.floor(i / perSlice);
    return perSlice === count ? `(${x}, ${y})` : `(${x}, ${y}, ${z})`;
  };
  for (let i = 0; i < count && gpu.length === texels.length; i += 1) {
    const checked: ChannelCheck[] = channels.map((c) => {
      const expected = texels[i * 4 + c] ?? Number.NaN;
      const actual = gpu[i * 4 + c] ?? Number.NaN;
      const tableFloor = c === 3 ? alphaFloor : colourFloor;
      const floor = amountChannels.includes(c)
        ? Math.max(tableFloor, ABSORBER_AMOUNT_FLOOR)
        : tableFloor;
      const relative = TWIN_TOLERANCE * Math.max(Math.abs(expected), floor);
      const allowed = Math.max(relative, halfFloor);
      const delta = Math.abs(actual - expected);
      floorCarried += delta > relative && delta <= allowed ? 1 : 0;
      above += Math.abs(expected) >= floor ? 1 : 0;
      const ratio = Number.isNaN(delta) ? Number.POSITIVE_INFINITY : delta / allowed;
      return { c, expected, actual, delta, floor, ratio };
    });
    const texelEnvelope = checked.some(({ ratio }) => ratio > 1)
      ? (options.envelope?.(i) ?? null)
      : null;
    for (const { c, expected, actual, delta, floor, ratio } of checked) {
      if (ratio > 1 && texelEnvelope !== null) {
        const tolerance = TWIN_TOLERANCE * Math.max(Math.abs(expected), floor);
        const low = texelEnvelope.low[c] ?? Number.NaN;
        const high = texelEnvelope.high[c] ?? Number.NaN;
        const illConditioned = Math.max(high - expected, expected - low) > tolerance;
        if (illConditioned && actual >= low - tolerance && actual <= high + tolerance) {
          const half = (high - low) / 2;
          worstInEnvelope = Math.max(
            worstInEnvelope,
            Math.abs(actual - (low + high) / 2) / (half + tolerance),
          );
          envelope.push(
            `${where(i)} c${c} at ${(texelEnvelope.tangentHeightM / 1_000).toFixed(2)} km, spread ${(((high - low) / Math.max(Math.abs(expected), floor)) * 1e3).toFixed(2)} × 10⁻³`,
          );
          continue;
        }
      }
      over += ratio > 1 ? 1 : 0;
      if (ratio > worst) {
        worst = ratio;
        at = `texel ${where(i)} channel ${c}: GPU ${show([actual])}, twin ${show([expected])}, ${((delta / Math.max(Math.abs(expected), floor)) * 1e3).toFixed(3)} × 10⁻³`;
      }
    }
  }
  return {
    worst,
    at,
    above,
    over,
    largest: colourLargest,
    floorCarried,
    channels: count * channels.length,
    envelope,
    worstInEnvelope,
  };
}

/** A readback as a twin's table: its texels in `f64`. */
function asTwinTable(texels: Float32Array, size: TableSize): TwinTable {
  return { ...size, texels: Float64Array.from(texels) };
}

/** Holds one kernel's readback to its twin's table and records the check. */
function checkTwin(
  checks: Checks,
  name: string,
  gpu: Float32Array,
  twin: TwinTable,
  alpha: boolean,
  options: TwinOptions = { arm: "stored" },
): void {
  recordAgreement(checks, name, twinAgreement(gpu, twin, alpha, options));
}

/** A Henyey–Greenstein phase of asymmetry g at each u = √(θ ÷ π) of a grid, sr⁻¹. */
function henyeyGreensteinAt(g: number, u: Float64Array): Float64Array {
  return u.map((x) => {
    const mu = Math.cos(Math.PI * x * x);
    return (1 - g * g) / (4 * Math.PI * (1 + g * g - 2 * g * mu) ** 1.5);
  });
}

/**
 * A medium of every profile and phase the kernels read (R08.T6.b): R08.T3.a's column of an
 * isothermal Earth-like gas under Rayleigh's phase, with a depolarisation ratio a channel; a haze
 * layer from 1 to 6 km with a tabulated phase on the kernels' own grid, a different asymmetry a
 * channel; a dust from the ground, tabulated, its phase on another grid (181 entries,
 * u = (i ÷ 180)^1.3), which the kernels read resampled; R05's Cornette–Shanks aerosol; and R05's
 * ozone, an absorbing tent. The coefficients are a test's, not a world's.
 */
function smokeTabulatedMedium(): AtmosphereMedium {
  const column = hydrostaticColumn({
    surfacePa: 101_325,
    temperature: { kind: "isothermal", temperatureK: 250 },
    meanMolarMassGPerMol: 28.97,
    referenceGravityMS2: 9.806,
    referenceRadiusM: EARTH_MEAN_RADIUS_M,
  });
  const even = Float64Array.from({ length: PHASE_TABLE_ENTRIES }, (_, i) => i / 255);
  const uneven = Float64Array.from({ length: 181 }, (_, i) => (i / 180) ** 1.3);
  const dustLevels = Float64Array.from({ length: 201 }, (_, i) => i * 50);
  const [, aerosol, ozone] = EARTH_REFERENCE.terms;
  if (aerosol === undefined || ozone === undefined) {
    throw new Error("Earth's medium has its aerosol and ozone");
  }
  return {
    name: "R08.T6.b tabulated",
    topHeightM: EARTH_REFERENCE.topHeightM,
    groundAlbedo: [0.25, 0.18, 0.12],
    terms: [
      {
        name: "gas",
        density: column.density,
        scattering: [4.8e-6, 1.15e-5, 2.87e-5],
        absorption: [0, 0, 0],
        // A depolarisation ratio of air's order, a different one a channel (R08.T3.c).
        phase: { kind: "rayleigh", depolarisation: [0.027, 0.028, 0.03] },
      },
      {
        name: "haze",
        density: tabulatedDensity(Float64Array.of(1_000, 2_000, 6_000), Float64Array.of(0, 1, 0)),
        scattering: [2e-5, 2.4e-5, 2.8e-5],
        absorption: [2e-6, 1.5e-6, 1e-6],
        phase: {
          kind: "tabulated",
          table: phaseTable(even, [
            henyeyGreensteinAt(0.75, even),
            henyeyGreensteinAt(0.7, even),
            henyeyGreensteinAt(0.65, even),
          ]),
        },
      },
      {
        name: "dust",
        density: tabulatedDensity(
          dustLevels,
          dustLevels.map((h) => Math.exp(-h / 1_500) - Math.exp(-10_000 / 1_500)),
        ),
        scattering: [8e-6, 7e-6, 6e-6],
        absorption: [1e-6, 1.5e-6, 2.5e-6],
        phase: {
          kind: "tabulated",
          table: phaseTable(uneven, [
            henyeyGreensteinAt(0.6, uneven),
            henyeyGreensteinAt(0.6, uneven),
            henyeyGreensteinAt(0.6, uneven),
          ]),
        },
      },
      aerosol,
      ozone,
    ],
  };
}

/** One frame R08.T6.b's per-frame check draws. */
interface TwinFrame {
  readonly name: string;
  readonly heightM: number;
  readonly elevationDeg: number;
  readonly surfaceM: number | null;
}

/**
 * R08.T6.b: every kernel's readback against the CPU twin (`tablesCpu.ts`) of the medium as the
 * kernels read it (`kernelMedium`), to 10⁻³ above 10⁻⁶ of the table's largest value, on R05's
 * Earth and on a medium of every profile and phase the kernels read; since R08.T6.c also on a
 * medium of four absorbers, one a molecule pair, under their curves of growth, and, for the
 * transmittance tables alone, on a Venus-class medium.
 *
 * @remarks
 * The transmittance table, its optical depth and absorber columns (R08.T6.c), is held to the twin
 * of the medium alone, as is the absorber-column table beside it. The Venus-class medium's
 * multiple-scattering table is not held: in its deep, nearly conservative air f_ms nears 1, where
 * Hillaire's series L₂ ÷ (1 − f_ms) diverges and `f32`'s 1 − f_ms rounds to 0 (0 ÷ 0 where the
 * sun is down), the thick worlds' source-function table's case (plan R08's Risks, "Deviations in
 * T6.c, as built"). Each later kernel is held to
 * its twin given the GPU's own tables as its inputs (the transmittance for the multiple
 * scattering, both for the per-frame kernels), so that each is judged on its own arithmetic and
 * not on the `rgba16float` rounding of what it reads; the multiple-scattering table's agreement
 * with the twin's own tables is reported beside it. The per-frame kernels run on both settings'
 * counts, at half the sky view's width and height (each texel still one ray, {@link twinSizes}),
 * over two frames: from 2 km at a sun 20° up with a surface 60 km ahead (the ray march's beyond the
 * aerial reach), and from 400 km with the sun 5° up (the limb). The camera stands at 2 km, as
 * R08.T6.b left it: R08.T6.g's group (`checkAtmosphereHeights`) holds the kernels from 1 m.
 */
export async function checkAtmosphereTwins(engine: RenderEngine, checks: Checks): Promise<void> {
  const absorbing = fourAbsorberMedium();
  const venus = venusClassMedium();
  const media: ReadonlyArray<TwinMedium> = [
    { ...EARTH_ONLY, scope: "everything" },
    {
      medium: smokeTabulatedMedium(),
      curves: NO_ABSORBER_CURVES,
      radiusM: EARTH_MEAN_RADIUS_M,
      scope: "everything",
    },
    { ...absorbing, radiusM: EARTH_MEAN_RADIUS_M, scope: "everything" },
    {
      medium: venus.medium,
      curves: NO_ABSORBER_CURVES,
      radiusM: venus.radiusM,
      scope: "transmittance",
    },
  ];
  for (const entry of media) {
    const read = kernelMedium(entry.medium);
    // The harness's checks run in order: each medium's tables are read back before the next's.
    // oxlint-disable-next-line no-await-in-loop
    await checkPlanetTwins(engine, checks, entry, read);
    if (entry.scope === "transmittance") {
      continue;
    }
    for (const setting of ["high", "low"] as const) {
      // Each setting's frames are read back before the next setting's atmosphere draws.
      // oxlint-disable-next-line no-await-in-loop
      await checkFrameTwins(engine, checks, entry, read, setting);
    }
  }
}

/**
 * A medium R08.T6.b's and T6.c's checks build tables for: with its lights' curves of growth, the
 * radius of its tables' ground, and what is held: `everything`, its multiple-scattering table and
 * its per-frame kernels too, or `transmittance`, its transmittance tables alone.
 */
interface TwinMedium {
  readonly medium: AtmosphereMedium;
  readonly curves: AbsorberCurveSlots;
  readonly radiusM: number;
  readonly scope: "everything" | "transmittance";
}

/** R05's Earth, on WGS 84's mean radius, with no absorber. */
const EARTH_ONLY = {
  medium: EARTH_REFERENCE,
  curves: NO_ABSORBER_CURVES,
  radiusM: EARTH_MEAN_RADIUS_M,
} as const;

/** A planet's per-planet tables, read back. */
interface PlanetReadback {
  /** The optical depths in rgb and absorber slot 0's column in alpha (R08.T6.c). */
  readonly transmittance: Float32Array;
  /** Absorber slots 1 to 4's columns (R08.T6.c). */
  readonly absorberColumns: Float32Array;
  readonly multiScattering: Float32Array;
}

/** Reads a planet's tables back with the harness's tolerance access. */
async function readPlanetTables(
  engine: RenderEngine,
  tables: AtmosphereTables,
): Promise<PlanetReadback> {
  const read = async (texture: TextureHandle): Promise<Float32Array> =>
    halfTexels(await engine.readTexture(texture, 0, undefined, "tolerance"));
  return {
    transmittance: await read(tables.transmittance),
    absorberColumns: await read(tables.absorberColumns),
    multiScattering: await read(tables.multiScattering),
  };
}

/** The GPU's stored transmittance tables as the twins read them, with the lights' curves. */
function storedTransmittance(gpu: PlanetReadback, curves: AbsorberCurveSlots): TwinTransmittance {
  return {
    transmittance: asTwinTable(gpu.transmittance, TRANSMITTANCE_SIZE),
    absorberColumns: asTwinTable(gpu.absorberColumns, TRANSMITTANCE_SIZE),
    curves,
  };
}

/**
 * The optical depth above which the stored transmittance fell under rgba16float's least normal,
 * 2⁻¹⁴: −ln 2⁻¹⁴ = 9.70, the channels the half float's floor carried while T was stored
 * (`decision-r08-t6b-floor.md` §5).
 */
const SUBNORMAL_TRANSMITTANCE_DEPTH = 14 * Math.LN2;

/** How the channels whose twin T was under 2⁻¹⁴ are held, now that the table stores τ. */
interface DeepChannels {
  readonly count: number;
  /** The worst |Δτ| ÷ τ among them. */
  readonly worst: number;
  readonly at: string;
}

/** The transmittance table's channels whose twin τ exceeds {@link SUBNORMAL_TRANSMITTANCE_DEPTH}. */
function deepChannels(gpu: Float32Array, twin: TwinTable): DeepChannels {
  let count = 0;
  let worst = 0;
  let at = "none";
  for (let i = 0; i < twin.texels.length; i += 4) {
    for (const c of [0, 1, 2]) {
      const tau = twin.texels[i + c] ?? Number.NaN;
      if (tau > SUBNORMAL_TRANSMITTANCE_DEPTH) {
        count += 1;
        const e = Math.abs((gpu[i + c] ?? Number.NaN) - tau) / tau;
        if (!(e <= worst)) {
          worst = e;
          const t = i / 4;
          at = `(${t % twin.widthTexels}, ${Math.floor(t / twin.widthTexels)}) c${c} at τ ${tau.toPrecision(4)}`;
        }
      }
    }
  }
  return { count, worst, at };
}

/**
 * The per-planet tables against the twin: the transmittance, stored as optical depth since
 * R08.T6.c, and the absorber columns against the twin of the medium; the multiple scattering
 * against its twin over the GPU's transmittance tables, with its agreement with the twin's own
 * tables reported beside it. The channels whose twin T is under 2⁻¹⁴ are reported with their worst
 * |Δτ| ÷ τ (`decision-r08-t6b-floor.md` §5); on R05's Earth there are some.
 */
async function checkPlanetTwins(
  engine: RenderEngine,
  checks: Checks,
  entry: TwinMedium,
  read: AtmosphereMedium,
): Promise<void> {
  const { medium, curves, radiusM } = entry;
  const tables = new AtmosphereTables(engine, medium, radiusM, curves);
  try {
    const gpu = await readPlanetTables(engine, tables);
    const twin = planetTwin(read, radiusM, curves);
    const absorbers = absorberCount(medium) > 0;
    checkTwin(
      checks,
      `${medium.name}: the transmittance table, as optical depth${absorbers ? " and absorber slot 0's column" : ""},`,
      gpu.transmittance,
      twin.transmittance,
      absorbers,
      { arm: "stored", amountChannels: [3] },
    );
    if (absorberCount(medium) > 1) {
      checkTwin(
        checks,
        `${medium.name}: the absorber-column table`,
        gpu.absorberColumns,
        twin.absorberColumns,
        true,
        { arm: "stored", amountChannels: [0, 1, 2, 3] },
      );
    }
    const deep = deepChannels(gpu.transmittance, twin.transmittance);
    checks.check(
      `R08.T6.c ${medium.name}: the channels whose transmittance was under 2⁻¹⁴ hold their optical depth to 10⁻³`,
      deep.worst <= TWIN_TOLERANCE && (medium !== EARTH_REFERENCE || deep.count > 0),
      `${deep.count} channels with twin τ above ${SUBNORMAL_TRANSMITTANCE_DEPTH.toFixed(2)}; worst |Δτ| ÷ τ ${deep.worst.toExponential(2)} at ${deep.at}`,
    );
    if (entry.scope === "transmittance") {
      return;
    }
    const overGpu = multiScatteringTwin(read, radiusM, storedTransmittance(gpu, curves));
    const own = twinAgreement(gpu.multiScattering, overGpu, false);
    const endToEnd = twinAgreement(gpu.multiScattering, twin.multiScattering, false);
    checks.check(
      `R08.T6.b ${medium.name}: the multiple-scattering table agrees with the CPU twin to 10⁻³`,
      own.worst <= 1 && own.above > 0,
      `over the GPU's transmittance, worst ${own.at}; ${own.above} channels above 10⁻⁶ of the largest, ${own.over} over the bound; over the twin's, worst ${endToEnd.at}, ${endToEnd.over} over`,
    );
  } finally {
    tables.dispose();
  }
}

/** A frame's three per-frame outputs, read back. */
interface FrameReadback {
  readonly skyView: Float32Array;
  readonly aerial: Float32Array;
  readonly rayMarch: Float32Array;
}

/** Reads the latest frame's sky view, aerial volume and ray march back. */
async function readFrameOutputs(
  engine: RenderEngine,
  atmosphere: HillaireAtmosphere,
): Promise<FrameReadback> {
  const { skyView, aerial, rayMarch } = atmosphere.frameTables;
  if (rayMarch === null) {
    throw new Error("the ray march drew no target");
  }
  const read = async (texture: TextureHandle): Promise<Float32Array> =>
    halfTexels(await engine.readTexture(texture, 0, undefined, "tolerance"));
  return {
    skyView: await read(skyView),
    aerial: await read(aerial),
    rayMarch: await read(rayMarch),
  };
}

/**
 * The setting's sizes with the sky-view table at half its width and height, each texel still one
 * ray of the shipped kernel: 96 × 54 on high and 64 × 32 on low. Both row counts are even, so that
 * no texel's ray is the horizon's own tangent (v = ½ in `view.wgsl`'s `skyViewUvToParams`), where
 * a ray meets the ground or misses it by its arithmetic's rounding; R05.T12.e's quarter, 48 × 27 on
 * high, has one (row 13).
 */
function twinSizes(sizes: TableSizes): TableSizes {
  return {
    ...sizes,
    skyView: {
      widthTexels: sizes.skyView.widthTexels / 2,
      heightTexels: sizes.skyView.heightTexels / 2,
    },
  };
}

/**
 * The three per-frame kernels' agreements with their twins over one frame, and whether its
 * composite, which reads the tables at the camera (`composite.wgsl`'s `transmittanceToSpace`) and
 * has no twin, came out finite and within `rgba16float`'s range.
 */
interface FrameAgreements {
  readonly skyView: TwinAgreement;
  readonly aerial: TwinAgreement;
  readonly rayMarch: TwinAgreement;
  readonly compositeWithinRange: boolean;
}

/** The per-planet tables an atmosphere built, read back, as the twins' planet for `read`. */
async function planetOf(
  engine: RenderEngine,
  atmosphere: HillaireAtmosphere,
  read: AtmosphereMedium,
  figure: SpheroidFigure,
): Promise<PlanetTwin> {
  const tables = await readPlanetTables(engine, atmosphere.tables);
  return {
    medium: read,
    bottomRadiusM: tableRadiusM(figure),
    ...storedTransmittance(tables, atmosphere.tables.curves),
    multiScattering: asTwinTable(tables.multiScattering, MULTI_SCATTERING_SIZE),
  };
}

/** Draws one frame and holds its per-frame outputs to their twins over the GPU's tables. */
async function frameAgreements(
  engine: RenderEngine,
  atmosphere: HillaireAtmosphere,
  planet: PlanetTwin,
  figure: SpheroidFigure,
  sizes: TableSizes,
  frame: TwinFrame,
): Promise<FrameAgreements> {
  const camera = equatorCamera(frame.heightM, FRAME_SIZE, figure);
  const sun = sunAt(frame.elevationDeg);
  const composite = await compositeFrame(engine, atmosphere, camera, sun, frame.surfaceM);
  const gpu = await readFrameOutputs(engine, atmosphere);
  const view = twinView(camera, sun, figure, NEAR_M);
  const aerial = aerialPerspectiveTwin(
    planet,
    view,
    sizes.aerialPerspective,
    sizes.aerialPerspectiveSamplesPerSlice,
    sizes.aerialPerspectiveReachM,
  );
  const surfaceDepth = frame.surfaceM === null ? 0 : NEAR_M / frame.surfaceM;
  return {
    skyView: twinAgreement(
      gpu.skyView,
      skyViewTwin(planet, view, sizes.skyView, sizes.skyViewSamples),
      false,
    ),
    aerial: twinAgreement(gpu.aerial, aerial, true),
    rayMarch: twinAgreement(
      gpu.rayMarch,
      rayMarchTwin(planet, view, sizes, () => surfaceDepth),
      true,
    ),
    compositeWithinRange: withinHalfRange(composite),
  };
}

/** Records an agreement as the check "R08.T6.b … agrees with the CPU twin to 10⁻³". */
function recordAgreement(checks: Checks, name: string, result: TwinAgreement): void {
  checks.check(
    `R08.T6.b ${name} agrees with the CPU twin to 10⁻³`,
    result.worst <= 1 && result.above > 0,
    `worst ${result.at}; ${result.above} channels above 10⁻⁶ of the largest, ${result.over} over the bound`,
  );
}

/**
 * The per-frame kernels on a setting's counts against their twins over the GPU's tables: from
 * 2 km at a sun 20° up with a surface 60 km ahead (the ray march's, beyond the aerial reach), and
 * from 400 km with the sun 5° up (the limb).
 */
async function checkFrameTwins(
  engine: RenderEngine,
  checks: Checks,
  entry: TwinMedium,
  read: AtmosphereMedium,
  setting: QualitySetting,
): Promise<void> {
  const frames: readonly TwinFrame[] = [
    { name: "2 km, a surface 60 km ahead", heightM: 2_000, elevationDeg: 20, surfaceM: 60_000 },
    { name: "400 km, the limb", heightM: 400_000, elevationDeg: 5, surfaceM: null },
  ];
  const medium = entry.medium;
  const sizes = twinSizes(TABLE_SIZES[setting]);
  const atmosphere = new HillaireAtmosphere(engine, medium, sizes, WGS84, entry.curves);
  try {
    const planet = await planetOf(engine, atmosphere, read, WGS84);
    for (const frame of frames) {
      // The harness's checks run in order: each frame is read back before the next draws.
      // oxlint-disable-next-line no-await-in-loop
      const result = await frameAgreements(engine, atmosphere, planet, WGS84, sizes, frame);
      const label = `${medium.name}, ${setting}, ${frame.name}`;
      recordAgreement(checks, `${label}: the sky view`, result.skyView);
      recordAgreement(checks, `${label}: the aerial perspective`, result.aerial);
      recordAgreement(checks, `${label}: the ray march`, result.rayMarch);
      checks.check(
        `R08.T6.c ${label}: the composite, its read of the tables at the camera, is finite and within rgba16float's range`,
        result.compositeWithinRange,
        result.compositeWithinRange ? "every channel" : "a channel is not",
      );
    }
  } finally {
    atmosphere.dispose();
  }
}

// --- R08.T6.g: heights and angles in f32 ---------------------------------------------------------

/** A sphere of radius `radiusM` as a figure. */
function sphereFigure(radiusM: number): SpheroidFigure {
  return { equatorialRadiusM: radiusM, polarRadiusM: radiusM };
}

/** One body of R08.T6.g's checks: its figure, its cameras' heights and its limb cameras. */
interface HeightsBody {
  readonly name: string;
  readonly figure: SpheroidFigure;
  /** The heights of the views on high, m: R08.T6.g's 2 m to 2 km, and 1 m on the 2 R⊕ body. */
  readonly heightsM: readonly number[];
  /** The limb cameras' distances from the centre, m. */
  readonly limbsM: readonly number[];
}

/**
 * `decision-r08-f32.md` §6.2's bodies, each with Earth's medium, whose aerosol, 1.2 km in scale
 * height, is the most sensitive profile the kernels hold: Earth (WGS 84), a 2 R⊕ sphere
 * (12,742,017.6 m, the largest solid body the generator makes) and a sphere of Saturn's
 * equatorial radius (60,268,000 m).
 */
const HEIGHTS_BODIES: readonly HeightsBody[] = [
  {
    name: "Earth",
    figure: WGS84,
    heightsM: [2, 20, 200, 2_000],
    limbsM: [WGS84.equatorialRadiusM + 400_000, 4.2e7],
  },
  {
    name: "the 2 R⊕ sphere",
    figure: sphereFigure(12_742_017.6),
    heightsM: [1, 2, 20, 200, 2_000],
    limbsM: [],
  },
  {
    name: "the Saturn-radius sphere",
    figure: sphereFigure(60_268_000),
    heightsM: [2, 20, 200, 2_000],
    limbsM: [60_268_000 + 400_000],
  },
];

/** The low setting's spot checks' heights, m. */
const LOW_HEIGHTS_M = [2, 2_000] as const;

/**
 * The largest colour value under which a stored table lies below what `rgba16float` can show at
 * the metric's floor: 2⁻²⁴ ÷ F, F = 3 × 10⁻⁴ (plan R08, Verification's twin bound).
 */
const FORMAT_LIMITED = 2 ** -24 / 3e-4;

/**
 * The kernel's own resolution of a ray (`decision-r08-f32.md` §10.2): `rayThrough`'s direction to
 * 4ε = 2⁻²² rad (about 2.4 × 10⁻⁷), through `normalize`'s `inverseSqrt`; the sky view's texel
 * direction to 5 × 10⁻⁷ rad, the precise forms' bound; and the ray march's offset from the body's
 * centre to 2ε|P|, through q_b's `f32` dot product with a vector of length |P|.
 */
const RAY_RESOLUTION_RAD = 2 ** -22;
const SKY_VIEW_RESOLUTION_RAD = 5e-7;
const ORIGIN_RESOLUTION = 2 ** -23;

/**
 * The most of a check's channels that may pass by the twin's envelope (`decision-r08-f32.md`
 * §10.3): 1%. Over it, the check fails.
 */
const ENVELOPE_SHARE_CAP = 0.01;

/**
 * A texel's twin envelope (`decision-r08-f32.md` §10.3): per channel, the least and greatest of
 * the twin's values along its nominal ray and along the rays the kernel cannot tell from it
 * (turned by the kernel's resolution about two axes across it and, in the ray march, moved by
 * 2ε|P|), and the nominal ray's tangent height above the ground's sphere or spheroid.
 */
interface TexelEnvelope {
  readonly low: readonly number[];
  readonly high: readonly number[];
  readonly tangentHeightM: number;
}

/** Each channel's least and greatest over a set of values, RGBA or RGB. */
function rangeOf(
  values: ReadonlyArray<ArrayLike<number>>,
  channels: number,
): {
  readonly low: number[];
  readonly high: number[];
} {
  const low = Array.from({ length: channels }, () => Number.POSITIVE_INFINITY);
  const high = Array.from({ length: channels }, () => Number.NEGATIVE_INFINITY);
  for (const value of values) {
    for (let c = 0; c < channels; c += 1) {
      const v = value[c] ?? Number.NaN;
      low[c] = Math.min(low[c] ?? v, v);
      high[c] = Math.max(high[c] ?? v, v);
    }
  }
  return { low, high };
}

/** Two unit vectors across a unit direction, and with it a right-handed frame. */
function across(d: Vec3): readonly [Vec3, Vec3] {
  const helper = Math.abs(d.z) < 0.9 ? { x: 0, y: 0, z: 1 } : { x: 1, y: 0, z: 0 };
  const e1 = normalise({
    x: d.y * helper.z - d.z * helper.y,
    y: d.z * helper.x - d.x * helper.z,
    z: d.x * helper.y - d.y * helper.x,
  });
  const e2 = {
    x: d.y * e1.z - d.z * e1.y,
    y: d.z * e1.x - d.x * e1.z,
    z: d.x * e1.y - d.y * e1.x,
  };
  return [e1, e2];
}

/** `d` turned by `angleRad` towards `e`, a unit vector across it. */
function turned(d: Vec3, e: Vec3, angleRad: number): Vec3 {
  const cos = Math.cos(angleRad);
  const sin = Math.sin(angleRad);
  return normalise({
    x: d.x * cos + e.x * sin,
    y: d.y * cos + e.y * sin,
    z: d.z * cos + e.z * sin,
  });
}

/** `provide`, answered once a texel. */
function memoised<T>(provide: (index: number) => T): (index: number) => T {
  const answers = new Map<number, { readonly value: T }>();
  return (index) => {
    const known = answers.get(index);
    if (known !== undefined) {
      return known.value;
    }
    const value = provide(index);
    answers.set(index, { value });
    return value;
  };
}

/**
 * A ray-march texel's envelope, or `null` where the kernel marches nothing. The texel's ray, turned
 * by {@link RAY_RESOLUTION_RAD} about two axes across it, and moved by `ORIGIN_RESOLUTION` |P|
 * along them, is clipped and marched again in the twin: eight rays beside the nominal.
 */
function rayMarchEnvelope(
  planet: PlanetTwin,
  view: TwinView,
  sizes: TableSizes,
  twin: TwinTable,
): (index: number) => TexelEnvelope | null {
  const { widthTexels: w, heightTexels: h } = twin;
  const medium = planet.medium;
  const figure = view.figure;
  const p = view.cameraM;
  const offsetM = ORIGIN_RESOLUTION * Math.hypot(p.x, p.y, p.z);
  return memoised((index) => {
    const nominal = rayMarchRayTwin(
      medium,
      view,
      ((index % w) + 0.5) / w,
      (Math.floor(index / w) + 0.5) / h,
      0,
      sizes.aerialPerspectiveReachM,
    );
    if (nominal === null) {
      return null;
    }
    const d = nominal.ray.direction;
    const values: Array<ArrayLike<number>> = [twin.texels.subarray(index * 4, index * 4 + 4)];
    for (const e of across(d)) {
      for (const sign of [1, -1]) {
        const turnedRay = rayMarchRayAlongTwin(
          medium,
          view,
          p,
          turned(d, e, sign * RAY_RESOLUTION_RAD),
          Number.POSITIVE_INFINITY,
        );
        values.push(rayMarchTexelTwin(planet, view, sizes, turnedRay));
        const moved = {
          x: p.x + e.x * sign * offsetM,
          y: p.y + e.y * sign * offsetM,
          z: p.z + e.z * sign * offsetM,
        };
        values.push(
          rayMarchTexelTwin(
            planet,
            view,
            sizes,
            rayMarchRayAlongTwin(medium, view, moved, d, Number.POSITIVE_INFINITY),
          ),
        );
      }
    }
    // The tangent height: the ray's closest approach to the centre in the space scaled to a sphere
    // of radius a, less a.
    const k = figure.equatorialRadiusM / figure.polarRadiusM;
    const sp = { x: p.x, y: p.y, z: p.z * k };
    const sd = { x: d.x, y: d.y, z: d.z * k };
    const cross = {
      x: sp.y * sd.z - sp.z * sd.y,
      y: sp.z * sd.x - sp.x * sd.z,
      z: sp.x * sd.y - sp.y * sd.x,
    };
    const closest = Math.hypot(cross.x, cross.y, cross.z) / Math.hypot(sd.x, sd.y, sd.z);
    return { ...rangeOf(values, 4), tangentHeightM: closest - figure.equatorialRadiusM };
  });
}

/**
 * A sky-view texel's envelope: its ray turned by {@link SKY_VIEW_RESOLUTION_RAD} in zenith and in
 * azimuth, marched again in the twin on the camera's sphere: four rays beside the nominal.
 */
function skyViewEnvelope(
  planet: PlanetTwin,
  view: TwinView,
  sizes: TableSizes,
  twin: TwinTable,
): (index: number) => TexelEnvelope {
  const medium = planet.medium;
  const sources = tableSources(planet);
  const bottomM = view.cameraRadiusM;
  const shell = { bottomRadiusM: bottomM, topRadiusM: bottomM + medium.topHeightM };
  const samples = Math.max(Math.floor(sizes.skyViewSamples), 2);
  const w = twin.widthTexels;
  return memoised((index) => {
    const ray = skyViewRayTwin(medium, view, index % w, Math.floor(index / w), sizes.skyView);
    const d = ray.direction;
    const values: Array<ArrayLike<number>> = [twin.texels.subarray(index * 4, index * 4 + 3)];
    for (const e of across(d)) {
      for (const sign of [1, -1]) {
        const direction = turned(d, e, sign * SKY_VIEW_RESOLUTION_RAD);
        const again = {
          ...ray,
          direction,
          tEndM: maxDistanceM(shell, ray.originM.z, direction.z),
        };
        values.push(
          marchTwin(
            medium,
            sources,
            { kind: "sphere", bottomRadiusM: bottomM },
            again,
            placedSteps(again, samples),
          ).luminance,
        );
      }
    }
    // The tangent height on the camera's sphere: r sin(zenith) less its radius.
    const r = ray.originM.z;
    return { ...rangeOf(values, 3), tangentHeightM: r * Math.hypot(d.x, d.y) - bottomM };
  });
}

/** The tangent height a limb camera's frame is centred on, and its half-span about it, m. */
const LIMB_CENTRE_M = 50_000;
const LIMB_HALF_SPAN_M = 150_000;

/**
 * A camera `distanceM` from the centre on the equator, looking along the equatorial limb at the
 * tangent height {@link LIMB_CENTRE_M}, its frame spanning ±{@link LIMB_HALF_SPAN_M} of tangent
 * height, so that the air and the space beyond it fill rows of the 64 × 32 frame. Nothing is drawn
 * in front of it, so the scene's own projection is not read.
 */
function limbCamera(distanceM: number, figure: SpheroidFigure): AtmosphereCamera {
  const tangentM = figure.equatorialRadiusM + LIMB_CENTRE_M;
  const sin = tangentM / distanceM;
  const cos = Math.sqrt(1 - sin * sin);
  const forward = { x: -cos, y: sin, z: 0 };
  const up = { x: sin, y: cos, z: 0 };
  const right = { x: 0, y: 0, z: -1 };
  const tanY = LIMB_HALF_SPAN_M / (distanceM * cos);
  const aspect = FRAME_SIZE.widthPx / FRAME_SIZE.heightPx;
  return {
    positionM: { x: distanceM, y: 0, z: 0 },
    orientation: quaternionFromRows([
      { x: right.x, y: up.x, z: -forward.x },
      { x: right.y, y: up.y, z: -forward.y },
      { x: right.z, y: up.z, z: -forward.z },
    ]),
    fovXRad: 2 * Math.atan(aspect * tanY),
    viewport: FRAME_SIZE,
  };
}

/** The sun at an elevation above the horizon of a {@link limbCamera}'s tangent point, ahead. */
function limbSun(distanceM: number, figure: SpheroidFigure, elevationDeg: number): SunState {
  const sin = (figure.equatorialRadiusM + LIMB_CENTRE_M) / distanceM;
  const cos = Math.sqrt(1 - sin * sin);
  const e = (elevationDeg * Math.PI) / 180;
  return {
    directionBodyFixed: {
      x: Math.cos(e) * -cos + Math.sin(e) * sin,
      y: Math.cos(e) * sin + Math.sin(e) * cos,
      z: 0,
    },
    distanceAu: 1,
    angularRadiusRad: SUN_ANGULAR_RADIUS_RAD,
  };
}

/** One view of R08.T6.g's checks. */
interface HeightsView {
  readonly name: string;
  readonly camera: AtmosphereCamera;
  readonly sun: SunState;
  readonly surfaceM: number | null;
}

/**
 * A body's views on a setting: on high, from each of its heights with the sun 20° up and 2° below
 * the horizon, a surface 60 km ahead (the march's, beyond the aerial reach; the sky view and the
 * aerial volume are the open sky's), and its limbs with the sun 20° up and 2° below their tangent
 * point's horizon; on low, from 2 m and 2 km with the sun 20° up.
 */
function heightsViews(body: HeightsBody, setting: QualitySetting): HeightsView[] {
  const views: HeightsView[] = [];
  const heights = setting === "high" ? body.heightsM : LOW_HEIGHTS_M;
  const suns = setting === "high" ? [20, -2] : [20];
  for (const heightM of heights) {
    for (const elevationDeg of suns) {
      views.push({
        name: `${heightM} m, the sun ${elevationDeg}°, a surface 60 km ahead`,
        camera: equatorCamera(heightM, FRAME_SIZE, body.figure),
        sun: sunAt(elevationDeg),
        surfaceM: 60_000,
      });
    }
  }
  if (setting === "high") {
    for (const distanceM of body.limbsM) {
      for (const elevationDeg of suns) {
        views.push({
          name: `the limb from ${(distanceM - body.figure.equatorialRadiusM).toExponential(2)} m, the sun ${elevationDeg}° at its tangent point`,
          camera: limbCamera(distanceM, body.figure),
          sun: limbSun(distanceM, body.figure, elevationDeg),
          surfaceM: null,
        });
      }
    }
  }
  return views;
}

/** Reads `rgba32float` targets back with the harness's tolerance access. */
async function readKernelArm(engine: RenderEngine, outputs: FrameOutputs): Promise<FrameReadback> {
  const read = async (texture: TextureHandle): Promise<Float32Array> =>
    new Float32Array(await engine.readTexture(texture, 0, undefined, "tolerance"));
  return {
    skyView: await read(outputs.skyView),
    aerial: await read(outputs.aerial),
    rayMarch: await read(outputs.rayMarch),
  };
}

/** One frame in both arms, with its composite's output. */
interface ArmFrame {
  readonly stored: FrameReadback;
  readonly kernel: FrameReadback | null;
  /** The composite's output, pre-exposed. */
  readonly composite: Float32Array;
}

/**
 * Draws one frame over a scene whose surface, if any, stands `surfaceM` ahead: the per-frame
 * kernels and the composite, read back, and, when `kernelArm`, the kernel arm over the same scene.
 */
async function drawArms(
  engine: RenderEngine,
  atmosphere: HillaireAtmosphere,
  view: HeightsView,
  kernelArm: boolean,
): Promise<ArmFrame> {
  const size = view.camera.viewport;
  const aspect = size.widthPx / size.heightPx;
  const target = (name: string): RenderTarget =>
    engine.createRenderTarget({
      name,
      size,
      format: "rgba16float",
      mips: 1,
      depth: true,
      category: "render-targets",
    });
  const scene = target("atmosphere scene");
  const output = target("atmosphere output");
  try {
    const draws =
      view.surfaceM === null
        ? []
        : [
            drawOf(
              fullScreenMesh(engine, "atmosphere surface", -view.surfaceM),
              engine.createMaterial(flatSpec("atmosphere surface")),
              [SURFACE_GREY, SURFACE_GREY, SURFACE_GREY, 1],
            ),
          ];
    scene.render(frameOf("atmosphere scene", draws, aspect));
    const atmosphereScene = {
      colour: scene.colour,
      depth: requireDepth(scene.depth),
      nearM: NEAR_M,
      exposureScale: exposureScale(15),
    };
    const composite = atmosphere.drawFrame(view.camera, view.sun, atmosphereScene);
    output.render(frameOf("atmosphere composite", [composite], aspect));
    const compositeTexels = halfTexels(await engine.readTexture(output.colour));
    const stored = await readFrameOutputs(engine, atmosphere);
    const kernel = kernelArm
      ? await readKernelArm(
          engine,
          atmosphere.drawKernelArm(view.camera, view.sun, atmosphereScene),
        )
      : null;
    return { stored, kernel, composite: compositeTexels };
  } finally {
    scene.dispose();
    output.dispose();
  }
}

/** Records one arm's agreement as R08.T6.g's check, with its largest value and its floor's count. */
function recordHeights(
  checks: Checks,
  name: string,
  result: TwinAgreement,
  arm: TwinOptions["arm"],
): void {
  const limited = arm === "stored" && result.largest < FORMAT_LIMITED ? ", format-limited" : "";
  const share = result.envelope.length / result.channels;
  const envelope =
    result.envelope.length === 0
      ? "none by the envelope"
      : `${result.envelope.length} by the envelope (${(share * 100).toFixed(2)}% of ${result.channels}; worst at ${result.worstInEnvelope.toFixed(2)} of its half-width): ${result.envelope.join("; ")}`;
  checks.check(
    `R08.T6.g ${name}, the ${arm} arm, agrees with the CPU twin to 10⁻³`,
    result.worst <= 1 && result.above > 0 && share <= ENVELOPE_SHARE_CAP,
    `worst ${result.at}; largest ${show([result.largest])}${limited}; ${result.above} channels above 10⁻⁶ of the largest, ${result.over} over the bound, ${result.floorCarried} carried by the floor; ${envelope}`,
  );
}

/**
 * The per-planet tables of `atmosphere` against their twins in both arms: the transmittance
 * against the twin of the medium, the multiple scattering against its twin over the GPU's stored
 * transmittance; the kernel arm (`AtmosphereTables.buildKernelArm`) reads the same stored table.
 *
 * @returns The twins' planet over the GPU's stored tables, which the per-frame twins read.
 */
async function checkPlanetArms(
  engine: RenderEngine,
  checks: Checks,
  body: HeightsBody,
  atmosphere: HillaireAtmosphere,
  read: AtmosphereMedium,
): Promise<PlanetTwin> {
  const bottomM = tableRadiusM(body.figure);
  const gpu = await readPlanetTables(engine, atmosphere.tables);
  const stored = storedTransmittance(gpu, atmosphere.tables.curves);
  const transmittance = transmittanceTwin(read, bottomM);
  const multiScattering = multiScatteringTwin(read, bottomM, stored);
  const arm = atmosphere.tables.buildKernelArm();
  try {
    const armTransmittance = new Float32Array(
      await engine.readTexture(arm.transmittance, 0, undefined, "tolerance"),
    );
    const armMultiScattering = new Float32Array(
      await engine.readTexture(arm.multiScattering, 0, undefined, "tolerance"),
    );
    const label = `${body.name}: the per-planet`;
    recordHeights(
      checks,
      `${label} transmittance table, as optical depth`,
      twinAgreement(gpu.transmittance, transmittance, false),
      "stored",
    );
    recordHeights(
      checks,
      `${label} transmittance table, as optical depth`,
      twinAgreement(armTransmittance, transmittance, false, { arm: "kernel" }),
      "kernel",
    );
    recordHeights(
      checks,
      `${label} multiple-scattering table, over the GPU's transmittance`,
      twinAgreement(gpu.multiScattering, multiScattering, false),
      "stored",
    );
    recordHeights(
      checks,
      `${label} multiple-scattering table, over the GPU's transmittance`,
      twinAgreement(armMultiScattering, multiScattering, false, { arm: "kernel" }),
      "kernel",
    );
  } finally {
    engine.releaseTexture(arm.transmittance);
    engine.releaseTexture(arm.absorberColumns);
    engine.releaseTexture(arm.multiScattering);
  }
  return {
    medium: read,
    bottomRadiusM: bottomM,
    ...stored,
    multiScattering: asTwinTable(gpu.multiScattering, MULTI_SCATTERING_SIZE),
  };
}

/** One view's three per-frame kernels against their twins, in each arm the frame drew. */
async function checkViewArms(
  engine: RenderEngine,
  checks: Checks,
  atmosphere: HillaireAtmosphere,
  planet: PlanetTwin,
  body: HeightsBody,
  sizes: TableSizes,
  setting: QualitySetting,
  view: HeightsView,
): Promise<void> {
  const frame = await drawArms(engine, atmosphere, view, setting === "high");
  const label = `${body.name}, ${setting}, ${view.name}`;
  checks.check(
    `R08.T6.g ${label}: the composite's frame is finite and within rgba16float`,
    withinHalfRange(frame.composite),
    `mean ${show(meanRgb(frame.composite))}`,
  );
  const twin = twinView(view.camera, view.sun, body.figure, NEAR_M);
  const surfaceDepth = view.surfaceM === null ? 0 : NEAR_M / view.surfaceM;
  const skyView = skyViewTwin(planet, twin, sizes.skyView, sizes.skyViewSamples);
  const aerial = aerialPerspectiveTwin(
    planet,
    twin,
    sizes.aerialPerspective,
    sizes.aerialPerspectiveSamplesPerSlice,
    sizes.aerialPerspectiveReachM,
  );
  const rayMarch = rayMarchTwin(planet, twin, sizes, () => surfaceDepth);
  // Only a view from above the atmosphere's top may pass a texel by the twin's envelope
  // (`decision-r08-f32.md` §10.3); from inside it the bound holds outright. The aerial volume has
  // none: from above the top it holds no air, and it passes outright everywhere.
  const fromAbove = twin.heightM >= planet.medium.topHeightM;
  const marchEnvelope = fromAbove ? rayMarchEnvelope(planet, twin, sizes, rayMarch) : undefined;
  const skyEnvelope = fromAbove ? skyViewEnvelope(planet, twin, sizes, skyView) : undefined;
  const arms: ReadonlyArray<readonly [TwinOptions["arm"], FrameReadback | null]> = [
    ["stored", frame.stored],
    ["kernel", frame.kernel],
  ];
  for (const [arm, floats] of arms) {
    if (floats !== null) {
      recordHeights(
        checks,
        `${label}: the sky view`,
        twinAgreement(
          floats.skyView,
          skyView,
          false,
          skyEnvelope === undefined ? { arm } : { arm, envelope: skyEnvelope },
        ),
        arm,
      );
      recordHeights(
        checks,
        `${label}: the aerial perspective`,
        twinAgreement(floats.aerial, aerial, true, { arm }),
        arm,
      );
      recordHeights(
        checks,
        `${label}: the ray march`,
        twinAgreement(
          floats.rayMarch,
          rayMarch,
          true,
          marchEnvelope === undefined ? { arm } : { arm, envelope: marchEnvelope },
        ),
        arm,
      );
    }
  }
}

/**
 * R08.T6.g: the kernels against the CPU twin where `f32`'s rounding of radii and WGSL's
 * trigonometry used to spend the bound (plan R08, Design notes 18 and 19; `decision-r08-f32.md`
 * §6.2, `decision-r08-t6b-floor.md` §4).
 *
 * @remarks
 * Each kernel is held in two arms under Verification's twin bound: the stored arm, on both
 * settings, its `rgba16float` table with the format's floor of 2⁻²⁴; and the kernel arm, on high,
 * its own output assembled to write `rgba32float`, over the same GPU tables, with no floor. Each
 * check reports its table's largest value and how many channels the floor carries, and a stored
 * table whose largest colour value is under 2.0 × 10⁻⁴ is marked format-limited. In a view from
 * above the atmosphere's top, a sky-view or ray-march texel the twin itself cannot resolve over the
 * kernel's own resolution of its ray passes if the GPU's value lies within the twin's envelope there,
 * widened by 10⁻³, at most 1% of a check's channels, each recorded (`decision-r08-f32.md` §10).
 * From inside the atmosphere, and in the per-planet tables, the bound holds outright. The sky view runs at half its width and height (`twinSizes`), each texel one ray of the
 * shipped kernel.
 */
export async function checkAtmosphereHeights(engine: RenderEngine, checks: Checks): Promise<void> {
  const read = kernelMedium(EARTH_REFERENCE);
  for (const body of HEIGHTS_BODIES) {
    let planet: PlanetTwin | null = null;
    for (const setting of ["high", "low"] as const) {
      const sizes = twinSizes(TABLE_SIZES[setting]);
      const atmosphere = new HillaireAtmosphere(engine, EARTH_REFERENCE, sizes, body.figure);
      try {
        // The per-planet tables are the same on both settings: held, and twinned, once.
        // The harness's checks run in order: the tables are read back before the first frame.
        // oxlint-disable-next-line no-await-in-loop
        planet ??= await checkPlanetArms(engine, checks, body, atmosphere, read);
        for (const view of heightsViews(body, setting)) {
          // The harness's checks run in order: each frame is read back before the next draws.
          // oxlint-disable-next-line no-await-in-loop
          await checkViewArms(engine, checks, atmosphere, planet, body, sizes, setting, view);
        }
      } finally {
        atmosphere.dispose();
      }
    }
  }
}

// --- R05.T12.c: the comparison captures ----------------------------------------------------------

/** One image the page hands the harness's main process to save, as `result.ts`'s `SmokeImage`. */
export interface CapturedImage {
  readonly name: string;
  readonly width: number;
  readonly height: number;
  /** RGBA, 8 bits a channel, row by row from the top, base64. */
  readonly rgba: string;
}

/** Hillaire 2020's planet: a 6,360 km sphere (sebh's `SetupEarthAtmosphere`). */
const HILLAIRE_PLANET: SpheroidFigure = { equatorialRadiusM: 6_360_000, polarRadiusM: 6_360_000 };

const CAPTURE_SIZE = { widthPx: 320, heightPx: 180 } as const;

/** The sRGB encoding of a display-linear value in [0, 1] (IEC 61966-2-1), to 8 bits. */
export function srgb8(linear: number): number {
  const v = linear <= 0.003_130_8 ? 12.92 * linear : 1.055 * linear ** (1 / 2.4) - 0.055;
  return Math.round(Math.min(Math.max(v, 0), 1) * 255);
}

/**
 * A frame as 8-bit sRGB: exposed so that its mean luminance sits at AgX's middle grey, 0.18, then
 * R02's tone curve.
 */
function toRgba8(texels: Float32Array): Uint8Array {
  let luminance = 0;
  for (let i = 0; i < texels.length; i += 4) {
    luminance +=
      0.2126 * (texels[i] ?? 0) + 0.7152 * (texels[i + 1] ?? 0) + 0.0722 * (texels[i + 2] ?? 0);
  }
  const mean = luminance / (texels.length / 4);
  const k = mean > 0 ? 0.18 / mean : 1;
  const out = new Uint8Array(texels.length);
  for (let i = 0; i < texels.length; i += 4) {
    const [r, g, b] = toneCurve([
      (texels[i] ?? 0) * k,
      (texels[i + 1] ?? 0) * k,
      (texels[i + 2] ?? 0) * k,
    ]);
    out[i] = srgb8(r);
    out[i + 1] = srgb8(g);
    out[i + 2] = srgb8(b);
    out[i + 3] = 255;
  }
  return out;
}

/** `bytes` as base64, for a {@link CapturedImage}. */
export function base64Of(bytes: Uint8Array): string {
  let binary = "";
  for (let i = 0; i < bytes.length; i += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(binary);
}

/** A camera `heightM` above the north pole of a sphere, looking along the horizon towards +x. */
function horizonCamera(heightM: number, radiusM: number, pitchDeg: number): AtmosphereCamera {
  const aspect = CAPTURE_SIZE.widthPx / CAPTURE_SIZE.heightPx;
  const p = (pitchDeg * Math.PI) / 180;
  // Camera −z (forward) to (cos p, 0, sin p), camera +y (up) to (−sin p, 0, cos p), camera +x to
  // (0, −1, 0)... built from the rows of the rotation (camera axes into body axes).
  const forward = { x: Math.cos(p), y: 0, z: Math.sin(p) };
  const up = { x: -Math.sin(p), y: 0, z: Math.cos(p) };
  const right = {
    x: forward.y * up.z - forward.z * up.y,
    y: forward.z * up.x - forward.x * up.z,
    z: forward.x * up.y - forward.y * up.x,
  };
  return {
    positionM: { x: 0, y: 0, z: radiusM + heightM },
    orientation: quaternionFromRows([
      { x: right.x, y: up.x, z: -forward.x },
      { x: right.y, y: up.y, z: -forward.y },
      { x: right.z, y: up.z, z: -forward.z },
    ]),
    fovXRad: 2 * Math.atan(aspect),
    viewport: CAPTURE_SIZE,
  };
}

/** The sun ahead of a {@link horizonCamera}, at an elevation above its horizon. */
function sunAhead(elevationDeg: number): SunState {
  const e = (elevationDeg * Math.PI) / 180;
  return {
    directionBodyFixed: { x: Math.cos(e), y: 0, z: Math.sin(e) },
    distanceAu: 1,
    // Hillaire 2020's comparison images are drawn without the sun's disc (§6).
    angularRadiusRad: 0,
  };
}

/**
 * R05.T12.c's comparison mode: Hillaire's reference medium on his 6,360 km planet, rendered hidden
 * from the ground at noon and at sunset and from orbit, for a person to set beside Hillaire 2020's
 * published images (CGF 39(4), DOI 10.1111/cgf.14050), which are kept local and untracked.
 */
export async function captureAtmosphere(engine: RenderEngine): Promise<CapturedImage[]> {
  const atmosphere = new HillaireAtmosphere(
    engine,
    HILLAIRE_REFERENCE,
    TABLE_SIZES.high,
    HILLAIRE_PLANET,
  );
  const shots = [
    { name: "hillaire-ground-noon", camera: horizonCamera(2, 6_360_000, 30), sun: sunAhead(60) },
    { name: "hillaire-ground-sunset", camera: horizonCamera(2, 6_360_000, 10), sun: sunAhead(1) },
    { name: "hillaire-orbit", camera: horizonCamera(400_000, 6_360_000, -20), sun: sunAhead(20) },
  ];
  const images: CapturedImage[] = [];
  try {
    for (const shot of shots) {
      // Each capture reads the GPU back before the next frame draws.
      // oxlint-disable-next-line no-await-in-loop
      const texels = await compositeFrame(engine, atmosphere, shot.camera, shot.sun, null);
      images.push({
        name: shot.name,
        width: CAPTURE_SIZE.widthPx,
        height: CAPTURE_SIZE.heightPx,
        rgba: base64Of(toRgba8(texels)),
      });
    }
  } finally {
    atmosphere.dispose();
  }
  return [...images, ...(await captureSunsets(engine))];
}

/** The sunset captures' narrow field, rad: 12° high, so that the sun's 0.53° disc spans 8 rows. */
const SUNSET_DISC_FOV_Y_RAD = (12 * Math.PI) / 180;

/**
 * R08.T6.g's by-hand captures: the horizon at sunset from 2 m on Earth and from 1 m on the 2 R⊕
 * body, Earth's medium, the sun's disc 0.5° up, for a person to look for a band at the horizon's
 * row, where R05's ray march erred 13–14 times its bound, and to see the disc that R05's
 * transmittance table drew 21–39% too dark from there (`decision-r08-f32.md` §6.2,
 * `decision-r08-remap.md` §6). Each is taken twice: across the comparison captures' 90° field,
 * where the disc is under a pixel, and across 12°, where it spans 8 rows (its exposure set by the
 * disc, so the sky there is dark).
 */
async function captureSunsets(engine: RenderEngine): Promise<CapturedImage[]> {
  const shots = [
    { name: "r08-sunset-earth-2m", figure: WGS84, heightM: 2, narrow: false },
    { name: "r08-sunset-earth-2m-disc", figure: WGS84, heightM: 2, narrow: true },
    {
      name: "r08-sunset-2-earth-radii-1m",
      figure: sphereFigure(12_742_017.6),
      heightM: 1,
      narrow: false,
    },
    {
      name: "r08-sunset-2-earth-radii-1m-disc",
      figure: sphereFigure(12_742_017.6),
      heightM: 1,
      narrow: true,
    },
  ];
  const aspect = CAPTURE_SIZE.widthPx / CAPTURE_SIZE.heightPx;
  const images: CapturedImage[] = [];
  for (const shot of shots) {
    const atmosphere = new HillaireAtmosphere(
      engine,
      EARTH_REFERENCE,
      TABLE_SIZES.high,
      shot.figure,
    );
    try {
      // Above the pole, whose datum stands at the polar radius, looking along the horizon.
      const wide = horizonCamera(shot.heightM, shot.figure.polarRadiusM, 0);
      const camera = shot.narrow
        ? { ...wide, fovXRad: 2 * Math.atan(aspect * Math.tan(SUNSET_DISC_FOV_Y_RAD / 2)) }
        : wide;
      const sun = { ...sunAhead(0.5), angularRadiusRad: SUN_ANGULAR_RADIUS_RAD };
      // Each capture reads the GPU back before the next frame draws.
      // oxlint-disable-next-line no-await-in-loop
      const texels = await compositeFrame(engine, atmosphere, camera, sun, null);
      images.push({
        name: shot.name,
        width: CAPTURE_SIZE.widthPx,
        height: CAPTURE_SIZE.heightPx,
        rgba: base64Of(toRgba8(texels)),
      });
    } finally {
      atmosphere.dispose();
    }
  }
  return images;
}
