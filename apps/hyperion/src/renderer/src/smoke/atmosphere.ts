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
 * T had under 2⁻¹⁴ on Earth's table and a Venus-class one. Since R08.T6.d Earth is the rebuilt
 * one (`earth.ts`'s `EARTH_REFERENCE`, its ozone under the reference Sun's curve) in R08.T6.b's
 * twins, the frames and the captures; R05.T12.b's oracle, R05.T12.e's steps and R08.T6.g's heights
 * keep R05's Earth (`R05_EARTH_REFERENCE`), the medium they were measured on, whose kernels'
 * geometry they hold. The Venus-class multiple-scattering table is held in both arms. R08.T6.i holds
 * the kernels whose steps are cut at layer edges (the sky view, the ray march and the multiple
 * scattering) in both arms on the slab Earth, `EARTH_SLAB_REFERENCE`, whose ozone has a base and a
 * top (`checkAtmosphereLayerEdges`). R08.T7 holds the per-view tables summed over several sources
 * to their twins under a K5 and M3 binary, each light on its own curves and multiple-scattering
 * layer, and the composite's pass-through of the scene's `METER_CLASS` (`checkSeveralSuns`); its
 * captures add the binary's sky (`captureAtmosphere`). Since R08.T7 the per-frame twins add the
 * tabulated terms' forward-lobe orders, so R08.T6.b's tabulated medium holds them too. R08.T8.a
 * holds the ray march's sky from inside the air, in the march band where it takes over from the sky
 * view, to its twin in both arms, and the composite's blend of the two (`checkMarchBand`).
 * R08.T8.b's ascent by hand, from 2 m to 10⁸ m, takes frames for the eye and figures across the
 * band's edges and the top, when the run is asked for captures (`captureAscent`).
 */

import {
  add,
  cross as crossProduct,
  dot,
  norm,
  normalise,
  scale,
  type Vec3,
} from "../geometry/vec3";
import { quaternion, quaternionFromRows, rotate } from "../view/camera/quaternion";
import { hydrostaticColumn } from "../view/atmosphere/column";
import {
  EARTH_REFERENCE,
  EARTH_REFERENCE_CURVES,
  EARTH_SLAB_REFERENCE,
  EARTH_SLAB_REFERENCE_CURVES,
  HILLAIRE_REFERENCE,
  R05_EARTH_REFERENCE,
  referenceSunIlluminanceLx,
} from "../view/atmosphere/earth";
import {
  type AtmosphereCamera,
  atmosphereInputs,
  bodyOfFigure,
  curveSlotOf,
  type FrameOutputs,
  frameSourcesOf,
  HillaireAtmosphere,
  type MarchBand,
  marchWeightOf,
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
import type { BodyGravity } from "../view/atmosphere/oblate";
import {
  maxDistanceM,
  opticalDepthToTop,
  transmittanceUvToRMu,
  unitFromTexelUv,
} from "../view/atmosphere/opticalDepth";
import {
  absorberCount,
  type AbsorberCurveSlots,
  type AtmosphereBody,
  AtmosphereTables,
  IRRADIANCE_SIZE,
  kernelMedium,
  lastPressureColumnSize,
  MULTI_SCATTERING_SIZE,
  NO_ABSORBER_CURVES,
  packedScaleHeightsM,
  PHASE_TABLE_ENTRIES,
  type TableSize,
  type TablesBand,
  tablesPlanOf,
  type TransmittanceTextures,
  TRANSMITTANCE_SIZE,
} from "../view/atmosphere/tables";
import {
  frameTwinView,
  type OblatePlanetTwin,
  oblateCompositeTransmittanceTwin,
  oblateRayMarchRayAlongTwin,
  oblateRayMarchRayTwin,
  oblateRayMarchTexelTwin,
  oblateSkyViewMarchTwin,
  oblateSkyViewRayTwin,
  type PlanetTwin,
  marchTwin,
  mediumMarchEdgesM,
  type RayMarchRay,
  piecedSteps,
  rayMarchRayAlongTwin,
  rayMarchRayTwin,
  rayMarchTexelTwin,
  skyViewRayTwin,
  surfaceLightingTwin,
  tableSources,
  type TwinBand,
  type TwinRay,
  type TwinTable,
  type TwinTransmittance,
  type TwinView,
  type TwinVolume,
} from "../view/atmosphere/tablesCpu";
import {
  earthBody,
  fourAbsorberMedium,
  lineAbsorberMedium,
  saturnBody,
  saturnClassMedium,
  venusClassMedium,
} from "../test/atmosphereFixtures";
import {
  type DrawItem,
  PresentationOnlyReadback,
  type RenderEngine,
  type RenderTarget,
  type TextureHandle,
  type ViewSize,
} from "../view/engine/types";
import { exposureScale } from "../view/photometry/exposure";
import { METER_CLASS } from "../view/post/meter";
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
  rectangleMesh,
  show,
  texel,
  triangleMesh,
} from "./harness";
import {
  type AtmosphereLookup,
  expectedLighting,
  type LightingProbeCase,
} from "../view/appearance/litBodyProbe";
import { surfaceLightingTablesOf } from "../view/atmosphere/surfaceLighting";
import { probeSurfaceLighting } from "./litBody";
import { attempt, handled, inOrder, type OrderedChecks } from "./ordered";
import type { TwinPool } from "./twinPool";

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

/**
 * Builds R05's Earth's tables and checks them against the oracle and their ranges: R05.T12.b's
 * check, on the medium it was measured on (`R05_EARTH_REFERENCE`, plan R08, R08.T6.d).
 */
export async function checkAtmosphereTables(engine: RenderEngine, checks: Checks): Promise<void> {
  const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, EARTH_MEAN_RADIUS_M);

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
    topRadiusM: EARTH_MEAN_RADIUS_M + R05_EARTH_REFERENCE.topHeightM,
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
    const expected = opticalDepthToTop(R05_EARTH_REFERENCE, EARTH_MEAN_RADIUS_M, rM, mu);
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
    !tables.setMedium(R05_EARTH_REFERENCE, EARTH_MEAN_RADIUS_M) && tables.builds === 1,
    `builds ${tables.builds}`,
  );
}

// --- R05.T12.c: frames of the atmosphere -------------------------------------------------------

/** WGS 84 (NIMA TR8350.2): the test planet's figure. */
const WGS84: SpheroidFigure = {
  equatorialRadiusM: 6_378_137,
  polarRadiusM: 6_378_137 * (1 - 1 / 298.257223563),
};

/**
 * Earth on WGS 84 with its normal gravity (R08.T3.d), whose s the kernels take since R08.T6.f
 * (Design note 17): one slice and one band, s within ±0.27% of 1, every per-view sphere R_α.
 */
const EARTH_BODY: AtmosphereBody = earthBody();

/** The gravity the smoke's WGS 84 atmospheres are built with: {@link EARTH_BODY}'s (R08.T6.f). */
const EARTH_GRAVITY: BodyGravity = EARTH_BODY.gravity;

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

/**
 * The checks' Sun at 1 au per channel, lx: R07's `starIlluminance` of R06's Sun (plan R08,
 * R08.T6.d, `earth.ts`'s `referenceSunIlluminanceLx`).
 */
const SUN_ILLUMINANCE_LX = referenceSunIlluminanceLx();

/** The sun at an elevation above the camera's horizon, towards the north (body +x up, +z north). */
function sunAt(elevationDeg: number): SunState {
  const e = (elevationDeg * Math.PI) / 180;
  return {
    directionBodyFixed: { x: Math.sin(e), y: 0, z: Math.cos(e) },
    illuminanceLx: SUN_ILLUMINANCE_LX,
    angularRadiusRad: SUN_ANGULAR_RADIUS_RAD,
  };
}

/** The grey a check surface is drawn in, pre-exposed. */
const SURFACE_GREY = 0.02;

/** The exposure the smoke's frames are drawn at, EV100: 15. */
const SMOKE_EV100 = 15;

/** The pre-exposure the smoke's frames are drawn at, {@link SMOKE_EV100}'s. */
const SMOKE_EXPOSURE_SCALE = exposureScale(SMOKE_EV100);

/**
 * The twins' view of a frame `atmosphere` draws for `suns` at {@link SMOKE_EXPOSURE_SCALE}: each
 * source's light weighted as the per-view tables hold it since R08.T7, by its illuminance times the
 * pre-exposure (`tablesCpu.ts`'s `frameTwinView`).
 */
function frameViewOf(
  atmosphere: HillaireAtmosphere,
  camera: AtmosphereCamera,
  suns: ReadonlyArray<SunState>,
  figure: SpheroidFigure,
): TwinView {
  const tables = atmosphere.tables;
  return frameTwinView(
    camera,
    suns,
    figure,
    NEAR_M,
    SMOKE_EXPOSURE_SCALE,
    absorberCount(tables.medium) > 0,
    tables.curves.length,
  );
}

/** A surface a composite frame's scene draws, made for the frame's viewport when it draws. */
type SceneSurface = (size: ViewSize) => DrawItem;

/** How {@link drawComposite} draws its frame's scene. */
interface CompositeScene {
  /** What its targets and frames are named after: `<label> scene`, `<label> output`. */
  readonly label: string;
  /** Its surface, if any, drawn {@link SURFACE_GREY}; black at depth 0 elsewhere. */
  readonly surface: SceneSurface | null;
  /** The frame's pre-exposure. */
  readonly preExposure: number;
}

/**
 * Draws one composite frame for `suns` (R08.T7's sky sources) over `scene`'s surface, if any, and
 * reads it back through `read` while its targets live: the output's, and the scene's, whose colour
 * a check may read beside it.
 */
async function drawComposite<T>(
  engine: RenderEngine,
  atmosphere: HillaireAtmosphere,
  camera: AtmosphereCamera,
  suns: ReadonlyArray<SunState>,
  scene: CompositeScene,
  read: (output: RenderTarget, sceneTarget: RenderTarget) => Promise<T>,
): Promise<T> {
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
  const sceneTarget = target(`${scene.label} scene`);
  const output = target(`${scene.label} output`);
  try {
    const draws = scene.surface === null ? [] : [scene.surface(size)];
    sceneTarget.render(frameOf(`${scene.label} scene`, draws, aspect));
    const composite = atmosphere.drawFrame(camera, suns, {
      colour: sceneTarget.colour,
      depth: requireDepth(sceneTarget.depth),
      nearM: NEAR_M,
      exposureScale: scene.preExposure,
    });
    output.render(frameOf(`${scene.label} composite`, [composite], aspect));
    return await read(output, sceneTarget);
  } finally {
    sceneTarget.dispose();
    output.dispose();
  }
}

/**
 * Renders one composite frame for `suns` (R08.T7's sky sources) over a scene whose surface, if any,
 * stands `surfaceM` ahead, pre-exposed by `preExposure` ({@link SMOKE_EXPOSURE_SCALE} unless
 * given).
 */
