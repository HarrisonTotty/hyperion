/**
 * The smoke page's atmosphere checks (plan R05, R05.T12.b): Earth's per-planet tables built on the
 * GPU and read back with the harness's tolerance access, since their kernels are
 * `presentation-only`.
 *
 * @remarks
 * The transmittance table agrees with the `f64` oracle (`view/atmosphere/opticalDepth.ts`) to 1%
 * at 20 fixed texels, and every texel of both tables is finite, transmittance within [0, 1] and
 * multiple scattering non-negative. R05.T12.c adds whole frames of the atmosphere on both settings
 * (`checkAtmosphereFrames`) and, when the run is asked for them, the comparison frames of
 * Hillaire's reference medium that a person sets beside his published images
 * (`captureAtmosphere`). R05.T12.e holds the sky view and the ray march at their settings' step
 * counts to the same kernels at 1,024 steps (`checkAtmosphereSteps`).
 */

import { add, normalise, scale, type Vec3 } from "../geometry/vec3";
import { quaternion, quaternionFromRows, rotate } from "../view/camera/quaternion";
import { EARTH_REFERENCE, HILLAIRE_REFERENCE } from "../view/atmosphere/earth";
import {
  type AtmosphereCamera,
  atmosphereInputs,
  HillaireAtmosphere,
  type SpheroidFigure,
  type SunState,
  TABLE_SIZES,
  type TableSizes,
} from "../view/atmosphere/hillaire";
import {
  grazingTwilight,
  LOW_TWIN_WORST,
  MARCH_TOLERANCE,
  TWILIGHT_TOLERANCE,
} from "../view/atmosphere/marchSteps";
import { maxDistanceM, opticalDepth, transmittanceUvToRMu } from "../view/atmosphere/opticalDepth";
import {
  AtmosphereTables,
  MULTI_SCATTERING_SIZE,
  TRANSMITTANCE_SIZE,
} from "../view/atmosphere/tables";
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
 * Below rgba16float's least normal, 2⁻¹⁴, a stored transmittance has lost its relative precision,
 * so a texel whose oracle lies below it is held to that absolute floor instead of to 1%.
 */
const HALF_NORMAL_MIN = 2 ** -14;

/** The 20 texels compared with the oracle: five across μ by four across r. */
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
    const { rM, mu } = transmittanceUvToRMu(
      shell,
      (x + 0.5) / width,
      (y + 0.5) / TRANSMITTANCE_SIZE.heightTexels,
    );
    const expected = opticalDepth(EARTH_REFERENCE, EARTH_MEAN_RADIUS_M, rM, mu).map((tau) =>
      Math.exp(-tau),
    );
    const actual = texel(transmittance, width, x, y).slice(0, 3);
    const agrees = expected.every((e, c) => {
      const a = actual[c] ?? Number.NaN;
      return Math.abs(a - e) <= Math.max(0.01 * e, HALF_NORMAL_MIN);
    });
    checks.check(
      `R05.T12.b transmittance texel (${x}, ${y}) agrees with the f64 oracle to 1%`,
      agrees,
      `h ${((rM - EARTH_MEAN_RADIUS_M) / 1_000).toFixed(2)} km, mu ${mu.toFixed(4)}: GPU ${show(actual)}, oracle ${show(expected)}`,
    );
  }
  const inRange = transmittance.every((v) => Number.isFinite(v) && v >= 0 && v <= 1);
  checks.check(
    "R05.T12.b every transmittance texel is finite and within [0, 1]",
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

/** A camera `heightM` above the equator at longitude 0, looking north along the horizon. */
function equatorCamera(heightM: number, size: ViewSize): AtmosphereCamera {
  const aspect = size.widthPx / size.heightPx;
  return {
    positionM: { x: WGS84.equatorialRadiusM + heightM, y: 0, z: 0 },
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

/** The sub-texel remapping of `common.wgsl`'s `subUvsToUnit` on one axis, clamped to [0, 1]. */
function subUvToUnit(uv: number, size: number): number {
  return Math.min(Math.max((uv - 0.5 / size) * (size / (size - 1)), 0), 1);
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
  const heightM = Math.min(Math.max(inputs.heightM, 1), EARTH_REFERENCE.topHeightM - 1);
  const rM = bottomM + heightM;
  const u = subUvToUnit((x + 0.5) / size.widthTexels, size.widthTexels);
  const v = subUvToUnit((y + 0.5) / size.heightTexels, size.heightTexels);
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
  return images;
}
