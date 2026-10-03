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
 * (`captureAtmosphere`).
 */

import { quaternion, quaternionFromRows } from "../view/camera/quaternion";
import { EARTH_REFERENCE, HILLAIRE_REFERENCE } from "../view/atmosphere/earth";
import {
  type AtmosphereCamera,
  HillaireAtmosphere,
  type SpheroidFigure,
  type SunState,
  TABLE_SIZES,
} from "../view/atmosphere/hillaire";
import { opticalDepth, transmittanceUvToRMu } from "../view/atmosphere/opticalDepth";
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