async function compositeFrame(
  engine: RenderEngine,
  atmosphere: HillaireAtmosphere,
  camera: AtmosphereCamera,
  suns: ReadonlyArray<SunState>,
  surfaceM: number | null,
  preExposure: number = SMOKE_EXPOSURE_SCALE,
): Promise<Float32Array> {
  const surface: SceneSurface | null =
    surfaceM === null
      ? null
      : () =>
          drawOf(
            fullScreenMesh(engine, "atmosphere surface", -surfaceM),
            engine.createMaterial(flatSpec("atmosphere surface")),
            [SURFACE_GREY, SURFACE_GREY, SURFACE_GREY, 1],
          );
  return drawComposite(
    engine,
    atmosphere,
    camera,
    suns,
    { label: "atmosphere", surface, preExposure },
    async (output) => halfTexels(await engine.readTexture(output.colour)),
  );
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
    const atmosphere = new HillaireAtmosphere(
      engine,
      EARTH_REFERENCE,
      TABLE_SIZES[setting],
      WGS84,
      EARTH_REFERENCE_CURVES,
    );
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
          [sunAt(c.elevationDeg)],
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
 * `view.wgsl`, on the camera's own sphere, in the frame whose x points to the sun's azimuth, the
 * view's one source, and the local azimuth φ = 2π u from it (R08.T7).
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
  const shell = { bottomRadiusM: bottomM, topRadiusM: bottomM + R05_EARTH_REFERENCE.topHeightM };
  const heightM = skyViewHeight(inputs.heightM, R05_EARTH_REFERENCE.topHeightM);
  const rM = bottomM + heightM;
  const u = unitOfUv((x + 0.5) / size.widthTexels, size.widthTexels);
  const v = unitOfUv((y + 0.5) / size.heightTexels, size.heightTexels);
  const beta = Math.acos(Math.sqrt(Math.max(heightM * (2 * bottomM + heightM), 0)) / rM);
  const zenithHorizon = Math.PI - beta;
  const zenith =
    v < 0.5 ? zenithHorizon * (1 - (1 - 2 * v) ** 2) : zenithHorizon + beta * (2 * v - 1) ** 2;
  const phi = 2 * Math.PI * u;
  const sinZenith = Math.sin(zenith);
  const direction = {
    x: sinZenith * Math.cos(phi),
    y: sinZenith * Math.sin(phi),
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
  const shell = sphereHits(camera.positionM, direction, a + R05_EARTH_REFERENCE.topHeightM);
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
    await compositeFrame(engine, atmosphere, camera, [sun], surfaceM);
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
    // R05.T12.e's GPU agreement, on the medium its bounds were measured on (plan R08, R08.T6.d).
    const shipped = new HillaireAtmosphere(engine, R05_EARTH_REFERENCE, reduced, WGS84);
    const converged = new HillaireAtmosphere(
      engine,
      R05_EARTH_REFERENCE,
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
 * `f32`'s spacing below its least normal, its smallest subnormal: 2⁻¹⁴⁹ (about 1.4 × 10⁻⁴⁵), the
 * kernel arm's floor as {@link HALF_SUBNORMAL_SPACING} is the stored arm's (R08.T8.b): a twin value
 * under what `f32` holds is a correct 0 on the GPU.
 *
 * @remarks
 * It is there for the far limb from 10⁷ m, where `oblateAerialPerspectiveTwin` marches e^(−h ÷ H)
 * with no clip at the top and holds 10⁻²⁰⁰ to 10⁻¹⁹⁴ about a camera 3,600 km up, which `f32`
 * underflows to 0. It moves no other check: a channel it carries is off by at most 2⁻¹⁴⁹ yet by
 * more than 10⁻³ of max(|L₀|, 10⁻⁶ L_max), which needs a table whose largest value L_max is under
 * 10⁹ × 2⁻¹⁴⁹, about 1.4 × 10⁻³⁶.
 */
const F32_SUBNORMAL_SPACING = 2 ** -149;

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
  readonly envelope: readonly EnvelopeChannel[];
  /**
   * The channels that passed by an envelope holding the split the kernel may take at its rounding
   * ({@link TexelEnvelope.split}), recorded apart and outside §10.3's cap (R08.T6.f).
   */
  readonly split: readonly EnvelopeChannel[];
  /** The worst envelope channel's distance from its envelope's middle, of its half-width. */
  readonly worstInEnvelope: number;
}

/** One channel that passed by its texel's twin envelope (`decision-r08-f32.md` §10.3). */
interface EnvelopeChannel {
  /** The texel, `(x, y)`, or `(x, y, z)` in a volume. */
  readonly where: string;
  /** The texel's row in its table. */
  readonly row: number;
  readonly channel: number;
  /** The texel's nominal ray's tangent height, m. */
  readonly tangentHeightM: number;
  /**
   * The envelope's half-width, (greatest − least) ÷ 2 over the twin's values, as a fraction of
   * max(|L₀|, floor), L₀ the twin's nominal value (R08.T8.b).
   */
  readonly halfWidth: number;
}

/** How a readback is held (R08.T6.g, `decision-r08-t6b-floor.md` §4, `decision-r08-f32.md` §10). */
interface TwinOptions {
  /**
   * `stored`: an `rgba16float` table, with the format's floor of 2⁻²⁴; `kernel`: a kernel's own
   * `rgba32float` output (the kernel arm), with `f32`'s, 2⁻¹⁴⁹ ({@link F32_SUBNORMAL_SPACING}),
   * since R08.T8.b.
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
 * max(10⁻³ · max(|twin|, 10⁻⁶ · largest), 2⁻²⁴) for a stored table, and with `f32`'s 2⁻¹⁴⁹ in
 * place of the 2⁻²⁴ for a kernel's own output (R08.T8.b; {@link F32_SUBNORMAL_SPACING}), the
 * largest taken over the colour channels, and over alpha apart when
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
  const formatFloor = options.arm === "stored" ? HALF_SUBNORMAL_SPACING : F32_SUBNORMAL_SPACING;
  const amountChannels = options.amountChannels ?? [];
  const perSlice = twin.widthTexels * twin.heightTexels;
  const count = texels.length / 4;
  let worst = gpu.length === texels.length ? 0 : Number.POSITIVE_INFINITY;
  let at = worst === 0 ? "none" : `${gpu.length / 4} texels read back, ${count} twinned`;
  let above = 0;
  let over = 0;
  let floorCarried = 0;
  let worstInEnvelope = 0;
  const envelope: EnvelopeChannel[] = [];
  const split: EnvelopeChannel[] = [];
  const rowOf = (i: number): number => Math.floor(i / twin.widthTexels) % twin.heightTexels;
  const where = (i: number): string => {
    const x = i % twin.widthTexels;
    const y = rowOf(i);
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
      const allowed = Math.max(relative, formatFloor);
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
          (texelEnvelope.split ? split : envelope).push({
            where: where(i),
            row: rowOf(i),
            channel: c,
            tangentHeightM: texelEnvelope.tangentHeightM,
            halfWidth: half / Math.max(Math.abs(expected), floor),
          });
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
    split,
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
  const [, aerosol, ozone] = R05_EARTH_REFERENCE.terms;
  if (aerosol === undefined || ozone === undefined) {
    throw new Error("R05's Earth has its aerosol and ozone");
  }
  return {
    name: "R08.T6.b tabulated",
    topHeightM: R05_EARTH_REFERENCE.topHeightM,
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

/** One frame R08.T6.b's and R08.T7's per-frame checks draw, with its sky's sources. */
interface TwinFrame {
  readonly name: string;
  readonly heightM: number;
  readonly suns: ReadonlyArray<SunState>;
  readonly surfaceM: number | null;
}

/**
 * R08.T6.b: every kernel's readback against the CPU twin (`tablesCpu.ts`) of the medium as the
 * kernels read it (`kernelMedium`), to 10⁻³ above 10⁻⁶ of the table's largest value, on Earth (R05's
 * until R08.T6.d, the rebuilt one since) and on a medium of every profile and phase the kernels
 * read; since R08.T6.c also on a medium of four absorbers, one a molecule pair, under their curves
 * of growth, and, for the per-planet tables alone, on a Venus-class medium and, since R08.T4.d, on a
 * medium with a line absorber, whose curve has pressure rows and whose tables carry the
 * pressure-weighted columns.
 *
 * @remarks
 * The transmittance table, its optical depth and absorber columns (R08.T6.c), is held to the twin
 * of the medium alone, as is the absorber-column table beside it. The Venus-class medium's
 * multiple-scattering table is held in both arms since R08.T6.d: in its deep, nearly conservative
 * air f_ms nears 1, where `f32`'s 1 − f_ms rounded to 0 (0 ÷ 0 where the sun is down, NaN at 160
 * channels in R08.T6.c), and the kernel now accumulates 1 − f_ms instead of forming it, storing
 * L₂ alone where it is 0, which the check counts (`decision-r08-t6b-floor.md` §9.3). Each later
 * kernel is held to
 * its twin given the GPU's own tables as its inputs (the transmittance for the multiple
 * scattering, both for the per-frame kernels), so that each is judged on its own arithmetic and
 * not on the `rgba16float` rounding of what it reads; the multiple-scattering table's agreement
 * with the twin's own tables is reported beside it. The per-frame kernels run on both settings'
 * counts, at half the sky view's width and height (each texel still one ray, {@link twinSizes}),
 * over two frames: from 2 km at a sun 20° up with a surface 60 km ahead (the ray march's beyond the
 * aerial reach), and from 400 km with the sun 5° up (the limb). The camera stands at 2 km, as
 * R08.T6.b left it: R08.T6.g's group (`checkAtmosphereHeights`) holds the kernels from 1 m.
 *
 * The twins run on the page's twin workers (`twins`) beside the GPU's work, and the checks are
 * recorded in the order they were in sequence (`ordered.ts`): each medium's own twins, which read
 * nothing of the GPU's, begin with the group, and each twin over the GPU's tables as soon as they
 * are read back.
 */
export async function checkAtmosphereTwins(
  engine: RenderEngine,
  checks: Checks,
  twins: TwinPool,
): Promise<void> {
  const absorbing = fourAbsorberMedium();
  const venus = venusClassMedium();
  const lines = lineAbsorberMedium();
  const media: ReadonlyArray<TwinMedium> = [
    { ...EARTH_REBUILT, scope: "everything" },
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
      scope: "tables",
    },
    // The per-planet tables alone, for the smoke's watchdog: the multiple-scattering kernel reads
    // the pressure-weighted columns and the curve's rows as the per-frame kernels do (R08.T4.d).
    { ...lines, radiusM: EARTH_MEAN_RADIUS_M, scope: "tables" },
  ];
  const begun = media.map((entry) => {
    const read = attempt(() => kernelMedium(entry.medium));
    return { entry, read, own: handled(ownTwins(twins, entry, read)) };
  });
  await inOrder(checks, async (ordered) => {
    for (const { entry, read: reading, own } of begun) {
      // The medium as the kernels read it, in its turn: a medium they refuse fails the group here.
      // oxlint-disable-next-line no-await-in-loop
      const read = await reading;
      // The harness's GPU work runs in order: each medium's tables are read back before the next's.
      // oxlint-disable-next-line no-await-in-loop
      await checkPlanetTwins(engine, ordered, entry, read, own, twins);
      if (entry.scope === "tables") {
        continue;
      }
      for (const setting of ["high", "low"] as const) {
        // Each setting's frames are read back before the next setting's atmosphere draws.
        // oxlint-disable-next-line no-await-in-loop
        await checkFrameTwins(engine, ordered, entry, read, setting, twins);
      }
    }
  });
}

/** A medium's own per-planet twins, which read nothing of the GPU's. */
interface OwnTwins extends TwinTransmittance {
  readonly multiScattering: TwinTable | null;
}

/**
 * A medium's own per-planet twins, as the medium's `scope` holds them: both tables, or, for the
 * per-planet tables alone, the transmittance tables without their multiple-scattering twin
 * (R08.T6.d, for the smoke's watchdog).
 */
async function ownTwins(
  twins: TwinPool,
  entry: TwinMedium,
  read: Promise<AtmosphereMedium>,
): Promise<OwnTwins> {
  const medium = await read;
  return entry.scope === "tables"
    ? {
        ...(await twins.transmittanceTables(medium, entry.radiusM, entry.curves)),
        multiScattering: null,
      }
    : twins.planet(medium, entry.radiusM, entry.curves);
}

/**
 * A medium R08.T6.b's and T6.c's checks build tables for: with its lights' curves of growth, the
 * radius of its tables' ground, and what is held: `everything`, its per-planet tables and its
 * per-frame kernels, or `tables`, its per-planet tables alone, the multiple scattering in both
 * arms (R08.T6.d: the Venus-class medium's, whose term is no longer NaN).
 */
interface TwinMedium {
  readonly medium: AtmosphereMedium;
  readonly curves: AbsorberCurveSlots;
  readonly radiusM: number;
  readonly scope: "everything" | "tables";
}

/**
 * The Earth R08.T6.d rebuilt, on WGS 84's mean radius, with its ozone under the reference Sun's
 * curve, in place of R05's Earth, which R05.T12.b's and R05.T12.e's checks keep.
 */
const EARTH_REBUILT = {
  medium: EARTH_REFERENCE,
  curves: EARTH_REFERENCE_CURVES,
  radiusM: EARTH_MEAN_RADIUS_M,
} as const;

/** A planet's per-planet tables, read back. */
interface PlanetReadback {
  /** The optical depths in rgb and absorber slot 0's column in alpha (R08.T6.c). */
  readonly transmittance: Float32Array;
  /** Absorber slots 1 to 4's columns (R08.T6.c). */
  readonly absorberColumns: Float32Array;
  /** The pressure-weighted columns, pressure channels 0 to 3 and channel 4 (R08.T4.d). */
  readonly absorberPressureColumns: Float32Array;
  readonly lastAbsorberPressureColumn: Float32Array;
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
    absorberPressureColumns: await read(tables.absorberPressureColumns),
    lastAbsorberPressureColumn: await read(tables.lastAbsorberPressureColumn),
    multiScattering: await read(tables.multiScattering),
  };
}

/**
 * The GPU's stored transmittance tables as the twins read them, with the lights' curves, the fifth
 * pressure channel's table at its size for the medium (`lastPressureColumnSize`).
 */
function storedTransmittance(
  gpu: PlanetReadback,
  medium: AtmosphereMedium,
  curves: AbsorberCurveSlots,
): TwinTransmittance {
  return {
    transmittance: asTwinTable(gpu.transmittance, TRANSMITTANCE_SIZE),
    absorberColumns: asTwinTable(gpu.absorberColumns, TRANSMITTANCE_SIZE),
    absorberPressureColumns: asTwinTable(gpu.absorberPressureColumns, TRANSMITTANCE_SIZE),
    lastAbsorberPressureColumn: asTwinTable(
      gpu.lastAbsorberPressureColumn,
      lastPressureColumnSize(medium),
    ),
    curves,
  };
}

/** Layer `layer` of a 2D array's readback of `layers` layers. */
function layerOf(texels: Float32Array, layers: number, layer: number): Float32Array {
  const per = texels.length / layers;
  return texels.subarray(layer * per, (layer + 1) * per);
}

/**
 * The GPU's stored transmittance tables of each κ slice as the twins read them (R08.T6.f): layer k
 * of each 2D array, with the lights' curves.
 */
function storedSlices(
  gpu: PlanetReadback,
  medium: AtmosphereMedium,
  curves: AbsorberCurveSlots,
  slices: number,
): TwinTransmittance[] {
  return Array.from({ length: slices }, (_, k) => ({
    transmittance: asTwinTable(layerOf(gpu.transmittance, slices, k), TRANSMITTANCE_SIZE),
    absorberColumns: asTwinTable(layerOf(gpu.absorberColumns, slices, k), TRANSMITTANCE_SIZE),
    absorberPressureColumns: asTwinTable(
      layerOf(gpu.absorberPressureColumns, slices, k),
      TRANSMITTANCE_SIZE,
    ),
    lastAbsorberPressureColumn: asTwinTable(
      layerOf(gpu.lastAbsorberPressureColumn, slices, k),
      lastPressureColumnSize(medium),
    ),
    curves,
  }));
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
 * |Δτ| ÷ τ (`decision-r08-t6b-floor.md` §5); on Earth there are some.
 */
async function checkPlanetTwins(
  engine: RenderEngine,
  ordered: OrderedChecks,
  entry: TwinMedium,
  read: AtmosphereMedium,
  ownTwin: Promise<OwnTwins>,
  twins: TwinPool,
): Promise<void> {
  const { medium, curves, radiusM } = entry;
  const tables = new AtmosphereTables(engine, medium, radiusM, curves);
  try {
    const gpu = await readPlanetTables(engine, tables);
    const overGpuTwin = twins.multiScattering(
      read,
      radiusM,
      storedTransmittance(gpu, read, curves),
    );
    // The per-planet tables alone are held in both arms (R08.T6.d): the kernel arm is drawn and read
    // back while the twin is computed, and its check reads it after the checks that came before it.
    const kernelArm =
      entry.scope === "tables" ? handled(readMultiScatteringArm(engine, tables)) : null;
    void ordered.later(async (checks) => {
      // The per-planet tables alone are held over the GPU's transmittance, in both arms: their own
      // multiple-scattering twin, end to end, is not formed (R08.T6.d, for the smoke's watchdog).
      const twin = await ownTwin;
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
      if (medium.pressureBar !== undefined) {
        checkTwin(
          checks,
          `${medium.name}: the pressure-column table (R08.T4.d)`,
          gpu.absorberPressureColumns,
          twin.absorberPressureColumns,
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
      const overGpu = await overGpuTwin;
      const own = twinAgreement(gpu.multiScattering, overGpu, false);
      const endToEnd =
        twin.multiScattering === null
          ? "the twin's own not formed"
          : describeEndToEnd(twinAgreement(gpu.multiScattering, twin.multiScattering, false));
      const inRange = withinHalfRange(gpu.multiScattering);
      checks.check(
        `R08.T6.b ${medium.name}: the multiple-scattering table agrees with the CPU twin to 10⁻³`,
        own.worst <= 1 && own.above > 0 && inRange,
        `over the GPU's transmittance, worst ${own.at}; ${own.above} channels above 10⁻⁶ of the largest (${show([own.largest])}), ${own.over} over the bound, ${own.floorCarried} carried by the floor; ${inRange ? "finite and within rgba16float" : "a channel is not finite or is past rgba16float"}; L₂ alone in ${secondOrderAlone(gpu.multiScattering)} channels (the twin ${secondOrderAlone(overGpu.texels)}); ${endToEnd}`,
      );
      if (kernelArm !== null) {
        checkMultiScatteringArm(checks, medium, await kernelArm, overGpu);
      }
    });
    // The kernel arm is read back before the tables go and the next GPU work begins.
    if (kernelArm !== null) {
      await kernelArm;
    }
  } finally {
    tables.dispose();
  }
}

/** The multiple-scattering table's agreement with the twin over the twin's own transmittance. */
function describeEndToEnd(result: TwinAgreement): string {
  return `over the twin's, worst ${result.at}, ${result.over} over`;
}

/**
 * The channels of a multiple-scattering table stored as L₂ alone, where its mean escape was 0
 * (`multiScattering.wgsl`, R08.T6.d): the sum of its alphas, each texel's count of them.
 */
function secondOrderAlone(texels: Float32Array | Float64Array): number {
  let alone = 0;
  for (let i = 3; i < texels.length; i += 4) {
    alone += texels[i] ?? 0;
  }
  return alone;
}

/**
 * A medium's multiple-scattering table in the kernel arm (`AtmosphereTables.buildKernelArm`), its
 * kernel's own `rgba32float` output over the stored transmittance, read back.
 */
async function readMultiScatteringArm(
  engine: RenderEngine,
  tables: AtmosphereTables,
): Promise<Float32Array> {
  const arm = tables.buildKernelArm();
  try {
    return new Float32Array(
      await engine.readTexture(arm.multiScattering, 0, undefined, "tolerance"),
    );
  } finally {
    engine.releaseTexture(arm.transmittance);
    engine.releaseTexture(arm.absorberColumns);
    engine.releaseTexture(arm.absorberPressureColumns);
    engine.releaseTexture(arm.lastAbsorberPressureColumn);
    engine.releaseTexture(arm.multiScattering);
  }
}

/**
 * The kernel arm's multiple-scattering table (`readMultiScatteringArm`) against `twin`, the twin
 * over the same stored transmittance: R08.T6.d's check of the Venus-class table, held to its twin
 * in both arms (`decision-r08-t6b-floor.md` §9.3), with its channels stored as L₂ alone.
 */
function checkMultiScatteringArm(
  checks: Checks,
  medium: AtmosphereMedium,
  own: Float32Array,
  twin: TwinTable,
): void {
  const result = twinAgreement(own, twin, false, { arm: "kernel" });
  const finite = own.every(Number.isFinite);
  checks.check(
    `R08.T6.d ${medium.name}: the multiple-scattering table, the kernel arm, agrees with the CPU twin to 10⁻³`,
    result.worst <= 1 && result.above > 0 && finite,
    `worst ${result.at}; largest ${show([result.largest])}; ${result.above} channels above 10⁻⁶ of the largest, ${result.over} over the bound; ${finite ? "finite" : "a channel is not finite"}; L₂ alone in ${secondOrderAlone(own)} channels (the twin ${secondOrderAlone(twin.texels)})`,
  );
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

/**
 * The twins' planet a frame's checks read, over the GPU's own per-planet tables: a sphere's, with
 * no body's gravity, whose twins are R08.T6.b's; or an oblate body's slices and bands, whose twins
 * are R08.T6.e's (R08.T6.f), Earth's among them, its per-view spheres R_α.
 */
type SmokePlanet =
  | { readonly kind: "sphere"; readonly twin: PlanetTwin }
  | { readonly kind: "oblate"; readonly twin: OblatePlanetTwin };

/** The per-planet tables an atmosphere built, read back, as the twins' planet for `read`. */
async function planetOf(
  engine: RenderEngine,
  atmosphere: HillaireAtmosphere,
  read: AtmosphereMedium,
  figure: SpheroidFigure,
): Promise<SmokePlanet> {
  const tables = await readPlanetTables(engine, atmosphere.tables);
  // An oblate figure's tables always carry a body, its gravity's or one of s ≡ 1 (`bodyOfFigure`).
  const body = atmosphere.tables.body ?? bodyOfFigure(figure);
  if (body !== null) {
    return { kind: "oblate", twin: oblatePlanetOf(tables, atmosphere.tables, read, body) };
  }
  // One layer a light slot under an absorber (R08.T7), read back together.
  const bySlot = layersOf(tables.multiScattering, atmosphere.tables.multiScatteringLayers).map(
    (layer) => asTwinTable(layer, MULTI_SCATTERING_SIZE),
  );
  const [first] = bySlot;
  if (first === undefined) {
    throw new Error("the multiple-scattering table has a layer");
  }
  return {
    kind: "sphere",
    twin: {
      medium: read,
      bottomRadiusM: tableRadiusM(figure),
      ...storedTransmittance(tables, read, atmosphere.tables.curves),
      multiScattering: first,
      ...(bySlot.length > 1 ? { multiScatteringBySlot: bySlot } : {}),
    },
  };
}

/**
 * An oblate body's twins' planet over the GPU's own tables (R08.T6.f): each κ slice's stored
 * transmittance tables and each band's multiple-scattering layers, as `oblatePlanetTwin` would have
 * built them; a band's own transmittance, which no per-view twin reads, stands as slice 0's.
 */
function oblatePlanetOf(
  gpu: PlanetReadback,
  tables: AtmosphereTables,
  read: AtmosphereMedium,
  body: AtmosphereBody,
): OblatePlanetTwin {
  const plan = tables.plan;
  const slices = storedSlices(gpu, read, tables.curves, plan.kappa.length);
  const perBand = tables.layersPerBand;
  const layers = layersOf(gpu.multiScattering, tables.multiScatteringLayers).map((layer) =>
    asTwinTable(layer, MULTI_SCATTERING_SIZE),
  );
  const [first] = slices;
  if (first === undefined) {
    throw new Error("the transmittance tables have a slice");
  }
  return {
    medium: read,
    body,
    referenceRadiusM: tableRadiusM(body.figure),
    curves: tables.curves,
    kappa: Float64Array.from(plan.kappa),
    slices,
    bands: plan.bands.map((band, b) => ({
      gravityRatio: band.gravityRatio,
      latitudeRad: band.latitudeRad,
      bottomRadiusM: band.bottomRadiusM,
      transmittance: first,
      multiScatteringBySlot: layers.slice(b * perBand, (b + 1) * perBand),
    })),
  };
}

/** A frame's three per-view twins, begun on the twin workers. */
interface FrameTwins {
  readonly skyView: Promise<TwinTable>;
  readonly aerial: Promise<TwinVolume>;
  readonly rayMarch: Promise<TwinTable>;
}

/**
 * Begins a frame's per-view twins on the twin workers: a sphere's (R08.T6.b) or an oblate body's
 * (R08.T6.e's, since R08.T6.f), over a scene of one reversed-Z depth.
 */
function frameTwinsOf(
  twins: TwinPool,
  planet: SmokePlanet,
  view: TwinView,
  sizes: TableSizes,
  depth: number,
): FrameTwins {
  const volume = sizes.aerialPerspective;
  const perSlice = sizes.aerialPerspectiveSamplesPerSlice;
  const reachM = sizes.aerialPerspectiveReachM;
  let begun: FrameTwins;
  switch (planet.kind) {
    case "sphere":
      begun = {
        skyView: twins.skyView(planet.twin, view, sizes.skyView, sizes.skyViewSamples),
        aerial: twins.aerialPerspective(planet.twin, view, volume, perSlice, reachM),
        rayMarch: twins.rayMarch(planet.twin, view, sizes, depth),
      };
      break;
    case "oblate":
      begun = {
        skyView: twins.oblateSkyView(planet.twin, view, sizes.skyView, sizes.skyViewSamples),
        aerial: twins.oblateAerialPerspective(planet.twin, view, volume, perSlice, reachM),
        rayMarch: twins.oblateRayMarch(planet.twin, view, sizes, depth),
      };
      break;
  }
  return begun;
}

/** A multiple-scattering readback's layers, each {@link MULTI_SCATTERING_SIZE}'s texels. */
function layersOf(texels: Float32Array, layers: number): Float32Array[] {
  const perLayer = MULTI_SCATTERING_SIZE.widthTexels * MULTI_SCATTERING_SIZE.heightTexels * 4;
  if (texels.length !== perLayer * layers) {
    throw new Error(`${texels.length / 4} texels read back for ${layers} layers`);
  }
  return Array.from({ length: layers }, (_, k) =>
    texels.subarray(k * perLayer, (k + 1) * perLayer),
  );
}

/**
 * Draws one frame, its twins begun on the twin workers first, and returns its per-frame outputs'
 * agreements with them over the GPU's tables, once the twins are done.
 */
async function frameAgreements(
  engine: RenderEngine,
  atmosphere: HillaireAtmosphere,
  planet: SmokePlanet,
  figure: SpheroidFigure,
  sizes: TableSizes,
  frame: TwinFrame,
  twins: TwinPool,
): Promise<() => Promise<FrameAgreements>> {
  const camera = equatorCamera(frame.heightM, FRAME_SIZE, figure);
  const view = frameViewOf(atmosphere, camera, frame.suns, figure);
  const surfaceDepth = frame.surfaceM === null ? 0 : NEAR_M / frame.surfaceM;
  const frameTwins = frameTwinsOf(twins, planet, view, sizes, surfaceDepth);
  const composite = await compositeFrame(engine, atmosphere, camera, frame.suns, frame.surfaceM);
  const gpu = await readFrameOutputs(engine, atmosphere);
  return async () => {
    const aerial = await frameTwins.aerial;
    return {
      skyView: twinAgreement(gpu.skyView, await frameTwins.skyView, false),
      aerial: twinAgreement(gpu.aerial, aerial, true),
      rayMarch: twinAgreement(gpu.rayMarch, await frameTwins.rayMarch, true),
      compositeWithinRange: withinHalfRange(composite),
    };
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
  ordered: OrderedChecks,
  entry: TwinMedium,
  read: AtmosphereMedium,
  setting: QualitySetting,
  twins: TwinPool,
): Promise<void> {
  const frames: readonly TwinFrame[] = [
    { name: "2 km, a surface 60 km ahead", heightM: 2_000, suns: [sunAt(20)], surfaceM: 60_000 },
    { name: "400 km, the limb", heightM: 400_000, suns: [sunAt(5)], surfaceM: null },
  ];
  const medium = entry.medium;
  const sizes = twinSizes(TABLE_SIZES[setting]);
  const atmosphere = new HillaireAtmosphere(
    engine,
    medium,
    sizes,
    WGS84,
    entry.curves,
    EARTH_GRAVITY,
  );
  try {
    const planet = await planetOf(engine, atmosphere, read, WGS84);
    for (const frame of frames) {
      // The harness's GPU work runs in order: each frame is read back before the next draws.
      // oxlint-disable-next-line no-await-in-loop
      const agreements = await frameAgreements(
        engine,
        atmosphere,
        planet,
        WGS84,
        sizes,
        frame,
        twins,
      );
      void ordered.later(async (checks) => {
        const result = await agreements();
        const label = `${medium.name}, ${setting}, ${frame.name}`;
        recordAgreement(checks, `${label}: the sky view`, result.skyView);
        recordAgreement(checks, `${label}: the aerial perspective`, result.aerial);
        recordAgreement(checks, `${label}: the ray march`, result.rayMarch);
        checks.check(
          `R08.T6.c ${label}: the composite, its read of the tables at the camera, is finite and within rgba16float's range`,
          result.compositeWithinRange,
          result.compositeWithinRange ? "every channel" : "a channel is not",
        );
      });
    }
  } finally {
    atmosphere.dispose();
  }
}

// --- R08.T7: several suns, and the sky through the air -------------------------------------------

/**
 * R08.T7's binary, a K5 (4,450 K) primary on light slot 0 and an M3 (3,300 K) companion on slot 1,
 * under the four-absorber medium with each light's own curves of growth (`fourAbsorberMedium`).
 */
const BINARY_LIGHTS_K = [4_450, 3_300] as const;

/** One degree, rad. */
const DEGREE_RAD = Math.PI / 180;

/**
 * A sky source at an elevation and an azimuth east of north at the equatorial camera (normal +x,
 * north +z, east +y), with its illuminance per channel, lx, and its light slot.
 */
function skySourceAt(
  elevationDeg: number,
  azimuthDeg: number,
  illuminanceLx: readonly [number, number, number],
  curveSlot: number,
  drawsDisc = true,
): SunState {
  const e = elevationDeg * DEGREE_RAD;
  const a = azimuthDeg * DEGREE_RAD;
  return {
    directionBodyFixed: {
      x: Math.sin(e),
      y: Math.cos(e) * Math.sin(a),
      z: Math.cos(e) * Math.cos(a),
    },
    illuminanceLx,
    angularRadiusRad: drawsDisc ? SUN_ANGULAR_RADIUS_RAD : 0.004_5,
    curveSlot,
    drawsDisc,
  };
}

/** The K5 primary at an elevation, north: `severalSuns.test.ts`'s illuminance. */
function primaryAt(elevationDeg: number): SunState {
  return skySourceAt(elevationDeg, 0, [9.1e4, 8.0e4, 5.9e4], 0);
}

/** The M3 companion at an elevation, 140° east of north. */
function companionAt(elevationDeg: number): SunState {
  return skySourceAt(elevationDeg, 140, [2.9e3, 1.8e3, 0.8e3], 1);
}

/** A full Moon's planetshine at 45°, 250° east of north, on slot 0, drawing no disc. */
const BINARY_PLANETSHINE = skySourceAt(45, 250, [0.3, 0.32, 0.33], 0, false);

/**
 * The binary's frames from 2 m over a surface 60 km ahead (the ray march's): by day, the primary
 * 20° up, the companion 5° and a planetshine source; and at twilight, the primary 8° and the
 * companion 3° below the horizon, feeding the sky through the planet's shadow in the tables.
 */
const BINARY_FRAMES: readonly TwinFrame[] = [
  {
    name: "2 m, the binary and planetshine up, a surface 60 km ahead",
    heightM: 2,
    suns: [primaryAt(20), companionAt(5), BINARY_PLANETSHINE],
    surfaceM: 60_000,
  },
  {
    name: "2 m, the binary set, a surface 60 km ahead",
    heightM: 2,
    suns: [primaryAt(-8), companionAt(-3)],
    surfaceM: 60_000,
  },
];

/**
 * R08.T7: the per-view tables summed over several suns in one march, held to their twins. Under
 * the binary's absorbers the multiple-scattering table has one layer a light, each against its
 * twin built with that light's curves over the GPU's transmittance; the binary's frames, each
 * source on its own light slot's curves and layer, agree with the twins to 10⁻³ on both settings;
 * and the composite keeps the scene's `METER_CLASS` in its output's alpha on all three of its paths.
 */
export async function checkSeveralSuns(
  engine: RenderEngine,
  checks: Checks,
  twins: TwinPool,
): Promise<void> {
  const binary = fourAbsorberMedium(BINARY_LIGHTS_K);
  const reading = attempt(() => kernelMedium(binary.medium));
  await inOrder(checks, async (ordered) => {
    const read = await reading;
    await checkLightLayers(engine, ordered, binary.medium, binary.curves, read, twins);
    for (const setting of ["high", "low"] as const) {
      // Each setting's frames are read back before the next setting's atmosphere draws.
      // oxlint-disable-next-line no-await-in-loop
      await checkBinaryFrames(engine, ordered, binary.medium, binary.curves, read, setting, twins);
    }
  });
}

/** The binary's per-planet multiple-scattering layers, each against its light's twin. */
async function checkLightLayers(
  engine: RenderEngine,
  ordered: OrderedChecks,
  medium: AtmosphereMedium,
  curves: AbsorberCurveSlots,
  read: AtmosphereMedium,
  twins: TwinPool,
): Promise<void> {
  const tables = new AtmosphereTables(engine, medium, EARTH_MEAN_RADIUS_M, curves);
  try {
    const gpu = await readPlanetTables(engine, tables);
    const stored = storedTransmittance(gpu, read, curves);
    const layers = tables.multiScatteringLayers;
    const layerTwins = Array.from({ length: layers }, (_, slot) =>
      twins.multiScattering(read, EARTH_MEAN_RADIUS_M, stored, slot),
    );
    void ordered.later(async (checks) => {
      const texels = layersOf(gpu.multiScattering, layers);
      checks.check(
        `R08.T7 ${medium.name}: the multiple-scattering table has one layer a light`,
        layers === BINARY_LIGHTS_K.length,
        `${layers} layers for ${BINARY_LIGHTS_K.length} lights`,
      );
      for (const [slot, twin] of layerTwins.entries()) {
        // The layers' twins are awaited in order, each after the one before it.
        // oxlint-disable-next-line no-await-in-loop
        const result = twinAgreement(texels[slot] ?? new Float32Array(0), await twin, false);
        checks.check(
          `R08.T7 ${medium.name}: the multiple-scattering layer of light slot ${slot}, built with its light's curves, agrees with the CPU twin to 10⁻³`,
          result.worst <= 1 &&
            result.above > 0 &&
            withinHalfRange(texels[slot] ?? new Float32Array(0)),
          `worst ${result.at}; ${result.above} channels above 10⁻⁶ of the largest, ${result.over} over the bound`,
        );
      }
      const [first, second] = texels;
      const apart = first !== undefined && second !== undefined ? largestApart(first, second) : 0;
      checks.check(
        `R08.T7 ${medium.name}: the two lights' layers differ by their curves`,
        apart > 1e-3,
        `largest relative difference ${apart.toExponential(2)}`,
      );
    });
  } finally {
    tables.dispose();
  }
}

/** The largest |a − b| ÷ |b| over the colour channels where b is above 10⁻⁶ of its largest. */
function largestApart(a: Float32Array, b: Float32Array): number {
  let most = 0;
  for (let i = 0; i < b.length; i += 4) {
    for (const c of [0, 1, 2]) {
      most = Math.max(most, Math.abs(b[i + c] ?? 0));
    }
  }
  let apart = 0;
  for (let i = 0; i < b.length; i += 4) {
    for (const c of [0, 1, 2]) {
      const y = b[i + c] ?? 0;
      if (Math.abs(y) > 1e-6 * most) {
        apart = Math.max(apart, Math.abs((a[i + c] ?? Number.NaN) - y) / Math.abs(y));
      }
    }
  }
  return apart;
}

/**
 * The binary's frames on a setting's counts against their twins over the GPU's tables, and the
 * composite's alpha over a scene of three meter classes.
 */
async function checkBinaryFrames(
  engine: RenderEngine,
  ordered: OrderedChecks,
  medium: AtmosphereMedium,
  curves: AbsorberCurveSlots,
  read: AtmosphereMedium,
  setting: QualitySetting,
  twins: TwinPool,
): Promise<void> {
  const sizes = twinSizes(TABLE_SIZES[setting]);
  const atmosphere = new HillaireAtmosphere(engine, medium, sizes, WGS84, curves, EARTH_GRAVITY);
  try {
    const planet = await planetOf(engine, atmosphere, read, WGS84);
    for (const frame of BINARY_FRAMES) {
      // The harness's GPU work runs in order: each frame is read back before the next draws.
      // oxlint-disable-next-line no-await-in-loop
      const agreements = await frameAgreements(
        engine,
        atmosphere,
        planet,
        WGS84,
        sizes,
        frame,
        twins,
      );
      void ordered.later(async (checks) => {
        const result = await agreements();
        const label = `${medium.name}, ${setting}, ${frame.name}`;
        const record = (what: string, held: TwinAgreement): void => {
          checks.check(
            `R08.T7 ${label}: the ${what}, summed over ${frame.suns.length} sources, agrees with the CPU twin to 10⁻³`,
            held.worst <= 1 && held.above > 0,
            `worst ${held.at}; ${held.above} channels above 10⁻⁶ of the largest, ${held.over} over the bound`,
          );
        };
        record("sky view", result.skyView);
        record("aerial perspective", result.aerial);
        record("ray march", result.rayMarch);
        checks.check(
          `R08.T7 ${label}: the composite is finite and within rgba16float's range`,
          result.compositeWithinRange,
          result.compositeWithinRange ? "every channel" : "a channel is not",
        );
      });
    }
    const [first] = BINARY_FRAMES;
    const classes = await compositeMeterClasses(
      engine,
      atmosphere,
      equatorCamera(2, FRAME_SIZE),
      first?.suns ?? [],
    );
    ordered.now((checks) => {
      checks.check(
        `R08.T7 ${medium.name}, ${setting}: the composite keeps each pixel's METER_CLASS in its output's alpha, over the sky, the aerial volume and the ray march`,
        classes.kept === classes.pixels && classes.classes.length === 3,
        `${classes.kept} of ${classes.pixels} pixels kept; the scene's classes ${classes.classes.join(", ")}`,
      );
    });
  } finally {
    atmosphere.dispose();
  }
}

/** What {@link compositeMeterClasses} found: the pixels whose alpha the composite kept. */
interface MeterClassesKept {
  readonly pixels: number;
  readonly kept: number;
  /** The distinct alphas the scene held, ascending. */
  readonly classes: readonly number[];
}

/**
 * Composites a frame over a scene of three meter classes in its alpha: a lit body 5 km ahead (the
 * aerial volume's path), an unlit one 60 km ahead (the ray march's) and the sky (nothing drawn, the
 * scene's clear), and compares the output's alpha with the scene's, pixel for pixel.
 */
async function compositeMeterClasses(
  engine: RenderEngine,
  atmosphere: HillaireAtmosphere,
  camera: AtmosphereCamera,
  suns: ReadonlyArray<SunState>,
): Promise<MeterClassesKept> {
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
  const scene = target("atmosphere meter-class scene");
  const output = target("atmosphere meter-class output");
  try {
    const material = engine.createMaterial(flatSpec("atmosphere meter-class body"));
    // Below the horizon, a wedge on the left and one on the right, each its own distance.
    const wedge = (name: string, side: -1 | 1, distanceM: number) =>
      triangleMesh(
        engine,
        name,
        [
          side * 0.05 * distanceM,
          -0.05 * distanceM,
          side * 10 * distanceM,
          -10 * distanceM,
          side * 0.05 * distanceM,
          -10 * distanceM,
        ],
        -distanceM,
      );
    scene.render(
      frameOf(
        "atmosphere meter-class scene",
        [
          drawOf(wedge("atmosphere lit body", -1, 5_000), material, [
            SURFACE_GREY,
            SURFACE_GREY,
            SURFACE_GREY,
            METER_CLASS.litBody,
          ]),
          drawOf(wedge("atmosphere unlit body", 1, 60_000), material, [
            SURFACE_GREY,
            SURFACE_GREY,
            SURFACE_GREY,
            METER_CLASS.unlitBody,
          ]),
        ],
        aspect,
      ),
    );
    const composite = atmosphere.drawFrame(camera, suns, {
      colour: scene.colour,
      depth: requireDepth(scene.depth),
      nearM: NEAR_M,
      exposureScale: SMOKE_EXPOSURE_SCALE,
    });
    output.render(frameOf("atmosphere meter-class composite", [composite], aspect));
    const before = halfTexels(await engine.readTexture(scene.colour));
    const after = halfTexels(await engine.readTexture(output.colour));
    let kept = 0;
    const classes = new Set<number>();
    for (let i = 3; i < before.length; i += 4) {
      classes.add(before[i] ?? Number.NaN);
      kept += before[i] === after[i] ? 1 : 0;
    }
    return { pixels: before.length / 4, kept, classes: [...classes].toSorted((a, b) => a - b) };
  } finally {
    scene.dispose();
    output.dispose();
  }
}

// --- R08.T6.g: heights and angles in f32 ---------------------------------------------------------

/** A sphere of radius `radiusM` as a figure. */
function sphereFigure(radiusM: number): SpheroidFigure {
  return { equatorialRadiusM: radiusM, polarRadiusM: radiusM };
}

/**
 * One body of R08.T6.g's and R08.T6.f's checks: its figure, its medium and gravity, its cameras'
 * heights, its limb cameras and any further view.
 */
interface HeightsBody {
  /**
   * The task whose check it is, which names the check: R08.T6.g's, R08.T6.i's slab Earth or
   * R08.T6.f's.
   */
  readonly task: string;
  readonly name: string;
  readonly figure: SpheroidFigure;
  readonly medium: AtmosphereMedium;
  readonly curves: AbsorberCurveSlots;
  /** The body's gravity (R08.T3.d), whose slicing the tables take (R08.T6.f); null for s ≡ 1. */
  readonly gravity: BodyGravity | null;
  /** The heights of the views on high, m: R08.T6.g's 2 m to 2 km, and 1 m on the 2 R⊕ body. */
  readonly heightsM: readonly number[];
  /** The limb cameras' distances from the centre, m, each looking along the equatorial limb. */
  readonly limbsM: readonly number[];
  /** Views on high beyond the heights' and the limbs' (R08.T6.f's Saturn-class figure's). */
  readonly moreViews: readonly HeightsView[];
}

/**
 * `decision-r08-f32.md` §6.2's spheres, each with R05's Earth medium, whose aerosol, 1.2 km in
 * scale height, is the most sensitive profile the kernels hold: a 2 R⊕ sphere (12,742,017.6 m, the
 * largest solid body the generator makes) and a sphere of Saturn's equatorial radius
 * (60,268,000 m). Earth (WGS 84), R08.T6.g's third body, is R08.T6.f's since its tables and views
 * stand on its slicing and spheres R_α ({@link SLICES_EARTH}); on a sphere they are R08.T6.g's.
 */
const HEIGHTS_BODIES: readonly HeightsBody[] = [
  {
    task: "R08.T6.g",
    name: "the 2 R⊕ sphere",
    figure: sphereFigure(12_742_017.6),
    medium: R05_EARTH_REFERENCE,
    curves: NO_ABSORBER_CURVES,
    gravity: null,
    heightsM: [1, 2, 20, 200, 2_000],
    limbsM: [],
    moreViews: [],
  },
  {
    task: "R08.T6.g",
    name: "the Saturn-radius sphere",
    figure: sphereFigure(60_268_000),
    medium: R05_EARTH_REFERENCE,
    curves: NO_ABSORBER_CURVES,
    gravity: null,
    heightsM: [2, 20, 200, 2_000],
    limbsM: [60_268_000 + 400_000],
    moreViews: [],
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
  /**
   * Whether the envelope also holds the twin's march at the split the kernel's `f32` may take
   * instead of the twin's, its share standing at its rounding ({@link SPLIT_RESOLUTION_STEPS};
   * R08.T6.f's sky view from above the air).
   */
  readonly split: boolean;
}

/**
 * How near its rounding a march's split share may stand, in steps, for the kernel's `f32` to take
 * the other side of it (`marchSteps.ts`'s marchSplit, floor(n √b ÷ (√b + √a) + ½)): 2⁻¹⁴. The
 * kernel forms each side's length, up to 10⁷ m, to about a metre, which leaves the share to about
 * n × 10⁻⁷ steps; the sky view from above the air holds its camera 1 m inside its shell, so that a
 * ray that dips and leaves through the top has its sides within metres of each other, and the
 * share of an odd count within about 10⁻⁵ of its rounding (R08.T6.f).
 */
const SPLIT_RESOLUTION_STEPS = 2 ** -14;

/**
 * Where a ray's split share stands against its rounding (`marchSteps.ts`'s marchSplit): the
 * argument floor takes, n √b ÷ (√b + √a) + ½, less the nearest integer, in steps.
 */
function splitRoundingOf(ray: TwinRay, samples: number): number {
  const o = ray.originM;
  const d = ray.direction;
  const nearestM = -(o.x * d.x + o.y * d.y + o.z * d.z);
  const lowestM = Math.min(Math.max(nearestM, ray.tStartM), ray.tEndM);
  const rootBefore = Math.sqrt(lowestM - ray.tStartM);
  const total = rootBefore + Math.sqrt(ray.tEndM - lowestM);
  if (!(total > 0)) {
    return 0.5;
  }
  const argument = (samples * rootBefore) / total + 0.5;
  return argument - Math.round(argument);
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
  planet: SmokePlanet,
  view: TwinView,
  sizes: TableSizes,
  twin: TwinTable,
): (index: number) => TexelEnvelope | null {
  const { widthTexels: w, heightTexels: h } = twin;
  const figure = view.figure;
  const p = view.cameraM;
  const offsetM = ORIGIN_RESOLUTION * Math.hypot(p.x, p.y, p.z);
  // A texel's ray, clipped and marched as the planet's twin clips and marches it: a sphere's
  // (R08.T6.b's) or an oblate body's (R08.T6.e's, R08.T6.f).
  const nominalAt = (u: number, v: number): RayMarchRay | null =>
    planet.kind === "sphere"
      ? rayMarchRayTwin(planet.twin.medium, view, u, v, 0, sizes)
      : oblateRayMarchRayTwin(planet.twin, view, u, v, 0, sizes);
  const along = (origin: Vec3, d: Vec3): RayMarchRay | null =>
    planet.kind === "sphere"
      ? rayMarchRayAlongTwin(planet.twin.medium, view, origin, d, Number.POSITIVE_INFINITY)
      : oblateRayMarchRayAlongTwin(planet.twin, view, origin, d, Number.POSITIVE_INFINITY);
  const texelOf = (ray: RayMarchRay | null): ArrayLike<number> =>
    planet.kind === "sphere"
      ? rayMarchTexelTwin(planet.twin, view, sizes, ray)
      : oblateRayMarchTexelTwin(planet.twin, view, sizes, ray);
  return memoised((index) => {
    const nominal = nominalAt(((index % w) + 0.5) / w, (Math.floor(index / w) + 0.5) / h);
    if (nominal === null) {
      return null;
    }
    const d = nominal.ray.direction;
    const values: Array<ArrayLike<number>> = [twin.texels.subarray(index * 4, index * 4 + 4)];
    for (const e of across(d)) {
      for (const sign of [1, -1]) {
        values.push(texelOf(along(p, turned(d, e, sign * RAY_RESOLUTION_RAD))));
        const moved = {
          x: p.x + e.x * sign * offsetM,
          y: p.y + e.y * sign * offsetM,
          z: p.z + e.z * sign * offsetM,
        };
        values.push(texelOf(along(moved, d)));
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
    return {
      ...rangeOf(values, 4),
      tangentHeightM: closest - figure.equatorialRadiusM,
      split: false,
    };
  });
}

/**
 * A sky-view texel's envelope: its ray turned by {@link SKY_VIEW_RESOLUTION_RAD} in zenith and in
 * azimuth, marched again in the twin on the camera's sphere: four rays beside the nominal.
 */
function skyViewEnvelope(
  planet: SmokePlanet,
  view: TwinView,
  sizes: TableSizes,
  twin: TwinTable,
): (index: number) => TexelEnvelope {
  if (planet.kind === "oblate") {
    return oblateSkyViewEnvelope(planet.twin, view, sizes, twin);
  }
  return sphereSkyViewEnvelope(planet.twin, view, sizes, twin);
}

/** {@link skyViewEnvelope} on a sphere's twins (R08.T6.g). */
function sphereSkyViewEnvelope(
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
  // The turned rays take the kernel's steps, cut at the medium's layer edges (R08.T6.i).
  const edgesM = mediumMarchEdgesM(medium);
  const sphere = { kind: "sphere", bottomRadiusM: bottomM } as const;
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
          marchTwin(medium, sources, sphere, again, piecedSteps(again, samples, edgesM, sphere))
            .luminance,
        );
      }
    }
    // The tangent height on the camera's sphere: r sin(zenith) less its radius.
    const r = ray.originM.z;
    return {
      ...rangeOf(values, 3),
      tangentHeightM: r * Math.hypot(d.x, d.y) - bottomM,
      split: false,
    };
  });
}

/**
 * {@link skyViewEnvelope} on an oblate body's twins (R08.T6.f): each texel's ray on its column's
 * sphere R_α, turned as on a sphere, marched again by R08.T6.e's oblate march.
 */
function oblateSkyViewEnvelope(
  planet: OblatePlanetTwin,
  view: TwinView,
  sizes: TableSizes,
  twin: TwinTable,
): (index: number) => TexelEnvelope {
  const samples = Math.max(Math.floor(sizes.skyViewSamples), 2);
  const w = twin.widthTexels;
  return memoised((index) => {
    const nominal = oblateSkyViewRayTwin(
      planet,
      view,
      index % w,
      Math.floor(index / w),
      sizes.skyView,
    );
    const { ray, bottomRadiusM, atmosphereM } = nominal;
    const shell = { bottomRadiusM, topRadiusM: bottomRadiusM + atmosphereM };
    const d = ray.direction;
    const values: Array<ArrayLike<number>> = [twin.texels.subarray(index * 4, index * 4 + 3)];
    for (const e of across(d)) {
      for (const sign of [1, -1]) {
        const direction = turned(d, e, sign * SKY_VIEW_RESOLUTION_RAD);
        const again = {
          ...nominal,
          ray: { ...ray, direction, tEndM: maxDistanceM(shell, ray.originM.z, direction.z) },
        };
        values.push(oblateSkyViewMarchTwin(planet, view, again, samples));
      }
    }
    // At its rounding the kernel may take the split the twin does not: a step moved to the side
    // before t* where the twin's share falls just short of it, from it where just over.
    const rounding = splitRoundingOf(ray, samples);
    const split = Math.abs(rounding) < SPLIT_RESOLUTION_STEPS;
    if (split) {
      values.push(oblateSkyViewMarchTwin(planet, view, nominal, samples, rounding < 0 ? 1 : -1));
    }
    const r = ray.originM.z;
    return {
      ...rangeOf(values, 3),
      tangentHeightM: r * Math.hypot(d.x, d.y) - bottomRadiusM,
      split,
    };
  });
}

/** The tangent height a limb camera's frame is centred on, and its half-span about it, m. */
const LIMB_CENTRE_M = 50_000;
const LIMB_HALF_SPAN_M = 150_000;

/**
 * Where a {@link limbCamera}'s frame looks, and where the camera stands (R08.T8.b): each member
 * absent takes R08.T6.g's limb frame, at longitude 0.
 */
interface LimbFrame {
  /** The tangent height the frame is centred on, m; {@link LIMB_CENTRE_M} where absent. */
  readonly centreM?: number;
  /**
   * The frame's half-span of tangent height at the tangent point, m, so that a row of the frame
   * spans 2 ÷ its height in rows of it (a sixteenth of it in the 64 × 32 frame);
   * {@link LIMB_HALF_SPAN_M} where absent.
   */
  readonly halfSpanM?: number;
  /**
   * The camera's longitude, rad: its position, its axes and the sun turned about the pole by it;
   * 0 where absent, where the camera lies on the x axis.
   */
  readonly longitudeRad?: number;
}

/** `v` turned about the pole (+z) by `angleRad`, from +x towards +y. */
function aboutPole(v: Vec3, angleRad: number): Vec3 {
  const cos = Math.cos(angleRad);
  const sin = Math.sin(angleRad);
  return { x: v.x * cos - v.y * sin, y: v.x * sin + v.y * cos, z: v.z };
}

/**
 * The distance from a {@link limbCamera} `distanceM` from the centre to the tangent point its
 * `frame` is centred on, m: the length its half-span of tangent height is seen across.
 */
function limbTangentDistanceM(
  distanceM: number,
  figure: SpheroidFigure,
  frame: LimbFrame = {},
): number {
  const sin = (figure.equatorialRadiusM + (frame.centreM ?? LIMB_CENTRE_M)) / distanceM;
  return distanceM * Math.sqrt(1 - sin * sin);
}

/**
 * A camera `distanceM` from the centre on the equator, looking along the equatorial limb at the
 * tangent height {@link LIMB_CENTRE_M}, its frame (64 × 32 unless `viewport` is given) spanning
 * ±{@link LIMB_HALF_SPAN_M} of tangent height, so that the air and the space beyond it fill its
 * rows, or at the centre, half-span and longitude `frame` gives. Nothing is drawn in front of it,
 * so the scene's own projection is not read.
 */
function limbCamera(
  distanceM: number,
  figure: SpheroidFigure,
  frame: LimbFrame = {},
  viewport: ViewSize = FRAME_SIZE,
): AtmosphereCamera {
  const longitudeRad = frame.longitudeRad ?? 0;
  const tangentM = figure.equatorialRadiusM + (frame.centreM ?? LIMB_CENTRE_M);
  const sin = tangentM / distanceM;
  const cos = Math.sqrt(1 - sin * sin);
  // At longitude 0 the camera stands on +x, the limb ahead in the plane z = 0.
  const forward = aboutPole({ x: -cos, y: sin, z: 0 }, longitudeRad);
  const up = aboutPole({ x: sin, y: cos, z: 0 }, longitudeRad);
  const right = { x: 0, y: 0, z: -1 };
  const toTangentM = limbTangentDistanceM(distanceM, figure, frame);
  const tanY = (frame.halfSpanM ?? LIMB_HALF_SPAN_M) / toTangentM;
  const aspect = viewport.widthPx / viewport.heightPx;
  return {
    positionM: aboutPole({ x: distanceM, y: 0, z: 0 }, longitudeRad),
    orientation: quaternionFromRows([
      { x: right.x, y: up.x, z: -forward.x },
      { x: right.y, y: up.y, z: -forward.y },
      { x: right.z, y: up.z, z: -forward.z },
    ]),
    fovXRad: 2 * Math.atan(aspect * tanY),
    viewport,
  };
}

/**
 * The sun at an elevation above the horizon of a {@link limbCamera}'s tangent point, ahead, for the
 * camera `frame` places.
 */
function limbSun(
  distanceM: number,
  figure: SpheroidFigure,
  elevationDeg: number,
  frame: LimbFrame = {},
): SunState {
  const sin = (figure.equatorialRadiusM + (frame.centreM ?? LIMB_CENTRE_M)) / distanceM;
  const cos = Math.sqrt(1 - sin * sin);
  const e = (elevationDeg * Math.PI) / 180;
  return {
    directionBodyFixed: aboutPole(
      {
        x: Math.cos(e) * -cos + Math.sin(e) * sin,
        y: Math.cos(e) * sin + Math.sin(e) * cos,
        z: 0,
      },
      frame.longitudeRad ?? 0,
    ),
    illuminanceLx: SUN_ILLUMINANCE_LX,
    angularRadiusRad: SUN_ANGULAR_RADIUS_RAD,
  };
}

/**
 * A camera `distanceM` from the polar axis in the plane y = 0, looking along +x over the north
 * pole's limb at the tangent height {@link LIMB_CENTRE_M}, its frame spanning
 * ±{@link LIMB_HALF_SPAN_M} of tangent height (R08.T6.f's Saturn-class limb over the pole).
 */
function polarLimbCamera(distanceM: number, figure: SpheroidFigure): AtmosphereCamera {
  const forward = { x: 1, y: 0, z: 0 };
  const up = { x: 0, y: 0, z: 1 };
  const right = { x: 0, y: -1, z: 0 };
  const tanY = LIMB_HALF_SPAN_M / distanceM;
  const aspect = FRAME_SIZE.widthPx / FRAME_SIZE.heightPx;
  return {
    positionM: { x: -distanceM, y: 0, z: figure.polarRadiusM + LIMB_CENTRE_M },
    orientation: quaternionFromRows([
      { x: right.x, y: up.x, z: -forward.x },
      { x: right.y, y: up.y, z: -forward.y },
      { x: right.z, y: up.z, z: -forward.z },
    ]),
    fovXRad: 2 * Math.atan(aspect * tanY),
    viewport: FRAME_SIZE,
  };
}

/** The sun at an elevation above the horizon of a {@link polarLimbCamera}'s tangent point, ahead. */
function polarLimbSun(elevationDeg: number): SunState {
  const e = (elevationDeg * Math.PI) / 180;
  return {
    directionBodyFixed: { x: Math.cos(e), y: 0, z: Math.sin(e) },
    illuminanceLx: SUN_ILLUMINANCE_LX,
    angularRadiusRad: SUN_ANGULAR_RADIUS_RAD,
  };
}

/**
 * A camera `heightM` above the geodetic latitude φ at longitude 0 of `figure`, looking north along
 * the horizon (R08.T6.f's Saturn-class figure's mid-latitude view).
 */
function latitudeCamera(
  figure: SpheroidFigure,
  latitudeRad: number,
  heightM: number,
): AtmosphereCamera {
  const a = figure.equatorialRadiusM;
  const e2 = 1 - (figure.polarRadiusM / a) ** 2;
  const sin = Math.sin(latitudeRad);
  const cos = Math.cos(latitudeRad);
  const n = a / Math.sqrt(1 - e2 * sin * sin);
  const up = { x: cos, y: 0, z: sin };
  const north = { x: -sin, y: 0, z: cos };
  const aspect = FRAME_SIZE.widthPx / FRAME_SIZE.heightPx;
  // Camera +x to east (+y here), +y to up and −z to north: the rows of that rotation.
  return {
    positionM: { x: (n + heightM) * cos, y: 0, z: (n * (1 - e2) + heightM) * sin },
    orientation: quaternionFromRows([
      { x: 0, y: up.x, z: -north.x },
      { x: 1, y: up.y, z: -north.y },
      { x: 0, y: up.z, z: -north.z },
    ]),
    fovXRad: 2 * Math.atan(aspect),
    viewport: FRAME_SIZE,
  };
}

/** The sun at an elevation above the horizon of a {@link latitudeCamera}, towards the north. */
function northSunAt(latitudeRad: number, elevationDeg: number): SunState {
  const e = (elevationDeg * Math.PI) / 180;
  const sin = Math.sin(latitudeRad);
  const cos = Math.cos(latitudeRad);
  // Up (cos φ, 0, sin φ) and north (−sin φ, 0, cos φ).
  return {
    directionBodyFixed: {
      x: Math.sin(e) * cos - Math.cos(e) * sin,
      y: 0,
      z: Math.sin(e) * sin + Math.cos(e) * cos,
    },
    illuminanceLx: SUN_ILLUMINANCE_LX,
    angularRadiusRad: SUN_ANGULAR_RADIUS_RAD,
  };
}

/**
 * How a check's channels that pass by the twin's envelope count (`decision-r08-f32.md` §10.3 and
 * §10.4): `capped`, at most {@link ENVELOPE_SHARE_CAP} of the check's channels; `missed`, at most
 * {@link FAR_LIMB_NEAR_SHARE_MISS}, a named miss of that cap (R08.T8.b's far limb from 10⁷ m);
 * `recorded`, their share recorded and not capped, every channel still within 10⁻³ or within its
 * envelope.
 */
type EnvelopeShare = "capped" | "missed" | "recorded";

/** The most of a check's channels that may pass by the envelope as `share` counts them. */
function envelopeCapOf(share: EnvelopeShare): number {
  let cap: number;
  switch (share) {
    case "capped":
      cap = ENVELOPE_SHARE_CAP;
      break;
    case "missed":
      cap = FAR_LIMB_NEAR_SHARE_MISS;
      break;
    case "recorded":
      cap = Number.POSITIVE_INFINITY;
      break;
  }
  return cap;
}

/** How a check's detail names the count of its envelope's channels. */
function describeEnvelopeShare(share: EnvelopeShare): string {
  let text: string;
  switch (share) {
    case "capped":
      text = `capped at ${percentOf(ENVELOPE_SHARE_CAP, ROUND_PERCENT)}`;
      break;
    case "missed":
      text = `capped at ${percentOf(FAR_LIMB_NEAR_SHARE_MISS, ROUND_PERCENT)}, a named miss of the`;
      text += ` ${percentOf(ENVELOPE_SHARE_CAP, ROUND_PERCENT)} cap`;
      break;
    case "recorded":
      text = "recorded, not capped";
      break;
  }
  return text;
}

/** One view of R08.T6.g's checks. */
interface HeightsView {
  readonly name: string;
  readonly camera: AtmosphereCamera;
  readonly sun: SunState;
  /**
   * The sun's elevation as the view's name gives it, deg: over the camera's horizon, or over its
   * tangent point's for a limb.
   */
  readonly sunElevationDeg: number;
  readonly surfaceM: number | null;
  /**
   * How the ray march's envelope channels count; `capped` where absent. `recorded` only for
   * R08.T8.b's far limb from 10⁸ and 10⁹ m (§10.4), where the tangent height's own resolution in
   * the march, 2ε|P| ≈ 12–120 m, is a sizeable share of the 1.2 km aerosol's scale height; `missed`
   * only for it from 10⁷ m ({@link FAR_LIMB_NEAR_SHARE_MISS}). The sky view's envelope stays capped
   * in every view: its texels' resolution does not depend on |P|.
   */
  readonly marchEnvelopeShare?: EnvelopeShare;
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
        sunElevationDeg: elevationDeg,
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
          sunElevationDeg: elevationDeg,
          surfaceM: null,
        });
      }
    }
    views.push(...body.moreViews);
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
      exposureScale: SMOKE_EXPOSURE_SCALE,
    };
    const composite = atmosphere.drawFrame(view.camera, [view.sun], atmosphereScene);
    output.render(frameOf("atmosphere composite", [composite], aspect));
    const compositeTexels = halfTexels(await engine.readTexture(output.colour));
    const stored = await readFrameOutputs(engine, atmosphere);
    const kernel = kernelArm
      ? await readKernelArm(
          engine,
          atmosphere.drawKernelArm(view.camera, [view.sun], atmosphereScene),
        )
      : null;
    return { stored, kernel, composite: compositeTexels };
  } finally {
    scene.dispose();
    output.dispose();
  }
}

/** How {@link recordHeights} names a check and counts its envelope channels. */
interface HeightsRecord {
  /** The arm as the check names it; `the stored arm` or `the kernel arm` where absent. */
  readonly label?: string;
  /** How the envelope channels count; `capped` where absent. */
  readonly envelopeShare?: EnvelopeShare;
}

/** The median of some values, NaN for none. */
function medianOf(values: readonly number[]): number {
  const sorted = values.toSorted((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  if (sorted.length % 2 === 1) {
    return sorted[middle] ?? Number.NaN;
  }
  return ((sorted[middle - 1] ?? Number.NaN) + (sorted[middle] ?? Number.NaN)) / 2;
}

/** A share of the twin's value, as the envelope's records give it: in units of 10⁻³. */
function perMille(share: number): string {
  return `${(share * 1e3).toPrecision(3)} × 10⁻³`;
}

/** How {@link percentOf} rounds: to `digits` decimal places, or to `digits` significant figures. */
interface PercentForm {
  readonly kind: "places" | "figures";
  readonly digits: number;
}

/**
 * A share as a percentage, as the checks' details give it: to two places unless `form` says
 * otherwise (a constant's round share to its one figure, a change that may be under 0.01%).
 */
function percentOf(share: number, form: PercentForm = { kind: "places", digits: 2 }): string {
  const percent = share * 100;
  const text =
    form.kind === "places" ? percent.toFixed(form.digits) : percent.toPrecision(form.digits);
  return `${text}%`;
}

/** {@link percentOf}'s form for a constant's round share: one significant figure. */
const ROUND_PERCENT: PercentForm = { kind: "figures", digits: 1 };

/** Envelope half-widths as the records give them: their median and the largest. */
function spreadOf(halfWidths: readonly number[]): string {
  if (halfWidths.length === 0) {
    return "no half-widths";
  }
  const median = perMille(medianOf(halfWidths));
  return `half-widths median ${median}, largest ${perMille(Math.max(...halfWidths))}`;
}

/** An envelope channel as its check records it: its texel, channel, tangent height and spread. */
function describeEnvelopeChannel(entry: EnvelopeChannel): string {
  return `${entry.where} c${entry.channel} at ${(entry.tangentHeightM / 1_000).toFixed(2)} km, spread ${(entry.halfWidth * 2 * 1e3).toFixed(2)} × 10⁻³`;
}

/**
 * Envelope channels summed up row by row, for a check whose share is over the cap (R08.T8.b): each
 * row's median tangent height, its count of channels, and its median and largest half-width
 * ({@link spreadOf}), so that a band shows without each channel's record.
 */
function envelopeRows(entries: readonly EnvelopeChannel[]): string {
  const rows = new Map<number, EnvelopeChannel[]>();
  for (const entry of entries) {
    const row = rows.get(entry.row);
    if (row === undefined) {
      rows.set(entry.row, [entry]);
    } else {
      row.push(entry);
    }
  }
  return [...rows.entries()]
    .toSorted(([a], [b]) => a - b)
    .map(([row, held]) => {
      const widths = held.map((entry) => entry.halfWidth);
      const tangentKm = medianOf(held.map((entry) => entry.tangentHeightM)) / 1_000;
      return `row ${row} (${tangentKm.toFixed(2)} km): ${held.length}, ${spreadOf(widths)}`;
    })
    .join("; ");
}

/**
 * Whether one arm's agreement holds as R08.T6.g's checks hold it: every channel within the bound or
 * its envelope, some above the floor, and the envelope channels' share within the cap `counting`
 * sets ({@link envelopeCapOf}).
 */
function heightsHeld(result: TwinAgreement, counting: EnvelopeShare): boolean {
  return (
    result.worst <= 1 &&
    result.above > 0 &&
    result.envelope.length / result.channels <= envelopeCapOf(counting)
  );
}

/**
 * Records one arm's agreement as `task`'s check (R08.T6.g's, or R08.T6.i's on the slab Earth), with
 * its largest value and its floor's count. Its envelope channels are recorded with their share, the
 * median and largest of their half-widths and the worst channel's place within its envelope, then
 * each channel while their share is within {@link ENVELOPE_SHARE_CAP}, or row by row
 * ({@link envelopeRows}) whenever it is over it, however they count (`decision-r08-f32.md` §10.4).
 */
function recordHeights(
  checks: Checks,
  task: string,
  name: string,
  result: TwinAgreement,
  arm: TwinOptions["arm"],
  record: HeightsRecord = {},
): void {
  const label = record.label ?? `the ${arm} arm`;
  const counting = record.envelopeShare ?? "capped";
  const limited = arm === "stored" && result.largest < FORMAT_LIMITED ? ", format-limited" : "";
  const share = result.envelope.length / result.channels;
  const widths = result.envelope.map((entry) => entry.halfWidth);
  const listed =
    share <= ENVELOPE_SHARE_CAP
      ? result.envelope.map(describeEnvelopeChannel).join("; ")
      : `by row, ${envelopeRows(result.envelope)}`;
  const envelope =
    result.envelope.length === 0
      ? `none by the envelope${counting === "capped" ? "" : ` (${describeEnvelopeShare(counting)})`}`
      : `${result.envelope.length} by the envelope (${percentOf(share)} of ${result.channels}, ${describeEnvelopeShare(counting)}; ${spreadOf(widths)}; worst at ${result.worstInEnvelope.toFixed(2)} of its half-width): ${listed}`;
  const split =
    result.split.length === 0
      ? ""
      : `; ${result.split.length} by the split's rounding (${percentOf(result.split.length / result.channels)}): ${result.split.map(describeEnvelopeChannel).join("; ")}`;
  // The kernel arm's floor is f32's own since R08.T8.b: say so where it carries a channel.
  const f32Floor =
    arm === "kernel" && result.floorCarried > 0
      ? " (f32's subnormal spacing, 2⁻¹⁴⁹: twin values under what f32 holds, as the aerial twin's" +
        " e^(−h ÷ H) unclipped at the top, 10⁻²⁰⁰ to 10⁻¹⁹⁴ from 10⁷ m, are a correct 0)"
      : "";
  checks.check(
    `${task} ${name}, ${label}, agrees with the CPU twin to 10⁻³`,
    heightsHeld(result, counting),
    `worst ${result.at}; largest ${show([result.largest])}${limited}; ${result.above} channels above 10⁻⁶ of the largest, ${result.over} over the bound, ${result.floorCarried} carried by the floor${f32Floor}; ${envelope}${split}`,
  );
}

/** The per-planet tables in the kernel arm (`AtmosphereTables.buildKernelArm`), read back. */
async function readKernelArmTables(
  engine: RenderEngine,
  tables: AtmosphereTables,
): Promise<{ readonly transmittance: Float32Array; readonly multiScattering: Float32Array }> {
  const arm = tables.buildKernelArm();
  try {
    return {
      transmittance: new Float32Array(
        await engine.readTexture(arm.transmittance, 0, undefined, "tolerance"),
      ),
      multiScattering: new Float32Array(
        await engine.readTexture(arm.multiScattering, 0, undefined, "tolerance"),
      ),
    };
  } finally {
    engine.releaseTexture(arm.transmittance);
    engine.releaseTexture(arm.absorberColumns);
    engine.releaseTexture(arm.absorberPressureColumns);
    engine.releaseTexture(arm.lastAbsorberPressureColumn);
    engine.releaseTexture(arm.multiScattering);
  }
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
  ordered: OrderedChecks,
  body: HeightsBody,
  atmosphere: HillaireAtmosphere,
  read: AtmosphereMedium,
  transmittanceOfMedium: Promise<TwinTable>,
  twins: TwinPool,
): Promise<SmokePlanet> {
  const bottomM = tableRadiusM(body.figure);
  const gpu = await readPlanetTables(engine, atmosphere.tables);
  const stored = storedTransmittance(gpu, read, atmosphere.tables.curves);
  const multiScatteringOverGpu = twins.multiScattering(read, bottomM, stored);
  // The kernel arm is drawn and read back while the twins are computed, and its checks read it
  // after the twins, as they came in sequence.
  const armReadback = handled(readKernelArmTables(engine, atmosphere.tables));
  void ordered.later(async (checks) => {
    const transmittance = await transmittanceOfMedium;
    const multiScattering = await multiScatteringOverGpu;
    const { transmittance: armTransmittance, multiScattering: armMultiScattering } =
      await armReadback;
    const label = `${body.name}: the per-planet`;
    recordHeights(
      checks,
      body.task,
      `${label} transmittance table, as optical depth`,
      twinAgreement(gpu.transmittance, transmittance, false),
      "stored",
    );
    recordHeights(
      checks,
      body.task,
      `${label} transmittance table, as optical depth`,
      twinAgreement(armTransmittance, transmittance, false, { arm: "kernel" }),
      "kernel",
    );
    recordHeights(
      checks,
      body.task,
      `${label} multiple-scattering table, over the GPU's transmittance`,
      twinAgreement(gpu.multiScattering, multiScattering, false),
      "stored",
    );
    recordHeights(
      checks,
      body.task,
      `${label} multiple-scattering table, over the GPU's transmittance`,
      twinAgreement(armMultiScattering, multiScattering, false, { arm: "kernel" }),
      "kernel",
    );
  });
  // The kernel arm is read back before the frames draw.
  await armReadback;
  return {
    kind: "sphere",
    twin: {
      medium: read,
      bottomRadiusM: bottomM,
      ...stored,
      multiScattering: asTwinTable(gpu.multiScattering, MULTI_SCATTERING_SIZE),
    },
  };
}

/** A latitude band's tables and their twins (R08.T6.f). */
interface BandTwins {
  readonly band: TablesBand;
  /** The band's own transmittance table as the GPU stored it. */
  readonly stored: Float32Array;
  /** Its twin of the medium in the band's frame. */
  readonly ofMedium: Promise<TwinTransmittance>;
  /** The band's twin over the GPU's own transmittance tables. */
  readonly overGpu: Promise<TwinBand>;
}

/**
 * Latitude band `band`'s own transmittance tables built anew (`AtmosphereTables.bandTransmittance`),
 * read back and released (R08.T6.f).
 */
async function readBandTransmittance(
  engine: RenderEngine,
  tables: AtmosphereTables,
  band: number,
): Promise<PlanetReadback> {
  const made: TransmittanceTextures = tables.bandTransmittance(band);
  const read = async (texture: TextureHandle): Promise<Float32Array> =>
    halfTexels(await engine.readTexture(texture, 0, undefined, "tolerance"));
  try {
    return {
      transmittance: await read(made.transmittance),
      absorberColumns: await read(made.absorberColumns),
      absorberPressureColumns: await read(made.absorberPressureColumns),
      lastAbsorberPressureColumn: await read(made.lastAbsorberPressureColumn),
      multiScattering: new Float32Array(0),
    };
  } finally {
    engine.releaseTexture(made.transmittance);
    engine.releaseTexture(made.absorberColumns);
    engine.releaseTexture(made.absorberPressureColumns);
    engine.releaseTexture(made.lastAbsorberPressureColumn);
  }
}

/**
 * The per-planet tables of an oblate body's `atmosphere` against their twins in both arms (R08.T6.f):
 * each κ slice's transmittance against the twin of the medium on its sphere κ_k R_ref; each latitude
 * band's own transmittance, built anew as the build builds it (`AtmosphereTables.bandTransmittance`),
 * against its twin in the band's frame, stored; and each band's multiple-scattering layers against
 * R08.T6.e's band (`twinBandOf`) over the GPU's band transmittance. The kernel arm reads the same
 * stored tables.
 *
 * @returns The twins' planet over the GPU's stored tables, which the per-view twins read.
 */
async function checkOblatePlanetArms(
  engine: RenderEngine,
  ordered: OrderedChecks,
  body: HeightsBody,
  atmosphereBody: AtmosphereBody,
  atmosphere: HillaireAtmosphere,
  read: AtmosphereMedium,
  slicesOfMedium: ReadonlyArray<Promise<TwinTransmittance>>,
  twins: TwinPool,
): Promise<SmokePlanet> {
  const tables = atmosphere.tables;
  const plan = tables.plan;
  const gpu = await readPlanetTables(engine, tables);
  const planet = oblatePlanetOf(gpu, tables, read, atmosphereBody);
  const curves = tables.curves;
  const perBand = tables.layersPerBand;
  const bands: BandTwins[] = [];
  for (const [b, band] of plan.bands.entries()) {
    // Each band's tables are read back before the next band's are built.
    // oxlint-disable-next-line no-await-in-loop
    const own = await readBandTransmittance(engine, tables, b);
    bands.push({
      band,
      stored: own.transmittance,
      ofMedium: twins.transmittanceTables(read, band.bottomRadiusM, curves, band.coefficientFactor),
      overGpu: twins.oblateBand(
        read,
        atmosphereBody,
        band.gravityRatio,
        curves,
        plan.bands.length === 1,
        storedTransmittance(own, read, curves),
      ),
    });
  }
  // The kernel arm is drawn and read back while the twins are computed, and its checks read it
  // after the twins, as they came in sequence.
  const armReadback = handled(readKernelArmTables(engine, tables));
  void ordered.later(async (checks) => {
    const arm = await armReadback;
    const label = `${body.name}: the per-planet`;
    const slices = plan.kappa.length;
    for (const [k, ofMedium] of slicesOfMedium.entries()) {
      // The slices' twins are awaited in order, each after the one before it.
      // oxlint-disable-next-line no-await-in-loop
      const twin = (await ofMedium).transmittance;
      const kappa = plan.kappa[k] ?? Number.NaN;
      const name = `${label} transmittance slice ${k} of ${slices} (κ ${kappa.toFixed(4)}), as optical depth`;
      recordHeights(
        checks,
        body.task,
        name,
        twinAgreement(layerOf(gpu.transmittance, slices, k), twin, false),
        "stored",
      );
      recordHeights(
        checks,
        body.task,
        name,
        twinAgreement(layerOf(arm.transmittance, slices, k), twin, false, { arm: "kernel" }),
        "kernel",
      );
    }
    const layers = tables.multiScatteringLayers;
    for (const [b, { band, stored, ofMedium, overGpu }] of bands.entries()) {
      const at = `band ${b} of ${plan.bands.length} (s ${band.gravityRatio.toFixed(4)})`;
      // The bands' twins are awaited in order, each after the one before it.
      // oxlint-disable-next-line no-await-in-loop
      const own = await ofMedium;
      recordHeights(
        checks,
        body.task,
        `${label} ${at}'s own transmittance table, as optical depth`,
        twinAgreement(stored, own.transmittance, false),
        "stored",
      );
      // The band's multiple scattering over the GPU's tables is awaited after its own table's.
      // oxlint-disable-next-line no-await-in-loop
      const twin = await overGpu;
      for (const [slot, layer] of twin.multiScatteringBySlot.entries()) {
        const index = b * perBand + slot;
        const name = `${label} ${at}'s multiple-scattering layer of light slot ${slot}, over the GPU's transmittance`;
        recordHeights(
          checks,
          body.task,
          name,
          twinAgreement(layerOf(gpu.multiScattering, layers, index), layer, false),
          "stored",
        );
        recordHeights(
          checks,
          body.task,
          name,
          twinAgreement(layerOf(arm.multiScattering, layers, index), layer, false, {
            arm: "kernel",
          }),
          "kernel",
        );
      }
    }
  });
  // The kernel arm is read back before the frames draw.
  await armReadback;
  return { kind: "oblate", twin: planet };
}

/** One arm's ray march against its twin in one view, as {@link checkViewArms} recorded it. */
interface MarchArm {
  /** The view's sun's elevation, deg ({@link HeightsView.sunElevationDeg}). */
  readonly sunElevationDeg: number;
  readonly arm: TwinOptions["arm"];
  readonly result: TwinAgreement;
}

/** What {@link checkViewArms} leaves its caller (R08.T8.b's far limb records them by distance). */
interface ViewArms {
  /** The view's ray-march agreements, each arm's, once its twins are in and its checks recorded. */
  readonly marches: Promise<readonly MarchArm[]>;
}

/** One view's three per-frame kernels against their twins, in each arm the frame drew. */
async function checkViewArms(
  engine: RenderEngine,
  ordered: OrderedChecks,
  atmosphere: HillaireAtmosphere,
  planet: SmokePlanet,
  body: HeightsBody,
  sizes: TableSizes,
  setting: QualitySetting,
  view: HeightsView,
  twins: TwinPool,
): Promise<ViewArms> {
  // The view's twins begin on the twin workers, and the frame draws while they run.
  const twin = frameViewOf(atmosphere, view.camera, [view.sun], body.figure);
  const surfaceDepth = view.surfaceM === null ? 0 : NEAR_M / view.surfaceM;
  const frameTwins = frameTwinsOf(twins, planet, twin, sizes, surfaceDepth);
  const frame = await drawArms(engine, atmosphere, view, setting === "high");
  const record = async (checks: Checks): Promise<readonly MarchArm[]> => {
    const held: MarchArm[] = [];
    const label = `${body.name}, ${setting}, ${view.name}`;
    checks.check(
      `${body.task} ${label}: the composite's frame is finite and within rgba16float`,
      withinHalfRange(frame.composite),
      `mean ${show(meanRgb(frame.composite))}`,
    );
    const skyView = await frameTwins.skyView;
    const aerial = await frameTwins.aerial;
    const rayMarch = await frameTwins.rayMarch;
    // Only a view from above the atmosphere's top may pass a texel by the twin's envelope
    // (`decision-r08-f32.md` §10.3); from inside it the bound holds outright. The aerial volume has
    // none: from above the top it holds no air, and it passes outright everywhere. On an oblate body
    // the camera's h* is held against the top (R08.T6.f).
    const scaledHeightM =
      twin.heightM * atmosphereInputs(view.camera, body.figure, body.gravity).gravityRatio;
    const fromAbove = scaledHeightM >= body.medium.topHeightM;
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
          body.task,
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
          body.task,
          `${label}: the aerial perspective`,
          twinAgreement(floats.aerial, aerial, true, { arm }),
          arm,
        );
        const march = twinAgreement(
          floats.rayMarch,
          rayMarch,
          true,
          marchEnvelope === undefined ? { arm } : { arm, envelope: marchEnvelope },
        );
        recordHeights(checks, body.task, `${label}: the ray march`, march, arm, {
          envelopeShare: view.marchEnvelopeShare ?? "capped",
        });
        held.push({ sunElevationDeg: view.sunElevationDeg, arm, result: march });
      }
    }
    return held;
  };
  return { marches: ordered.later(record) };
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
 * shipped kernel. The medium is R05's Earth, which R08.T6.g measured these figures on: the group
 * holds the kernels' geometry, and R08.T6.b's twins hold the rebuilt Earth (R08.T6.d).
 *
 * The twins run on the page's twin workers (`twins`) beside the GPU's work, and the checks are
 * recorded in the order they were in sequence (`ordered.ts`): each body's transmittance twin, which
 * reads nothing of the GPU's, begins with the group, and each twin over the GPU's tables as soon as
 * they are read back.
 */
export async function checkAtmosphereHeights(
  engine: RenderEngine,
  checks: Checks,
  twins: TwinPool,
): Promise<void> {
  const bodies = HEIGHTS_BODIES.map((body) => {
    const read = kernelMedium(body.medium);
    return { body, read, transmittance: twins.transmittance(read, tableRadiusM(body.figure)) };
  });
  await inOrder(checks, async (ordered) => {
    for (const { body, read, transmittance } of bodies) {
      let planet: SmokePlanet | null = null;
      for (const setting of ["high", "low"] as const) {
        const sizes = twinSizes(TABLE_SIZES[setting]);
        const atmosphere = new HillaireAtmosphere(
          engine,
          body.medium,
          sizes,
          body.figure,
          body.curves,
          body.gravity,
        );
        try {
          // The per-planet tables are the same on both settings: held, and twinned, once.
          // The harness's GPU work runs in order: the tables are read back before the first frame.
          // oxlint-disable-next-line no-await-in-loop
          planet ??= await checkPlanetArms(
            engine,
            ordered,
            body,
            atmosphere,
            read,
            transmittance,
            twins,
          );
          for (const view of heightsViews(body, setting)) {
            // The harness's GPU work runs in order: each frame is read back before the next draws.
            // oxlint-disable-next-line no-await-in-loop
            await checkViewArms(
              engine,
              ordered,
              atmosphere,
              planet,
              body,
              sizes,
              setting,
              view,
              twins,
            );
          }
        } finally {
          atmosphere.dispose();
        }
      }
    }
  });
}

// --- R08.T6.i: the marches cut at layer edges ---------------------------------------------------

/** R08.T6.i's body: Earth (WGS 84) with T6.d's ozone slab, whose base and top are layer edges. */
const SLAB_EARTH: HeightsBody = {
  task: "R08.T6.i",
  name: "the slab Earth",
  figure: WGS84,
  medium: EARTH_SLAB_REFERENCE,
  curves: EARTH_SLAB_REFERENCE_CURVES,
  gravity: EARTH_GRAVITY,
  heightsM: [2, 2_000],
  limbsM: [WGS84.equatorialRadiusM + 400_000],
  moreViews: [],
};

/**
 * R08.T6.i's three views: from 2 m and 2 km with the sun 20° up and a surface 60 km ahead, and the
 * limb from 400 km with the sun 20° up at its tangent point, so that the group adds a bounded time
 * to the smoke (`decision-r08-t6d-findings.md` §8).
 */
function layerEdgeViews(): HeightsView[] {
  return [
    ...SLAB_EARTH.heightsM.map((heightM) => ({
      name: `${heightM} m, the sun 20°, a surface 60 km ahead`,
      camera: equatorCamera(heightM, FRAME_SIZE, SLAB_EARTH.figure),
      sun: sunAt(20),
      sunElevationDeg: 20,
      surfaceM: 60_000,
    })),
    ...SLAB_EARTH.limbsM.map((distanceM) => ({
      name: `the limb from ${(distanceM - SLAB_EARTH.figure.equatorialRadiusM).toExponential(2)} m, the sun 20° at its tangent point`,
      camera: limbCamera(distanceM, SLAB_EARTH.figure),
      sun: limbSun(distanceM, SLAB_EARTH.figure, 20),
      sunElevationDeg: 20,
      surfaceM: null,
    })),
  ];
}

/**
 * R08.T6.i: the sky view and the ray march cut each side of their rays' split at the medium's layer
 * edges (`common.wgsl`'s `MarchWalk`), held against their twins on the slab Earth (`earth.ts`'s
 * `EARTH_SLAB_REFERENCE`, its ozone a slab from 70 to 3 hPa with ramps of 78 and 10 m), in R08.T6.g's
 * two arms, on high, over the GPU's own per-planet tables.
 *
 * @remarks
 * Three views ({@link layerEdgeViews}): from the ground every ray that climbs out crosses the slab's
 * base and top, and the limb's rays cross each twice. The per-planet tables are not twinned here,
 * for the smoke's watchdog: R08.T6.b's group holds the multiple-scattering kernel, which cuts its
 * steps too, on the four-absorber fixture, whose layers have edges, and that fixture's per-frame
 * kernels on both settings. The medium the kernels read is `kernelMedium`'s, whose edges are widened
 * to their levels; the twins take it as given. The twins run on the page's twin workers (`twins`)
 * beside the GPU's work, the checks recorded in the order they were in sequence (`ordered.ts`).
 */
export async function checkAtmosphereLayerEdges(
  engine: RenderEngine,
  checks: Checks,
  twins: TwinPool,
): Promise<void> {
  const read = kernelMedium(EARTH_SLAB_REFERENCE);
  const sizes = twinSizes(TABLE_SIZES.high);
  await inOrder(checks, async (ordered) => {
    const atmosphere = new HillaireAtmosphere(
      engine,
      EARTH_SLAB_REFERENCE,
      sizes,
      SLAB_EARTH.figure,
      EARTH_SLAB_REFERENCE_CURVES,
      EARTH_GRAVITY,
    );
    try {
      const planet = await planetOf(engine, atmosphere, read, SLAB_EARTH.figure);
      for (const view of layerEdgeViews()) {
        // The harness's GPU work runs in order: each frame is read back before the next draws.
        // oxlint-disable-next-line no-await-in-loop
        await checkViewArms(
          engine,
          ordered,
          atmosphere,
          planet,
          SLAB_EARTH,
          sizes,
          "high",
          view,
          twins,
        );
      }
    } finally {
      atmosphere.dispose();
    }
  });
}

// --- R08.T6.f: slices and bands, the kernels ----------------------------------------------------

/**
 * R08.T6.f's Earth: WGS 84 with its normal gravity (one slice and one band, s within ±0.27% of 1,
 * every per-view sphere R_α), R05's Earth medium, on R08.T6.g's heights and limbs, whose views were
 * R08.T6.g's until R08.T6.f put them on the oblate twins.
 */
const SLICES_EARTH: HeightsBody = {
  task: "R08.T6.f",
  name: "Earth",
  figure: WGS84,
  medium: R05_EARTH_REFERENCE,
  curves: NO_ABSORBER_CURVES,
  gravity: EARTH_GRAVITY,
  heightsM: [2, 20, 200, 2_000],
  limbsM: [WGS84.equatorialRadiusM + 400_000, 4.2e7],
  moreViews: [],
};

/** Saturn's figure and gravity (`atmosphereFixtures.ts`): five slices and four bands. */
const SATURN_BODY: AtmosphereBody = saturnBody();

/** 45°, the Saturn-class figure's mid-latitude view, where s and R_α vary along every ray. */
const SATURN_MID_LATITUDE_RAD = Math.PI / 4;

/**
 * R08.T6.f's Saturn-class figure (Design note 17): Saturn's datum and gravity, five slices and four
 * bands, under the H₂–He air of R08.T6.e's tests (`saturnClassMedium`, its top 16 scale heights up),
 * on R08.T6.g's views from 2 m, 20 m, 200 m and 2 km at the equator, the limb from above the air
 * over the equator and over the pole, and a view from 2 km at 45° with the sun along the meridian.
 */
function slicesSaturn(): HeightsBody {
  const figure = SATURN_BODY.figure;
  const medium = saturnClassMedium();
  // A camera whose h* is above the top (795 km): 1,000 km over the equator (h* 868 km), and over
  // the pole's limb 795 km above the datum at 81.5° (h* about 905 km).
  const equatorialLimbM = figure.equatorialRadiusM + 1_000_000;
  const polarLimbM = 10_000_000;
  return {
    task: "R08.T6.f",
    name: "the Saturn-class figure",
    figure,
    medium,
    curves: NO_ABSORBER_CURVES,
    gravity: SATURN_BODY.gravity,
    heightsM: [2, 20, 200, 2_000],
    limbsM: [equatorialLimbM],
    moreViews: [
      {
        name: "the limb over the pole from above the air, the sun 20° at its tangent point",
        camera: polarLimbCamera(polarLimbM, figure),
        sun: polarLimbSun(20),
        sunElevationDeg: 20,
        surfaceM: null,
      },
      {
        name: "2 km at 45°, the sun 20° up towards the north, a surface 60 km ahead",
        camera: latitudeCamera(figure, SATURN_MID_LATITUDE_RAD, 2_000),
        sun: northSunAt(SATURN_MID_LATITUDE_RAD, 20),
        sunElevationDeg: 20,
        surfaceM: 60_000,
      },
    ],
  };
}

/**
 * R08.T6.f: the kernels with T6.e's slicing (`oblate.wgsl`) against R08.T6.e's oblate twin, in
 * R08.T6.g's two arms, on Earth (`checkAtmosphereSlicesEarth`) and on a Saturn-class figure
 * (`checkAtmosphereSlicesSaturn`): each κ slice's transmittance table and each latitude band's
 * own transmittance and multiple-scattering layers (`checkOblatePlanetArms`), and the per-view
 * kernels on R08.T6.g's views, from inside the air outright and from above it under
 * `decision-r08-f32.md` §10's envelope.
 *
 * @remarks
 * The twins run on the page's twin workers beside the GPU's work, the checks recorded in the order
 * they were in sequence (`ordered.ts`).
 */
async function checkSlices(
  engine: RenderEngine,
  checks: Checks,
  twins: TwinPool,
  body: HeightsBody,
): Promise<void> {
  const gravity = body.gravity;
  if (gravity === null) {
    throw new Error(`R08.T6.f's ${body.name} has a body's gravity`);
  }
  const atmosphereBody: AtmosphereBody = { figure: body.figure, gravity };
  const read = kernelMedium(body.medium);
  const plan = tablesPlanOf(atmosphereBody, tableRadiusM(body.figure));
  // Each slice's twin of the medium, which reads nothing of the GPU's, begins with the group.
  const slicesOfMedium = plan.sliceRadiiM.map((radiusM) =>
    twins.transmittanceTables(read, radiusM, body.curves),
  );
  await inOrder(checks, async (ordered) => {
    let planet: SmokePlanet | null = null;
    for (const setting of ["high", "low"] as const) {
      const sizes = twinSizes(TABLE_SIZES[setting]);
      const atmosphere = new HillaireAtmosphere(
        engine,
        body.medium,
        sizes,
        body.figure,
        body.curves,
        gravity,
      );
      try {
        // The per-planet tables are the same on both settings: held, and twinned, once.
        // oxlint-disable-next-line no-await-in-loop
        planet ??= await checkOblatePlanetArms(
          engine,
          ordered,
          body,
          atmosphereBody,
          atmosphere,
          read,
          slicesOfMedium,
          twins,
        );
        for (const view of heightsViews(body, setting)) {
          // The harness's GPU work runs in order: each frame is read back before the next draws.
          // oxlint-disable-next-line no-await-in-loop
          await checkViewArms(
            engine,
            ordered,
            atmosphere,
            planet,
            body,
            sizes,
            setting,
            view,
            twins,
          );
        }
      } finally {
        atmosphere.dispose();
      }
    }
  });
}

/** R08.T6.f on Earth ({@link SLICES_EARTH}; `checkSlices`). */
export async function checkAtmosphereSlicesEarth(
  engine: RenderEngine,
  checks: Checks,
  twins: TwinPool,
): Promise<void> {
  await checkSlices(engine, checks, twins, SLICES_EARTH);
}

/** R08.T6.f on the Saturn-class figure ({@link slicesSaturn}; `checkSlices`). */
export async function checkAtmosphereSlicesSaturn(
  engine: RenderEngine,
  checks: Checks,
  twins: TwinPool,
): Promise<void> {
  await checkSlices(engine, checks, twins, slicesSaturn());
}

// --- R08.T8.a: the march band --------------------------------------------------------------------

/**
 * R08.T8.a's Earth: R08.T6.f's ({@link SLICES_EARTH}), WGS 84 with its normal gravity on the
 * oblate twins and R05's Earth medium, its views the march band's ({@link bandViews}) rather than
 * R08.T6.g's heights and limbs.
 */
const BAND_EARTH: HeightsBody = { ...SLICES_EARTH, task: "R08.T8.a", heightsM: [], limbsM: [] };

/** How far below the visible horizon's dip a {@link bandCamera}'s frame reaches, rad: 0.5°. */
const BAND_FRAME_BELOW_DIP_RAD = 0.5 * DEGREE_RAD;

/** How far above the horizontal a {@link bandCamera}'s frame reaches, rad: 3°. */
const BAND_FRAME_ABOVE_RAD = 3 * DEGREE_RAD;

/**
 * The dip below the horizontal of the datum's visible horizon in the meridian plane, rad, from a
 * camera `heightM` above the equator of `figure`.
 */
function meridianDipRad(heightM: number, figure: SpheroidFigure): number {
  const a = figure.equatorialRadiusM;
  // The ellipse x²/a² + z²/c² = 1 touched by the line from the camera at (a + h, 0), whose dip
  // below the horizontal has tan = √(h (2a + h)) ÷ c.
  return Math.atan(Math.sqrt(heightM * (2 * a + heightM)) / figure.polarRadiusM);
}

/**
 * A camera `heightM` above the equator at longitude 0 of `figure`, looking north along the
 * horizon, pitched so that its frame (64 × 32 unless `viewport` is given) spans from
 * {@link BAND_FRAME_BELOW_DIP_RAD} below the visible horizon's dip ({@link meridianDipRad}) to
 * {@link BAND_FRAME_ABOVE_RAD} above the horizontal: the ground's last rows, the limb's rays, which
 * pass below the camera and climb out, and the sky above. Nothing is drawn in front of it, so the
 * scene's own projection is not read.
 */
function bandCamera(
  heightM: number,
  figure: SpheroidFigure,
  viewport: ViewSize = FRAME_SIZE,
): AtmosphereCamera {
  const a = figure.equatorialRadiusM;
  const bottomRad = -(meridianDipRad(heightM, figure) + BAND_FRAME_BELOW_DIP_RAD);
  const pitchRad = (BAND_FRAME_ABOVE_RAD + bottomRad) / 2;
  const tanY = Math.tan((BAND_FRAME_ABOVE_RAD - bottomRad) / 2);
  // Body +x is up and +z north at the camera: forward and up pitched by `pitchRad`, east (+y) to
  // the right.
  const forward = { x: Math.sin(pitchRad), y: 0, z: Math.cos(pitchRad) };
  const up = { x: Math.cos(pitchRad), y: 0, z: -Math.sin(pitchRad) };
  const right = { x: 0, y: 1, z: 0 };
  const aspect = viewport.widthPx / viewport.heightPx;
  return {
    positionM: { x: a + heightM, y: 0, z: 0 },
    orientation: quaternionFromRows([
      { x: right.x, y: up.x, z: -forward.x },
      { x: right.y, y: up.y, z: -forward.y },
      { x: right.z, y: up.z, z: -forward.z },
    ]),
    fovXRad: 2 * Math.atan(aspect * tanY),
    viewport,
  };
}

/** A camera height of R08.T8.a's views: its h\*, m, and what it stands for. */
interface BandPlace {
  readonly label: string;
  readonly heightStarM: number;
}

/** The middle of `setting`'s march band, where the march's share w is ½. */
function bandMiddle(setting: QualitySetting): BandPlace {
  const { lowerShareOfTop, upperShareOfTop } = TABLE_SIZES[setting].marchBand;
  return {
    label: "the band's middle",
    heightStarM: ((lowerShareOfTop + upperShareOfTop) / 2) * BAND_EARTH.medium.topHeightM,
  };
}

/**
 * s = g ÷ g_ref on `body`'s equator (Design note 17), where every {@link bandCamera} stands: a
 * camera there at h\* stands h\* ÷ s above the datum.
 */
function equatorGravityRatio(body: HeightsBody): number {
  const { figure, gravity } = body;
  return atmosphereInputs(
    { positionM: { x: figure.equatorialRadiusM, y: 0, z: 0 } },
    figure,
    gravity,
  ).gravityRatio;
}

/**
 * One R08.T8.a view: a {@link bandCamera} on the equator at `place`'s h\*, the sun ahead at an
 * elevation above the horizontal, nothing drawn, so that every pixel is sky.
 */
function bandView(place: BandPlace, elevationDeg: number): HeightsView {
  const { figure } = BAND_EARTH;
  // s at the equator, where the camera stands at every height: its geodetic height is h* ÷ s.
  const s = equatorGravityRatio(BAND_EARTH);
  return {
    name: `h* ${(place.heightStarM / 1_000).toFixed(1)} km (${place.label}), the sun ${elevationDeg}°, the sky from inside`,
    camera: bandCamera(place.heightStarM / s, figure),
    sun: sunAt(elevationDeg),
    sunElevationDeg: elevationDeg,
    surfaceM: null,
  };
}

/**
 * R08.T8.a's views on a setting: on high, from the band's middle (w = ½) and from 100 m under the
 * top (w = 1), each with the sun 20° up and 2° below the horizon; on low, from its band's middle
 * with the sun 20° up.
 */
function bandViews(setting: QualitySetting): HeightsView[] {
  const middle = bandMiddle(setting);
  if (setting === "low") {
    return [bandView(middle, 20)];
  }
  const underTop = {
    label: "100 m under the top",
    heightStarM: BAND_EARTH.medium.topHeightM - 100,
  };
  return [middle, underTop].flatMap((place) => [20, -2].map((e) => bandView(place, e)));
}

/**
 * A march band wholly above the blend's camera, whose h\* is 0.4 of the top: w = 0 under it, the
 * sky view alone.
 */
const BAND_ABOVE_CAMERA: MarchBand = { lowerShareOfTop: 0.6, upperShareOfTop: 0.8 };

/** A march band wholly below the blend's camera: w = 1 under it, the ray march alone. */
const BAND_BELOW_CAMERA: MarchBand = { lowerShareOfTop: 0.1, upperShareOfTop: 0.2 };

/**
 * The composite's blend's bound, as a share of the largest of the three outputs it compares: 2⁻¹⁰,
 * with {@link HALF_SUBNORMAL_SPACING}'s 2⁻²⁴ added.
 *
 * @remarks
 * Each output is rounded once to `rgba16float`: by at most 2⁻¹¹ of itself where it is normal and
 * 2⁻²⁵ below 2⁻¹⁴. (1 − w) c₀ + w c₁ carries c₀'s and c₁'s roundings, weighted to at most 2⁻¹¹ of
 * the larger, and c_w its own: 2⁻¹⁰ of the largest in all, 2⁻²⁴ where they are subnormal. The
 * kernels' `f32` arithmetic adds some 2⁻²³ of it.
 */
const BLEND_SHARE = 2 ** -10;

/** The composite's blend against its two ends, channel by channel. */
interface BlendAgreement {
  /** The worst |c_w − ((1 − w) c₀ + w c₁)| as a fraction of its bound: at most 1 to agree. */
  readonly worst: number;
  readonly at: string;
  /** The largest colour value of the three outputs. */
  readonly largest: number;
  /** The channels the floor of 2⁻²⁴ carries: over 2⁻¹⁰ of the largest, within the bound. */
  readonly floorCarried: number;
  /** The channels where c₀ and c₁ differ by more than the bound, which the check tells apart. */
  readonly distinct: number;
  /** The channels compared. */
  readonly channels: number;
}

/** A value to six figures, for a blend's worst channel, whose bound is 2⁻¹⁰ of it. */
function precise(value: number): string {
  return value.toPrecision(6);
}

/** A march band as its edges' shares of the top. */
function bandOf(band: MarchBand): string {
  return `${band.lowerShareOfTop}–${band.upperShareOfTop} of the top`;
}

/**
 * The composite at the march's share `weight` (c_w) against (1 − w) c₀ + w c₁, its outputs with
 * the sky view's read alone (c₀) and the march's alone (c₁), each channel within 2⁻¹⁰ of the
 * largest of the three plus 2⁻²⁴ ({@link BLEND_SHARE}).
 */
function blendAgreement(
  blended: Float32Array,
  tables: Float32Array,
  march: Float32Array,
  weight: number,
  widthPx: number,
): BlendAgreement {
  const channels = blended.length;
  if (tables.length !== channels || march.length !== channels) {
    return {
      worst: Number.POSITIVE_INFINITY,
      at: `${channels / 4}, ${tables.length / 4} and ${march.length / 4} texels read back`,
      largest: Number.NaN,
      floorCarried: 0,
      distinct: 0,
      channels,
    };
  }
  let worst = 0;
  let at = "every channel exact";
  let largest = 0;
  let floorCarried = 0;
  let distinct = 0;
  for (let i = 0; i < channels; i += 1) {
    const cw = blended[i] ?? Number.NaN;
    const c0 = tables[i] ?? Number.NaN;
    const c1 = march[i] ?? Number.NaN;
    const expected = (1 - weight) * c0 + weight * c1;
    const larger = Math.max(Math.abs(cw), Math.abs(c0), Math.abs(c1));
    const relative = BLEND_SHARE * larger;
    const allowed = relative + HALF_SUBNORMAL_SPACING;
    const delta = Math.abs(cw - expected);
    // The colour's largest: alpha carries the scene's meter class through (R08.T7).
    largest = i % 4 === 3 ? largest : Math.max(largest, larger);
    floorCarried += delta > relative && delta <= allowed ? 1 : 0;
    distinct += Math.abs(c1 - c0) > allowed ? 1 : 0;
    const ratio = Number.isNaN(delta) ? Number.POSITIVE_INFINITY : delta / allowed;
    if (ratio > worst) {
      const texelIndex = Math.floor(i / 4);
      worst = ratio;
      at = `texel (${texelIndex % widthPx}, ${Math.floor(texelIndex / widthPx)}) channel ${i % 4}: composite ${precise(cw)}, the blend ${precise(expected)} of ${precise(c0)} and ${precise(c1)}`;
    }
  }
  return { worst, at, largest, floorCarried, distinct, channels };
}

/**
 * The composite's blend on the GPU (R08.T8.a): `view`, from inside the setting's march band, drawn
 * three times, by `atmosphere` on the setting's band and by two more on `sizes` with a band wholly
 * above the camera (w = 0) and one wholly below it (w = 1), and held to the blend of the two ends
 * ({@link blendAgreement}), w as the CPU forms it (`hillaire.ts`'s `marchWeightOf`).
 */
async function checkCompositeBlend(
  engine: RenderEngine,
  ordered: OrderedChecks,
  atmosphere: HillaireAtmosphere,
  sizes: TableSizes,
  view: HeightsView,
): Promise<void> {
  const body = BAND_EARTH;
  const inputs = atmosphereInputs(view.camera, body.figure, body.gravity);
  const heightStarM = inputs.heightM * inputs.gravityRatio;
  const weightOf = (band: MarchBand): number =>
    marchWeightOf(heightStarM, body.medium.topHeightM, band);
  const suns = [view.sun];
  // The same frame on each band, each read back before the next draws.
  const frameOn = async (marchBand: MarchBand): Promise<Float32Array> => {
    const other = new HillaireAtmosphere(
      engine,
      body.medium,
      { ...sizes, marchBand },
      body.figure,
      body.curves,
      body.gravity,
    );
    try {
      return await compositeFrame(engine, other, view.camera, suns, null);
    } finally {
      other.dispose();
    }
  };
  const blended = await compositeFrame(engine, atmosphere, view.camera, suns, null);
  const tables = await frameOn(BAND_ABOVE_CAMERA);
  const march = await frameOn(BAND_BELOW_CAMERA);
  ordered.now((checks) => {
    const weight = weightOf(sizes.marchBand);
    const above = weightOf(BAND_ABOVE_CAMERA);
    const below = weightOf(BAND_BELOW_CAMERA);
    // The kernels read w as the uniform narrows it, to f32.
    const result = blendAgreement(
      blended,
      tables,
      march,
      Math.fround(weight),
      view.camera.viewport.widthPx,
    );
    const label = `${body.task} ${body.name}, high, ${view.name}`;
    checks.check(
      `${label}: the composites on the setting's band and on bands above and below the camera are finite and within rgba16float`,
      [blended, tables, march].every(withinHalfRange),
      `means ${show(meanRgb(blended))}; ${show(meanRgb(tables))}; ${show(meanRgb(march))}`,
    );
    checks.check(
      `${label}: the composite blends the sky view's and the march's by the march's share w, to rgba16float's rounding (2⁻¹⁰ of the largest of the three, plus 2⁻²⁴)`,
      result.worst <= 1 &&
        weight > 0 &&
        weight < 1 &&
        above === 0 &&
        below === 1 &&
        result.distinct > 0,
      `w ${weight} by marchWeightOf at h* ${heightStarM.toFixed(3)} m (the band ${bandOf(sizes.marchBand)}; ${Math.fround(weight)} in f32), ${above} on a band ${bandOf(BAND_ABOVE_CAMERA)}, ${below} on one ${bandOf(BAND_BELOW_CAMERA)}; worst ${result.worst.toFixed(3)} of the bound at ${result.at}; largest ${show([result.largest])}; ${result.floorCarried} channels carried by the floor; ${result.distinct} of ${result.channels} channels where the two ends differ by more than the bound`,
    );
  });
}

/**
 * R08.T8.a: the ray march's new regime, the sky from inside the air, held where it takes over from
 * the sky-view table in the march band (plan R08, R08.T8; `TableSizes.marchBand`), and the
 * composite's blend of the two.
 *
 * @remarks
 * On R08.T6.f's Earth ({@link BAND_EARTH}), R05's Earth medium on WGS 84 with its normal gravity
 * and the oblate twins, from cameras inside the air looking along the horizon, their frames from
 * just below the visible horizon's dip to 3° above the horizontal ({@link bandViews}), nothing
 * drawn: on high from the band's middle (w = ½) and from 100 m under the top (w = 1), the sun 20°
 * up and 2° below the horizon; on low from its band's middle, the sun 20° up. The three per-frame
 * kernels are held against their twins in R08.T6.g's two arms (`checkViewArms`), the kernel arm on
 * high, outright, as from inside the air: every pixel's ray march is a sky pixel's, its two-sided
 * segments under the sun-aware warp, in both arms. The per-planet tables are read back for the
 * twins but not twinned here: R08.T6.f's Earth group holds them. Then the composite's blend on the GPU
 * ({@link checkCompositeBlend}), from the high band's middle with the sun 20° up, its disc out of
 * the frame so that no pixel stands at the half float's clamp. The twins run on the page's twin
 * workers (`twins`) beside the GPU's work, the checks recorded in the order they were in sequence
 * (`ordered.ts`).
 */
export async function checkMarchBand(
  engine: RenderEngine,
  checks: Checks,
  twins: TwinPool,
): Promise<void> {
  const body = BAND_EARTH;
  const read = kernelMedium(body.medium);
  await inOrder(checks, async (ordered) => {
    let planet: SmokePlanet | null = null;
    for (const setting of ["high", "low"] as const) {
      const sizes = twinSizes(TABLE_SIZES[setting]);
      const atmosphere = new HillaireAtmosphere(
        engine,
        body.medium,
        sizes,
        body.figure,
        body.curves,
        body.gravity,
      );
      try {
        // The per-planet tables are the same on both settings: read back once.
        // oxlint-disable-next-line no-await-in-loop
        planet ??= await planetOf(engine, atmosphere, read, body.figure);
        for (const view of bandViews(setting)) {
          // The harness's GPU work runs in order: each frame is read back before the next draws.
          // oxlint-disable-next-line no-await-in-loop
          await checkViewArms(
            engine,
            ordered,
            atmosphere,
            planet,
            body,
            sizes,
            setting,
            view,
            twins,
          );
        }
        if (setting === "high") {
          // After the high views' frames, before the low setting's tables are made.
          // oxlint-disable-next-line no-await-in-loop
          await checkCompositeBlend(
            engine,
            ordered,
            atmosphere,
            sizes,
            bandView(bandMiddle(setting), 20),
          );
        }
      } finally {
        atmosphere.dispose();
      }
    }
  });
}

// --- R08.T8.b: low's pick, the composite from above the top, and the far limb -------------------

/**
 * The factor the composite multiplies the march's rgb and each disc by (`composite.wgsl`'s
 * `view.sky.w`): 1, since the per-view tables hold pre-exposed luminance (R08.T7;
 * `HillaireAtmosphere.drawFrame` sets it so, and {@link frameViewOf} weights the twins' sources by
 * the pre-exposure to match).
 */
const COMPOSITE_FACTOR = 1;

/**
 * Half an `rgba16float` step as a share of a normal value it rounds, 2⁻¹¹, and below 2⁻¹⁴ half
 * the subnormals' spacing, 2⁻²⁵: the output's rounding to nearest.
 */
const HALF_ROUNDING = 2 ** -11;
const HALF_SUBNORMAL_ROUNDING = HALF_SUBNORMAL_SPACING / 2;

/** `f32`'s two roundings of colour × alpha + rgb, as a share of |colour × alpha| + |rgb|: 2⁻²³. */
const F32_TWO_ROUNDINGS = 2 ** -23;

/** Low's pick's camera, m above the equator: R08.T6.g's limb from 400 km. */
const PICK_HEIGHT_M = 400_000;

/** A surface over the lower right of a frame: the pixels from a column and a row on. */
interface PickSurface {
  /** The first column it covers, px: its left edge on the boundary before it. */
  readonly fromXPx: number;
  /** The first row it covers, px: its top edge on the boundary before it. */
  readonly fromYPx: number;
  /** How far ahead it stands, m, perpendicular to the view: one depth over all of it. */
  readonly distanceM: number;
}

/**
 * The depth edge's surface (R08.T8.b): from column 33 and row 17 of the 64 × 32 frame, its edges
 * on odd boundaries, so that the pixel before each edge (column 32, row 16), whose nearest texel's
 * source pixel (33, or 17) is covered, keeps an uncovered texel, the depth-nearest; 2,000 km ahead,
 * past the limb rays' entry into the air, so that its texels carry light and an alpha under 1.
 */
const PICK_SURFACE: PickSurface = { fromXPx: 33, fromYPx: 17, distanceM: 2_000_000 };

/** Whether a surface covers pixel (x, y) of a frame. */
type Coverage = (xPx: number, yPx: number) => boolean;

/** The coverage of a frame with nothing drawn. */
function uncovered(): boolean {
  return false;
}

/** A march texel's place, (x, y). */
type TexelPlace = readonly [number, number];

/**
 * The view-space rectangle at `surface`'s distance whose left and top edges fall on the pixel
 * boundaries before its first column and row under the harness's projection (`harness.ts`,
 * tan(fov_y ÷ 2) = 1: x_ndc = x ÷ (aspect · d), y_ndc = y ÷ d), reaching past the frame's right
 * and bottom: corners (lower left, upper right), m.
 */
function pickRectangle(
  surface: PickSurface,
  size: ViewSize,
): readonly [number, number, number, number] {
  const d = surface.distanceM;
  const aspect = size.widthPx / size.heightPx;
  const left = ((2 * surface.fromXPx) / size.widthPx - 1) * aspect * d;
  const top = (1 - (2 * surface.fromYPx) / size.heightPx) * d;
  return [left, -4 * d, 4 * aspect * d, top];
}

/** A frame of low's pick's checks, read back: the composite, the scene's colour and the march. */
interface PickFrame {
  readonly composite: Float32Array;
  readonly scene: Float32Array;
  readonly march: Float32Array;
}

/**
 * Draws one composite frame for `suns` over a scene that is black at depth 0 but for `surface`,
 * if any, drawn {@link SURFACE_GREY}, and reads back the output, the scene's colour and the march
 * target.
 */
async function pickFrame(
  engine: RenderEngine,
  atmosphere: HillaireAtmosphere,
  camera: AtmosphereCamera,
  suns: ReadonlyArray<SunState>,
  surface: PickSurface | null,
): Promise<PickFrame> {
  const drawn: SceneSurface | null =
    surface === null
      ? null
      : (size) =>
          drawOf(
            rectangleMesh(
              engine,
              "atmosphere pick surface",
              pickRectangle(surface, size),
              -surface.distanceM,
            ),
            engine.createMaterial(flatSpec("atmosphere pick surface")),
            [SURFACE_GREY, SURFACE_GREY, SURFACE_GREY, 1],
          );
  return drawComposite(
    engine,
    atmosphere,
    camera,
    suns,
    { label: "atmosphere pick", surface: drawn, preExposure: SMOKE_EXPOSURE_SCALE },
    async (output, scene) => {
      const { rayMarch } = atmosphere.frameTables;
      if (rayMarch === null) {
        throw new Error("the ray march drew no target");
      }
      return {
        composite: halfTexels(await engine.readTexture(output.colour)),
        scene: halfTexels(await engine.readTexture(scene.colour)),
        march: halfTexels(await engine.readTexture(rayMarch, 0, undefined, "tolerance")),
      };
    },
  );
}

/**
 * The march texel `composite.wgsl`'s `rayMarched` reads for pixel (x, y) at the scale `marchScale`
 * under 1 (low's ½), emulated in `f64`, where it is exact: of the candidates base and base + 1 on
 * each axis, base = trunc(p · scale − ½), each clamped to the target, the one whose source pixel
 * min(⌊(t + ½) ÷ scale⌋, full − 1) has the depth nearest the pixel's, then of those that tie the
 * one whose centre (t + ½) ÷ scale is nearest the pixel's centre p + ½, and of those the first in
 * the shader's order (rows, then columns), its comparisons strict. A depth's error is 0 where the
 * source pixel is covered as the pixel is and above 0 elsewhere, since every covered pixel holds
 * the surface's one depth and every other 0.
 */
function pickedTexel(
  xPx: number,
  yPx: number,
  viewport: ViewSize,
  marchScale: number,
  covered: Coverage,
): TexelPlace {
  const { widthPx: w, heightPx: h } = viewport;
  const sizeX = Math.ceil(w * marchScale);
  const sizeY = Math.ceil(h * marchScale);
  const baseX = Math.trunc(xPx * marchScale - 0.5);
  const baseY = Math.trunc(yPx * marchScale - 0.5);
  const own = covered(xPx, yPx);
  let best: TexelPlace = [0, 0];
  let bestError = Number.POSITIVE_INFINITY;
  let bestDistance = Number.POSITIVE_INFINITY;
  for (let j = 0; j < 2; j += 1) {
    for (let i = 0; i < 2; i += 1) {
      const tx = Math.min(Math.max(baseX + i, 0), sizeX - 1);
      const ty = Math.min(Math.max(baseY + j, 0), sizeY - 1);
      const sourceX = Math.min(Math.floor((tx + 0.5) / marchScale), w - 1);
      const sourceY = Math.min(Math.floor((ty + 0.5) / marchScale), h - 1);
      const error = covered(sourceX, sourceY) === own ? 0 : 1;
      const dx = (tx + 0.5) / marchScale - (xPx + 0.5);
      const dy = (ty + 0.5) / marchScale - (yPx + 0.5);
      const distance = dx * dx + dy * dy;
      if (error < bestError || (error === bestError && distance < bestDistance)) {
        best = [tx, ty];
        bestError = error;
        bestDistance = distance;
      }
    }
  }
  return best;
}

/**
 * The texel R05.T12.c's pick took wherever the four depths tie, the first candidate: trunc(p ·
 * scale − ½) on each axis, clamped (`decision-r08-t8-leans.md` §6.2).
 */
function firstCandidate(
  xPx: number,
  yPx: number,
  viewport: ViewSize,
  marchScale: number,
): TexelPlace {
  const clamped = (p: number, full: number): number =>
    Math.min(Math.max(Math.trunc(p * marchScale - 0.5), 0), Math.ceil(full * marchScale) - 1);
  return [clamped(xPx, viewport.widthPx), clamped(yPx, viewport.heightPx)];
}

/** The texel whose centre is nearest pixel (x, y)'s: ⌊p · scale⌋ on each axis. */
function nearestTexel(xPx: number, yPx: number, marchScale: number): TexelPlace {
  return [Math.floor(xPx * marchScale), Math.floor(yPx * marchScale)];
}

/** A frame's pixels against the march texels a pick names, each channel within its bound. */
interface PickAgreement {
  readonly pixels: number;
  /** The pixels with a colour channel over its bound. */
  readonly over: number;
  /** The worst |output − expected| as a fraction of its bound. */
  readonly worst: number;
  readonly at: string;
}

/** One pixel against its march texel ({@link pickedPixel}). */
interface PickedPixel {
  /** The worst channel's |output − expected| as a fraction of its bound. */
  readonly ratio: number;
  /** That channel, its output and what the texel draws. */
  readonly text: string;
}

/**
 * Whether pixel `k`'s colour channels agree with what the composite draws from above the top
 * from march texel `t` (`composite.wgsl`'s fragment): over the sky, depth 0, the scene's colour
 * (black) times the transmittance plus rgb × the factor; over a surface, its colour × alpha +
 * rgb × the factor. Each channel is held to half an `rgba16float` step of its value
 * ({@link HALF_ROUNDING}, 2⁻²⁵ among the subnormals), plus `f32`'s two roundings of a surface's
 * terms.
 */
function pickedPixel(
  frame: PickFrame,
  marchWidth: number,
  k: number,
  t: TexelPlace,
  surface: boolean,
): PickedPixel {
  const m = (t[1] * marchWidth + t[0]) * 4;
  const alpha = frame.march[m + 3] ?? Number.NaN;
  let ratio = 0;
  let text = "";
  for (let c = 0; c < 3; c += 1) {
    const rgb = (frame.march[m + c] ?? Number.NaN) * COMPOSITE_FACTOR;
    const colour = surface ? (frame.scene[k * 4 + c] ?? Number.NaN) * alpha : 0;
    const expected = colour + rgb;
    const terms = surface ? F32_TWO_ROUNDINGS * (Math.abs(colour) + Math.abs(rgb)) : 0;
    const bound = HALF_ROUNDING * Math.abs(expected) + HALF_SUBNORMAL_ROUNDING + terms;
    const actual = frame.composite[k * 4 + c] ?? Number.NaN;
    const off = Math.abs(actual - expected) / bound;
    const channelRatio = Number.isNaN(off) ? Number.POSITIVE_INFINITY : off;
    if (channelRatio >= ratio) {
      ratio = channelRatio;
      const drawn = `texel (${t[0]}, ${t[1]})'s ${precise(expected)}`;
      text = `channel ${c}: output ${precise(actual)}, ${drawn}`;
    }
  }
  return { ratio, text };
}

/**
 * A frame's pixels against the march texel `pick` names for each, from above the top, over the
 * pixels `within` keeps ({@link pickedPixel}).
 */
function pickAgreement(
  frame: PickFrame,
  viewport: ViewSize,
  marchScale: number,
  covered: Coverage,
  pick: (xPx: number, yPx: number) => TexelPlace,
  within: (xPx: number, yPx: number) => boolean = () => true,
): PickAgreement {
  const marchWidth = Math.ceil(viewport.widthPx * marchScale);
  let pixels = 0;
  let over = 0;
  let worst = 0;
  let at = "none";
  for (let y = 0; y < viewport.heightPx; y += 1) {
    for (let x = 0; x < viewport.widthPx; x += 1) {
      if (within(x, y)) {
        const k = y * viewport.widthPx + x;
        const { ratio, text } = pickedPixel(frame, marchWidth, k, pick(x, y), covered(x, y));
        pixels += 1;
        over += ratio > 1 ? 1 : 0;
        if (ratio > worst) {
          worst = ratio;
          at = `pixel (${x}, ${y}) ${text}`;
        }
      }
    }
  }
  return { pixels, over, worst, at };
}

/** Whether a pixel stands within two rows or columns of a surface's edge: both sides in reach. */
function nearEdge(covered: Coverage, viewport: ViewSize): (xPx: number, yPx: number) => boolean {
  return (xPx, yPx) => {
    let some = false;
    let none = false;
    for (let y = Math.max(yPx - 2, 0); y <= Math.min(yPx + 2, viewport.heightPx - 1); y += 1) {
      for (let x = Math.max(xPx - 2, 0); x <= Math.min(xPx + 2, viewport.widthPx - 1); x += 1) {
        if (covered(x, y)) {
          some = true;
        } else {
          none = true;
        }
      }
    }
    return some && none;
  };
}

/** The pixels whose scene colour shows their coverage: grey where covered, black elsewhere. */
function coverageShown(scene: Float32Array, viewport: ViewSize, covered: Coverage): number {
  let shown = 0;
  for (let y = 0; y < viewport.heightPx; y += 1) {
    for (let x = 0; x < viewport.widthPx; x += 1) {
      const red = scene[(y * viewport.widthPx + x) * 4] ?? Number.NaN;
      shown += (covered(x, y) ? red > 0 : red === 0) ? 1 : 0;
    }
  }
  return shown;
}

/** The march texels with a lit colour channel. */
function litTexels(march: Float32Array): number {
  let lit = 0;
  for (let i = 0; i < march.length; i += 4) {
    lit += (march[i] ?? 0) > 0 || (march[i + 1] ?? 0) > 0 || (march[i + 2] ?? 0) > 0 ? 1 : 0;
  }
  return lit;
}

/**
 * R08.T8.b: low's half-resolution march as the composite reads it (`composite.wgsl`'s
 * `rayMarched`; `decision-r08-t8-leans.md` §6.2), a regression check of its tie-break.
 *
 * @remarks
 * On low ({@link TABLE_SIZES}, the march at ½), R08.T6.f's Earth (WGS 84 with its normal gravity,
 * R05's Earth medium), from 400 km looking along the limb ({@link limbCamera}'s frame, the march's
 * texels changing down it), the sun 20° above the tangent point's horizon and out of the frame.
 * Over a black scene at depth 0 every output pixel (x, y) must be the march target's texel
 * (⌊x ÷ 2⌋, ⌊y ÷ 2⌋), read back, times the composite's factor, to half an `rgba16float` step: from
 * above the top a sky pixel is the march's rgb alone. The pick of before, the first candidate
 * (trunc(x ÷ 2 − ½) on each axis), is counted against the same bound and must fail somewhere, so
 * that the check fails before the fix. Then a surface over the lower right ({@link PICK_SURFACE}),
 * its edges on pixel boundaries: every pixel must be the texel the shader's pick takes by its
 * depths and then its distance ({@link pickedTexel}), the surface's colour × alpha + rgb where
 * covered; the pixels within two rows or columns of its edge whose depth-nearest texel is not the
 * nearest are counted, and must include some that the nearest would draw wrong.
 */
export async function checkLowPick(engine: RenderEngine, checks: Checks): Promise<void> {
  const body = SLICES_EARTH;
  const sizes = TABLE_SIZES.low;
  const marchScale = sizes.rayMarchScale;
  const atmosphere = new HillaireAtmosphere(
    engine,
    body.medium,
    sizes,
    body.figure,
    body.curves,
    body.gravity,
  );
  try {
    const distanceM = body.figure.equatorialRadiusM + PICK_HEIGHT_M;
    const camera = limbCamera(distanceM, body.figure);
    const suns = [limbSun(distanceM, body.figure, 20)];
    const viewport = camera.viewport;
    const texels =
      Math.ceil(viewport.widthPx * marchScale) * Math.ceil(viewport.heightPx * marchScale);
    const name = "R08.T8.b low's half-resolution pick";
    const bound = "half an rgba16float step, 2⁻¹¹ of the value (2⁻²⁵ among the subnormals)";

    const sky = await pickFrame(engine, atmosphere, camera, suns, null);
    const nearest = pickAgreement(sky, viewport, marchScale, uncovered, (x, y) =>
      nearestTexel(x, y, marchScale),
    );
    const first = pickAgreement(sky, viewport, marchScale, uncovered, (x, y) =>
      firstCandidate(x, y, viewport, marchScale),
    );
    const black = coverageShown(sky.scene, viewport, uncovered);
    checks.check(
      `${name}: each sky pixel reads its nearest texel`,
      sky.march.length === texels * 4 &&
        black === nearest.pixels &&
        nearest.worst <= 1 &&
        first.over > 0,
      [
        `${nearest.pixels} pixels against texel (⌊x ÷ 2⌋, ⌊y ÷ 2⌋) × ${COMPOSITE_FACTOR}`,
        `${nearest.over} over ${bound}`,
        `worst ${nearest.worst.toFixed(3)} of it at ${nearest.at}`,
        `the first candidate, trunc(x ÷ 2 − ½): ${first.over} pixels over the same bound`,
        `worst ${first.worst.toExponential(2)} of it at ${first.at}`,
        `${sky.march.length / 4} texels read back of ${texels}, ${litTexels(sky.march)} lit`,
        `${black} pixels of the scene black`,
      ].join("; "),
    );

    const edge = await pickFrame(engine, atmosphere, camera, suns, PICK_SURFACE);
    const { fromXPx, fromYPx } = PICK_SURFACE;
    const surfaceKm = PICK_SURFACE.distanceM / 1_000;
    const covered: Coverage = (x, y) => x >= fromXPx && y >= fromYPx;
    const picked = (x: number, y: number): TexelPlace =>
      pickedTexel(x, y, viewport, marchScale, covered);
    const near = nearEdge(covered, viewport);
    const moved = (x: number, y: number): boolean => {
      const [px, py] = picked(x, y);
      const [nx, ny] = nearestTexel(x, y, marchScale);
      return near(x, y) && (px !== nx || py !== ny);
    };
    const whole = pickAgreement(edge, viewport, marchScale, covered, picked);
    const atEdge = pickAgreement(edge, viewport, marchScale, covered, picked, near);
    const kept = pickAgreement(edge, viewport, marchScale, covered, picked, moved);
    const nearestThere = pickAgreement(
      edge,
      viewport,
      marchScale,
      covered,
      (x, y) => nearestTexel(x, y, marchScale),
      moved,
    );
    const shown = coverageShown(edge.scene, viewport, covered);
    checks.check(
      `${name}: a surface's edge keeps the depth-nearest texel`,
      edge.march.length === texels * 4 &&
        shown === whole.pixels &&
        whole.worst <= 1 &&
        kept.pixels > 0 &&
        nearestThere.over > 0,
      [
        `a surface ${surfaceKm} km ahead from column ${fromXPx} and row ${fromYPx}`,
        `${shown} of ${whole.pixels} pixels covered as drawn`,
        `${whole.pixels} pixels against the shader's pick by depth, then distance, emulated`,
        `${whole.over} over ${bound}, plus f32's 2⁻²³ of a surface's terms`,
        `worst ${whole.worst.toFixed(3)} of it at ${whole.at}`,
        `${atEdge.pixels} within two rows or columns of the edge, ${atEdge.over} over`,
        `${kept.pixels} of them keep a depth-nearest texel not the nearest, ${kept.over} over`,
        `the nearest would put ${nearestThere.over} of those over`,
        `worst ${nearestThere.worst.toExponential(2)} of the bound at ${nearestThere.at}`,
        `${litTexels(edge.march)} of ${edge.march.length / 4} texels lit`,
      ].join("; "),
    );
  } finally {
    atmosphere.dispose();
  }
}

/**
 * The composite-from-above check's frame (R08.T8.b), at longitude 1 rad ({@link
 * FAR_LIMB_LONGITUDE_RAD}): from {@link ORBITAL_SUNSET_HEIGHT_M}, centred on the ray whose tangent
 * point stands {@link ORBITAL_SUNSET_TANGENT_M} up, ±4 km of tangent height about it (0.20° high,
 * 0.41° across), so that the 64 × 32 frame's farthest pixel centre lies at 0.84 of the Sun's
 * angular radius from its centre and every pixel is inside the disc.
 */
function discFrame(): LimbFrame {
  return {
    centreM: ORBITAL_SUNSET_TANGENT_M,
    halfSpanM: 4_000,
    longitudeRad: FAR_LIMB_LONGITUDE_RAD,
  };
}

/**
 * The share of the twin's transmittance a disc's is held to, 1%, of it or of
 * {@link DISC_TRANSMITTANCE_FLOOR} where it is below that.
 */
const DISC_TRANSMITTANCE_SHARE = 0.01;
const DISC_TRANSMITTANCE_FLOOR = 1e-3;

/** The unit ray through pixel (x, y)'s centre of a view, as `view.wgsl`'s `rayThrough` forms it. */
function pixelRay(view: TwinView, xPx: number, yPx: number): Vec3 {
  const nx = ((xPx + 0.5) / view.viewport.widthPx) * 2 - 1;
  const ny = 1 - ((yPx + 0.5) / view.viewport.heightPx) * 2;
  return normalise(add(view.forward, add(scale(view.right, nx), scale(view.upRay, ny))));
}

/** Each colour channel's least and greatest of some values. */
interface ChannelRange {
  readonly low: [number, number, number];
  readonly high: [number, number, number];
}

/** A range that holds nothing yet. */
function emptyRange(): ChannelRange {
  const inf = Number.POSITIVE_INFINITY;
  return { low: [inf, inf, inf], high: [-inf, -inf, -inf] };
}

/** `range` widened to hold `value` in channel `c`. */
function widen(range: ChannelRange, c: 0 | 1 | 2, value: number): void {
  range.low[c] = Math.min(range.low[c], value);
  range.high[c] = Math.max(range.high[c], value);
}

/** A range as its check records it, channel by channel. */
function describeRange(range: ChannelRange): string {
  return (["red", "green", "blue"] as const)
    .map((name, c) => `${name} ${show([range.low[c] ?? Number.NaN, range.high[c] ?? Number.NaN])}`)
    .join(", ");
}

/**
 * R08.T8.b's composite from above the top, per channel (`composite.wgsl`'s
 * `transmittanceFromAbove`): one frame through the limb at the Sun's disc ({@link discFrame}),
 * nothing drawn, each pixel's (composite − the stored march's rgb × the factor) ÷ (the disc × the
 * factor) against its twin, `oblateCompositeTransmittanceTwin` over the GPU's tables along the
 * pixel's ray.
 *
 * @remarks
 * The scene is black, so the composite is the march's rgb plus the disc through its transmittance
 * per channel, each term clamped at `rgba16float`'s largest after the factor; no channel may reach
 * it. Each channel is held to 1% of the twin's T (of 10⁻³ where T is below it) plus the output's
 * rounding, half an `rgba16float` step of it, over the disc. The disc is `frameSourcesOf`'s at
 * {@link SMOKE_EXPOSURE_SCALE}, narrowed to `f32` as the uniform holds it.
 */
async function checkDiscFromAbove(
  engine: RenderEngine,
  ordered: OrderedChecks,
  atmosphere: HillaireAtmosphere,
  planet: SmokePlanet,
  body: HeightsBody,
): Promise<void> {
  if (planet.kind !== "oblate") {
    throw new Error(`R08.T8.b's ${body.name} stands on the oblate twins`);
  }
  const figure = body.figure;
  const frame = discFrame();
  const distanceM = figure.equatorialRadiusM + ORBITAL_SUNSET_HEIGHT_M;
  const camera = limbCamera(distanceM, figure, frame);
  const sun = limbSun(distanceM, figure, 0, frame);
  const composite = await compositeFrame(engine, atmosphere, camera, [sun], null);
  const { rayMarch } = atmosphere.frameTables;
  if (rayMarch === null) {
    throw new Error("the ray march drew no target");
  }
  const march = halfTexels(await engine.readTexture(rayMarch, 0, undefined, "tolerance"));
  const tables = atmosphere.tables;
  const [source] = frameSourcesOf(
    [sun],
    SMOKE_EXPOSURE_SCALE,
    absorberCount(tables.medium) > 0,
    tables.curves.length,
  );
  if (source === undefined) {
    throw new Error("the frame has its sun");
  }
  const disc = [0, 1, 2].map((c) => Math.fround(source.disc[c] ?? Number.NaN) * COMPOSITE_FACTOR);
  const view = frameViewOf(atmosphere, camera, [sun], figure);
  ordered.now((checks) => {
    const { widthPx: w, heightPx: h } = camera.viewport;
    const s = sun.directionBodyFixed;
    const sinRadius = Math.sin(sun.angularRadiusRad);
    const twinRange = emptyRange();
    const gpuRange = emptyRange();
    let worst = 0;
    let at = "none";
    let outside = 0;
    let farthest = 0;
    let brightest = 0;
    let saturated = 0;
    const ratios: number[][] = Array.from({ length: h }, () => []);
    for (let y = 0; y < h; y += 1) {
      for (let x = 0; x < w; x += 1) {
        const k = y * w + x;
        const d = pixelRay(view, x, y);
        const share = norm(crossProduct(d, s)) / sinRadius;
        farthest = Math.max(farthest, share);
        outside += share <= 1 && dot(d, s) > 0 ? 0 : 1;
        const t = oblateCompositeTransmittanceTwin(planet.twin, view, d, curveSlotOf(sun));
        ratios[y]?.push(t[0] / t[2]);
        for (const c of [0, 1, 2] as const) {
          const out = composite[k * 4 + c] ?? Number.NaN;
          const dc = disc[c] ?? Number.NaN;
          const gpu = (out - (march[k * 4 + c] ?? Number.NaN) * COMPOSITE_FACTOR) / dc;
          const rounding = (HALF_ROUNDING * Math.abs(out) + HALF_SUBNORMAL_ROUNDING) / dc;
          const bound =
            DISC_TRANSMITTANCE_SHARE * Math.max(t[c], DISC_TRANSMITTANCE_FLOOR) + rounding;
          const off = Math.abs(gpu - t[c]) / bound;
          const ratio = Number.isNaN(off) ? Number.POSITIVE_INFINITY : off;
          widen(twinRange, c, t[c]);
          widen(gpuRange, c, gpu);
          brightest = Math.max(brightest, out);
          saturated += out >= HALF_FLOAT_MAX || dc * t[c] >= HALF_FLOAT_MAX ? 1 : 0;
          if (ratio > worst) {
            worst = ratio;
            const apart = percentOf((gpu - t[c]) / t[c], { kind: "places", digits: 3 });
            const values = `GPU ${precise(gpu)}, twin ${precise(t[c])}, ${apart}`;
            at = `pixel (${x}, ${y}) channel ${c}: ${values}`;
          }
        }
      }
    }
    const rowMedian = (y: number): string => show([medianOf(ratios[y] ?? [])]);
    checks.check(
      "R08.T8.b the composite from above the top: each disc through the limb, per channel",
      worst <= 1 && outside === 0 && saturated === 0 && march.length === w * h * 4,
      [
        `worst ${worst.toFixed(3)} of the bound at ${at}`,
        `the bound ${percentOf(DISC_TRANSMITTANCE_SHARE, ROUND_PERCENT)} of max(T, 10⁻³), plus` +
          " the output's rounding, 2⁻¹¹ of it, over the disc",
        `T, the twin's: ${describeRange(twinRange)}; the GPU's: ${describeRange(gpuRange)}`,
        `red ÷ blue, a row's median: ${rowMedian(0)} on the top, ${rowMedian(h - 1)} the lowest`,
        `${w * h - outside} of ${w * h} pixels inside the disc`,
        `the farthest at ${farthest.toFixed(3)} of sin ρ`,
        `the disc ${show(disc)}, pre-exposed at EV100 ${SMOKE_EV100}` +
          `, the factor ${COMPOSITE_FACTOR}`,
        `the brightest output ${show([brightest])}, ${saturated} channels at rgba16float's largest`,
      ].join("; "),
    );
  });
}

/**
 * R08.T8.b's far limb's Earth: R08.T6.f's ({@link SLICES_EARTH}), WGS 84 with its normal gravity
 * on the oblate twins and R05's Earth medium, its views the far limb's ({@link farLimbViews}).
 */
const FAR_LIMB_EARTH: HeightsBody = {
  ...SLICES_EARTH,
  task: "R08.T8.b",
  heightsM: [],
  limbsM: [],
  moreViews: [],
};

/** The far limb's cameras' distances from the centre, m: all beyond a ÷ 2. */
const FAR_LIMB_DISTANCES_M = [1e7, 1e8, 1e9] as const;

/**
 * The distance from which the far limb's ray-march envelope share is recorded rather than capped,
 * m (`decision-r08-f32.md` §10.4): there 2ε|P|, 12 to 120 m, is a sizeable share of the aerosol's
 * 1.2 km.
 */
const FAR_LIMB_RECORDED_FROM_M = 1e8;

/**
 * The far limb's ray-march envelope share from 10⁷ m, at most: 7%, the plan's 1% cap at 10⁷ m
 * missed (measured 2026-10-22, by view and arm 3.59%, 5.88%, 4.52% and 6.54% on both variants,
 * the worst rounded up to the next point); the narrowed frame puts its rows on the limb's lowest
 * 3 km, where the texels need the envelope (half-widths 1.9 to 16 × 10⁻³); reported to "main".
 */
const FAR_LIMB_NEAR_SHARE_MISS = 0.07;

/** The far limb's suns, deg above the horizon of the frame's centre's tangent point. */
const FAR_LIMB_SUNS_DEG = [20, -2] as const;

/**
 * The far limb's cameras' longitude, rad: off every axis, so that the camera's position in `f32`
 * carries the 2ε|P| rounding the envelope stands for; R08.T6.g's and R08.T6.f's limbs stand at 0.
 */
const FAR_LIMB_LONGITUDE_RAD = 1;

/** Texels a row of the far limb's frames gives the medium's lowest scale height: 5, at least 4. */
const FAR_LIMB_ROWS_PER_SCALE_HEIGHT = 5;

/** The far limb's frames' centre, in the medium's lowest scale heights above the datum. */
const FAR_LIMB_CENTRE_SCALE_HEIGHTS = 3;

/**
 * The least scale height of a medium's exponential terms, m: R05's Earth's aerosol's, 1,200 m.
 *
 * @throws Error for a medium with no exponential term.
 */
function lowestScaleHeightM(medium: AtmosphereMedium): number {
  let lowest = Number.POSITIVE_INFINITY;
  for (const term of medium.terms) {
    const density = term.density;
    switch (density.kind) {
      case "exponential":
        lowest = Math.min(lowest, density.scaleHeightM);
        break;
      case "tent":
      case "tabulated":
        break;
    }
  }
  if (!Number.isFinite(lowest)) {
    throw new Error(`${medium.name} has no exponential term to narrow the far limb by`);
  }
  return lowest;
}

/**
 * The far limb's frame (`decision-r08-f32.md` §10.4, views narrowed so that the limb's lowest
 * scale height spans at least four texels): {@link limbCamera}'s rows each span its half-span ÷ 16
 * of tangent height, so {@link FAR_LIMB_ROWS_PER_SCALE_HEIGHT} rows a scale height, 240 m on R05's
 * Earth (half-span 3,840 m), about {@link FAR_LIMB_CENTRE_SCALE_HEIGHTS} scale heights (3.6 km), at
 * longitude {@link FAR_LIMB_LONGITUDE_RAD}.
 */
function farLimbFrame(medium: AtmosphereMedium): LimbFrame {
  const scaleHeightM = lowestScaleHeightM(medium);
  const rowM = scaleHeightM / FAR_LIMB_ROWS_PER_SCALE_HEIGHT;
  return {
    centreM: FAR_LIMB_CENTRE_SCALE_HEIGHTS * scaleHeightM,
    halfSpanM: (rowM * FRAME_SIZE.heightPx) / 2,
    longitudeRad: FAR_LIMB_LONGITUDE_RAD,
  };
}

/**
 * The far limb's views from `distanceM` from the centre on high: {@link farLimbFrame}'s, the sun
 * at each of {@link FAR_LIMB_SUNS_DEG}, nothing drawn; the ray march's envelope share recorded,
 * not capped, from {@link FAR_LIMB_RECORDED_FROM_M}, and under it held to the named miss
 * {@link FAR_LIMB_NEAR_SHARE_MISS}.
 */
function farLimbViews(distanceM: number): HeightsView[] {
  const { figure, medium } = FAR_LIMB_EARTH;
  const frame = farLimbFrame(medium);
  const rowM = (frame.halfSpanM ?? Number.NaN) / (FRAME_SIZE.heightPx / 2);
  const share: EnvelopeShare = distanceM >= FAR_LIMB_RECORDED_FROM_M ? "recorded" : "missed";
  const centreKm = ((frame.centreM ?? Number.NaN) / 1_000).toFixed(1);
  const place = `rows of ${rowM} m about ${centreKm} km, longitude ${FAR_LIMB_LONGITUDE_RAD} rad`;
  const from = `the far limb from ${distanceM.toExponential(0)} m (${place})`;
  return FAR_LIMB_SUNS_DEG.map((elevationDeg) => ({
    name: `${from}, the sun ${elevationDeg}° at its tangent point`,
    camera: limbCamera(distanceM, figure, frame),
    sun: limbSun(distanceM, figure, elevationDeg, frame),
    sunElevationDeg: elevationDeg,
    surfaceM: null,
    marchEnvelopeShare: share,
  }));
}

/**
 * Records the far limb's ray march from one distance over its views and both arms
 * (`decision-r08-f32.md` §10.4: "the median and largest envelope half-widths are recorded per
 * distance"): each check's own verdict, as {@link recordHeights} gave it ({@link heightsHeld}), and
 * their envelope channels together, their share, the median and largest half-width and the worst
 * place within an envelope.
 */
function recordFarLimbDistance(
  checks: Checks,
  distanceM: number,
  share: EnvelopeShare,
  marches: readonly MarchArm[],
): void {
  const channels = marches.reduce((sum, { result }) => sum + result.channels, 0);
  const envelope = marches.flatMap(({ result }) => result.envelope);
  const widths = envelope.map((entry) => entry.halfWidth);
  const held = marches.every(({ result }) => heightsHeld(result, share));
  const each = marches.map(({ sunElevationDeg, arm, result }) => {
    const own = result.envelope.map((entry) => entry.halfWidth);
    const sun = `the sun ${sunElevationDeg}° at its tangent point`;
    const ownShare = percentOf(own.length / Math.max(result.channels, 1));
    const byEnvelope = `${ownShare} by the envelope, ${spreadOf(own)}`;
    return `${sun}, the ${arm} arm: worst ${result.worst.toFixed(3)}, ${byEnvelope}`;
  });
  const worstInEnvelope = Math.max(0, ...marches.map(({ result }) => result.worstInEnvelope));
  const counted = describeEnvelopeShare(share);
  const from = `R08.T8.b the far limb from ${distanceM.toExponential(0)} m`;
  checks.check(
    `${from}: the ray march over its suns and arms, its envelope ${counted}`,
    held && marches.length > 0,
    [
      `${marches.length} checks, ${channels} channels`,
      `${envelope.length} by the envelope (${percentOf(envelope.length / Math.max(channels, 1))})`,
      spreadOf(widths),
      `the worst at ${worstInEnvelope.toFixed(2)} of its half-width`,
      ...each,
    ].join("; "),
  );
}

/**
 * R08.T8.b's far limb (plan R08, R08.T8; `decision-r08-f32.md` §10.4): the per-frame kernels
 * against their twins from 10⁷, 10⁸ and 10⁹ m, and the composite from above the top per channel
 * ({@link checkDiscFromAbove}).
 *
 * @remarks
 * On {@link FAR_LIMB_EARTH}, on high, in R08.T6.g's two arms ({@link checkViewArms}), from each
 * distance with the sun 20° up and 2° below the horizon of the frame's centre's tangent point
 * ({@link farLimbViews}), each frame narrowed to five rows a scale height of the medium's lowest
 * ({@link farLimbFrame}) at longitude 1 rad: every limb view before them stands at longitude 0, on
 * an axis, where the camera's 2ε|P| rounding goes untested. The sky view's envelope is capped at
 * 1% in every view (its texels' resolution does not depend on |P|), the aerial perspective held
 * outright (the kernel arm on `f32`'s floor, {@link F32_SUBNORMAL_SPACING}), and the ray march's
 * envelope share from 10⁷ m held to {@link FAR_LIMB_NEAR_SHARE_MISS}, a named miss of the plan's 1%
 * cap there, and recorded, not capped, from 10⁸ and 10⁹ m, where every texel stays within 10⁻³ or
 * within its envelope; each distance's envelope half-widths are recorded together
 * ({@link recordFarLimbDistance}). The twins run on the page's twin workers beside the GPU's work,
 * the checks recorded in the order they were in sequence (`ordered.ts`).
 */
export async function checkFarLimb(
  engine: RenderEngine,
  checks: Checks,
  twins: TwinPool,
): Promise<void> {
  const body = FAR_LIMB_EARTH;
  const read = kernelMedium(body.medium);
  const sizes = twinSizes(TABLE_SIZES.high);
  await inOrder(checks, async (ordered) => {
    const atmosphere = new HillaireAtmosphere(
      engine,
      body.medium,
      sizes,
      body.figure,
      body.curves,
      body.gravity,
    );
    try {
      const planet = await planetOf(engine, atmosphere, read, body.figure);
      for (const distanceM of FAR_LIMB_DISTANCES_M) {
        const views = farLimbViews(distanceM);
        const marches: Array<Promise<readonly MarchArm[]>> = [];
        for (const view of views) {
          // The harness's GPU work runs in order: each frame is read back before the next draws.
          // oxlint-disable-next-line no-await-in-loop
          const arms = await checkViewArms(
            engine,
            ordered,
            atmosphere,
            planet,
            body,
            sizes,
            "high",
            view,
            twins,
          );
          marches.push(arms.marches);
        }
        const share = views[0]?.marchEnvelopeShare ?? "capped";
        void ordered.later(async (slot) => {
          recordFarLimbDistance(slot, distanceM, share, (await Promise.all(marches)).flat());
        });
      }
      // Its twin runs on the page, while the far limb's run on the workers.
      await checkDiscFromAbove(engine, ordered, atmosphere, planet, body);
    } finally {
      atmosphere.dispose();
    }
  });
}

// --- R08.T9.b: surface lighting ------------------------------------------------------------------

/** One body of R08.T9.b's smoke: its figure and gravity, its medium and curves, its lit heights. */
interface SurfaceBody {
  readonly name: string;
  readonly body: AtmosphereBody;
  readonly medium: AtmosphereMedium;
  readonly curves: AbsorberCurveSlots;
  /** The lit points' geodetic heights above the datum, m. */
  readonly heightsM: ReadonlyArray<number>;
}

/** Earth on WGS 84 with its normal gravity: one slice and one band, its ozone under the Sun. */
function surfaceEarth(): SurfaceBody {
  return {
    name: "Earth",
    body: earthBody(),
    medium: EARTH_REFERENCE,
    curves: EARTH_REFERENCE_CURVES,
    heightsM: [0, 2_000, 30_000],
  };
}

/** The Saturn-class figure: five slices and four bands, its fitted lookups' scale height read. */
function surfaceSaturn(): SurfaceBody {
  return {
    name: "the Saturn-class figure",
    body: SATURN_BODY,
    medium: saturnClassMedium(),
    curves: NO_ABSORBER_CURVES,
    heightsM: [0, 50_000, 300_000],
  };
}

/** The irradiance table's rows one twin job takes: 4 of its 16, so that its four jobs share the workers. */
const IRRADIANCE_JOB_ROWS = 4;

/**
 * One layer of the sky's irradiance twinned over the GPU's own tables (R08.T9.b): the band's
 * transmittance tables and its multiple-scattering layers as the GPU stored them, light slot `slot`
 * in the band's frame, its rows in jobs on the twin workers, merged.
 */
async function irradianceOverGpu(
  twins: TwinPool,
  read: AtmosphereMedium,
  band: TablesBand,
  transmittance: TwinTransmittance,
  multiScatteringBySlot: ReadonlyArray<TwinTable>,
  slot: number,
): Promise<TwinTable> {
  const { widthTexels: w, heightTexels: h } = IRRADIANCE_SIZE;
  const jobs: Promise<TwinTable>[] = [];
  for (let row = 0; row < h; row += IRRADIANCE_JOB_ROWS) {
    const texels: (readonly [number, number])[] = [];
    for (let y = row; y < Math.min(row + IRRADIANCE_JOB_ROWS, h); y += 1) {
      for (let x = 0; x < w; x += 1) {
        texels.push([x, y]);
      }
    }
    jobs.push(
      twins.irradiance(
        read,
        band.bottomRadiusM,
        transmittance,
        multiScatteringBySlot,
        slot,
        band.coefficientFactor,
        texels,
      ),
    );
  }
  const merged = new Float64Array(w * h * 4);
  // Each job leaves the texels of others' rows 0.
  for (const part of await Promise.all(jobs)) {
    part.texels.forEach((value, i) => {
      merged[i] = (merged[i] ?? 0) + value;
    });
  }
  return { widthTexels: w, heightTexels: h, texels: merged };
}

/**
 * R08.T9.b's pinned lookups at a body's heights: the sun's transmittance at four latitudes, six zenith
 * cosines from the zenith to 2° below the horizon and three azimuths; the sky's irradiance at the
 * same latitudes and cosines, and at a light slot past the medium's, held to its last; and a wide
 * source's transmittance at three cosines, two azimuths and three angular radii to 9.8°.
 */
function surfaceLightingCases(heightsM: ReadonlyArray<number>): LightingProbeCase[] {
  const latitudes = [0, 35, 60, 89].map((deg) => deg * DEGREE_RAD);
  const cosines = [1, 0.6, 0.2, 0.05, 0.01, -0.035];
  const cases: LightingProbeCase[] = [];
  for (const altitudeM of heightsM) {
    for (const latitudeRad of latitudes) {
      for (const mu of cosines) {
        for (const azimuthDeg of [0, 90, -150]) {
          cases.push({
            kind: "atmosphere",
            lookup: "sun",
            altitudeM,
            mu,
            latitudeRad,
            azimuthRad: azimuthDeg * DEGREE_RAD,
            angularRadiusRad: 0,
            curve: 0,
          });
        }
        for (const curve of [0, 3]) {
          cases.push({
            kind: "atmosphere",
            lookup: "sky",
            altitudeM,
            mu,
            latitudeRad,
            azimuthRad: 0,
            angularRadiusRad: 0,
            curve,
          });
        }
      }
      for (const mu of [0.6, 0.1, 0.02]) {
        for (const azimuthDeg of [0, -150]) {
          for (const angularRadiusRad of [1e-4, 0.05, 9.8 * DEGREE_RAD]) {
            cases.push({
              kind: "atmosphere",
              lookup: "source",
              altitudeM,
              mu,
              latitudeRad,
              azimuthRad: azimuthDeg * DEGREE_RAD,
              angularRadiusRad,
              curve: 0,
            });
          }
        }
      }
    }
  }
  return cases;
}

/**
 * The probe's results for the cases of one lookup, and their twin's, as two tables of one texel a
 * case (rgb, alpha 0), for {@link twinAgreement}.
 */
function lookupTables(
  gpu: Float32Array,
  expected: ReadonlyArray<ReadonlyArray<number>>,
  cases: ReadonlyArray<LightingProbeCase>,
  lookup: AtmosphereLookup,
): { readonly gpu: Float32Array; readonly twin: TwinTable } {
  const picked = cases.flatMap((probe, i) =>
    probe.kind === "atmosphere" && probe.lookup === lookup ? [i] : [],
  );
  const gpuTexels = new Float32Array(picked.length * 4);
  const twinTexels = new Float64Array(picked.length * 4);
  picked.forEach((i, k) => {
    for (const c of [0, 1, 2]) {
      gpuTexels[k * 4 + c] = gpu[i * 4 + c] ?? Number.NaN;
      twinTexels[k * 4 + c] = expected[i]?.[c] ?? Number.NaN;
    }
  });
  return {
    gpu: gpuTexels,
    twin: { widthTexels: picked.length, heightTexels: 1, texels: twinTexels },
  };
}

/**
 * R08.T9.b on one body: the sky's irradiance table, every band and light slot's layer, stored in
 * `rgba32float` (its kernel's own output, so with no floor and no kernel arm), against its twin over
 * the GPU's own band transmittance tables and multiple-scattering layers
 * (`tablesCpu.ts`'s `irradianceTwin`, in each band's frame); and `surfaceLighting.wgsl`'s three
 * lookups, run in the catalogue's lit-body probe with the body's stored tables bound, against
 * their twin over the GPU's slices and irradiance (`surfaceLightingTwin`, the body's table's
 * scale heights). Each to 10⁻³ above 10⁻⁶ of the largest value of its kind.
 */
async function checkSurfaceLighting(
  engine: RenderEngine,
  checks: Checks,
  twins: TwinPool,
  surface: SurfaceBody,
): Promise<void> {
  const read = kernelMedium(surface.medium);
  const tables = new AtmosphereTables(
    engine,
    surface.medium,
    tableRadiusM(surface.body.figure),
    surface.curves,
    surface.body,
  );
  try {
    await inOrder(checks, async (ordered) => {
      const plan = tables.plan;
      const perBand = tables.layersPerBand;
      const layers = tables.multiScatteringLayers;
      const gpu = await readPlanetTables(engine, tables);
      // Stored in `rgba32float`, its kernel's own output, so it needs no kernel arm.
      const irradiance = new Float32Array(
        await engine.readTexture(tables.irradiance(), 0, undefined, "tolerance"),
      );
      const multiScattering = layersOf(gpu.multiScattering, layers).map((layer) =>
        asTwinTable(layer, MULTI_SCATTERING_SIZE),
      );
      const twinsOverGpu: Promise<TwinTable>[] = [];
      for (const [b, band] of plan.bands.entries()) {
        // A body of one slice at κ = 1 and one band reads the slice's tables, as the build does.
        let own = storedSlices(gpu, read, tables.curves, plan.kappa.length)[0];
        if (!plan.shared) {
          // A band's own tables are built anew and read back before the next band's.
          // oxlint-disable-next-line no-await-in-loop
          const built = await readBandTransmittance(engine, tables, b);
          own = storedTransmittance(built, read, tables.curves);
        }
        if (own === undefined) {
          throw new Error(`${surface.name}'s tables have a slice`);
        }
        const bandLayers = multiScattering.slice(b * perBand, (b + 1) * perBand);
        for (let slot = 0; slot < perBand; slot += 1) {
          // Marked handled: a failure before the checks read it, or while an earlier one is read,
          // leaves no unhandled rejection, and the checks that read it still see it.
          twinsOverGpu.push(handled(irradianceOverGpu(twins, read, band, own, bandLayers, slot)));
        }
      }
      // The lookups through the probe, with the body's stored tables bound.
      const cases = surfaceLightingCases(surface.heightsM);
      const lookups = await probeSurfaceLighting(engine, cases, surfaceLightingTablesOf(tables));
      void ordered.later(async (later) => {
        for (const [index, pending] of twinsOverGpu.entries()) {
          // The layers' twins are awaited in order, each after the one before it.
          // oxlint-disable-next-line no-await-in-loop
          const twin = await pending;
          const band = Math.floor(index / perBand);
          const name = `${surface.name}: the sky's irradiance, band ${band} of ${plan.bands.length}'s layer of light slot ${index % perBand}, over the GPU's tables`;
          recordHeights(
            later,
            "R08.T9.b",
            name,
            twinAgreement(layerOf(irradiance, layers, index), twin, false, { arm: "kernel" }),
            "kernel",
            { label: "stored in rgba32float, the kernel's own output" },
          );
        }
        const planet = oblatePlanetOf(gpu, tables, read, surface.body);
        const irradianceTables = plan.bands.map((_, b) =>
          Array.from({ length: perBand }, (__, slot) =>
            asTwinTable(layerOf(irradiance, layers, b * perBand + slot), IRRADIANCE_SIZE),
          ),
        );
        const lighting = surfaceLightingTwin(
          planet,
          irradianceTables,
          packedScaleHeightsM(surface.medium),
        );
        const expected = expectedLighting(cases, lighting);
        for (const lookup of ["sun", "sky", "source"] as const) {
          const { gpu: probed, twin } = lookupTables(lookups, expected, cases, lookup);
          recordHeights(
            later,
            "R08.T9.b",
            `${surface.name}: surfaceLighting.wgsl's ${lookup} lookup at ${twin.widthTexels} points, over the GPU's tables`,
            twinAgreement(probed, twin, false, { arm: "kernel" }),
            "kernel",
          );
        }
      });
    });
  } finally {
    tables.dispose();
  }
}

/** R08.T9.b on Earth ({@link surfaceEarth}; `checkSurfaceLighting`). */
export async function checkSurfaceLightingEarth(
  engine: RenderEngine,
  checks: Checks,
  twins: TwinPool,
): Promise<void> {
  await checkSurfaceLighting(engine, checks, twins, surfaceEarth());
}

/** R08.T9.b on the Saturn-class figure ({@link surfaceSaturn}; `checkSurfaceLighting`). */
export async function checkSurfaceLightingSaturn(
  engine: RenderEngine,
  checks: Checks,
  twins: TwinPool,
): Promise<void> {
  await checkSurfaceLighting(engine, checks, twins, surfaceSaturn());
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
 * The exposure a capture is encoded at ({@link toRgba8}): the factor that sets the frame's mean
 * luminance (Rec. 709's weights) at AgX's middle grey, 0.18, or 1 for a black frame.
 */
function exposureOf(texels: Float32Array): number {
  let luminance = 0;
  for (let i = 0; i < texels.length; i += 4) {
    luminance +=
      0.2126 * (texels[i] ?? 0) + 0.7152 * (texels[i + 1] ?? 0) + 0.0722 * (texels[i + 2] ?? 0);
  }
  const mean = luminance / (texels.length / 4);
  return mean > 0 ? 0.18 / mean : 1;
}

/**
 * A frame as 8-bit sRGB: exposed so that its mean luminance sits at AgX's middle grey, 0.18
 * ({@link exposureOf}), then R02's tone curve.
 */
function toRgba8(texels: Float32Array): Uint8Array {
  return rgba8At(texels, exposureOf(texels));
}

/** A frame as 8-bit sRGB at the exposure `k`, through R02's tone curve, as {@link toRgba8}. */
function rgba8At(texels: Float32Array, k: number): Uint8Array {
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
    illuminanceLx: SUN_ILLUMINANCE_LX,
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
      const texels = await compositeFrame(engine, atmosphere, shot.camera, [shot.sun], null);
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
  return [
    ...images,
    ...(await captureSunsets(engine)),
    ...(await captureOrbitalSunsets(engine)),
    ...(await captureBinarySky(engine)),
    ...(await captureSaturnLimbs(engine)),
  ];
}

/** The Saturn-class limb captures' tangent height at the frame's centre and half-span, m. */
const SATURN_LIMB_CENTRE_M = 300_000;
const SATURN_LIMB_HALF_SPAN_M = 600_000;

/**
 * A capture camera at `cameraM` looking along the unit `forward` past a limb's tangent point
 * `distanceM` ahead, the unit `up` the tangent point's vertical, its frame spanning
 * {@link SATURN_LIMB_HALF_SPAN_M} of tangent height about its centre.
 */
function captureLimbCamera(
  cameraM: Vec3,
  forward: Vec3,
  up: Vec3,
  distanceM: number,
): AtmosphereCamera {
  const right = {
    x: forward.y * up.z - forward.z * up.y,
    y: forward.z * up.x - forward.x * up.z,
    z: forward.x * up.y - forward.y * up.x,
  };
  const aspect = CAPTURE_SIZE.widthPx / CAPTURE_SIZE.heightPx;
  const tanY = SATURN_LIMB_HALF_SPAN_M / distanceM;
  return {
    positionM: cameraM,
    orientation: quaternionFromRows([
      { x: right.x, y: up.x, z: -forward.x },
      { x: right.y, y: up.y, z: -forward.y },
      { x: right.z, y: up.z, z: -forward.z },
    ]),
    fovXRad: 2 * Math.atan(aspect * tanY),
    viewport: CAPTURE_SIZE,
  };
}

/**
 * R08.T6.f's by-hand captures: the Saturn-class figure's limb from orbit, above the air (its tables
 * in five κ slices and four latitude bands, `saturnClassMedium`), over the equator and over the
 * pole, each camera 12,000 km along the limb from a tangent point 300 km up (1,477 km above the
 * equator, 1,366 km above the datum near the pole, so that the composite draws the ray march), the
 * sun 10° above the tangent point's horizon ahead, for a person to see the air's height follow
 * 1 ÷ s: about 1.15 times as tall over the equator as H, and 0.87 times over the pole.
 */
async function captureSaturnLimbs(engine: RenderEngine): Promise<CapturedImage[]> {
  const figure = SATURN_BODY.figure;
  const atmosphere = new HillaireAtmosphere(
    engine,
    saturnClassMedium(),
    TABLE_SIZES.high,
    figure,
    NO_ABSORBER_CURVES,
    SATURN_BODY.gravity,
  );
  const distanceM = 12_000_000;
  const e = (10 * Math.PI) / 180;
  const equatorTangentM = figure.equatorialRadiusM + SATURN_LIMB_CENTRE_M;
  const poleTangentM = figure.polarRadiusM + SATURN_LIMB_CENTRE_M;
  const shots = [
    {
      name: "r08-saturn-limb-equator",
      // Along −y past the tangent point (a + h, 0, 0), up +x.
      camera: captureLimbCamera(
        { x: equatorTangentM, y: distanceM, z: 0 },
        { x: 0, y: -1, z: 0 },
        { x: 1, y: 0, z: 0 },
        distanceM,
      ),
      sun: { x: Math.sin(e), y: -Math.cos(e), z: 0 },
    },
    {
      name: "r08-saturn-limb-pole",
      // Along +x past the tangent point (0, 0, c + h), up +z.
      camera: captureLimbCamera(
        { x: -distanceM, y: 0, z: poleTangentM },
        { x: 1, y: 0, z: 0 },
        { x: 0, y: 0, z: 1 },
        distanceM,
      ),
      sun: { x: Math.cos(e), y: 0, z: Math.sin(e) },
    },
  ];
  const images: CapturedImage[] = [];
  try {
    for (const shot of shots) {
      const sun: SunState = {
        directionBodyFixed: shot.sun,
        illuminanceLx: SUN_ILLUMINANCE_LX,
        angularRadiusRad: 0,
      };
      // Each capture reads the GPU back before the next frame draws.
      // oxlint-disable-next-line no-await-in-loop
      const texels = await compositeFrame(engine, atmosphere, shot.camera, [sun], null);
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

/** A source at an elevation and an azimuth to the right of a {@link horizonCamera}'s view. */
function sourceAhead(
  elevationDeg: number,
  azimuthDeg: number,
  illuminanceLx: readonly [number, number, number],
  curveSlot: number,
): SunState {
  const e = elevationDeg * DEGREE_RAD;
  const a = azimuthDeg * DEGREE_RAD;
  // The camera's right is −y (`horizonCamera`).
  return {
    directionBodyFixed: {
      x: Math.cos(e) * Math.cos(a),
      y: -Math.cos(e) * Math.sin(a),
      z: Math.sin(e),
    },
    illuminanceLx,
    angularRadiusRad: SUN_ANGULAR_RADIUS_RAD,
    curveSlot,
  };
}

/**
 * R08.T7's by-hand captures of a binary sky on an Earth fixture (the four-absorber Earth-like
 * medium, `fourAbsorberMedium`, under the K5 and M3 pair's own curves): from 2 m across 90°, both
 * twilights (the primary 5° and the companion 3° below the horizon, 55° apart), the companion's
 * day after the primary's dusk, and both up; for a person to look for two glows, each its own
 * star's colour, and their timings to be taken with R08.T11's.
 */
async function captureBinarySky(engine: RenderEngine): Promise<CapturedImage[]> {
  const binary = fourAbsorberMedium(BINARY_LIGHTS_K);
  const atmosphere = new HillaireAtmosphere(
    engine,
    binary.medium,
    TABLE_SIZES.high,
    WGS84,
    binary.curves,
  );
  const primary = (e: number): SunState => sourceAhead(e, -25, [9.1e4, 8.0e4, 5.9e4], 0);
  const companion = (e: number): SunState => sourceAhead(e, 30, [2.9e3, 1.8e3, 0.8e3], 1);
  const shots = [
    { name: "r08-binary-both-twilights", suns: [primary(-5), companion(-3)] },
    { name: "r08-binary-companion-day", suns: [primary(-9), companion(4)] },
    { name: "r08-binary-both-up", suns: [primary(12), companion(4)] },
  ];
  const images: CapturedImage[] = [...(await captureMoonlitNight(engine))];
  try {
    for (const shot of shots) {
      // Each capture reads the GPU back before the next frame draws.
      // oxlint-disable-next-line no-await-in-loop
      const texels = await compositeFrame(
        engine,
        atmosphere,
        horizonCamera(2, WGS84.polarRadiusM, 5),
        shot.suns,
        null,
      );
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

/**
 * R08.T7's moonlit night on the rebuilt Earth from 2 m: the Sun beyond the twilight limit, which
 * the sky's sources skip (`sources.ts`), and a full Moon 45° up ahead as planetshine, 0.32 lx,
 * drawing no disc; across 90°, auto-exposed as every capture is, for a person to compare the sky
 * with the moonlit ground. The stars, which it should wash out, are R06's layer, not drawn here
 * (R08.T10.b's view draws both).
 */
async function captureMoonlitNight(engine: RenderEngine): Promise<CapturedImage[]> {
  const atmosphere = new HillaireAtmosphere(
    engine,
    EARTH_REFERENCE,
    TABLE_SIZES.high,
    WGS84,
    EARTH_REFERENCE_CURVES,
  );
  try {
    const moon: SunState = {
      ...sourceAhead(45, 0, [0.3, 0.32, 0.33], 0),
      angularRadiusRad: 0.004_5,
      drawsDisc: false,
    };
    const texels = await compositeFrame(
      engine,
      atmosphere,
      horizonCamera(2, WGS84.polarRadiusM, 5),
      [moon],
      null,
    );
    return [
      {
        name: "r08-moonlit-night",
        width: CAPTURE_SIZE.widthPx,
        height: CAPTURE_SIZE.heightPx,
        rgba: base64Of(toRgba8(texels)),
      },
    ];
  } finally {
    atmosphere.dispose();
  }
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
      EARTH_REFERENCE_CURVES,
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
      const texels = await compositeFrame(engine, atmosphere, camera, [sun], null);
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

/** The orbital sunset captures' camera, m above the equator. */
const ORBITAL_SUNSET_HEIGHT_M = 400_000;

/** The tangent height of the ray to the sun's centre in the orbital sunset captures, m. */
const ORBITAL_SUNSET_TANGENT_M = 12_000;

/**
 * The orbital sunset captures' fields, rad: 2° high, so that the sun's 0.53° disc spans 48 rows and
 * sets the frame's exposure, and 20°, the limb about it.
 */
const ORBITAL_SUNSET_SHOTS = [
  { name: "r08-orbital-sunset-earth-400km-disc", fovYRad: 2 * DEGREE_RAD },
  { name: "r08-orbital-sunset-earth-400km", fovYRad: 20 * DEGREE_RAD },
] as const;

/**
 * R08.T8.b's by-hand captures: an orbital sunset, Earth's medium on WGS 84 with Earth's gravity,
 * from 400 km over the equator, the view centred on the sun's disc seen through the limb, the ray
 * to its centre 12 km above the datum at its tangent point (the disc spanning about 2 to 22 km),
 * for a person to see the disc redden towards its lower limb.
 *
 * @remarks
 * Until R08.T8.b the composite dimmed what lies behind the sky from above the top, and each disc,
 * by the march's mean transmittance over the channels, which drew the disc grey there and stepped
 * its colour at the top; since, by the transmittance per channel from the ray's entry into the air
 * (`composite.wgsl`'s `transmittanceFromAbove`).
 */
async function captureOrbitalSunsets(engine: RenderEngine): Promise<CapturedImage[]> {
  const figure = WGS84;
  const atmosphere = new HillaireAtmosphere(
    engine,
    EARTH_REFERENCE,
    TABLE_SIZES.high,
    figure,
    EARTH_REFERENCE_CURVES,
    EARTH_GRAVITY,
  );
  const distanceM = figure.equatorialRadiusM + ORBITAL_SUNSET_HEIGHT_M;
  // The view and the sun's centre along the ray tangent 12 km up, the frame's up that tangent
  // point's vertical (`limbCamera`'s and `limbSun`'s, the sun 0° over its horizon).
  const centre: LimbFrame = { centreM: ORBITAL_SUNSET_TANGENT_M };
  const sun = limbSun(distanceM, figure, 0, centre);
  const toTangentM = limbTangentDistanceM(distanceM, figure, centre);
  const images: CapturedImage[] = [];
  try {
    for (const shot of ORBITAL_SUNSET_SHOTS) {
      // The shot's field as the half-span of tangent height it spans at the tangent point.
      const frame = { ...centre, halfSpanM: Math.tan(shot.fovYRad / 2) * toTangentM };
      const camera = limbCamera(distanceM, figure, frame, CAPTURE_SIZE);
      // Each capture reads the GPU back before the next frame draws.
      // oxlint-disable-next-line no-await-in-loop
      const texels = await compositeFrame(engine, atmosphere, camera, [sun], null);
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

// --- R08.T8.b: the ascent by hand ----------------------------------------------------------------

/** The ascent's two suns: the day's, the same direction at every height, and the last light. */
type AscentSun = "day" | "lastlight";

/** The ascent's suns, in the order its frames are drawn. */
const ASCENT_SUNS: readonly AscentSun[] = ["day", "lastlight"];

/** The ascent's day sun over the horizontal, deg, 90° to the right of the view (east). */
const ASCENT_DAY_ELEVATION_DEG = 20;

/** The day sun's azimuth from the view's, deg, clockwise seen from above. */
const ASCENT_DAY_AZIMUTH_DEG = 90;

/**
 * The ascent's last light over the camera's own visible horizon, deg: inside
 * `decision-r08-t8-leans.md` §5.1's window (the Sun 0–1.5° over that horizon, 90–180° from the
 * view's azimuth), where the march's seam at the horizontal was found and T8.a's warp fixed it.
 */
const ASCENT_LAST_LIGHT_OVER_HORIZON_DEG = 0.5;

/** The last light's azimuth from the view's, deg, clockwise seen from above: behind, right. */
const ASCENT_LAST_LIGHT_AZIMUTH_DEG = 150;

/** How far below and above its place, in h\*, each frame of the ascent's pairs stands, m. */
const ASCENT_PAIR_OFFSET_M = 50;

/** The floor of a pair's relative change, a share of its brightest channel: Design note 10's. */
const ASCENT_FLOOR_SHARE = 6e-3;

/**
 * The last light's pre-exposure, EV100: `AUTO`'s for a frame whose mean is 1 cd/m² (EV100 =
 * log₂(L̄ S ÷ K), S = 100, K = 12.5), about the band's at the last light.
 *
 * @remarks
 * At the smoke's EV100 15 the last light from the top (some 0.04 cd/m² at its brightest) falls in
 * `rgba16float`'s subnormals, one step of 2⁻²⁴ about 6% of the frame's brightest, which a frame's
 * own exposure turns into bands a hundred codes apart. The day's frames keep the smoke's
 * {@link SMOKE_EXPOSURE_SCALE}.
 */
const ASCENT_LAST_LIGHT_EV100 = 3;

/** The pre-exposure the ascent's frames under `sun` are drawn at. */
function ascentPreExposure(sun: AscentSun): number {
  let preExposure: number;
  switch (sun) {
    case "day":
      preExposure = SMOKE_EXPOSURE_SCALE;
      break;
    case "lastlight":
      preExposure = exposureScale(ASCENT_LAST_LIGHT_EV100);
      break;
  }
  return preExposure;
}

/**
 * A sun at `elevationDeg` over the horizontal of a {@link bandCamera} and `azimuthDeg` from its
 * view's azimuth (north), clockwise seen from above, the Sun's disc at 1 au.
 */
function ascentSun(elevationDeg: number, azimuthDeg: number): SunState {
  const e = elevationDeg * DEGREE_RAD;
  const a = azimuthDeg * DEGREE_RAD;
  // At the camera body +x is up, +z north and +y east, the view's right.
  return {
    directionBodyFixed: {
      x: Math.sin(e),
      y: Math.cos(e) * Math.sin(a),
      z: Math.cos(e) * Math.cos(a),
    },
    illuminanceLx: SUN_ILLUMINANCE_LX,
    angularRadiusRad: SUN_ANGULAR_RADIUS_RAD,
  };
}

/**
 * The ascent's `sun` for a camera `heightM` above the equator's datum: the day sun's fixed
 * direction, or the last light {@link ASCENT_LAST_LIGHT_OVER_HORIZON_DEG} over the visible horizon
 * whose dip the frame takes ({@link meridianDipRad}).
 */
function ascentSunAt(sun: AscentSun, heightM: number): SunState {
  let state: SunState;
  switch (sun) {
    case "day":
      state = ascentSun(ASCENT_DAY_ELEVATION_DEG, ASCENT_DAY_AZIMUTH_DEG);
      break;
    case "lastlight": {
      // The meridian's dip, the frame's: toward 150° the datum's own is within 0.02° of it at every
      // height here (its tan δ 0.084% smaller), against the 0.5° the sun stands over it.
      const dipDeg = meridianDipRad(heightM, SLICES_EARTH.figure) / DEGREE_RAD;
      state = ascentSun(ASCENT_LAST_LIGHT_OVER_HORIZON_DEG - dipDeg, ASCENT_LAST_LIGHT_AZIMUTH_DEG);
      break;
    }
  }
  return state;
}

/** One height of the ascent's frames for the eye. */
interface AscentPlace {
  /** The height in a capture's name, as `040km` or `1e8m`. */
  readonly tag: string;
  /** What the height is, for a check's detail. */
  readonly label: string;
  /** The camera's geodetic height above the equator's datum, m. */
  readonly heightM: number;
  /** Its h\*, the height the medium is read at, m: s times the geodetic height. */
  readonly heightStarM: number;
}

/**
 * A height as a capture's name gives it: kilometres to three whole digits, and any tenth after a
 * `p`, since the harness saves only names of `a`–`z`, `0`–`9` and `-` (`src/smoke/result.ts`).
 */
function kmTag(heightM: number): string {
  const tenths = Math.round(heightM / 100);
  const whole = String(Math.floor(tenths / 10)).padStart(3, "0");
  return tenths % 10 === 0 ? `${whole}km` : `${whole}p${tenths % 10}km`;
}

/** A height in kilometres to three decimals, for a check's detail. */
function kmText(heightM: number): string {
  return `${(heightM / 1_000).toFixed(3)} km`;
}

/**
 * The ascent's heights for the eye on `setting`: in h\* 2 m, 10 km, the band's lower edge, middle
 * and upper edge, and 100 m under and over the top; then 400 km, 10⁷ m and 10⁸ m above the datum,
 * geodetic, where the march draws everything at any s.
 */
function ascentEyePlaces(setting: QualitySetting): AscentPlace[] {
  const body = SLICES_EARTH;
  const topM = body.medium.topHeightM;
  const { lowerShareOfTop, upperShareOfTop } = TABLE_SIZES[setting].marchBand;
  const s = equatorGravityRatio(body);
  const star = (label: string, heightStarM: number, tag = kmTag(heightStarM)): AscentPlace => ({
    tag,
    label: `h* ${kmText(heightStarM)} (${label})`,
    heightM: heightStarM / s,
    heightStarM,
  });
  const geodetic = (label: string, heightM: number, tag: string): AscentPlace => ({
    tag,
    label: `${label} geodetic`,
    heightM,
    heightStarM: heightM * s,
  });
  return [
    star("the ground", 2, "2m"),
    star("under the band", 10_000),
    star("the band's lower edge", lowerShareOfTop * topM),
    star("the band's middle", ((lowerShareOfTop + upperShareOfTop) / 2) * topM),
    star("the band's upper edge", upperShareOfTop * topM),
    star("100 m under the top", topM - 100),
    star("100 m over the top", topM + 100),
    geodetic("400 km", 400_000, "400km"),
    geodetic("10⁷ m", 1e7, "1e7m"),
    geodetic("10⁸ m", 1e8, "1e8m"),
  ];
}

/** A place of the ascent's figures, a pair of frames {@link ASCENT_PAIR_OFFSET_M} either side. */
interface AscentPair {
  readonly label: string;
  /** Whether the frame changes its regime there (an edge of the band, or the top). */
  readonly edge: boolean;
  /** The place's h\*, m. */
  readonly heightStarM: number;
}

/**
 * The ascent's pairs on `setting`: the band's lower and upper edges and the top, where a step at
 * the switch would stand, and two references away from any edge, the band's middle and h\* 10 km.
 */
function ascentPairs(setting: QualitySetting): AscentPair[] {
  const topM = SLICES_EARTH.medium.topHeightM;
  const { lowerShareOfTop, upperShareOfTop } = TABLE_SIZES[setting].marchBand;
  return [
    { label: "the band's lower edge", edge: true, heightStarM: lowerShareOfTop * topM },
    { label: "the band's upper edge", edge: true, heightStarM: upperShareOfTop * topM },
    { label: "the medium's top", edge: true, heightStarM: topM },
    {
      label: "the band's middle",
      edge: false,
      heightStarM: ((lowerShareOfTop + upperShareOfTop) / 2) * topM,
    },
    { label: "h* 10 km", edge: false, heightStarM: 10_000 },
  ];
}

/** A pair's change from its lower frame to its upper. */
interface PairChange {
  /** The worst |c₂ − c₁| ÷ max(c₁, floor) over every pixel and colour channel. */
  readonly relative: number;
  readonly relativeAt: string;
  /** The largest change in 8-bit sRGB codes, both frames encoded at the lower frame's exposure. */
  readonly codes: number;
  readonly codesAt: string;
  /** The pair's brightest colour channel, pre-exposed: the floor's base. */
  readonly brightest: number;
  /** The colour channels that change by 2 codes or more, §5's visibility line. */
  readonly stepped: number;
  /** The first and last rows, from the top, of those channels; `null` for none. */
  readonly steppedRows: readonly [number, number] | null;
}

/** The codes a pair's channels step by from which `decision-r08-t8-leans.md` §5 calls it seen. */
const VISIBLE_STEP_CODES = 2;

/**
 * `below` against `above`, two frames of one size: the relative change at Design note 10's floor,
 * {@link ASCENT_FLOOR_SHARE} of the pair's brightest colour channel, and the change in codes at one
 * exposure, `below`'s ({@link exposureOf}), so that the frame's own metering hides no step.
 */
function pairChange(below: Float32Array, above: Float32Array, widthPx: number): PairChange {
  if (below.length !== above.length) {
    const at = `${below.length / 4} and ${above.length / 4} texels read back`;
    return {
      relative: Number.POSITIVE_INFINITY,
      relativeAt: at,
      codes: 255,
      codesAt: at,
      brightest: Number.NaN,
      // Every colour channel of the lower frame, its alpha (the meter class) left out.
      stepped: (below.length / 4) * 3,
      steppedRows: null,
    };
  }
  let brightest = 0;
  for (const frame of [below, above]) {
    for (let i = 0; i < frame.length; i += 1) {
      // Alpha carries the scene's meter class (R08.T7), not light.
      brightest = i % 4 === 3 ? brightest : Math.max(brightest, frame[i] ?? 0);
    }
  }
  const floor = ASCENT_FLOOR_SHARE * brightest;
  const k = exposureOf(below);
  const codesBelow = rgba8At(below, k);
  const codesAbove = rgba8At(above, k);
  let relative = 0;
  let relativeAt = "no channel changes";
  let codes = 0;
  let codesAt = "no code changes";
  let stepped = 0;
  let steppedRows: readonly [number, number] | null = null;
  const rowOf = (i: number): number => Math.floor(i / 4 / widthPx);
  const where = (i: number): string => `(${Math.floor(i / 4) % widthPx}, ${rowOf(i)}) c${i % 4}`;
  for (let i = 0; i < below.length; i += 1) {
    if (i % 4 === 3) {
      continue;
    }
    const c1 = below[i] ?? Number.NaN;
    const c2 = above[i] ?? Number.NaN;
    const denominator = Math.max(c1, floor);
    const change = Math.abs(c2 - c1);
    const share =
      denominator > 0 ? change / denominator : change === 0 ? 0 : Number.POSITIVE_INFINITY;
    if (Number.isNaN(share) || share > relative) {
      relative = Number.isNaN(share) ? Number.POSITIVE_INFINITY : share;
      relativeAt = `${where(i)}, ${show([c1])} → ${show([c2])}`;
    }
    const step = Math.abs((codesAbove[i] ?? 0) - (codesBelow[i] ?? 0));
    if (step > codes) {
      codes = step;
      const linear = `${show([c1])} → ${show([c2])}`;
      codesAt = `${where(i)}, ${codesBelow[i] ?? 0} → ${codesAbove[i] ?? 0} (${linear})`;
    }
    if (step >= VISIBLE_STEP_CODES) {
      const row = rowOf(i);
      stepped += 1;
      steppedRows =
        steppedRows === null
          ? [row, row]
          : [Math.min(steppedRows[0], row), Math.max(steppedRows[1], row)];
    }
  }
  return { relative, relativeAt, codes, codesAt, brightest, stepped, steppedRows };
}

/** The step across the horizontal in a frame for the eye, against the sky's own over it. */
interface HorizontalStep {
  /** The rows just over and just under the horizontal, from the top. */
  readonly overRow: number;
  readonly underRow: number;
  /** Their centre rays' elevations over the horizontal, deg. */
  readonly overDeg: number;
  readonly underDeg: number;
  /** Per colour channel, the median over the columns of |Δcode| from the row over to the next. */
  readonly across: readonly number[];
  /** Per colour channel, the same median between the two rows over the horizontal. */
  readonly own: readonly number[];
}

/**
 * The step across the horizontal in `codes`, a {@link bandCamera}'s frame for the eye in 8-bit
 * sRGB, the horizontal found from the camera's own geometry (its rows' centre rays against the
 * datum's normal at it), or `null` where fewer than two rows lie over it or none under it.
 *
 * @remarks
 * The camera has no roll, its right horizontal, so a pixel's elevation has its row's sign in every
 * column: the horizontal falls between the same two rows across the frame.
 */
function horizontalStep(
  codes: Uint8Array,
  camera: AtmosphereCamera,
  figure: SpheroidFigure,
): HorizontalStep | null {
  const { widthPx, heightPx } = camera.viewport;
  const normal = atmosphereInputs(camera, figure).normal;
  const forward = rotate(camera.orientation, { x: 0, y: 0, z: -1 });
  const up = rotate(camera.orientation, { x: 0, y: 1, z: 0 });
  const tanY = Math.tan(camera.fovXRad / 2) / (widthPx / heightPx);
  const elevationDeg = (row: number): number => {
    const v = 1 - (2 * (row + 0.5)) / heightPx;
    return Math.asin(dot(normalise(add(forward, scale(up, v * tanY))), normal)) / DEGREE_RAD;
  };
  let overRow = -1;
  for (let row = 0; row < heightPx && elevationDeg(row) > 0; row += 1) {
    overRow = row;
  }
  const underRow = overRow + 1;
  if (overRow < 1 || underRow >= heightPx) {
    return null;
  }
  const code = (row: number, column: number, channel: number): number =>
    codes[(row * widthPx + column) * 4 + channel] ?? 0;
  const medianStep = (from: number, to: number, channel: number): number =>
    medianOf(
      Array.from({ length: widthPx }, (_, column) =>
        Math.abs(code(to, column, channel) - code(from, column, channel)),
      ),
    );
  return {
    overRow,
    underRow,
    overDeg: elevationDeg(overRow),
    underDeg: elevationDeg(underRow),
    across: [0, 1, 2].map((c) => medianStep(overRow, underRow, c)),
    own: [0, 1, 2].map((c) => medianStep(overRow - 1, overRow, c)),
  };
}

/** A {@link HorizontalStep} as the ascent's check records it, the excess its own figure. */
function describeHorizontal(tag: string, step: HorizontalStep | null): string {
  if (step === null) {
    return `${tag}: no horizontal between two rows over it and one under it`;
  }
  const excess = step.across.map((a, c) => a - (step.own[c] ?? 0));
  const rows = `rows ${step.overRow}/${step.underRow}`;
  const elevations = `${step.overDeg.toFixed(4)}°, ${step.underDeg.toFixed(4)}°`;
  return [
    `${tag}: ${rows} (${elevations}), across ${step.across.join(", ")}`,
    `the sky's own ${step.own.join(", ")}`,
    `excess ${excess.join(", ")}`,
  ].join(", ");
}

/** How the ascent's sun stands, for a check's detail. */
function describeAscentSun(sun: AscentSun): string {
  let text: string;
  switch (sun) {
    case "day":
      text = `the day sun (${ASCENT_DAY_ELEVATION_DEG}° over the horizontal`;
      text += `, ${ASCENT_DAY_AZIMUTH_DEG}° right of the view`;
      text += `, pre-exposed at the smoke's EV100 ${SMOKE_EV100})`;
      break;
    case "lastlight":
      text = `the last light (${ASCENT_LAST_LIGHT_OVER_HORIZON_DEG}° over the visible horizon`;
      text += `, ${ASCENT_LAST_LIGHT_AZIMUTH_DEG}° right of the view`;
      text += `, pre-exposed at EV100 ${ASCENT_LAST_LIGHT_EV100})`;
      break;
  }
  return text;
}

/** A time in seconds to a tenth, for a check's detail. */
function secondsOf(ms: number): string {
  return `${(ms / 1_000).toFixed(1)} s`;
}

/**
 * R08.T8.b's ascent by hand (plan R08, R08.T8's acceptance: "by hand an ascent from R05's test
 * planet to 10⁸ m shows no step at the switch"): R05's test planet ({@link SLICES_EARTH}, WGS 84
 * with its normal gravity, R05's Earth medium), a vertical ascent over the equator at longitude 0
 * looking north along the horizon ({@link bandCamera}), on high and on low, under a day sun and at
 * the camera's last light ({@link ascentSunAt}).
 *
 * @remarks
 * Frames for the eye at 320 × 180 ({@link ascentEyePlaces}), each at its own exposure as every
 * capture, named `r08-ascent-<setting>-<sun>-<height>`. Figures at 64 × 32 from pairs of frames
 * {@link ASCENT_PAIR_OFFSET_M} under and over the band's edges, the top and two references
 * ({@link ascentPairs}): the worst relative change at Design note 10's floor and the largest
 * change in codes at the lower frame's exposure ({@link pairChange}), where a step at the switch
 * would show as an edge's figure far above the references'. In each frame for the eye at the last
 * light from inside the air, the step across the horizontal in codes beside the sky's own over it
 * ({@link horizontalStep}; `decision-r08-t8-leans.md` §5's line, 2 codes sustained along the
 * row). The frames are pre-exposed at the smoke's EV100 15 by day and at
 * {@link ASCENT_LAST_LIGHT_EV100} at the last light, whose sky EV100 15 puts in `rgba16float`'s
 * subnormals. One check per setting holds every frame finite and within `rgba16float`, and records
 * the figures in its detail: they are judged by hand, not held.
 */
export async function captureAscent(
  engine: RenderEngine,
  checks: Checks,
): Promise<CapturedImage[]> {
  const body = SLICES_EARTH;
  const topM = body.medium.topHeightM;
  const s = equatorGravityRatio(body);
  const images: CapturedImage[] = [];
  for (const setting of ["high", "low"] as const) {
    const sizes = TABLE_SIZES[setting];
    const weightOf = (heightStarM: number): number =>
      marchWeightOf(heightStarM, topM, sizes.marchBand);
    const atmosphere = new HillaireAtmosphere(
      engine,
      body.medium,
      sizes,
      body.figure,
      body.curves,
      body.gravity,
    );
    // A frame `heightM` above the datum (geodetic), the sun as it stands for that height.
    const frameAt = (
      heightM: number,
      camera: AtmosphereCamera,
      sun: AscentSun,
    ): Promise<Float32Array> =>
      compositeFrame(
        engine,
        atmosphere,
        camera,
        [ascentSunAt(sun, heightM)],
        null,
        ascentPreExposure(sun),
      );
    let inRange = true;
    let frames = 0;
    const sections: string[] = [];
    const horizontals: string[] = [];
    let pairMs = 0;
    let eyeMs = 0;
    try {
      for (const sun of ASCENT_SUNS) {
        const edges: string[] = [];
        const references: string[] = [];
        const pairsStart = performance.now();
        for (const pair of ascentPairs(setting)) {
          const belowStarM = pair.heightStarM - ASCENT_PAIR_OFFSET_M;
          const aboveStarM = pair.heightStarM + ASCENT_PAIR_OFFSET_M;
          const belowM = belowStarM / s;
          const aboveM = aboveStarM / s;
          // Each frame reads the GPU back before the next draws.
          // oxlint-disable-next-line no-await-in-loop
          const below = await frameAt(belowM, bandCamera(belowM, body.figure), sun);
          // The pair's upper frame draws once the lower one is read back.
          // oxlint-disable-next-line no-await-in-loop
          const above = await frameAt(aboveM, bandCamera(aboveM, body.figure), sun);
          frames += 2;
          inRange = inRange && withinHalfRange(below) && withinHalfRange(above);
          const change = pairChange(below, above, FRAME_SIZE.widthPx);
          const weights = [weightOf(belowStarM), weightOf(aboveStarM)] as const;
          (pair.edge ? edges : references).push(describePair(pair, weights, change));
        }
        pairMs += performance.now() - pairsStart;
        const sides = `the edges: ${edges.join("; ")}; the references: ${references.join("; ")}`;
        sections.push(`${describeAscentSun(sun)}: ${sides}`);
        const eyeStart = performance.now();
        for (const place of ascentEyePlaces(setting)) {
          const camera = bandCamera(place.heightM, body.figure, CAPTURE_SIZE);
          // Each frame for the eye reads the GPU back before the next draws.
          // oxlint-disable-next-line no-await-in-loop
          const texels = await frameAt(place.heightM, camera, sun);
          frames += 1;
          inRange = inRange && withinHalfRange(texels);
          const rgba = toRgba8(texels);
          images.push({
            name: `r08-ascent-${setting}-${sun}-${place.tag}`,
            width: CAPTURE_SIZE.widthPx,
            height: CAPTURE_SIZE.heightPx,
            rgba: base64Of(rgba),
          });
          if (sun === "lastlight" && place.heightStarM < topM) {
            horizontals.push(
              describeHorizontal(place.label, horizontalStep(rgba, camera, body.figure)),
            );
          }
        }
        eyeMs += performance.now() - eyeStart;
      }
    } finally {
      atmosphere.dispose();
    }
    const band = sizes.marchBand;
    const lowerM = band.lowerShareOfTop * topM;
    const edgesM = `h* ${kmText(lowerM)} to ${kmText(band.upperShareOfTop * topM)}`;
    const pairsHeading =
      `the pairs at 64 × 32 (relative at a floor of ${ASCENT_FLOOR_SHARE} of the pair's ` +
      "brightest channel, codes at the lower frame's exposure)";
    const horizontalHeading =
      "the horizontal at the last light (320 × 180, each frame at its own exposure, the median " +
      "over the columns of |Δcode|, R, G, B)";
    checks.check(
      `R08.T8.b the ascent, ${setting}: no step at the switch`,
      inRange,
      [
        `${frames} frames finite and within rgba16float: ${String(inRange)}`,
        `s ${s.toFixed(6)} at the equator; the band ${bandOf(band)} (${edgesM})`,
        `${pairsHeading}: ${sections.join(" | ")}`,
        `${horizontalHeading}: ${horizontals.join("; ")}`,
        `times: the pairs ${secondsOf(pairMs)}, the frames for the eye ${secondsOf(eyeMs)}`,
      ].join(" || "),
    );
  }
  return images;
}

/** One of the ascent's pairs as its check records it: its place, w at both frames, its change. */
function describePair(
  pair: AscentPair,
  weights: readonly [number, number],
  change: PairChange,
): string {
  const place = `h* ${kmText(pair.heightStarM)} ± ${ASCENT_PAIR_OFFSET_M} m`;
  const w = `w ${show([weights[0]])} → ${show([weights[1]])}`;
  const relativeShare = percentOf(change.relative, { kind: "figures", digits: 3 });
  const relative = `${relativeShare} at ${change.relativeAt}`;
  const rows = change.steppedRows === null ? "" : ` on rows ${change.steppedRows.join("–")}`;
  const codes = [
    `${change.codes} codes at ${change.codesAt}`,
    `${change.stepped} channels by ${VISIBLE_STEP_CODES} codes or more${rows}`,
    `brightest ${show([change.brightest])}`,
  ].join(", ");
  return `${pair.label} (${place}, ${w}): ${relative}; ${codes}`;
}
