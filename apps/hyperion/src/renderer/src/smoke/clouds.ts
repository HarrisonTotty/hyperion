/**
 * The smoke page's clouds checks (plan R11, R11.T7.b), in part 26: the field, the `f32`
 * δ-Eddington and the ground's tap on the run's adapter against their `f64` twins, through the
 * clouds' probe (`view/clouds/cloudProbe.ts`), and the two-dimensional layer drawn over a frame with
 * every texel finite.
 *
 * @remarks
 * One Earth-like body's clouds are made by the layer's own `CloudLayers` (its noise tile, coverage
 * texture and rows), and its twin by the same functions on the CPU. The GPU's filtering and `f32`
 * are not bit-identical to the twins, and need not be (plan R11, Design note 8), so each value is
 * held within a stated tolerance:
 * - the coverage texture's z_c and N at a place to {@link COVERAGE_TOLERANCE}, its wind to
 *   {@link WIND_TOLERANCE_MPS}: the bilinear weights' precision;
 * - z_r, the resolved structure, to {@link RESOLVED_TOLERANCE}, the noise's trilinear weights' over
 *   up to twelve octaves; s, which reads no texture, to {@link UNRESOLVED_TOLERANCE};
 * - the share f and the mean τ formed from the GPU's own z_c, N, z_r and s, which isolates the
 *   normal's tail and density in `f32`, to {@link SHARE_TOLERANCE} and {@link TAU_RELATIVE_TOLERANCE};
 * - the share end to end at footprints that leave s at least {@link COARSE_SD}, where the threshold
 *   is not steep, to {@link COARSE_SHARE_TOLERANCE};
 * - δ-Eddington's R and T_diff to {@link SLAB_TOLERANCE}, T_dir to {@link DIRECT_RELATIVE_TOLERANCE}
 *   of itself (`cloudOptics.wgsl`'s header: 8.0 × 10⁻⁵ in R emulated, before WGSL's `exp`);
 * - the ground's three factors to {@link GROUND_TOLERANCE}, the share's end-to-end error at the
 *   sun's footprint, and exactly (1, 1, 0) above the shell.
 */

import type { Vec3 } from "../geometry/vec3";
import { createAirlessTables } from "../view/atmosphere/surfaceLighting";
import { lookAlong } from "../view/camera/quaternion";
import { CLOUD_ENTRIES } from "../view/engine/catalogue";
import { BUFFER_USAGE } from "../view/engine/gpuFlags";
import type { RenderEngine } from "../view/engine/types";
import {
  cloudFieldParams,
  cloudSeedOf,
  normalDensity,
  normalTail,
} from "../view/clouds/cloudDensity";
import {
  CLOUD_LAYER_MATERIAL,
  CLOUD_SEASON_STEP_RAD,
  type CloudFrame,
  cloudLayerDraw,
  cloudLayerHeightM,
  CloudLayers,
  cloudLayerUniforms,
  cloudShellOf,
  largestWindMps,
  seasonCoverage,
} from "../view/clouds/cloudLayer";
import { cloudNoiseTexels } from "../view/clouds/cloudNoise";
import {
  CLOUD_PROBE_MATERIAL,
  CLOUD_PROBE_NAMES,
  CLOUD_PROBE_OUTPUTS,
  type CloudProbeCase,
  decodeCloudProbe,
  expectedCloudProbe,
  packCloudProbeCases,
} from "../view/clouds/cloudProbe";
import type { CloudLightInput } from "../view/clouds/cloudShadow";
import { type CloudCondensate, STAND_IN_CONDENSATES } from "../view/clouds/condensates";
import type { GraphicsStatusStore } from "../view/engine/status";
import { checkCatalogue } from "./catalogue";
import { type Checks, drawOf, frameOf, fullScreenMesh, halfTexels } from "./harness";

/** The coverage texture's z_c and N, absolute. */
const COVERAGE_TOLERANCE = 5e-3;
/** Its wind, m s⁻¹. */
const WIND_TOLERANCE_MPS = 0.05;
/** z_r, absolute, in standard deviations of z. */
const RESOLVED_TOLERANCE = 0.02;
/** s, absolute. */
const UNRESOLVED_TOLERANCE = 1e-4;
/** f from the GPU's own z_c, N, z_r and s, absolute. */
const SHARE_TOLERANCE = 1e-4;
/** The mean τ from the GPU's own inputs, relative, beside 10⁻⁶ absolute. */
const TAU_RELATIVE_TOLERANCE = 1e-3;
/** The least s at which the share is held end to end. */
const COARSE_SD = 0.3;
/** The share end to end at such a footprint, absolute. */
const COARSE_SHARE_TOLERANCE = 0.02;
/** δ-Eddington's R and T_diff, absolute. */
const SLAB_TOLERANCE = 2e-4;
/** T_dir, relative, beside 10⁻⁷ absolute. */
const DIRECT_RELATIVE_TOLERANCE = 2e-5;
/** The ground's factors, absolute. */
const GROUND_TOLERANCE = 0.02;

const RAD_PER_DEG = Math.PI / 180;

/** An Earth-like datum, m (WGS 84's semi-axes). */
const FIGURE = { equatorialRadiusM: 6_378_137, polarRadiusM: 6_356_752.3 };

/** Liquid water's stand-in deck, from 1 km to 3 km. */
const CONDENSATE: CloudCondensate = {
  condensate: "H2O",
  phase: "liquid",
  asymmetry: STAND_IN_CONDENSATES.liquidWater.asymmetry,
  coAlbedo: STAND_IN_CONDENSATES.liquidWater.coAlbedo,
  inCloudOpticalDepth550: STAND_IN_CONDENSATES.liquidWater.inCloudOpticalDepth550,
  baseAltitudeM: 1_000,
  topAltitudeM: 3_000,
  opticsSource: "stand-in",
  opticalDepthSource: "stand-in",
};

/** The body's frame: a cloud fraction of 0.67, a season of 30°, mid-afternoon of a day. */
const FRAME: CloudFrame = {
  id: "0200080020000000.0103",
  clouds: { cloudFraction: 0.67, surfacePressurePa: 101_325, decks: [] },
  condensate: CONDENSATE,
  figure: FIGURE,
  season: { subsolarLatitudeRad: 120 * CLOUD_SEASON_STEP_RAD },
  time: { seconds: 123_456, nanos: 500_000_000 },
  rate: 1,
  octaves: 12,
};

/** The twins' inputs for {@link FRAME}, as `CloudLayers.frame` makes them. */
function twinOf(noise: Uint8Array): CloudLightInput {
  const coverage = seasonCoverage(FRAME.season, FRAME.clouds.cloudFraction);
  const params = cloudFieldParams({
    seed: cloudSeedOf(FRAME.id),
    condensate: FRAME.condensate,
    radiusM: FIGURE.equatorialRadiusM,
    time: FRAME.time,
    rate: FRAME.rate,
    windMps: largestWindMps(coverage),
    octaves: FRAME.octaves,
  });
  return {
    coverage,
    noise,
    params,
    shell: cloudShellOf(FIGURE, cloudLayerHeightM(CONDENSATE)),
    optics: CONDENSATE,
    datumRadiusM: FIGURE.equatorialRadiusM,
  };
}

/** The unit direction at a geocentric latitude and longitude. */
function directionAt(latitudeDeg: number, longitudeDeg: number): Vec3 {
  const phi = latitudeDeg * RAD_PER_DEG;
  const lambda = longitudeDeg * RAD_PER_DEG;
  return {
    x: Math.cos(phi) * Math.cos(lambda),
    y: Math.cos(phi) * Math.sin(lambda),
    z: Math.sin(phi),
  };
}

/** The point along `u` on the spheroid of semi-axes `a` and `c`. */
function onSpheroid(u: Vec3, a: number, c: number): Vec3 {
  const k = a / c;
  const r = a / Math.hypot(u.x, u.y, u.z * k);
  return { x: u.x * r, y: u.y * r, z: u.z * r };
}

/** The unit vector of `v`. */
function unit(v: Vec3): Vec3 {
  const n = Math.hypot(v.x, v.y, v.z);
  return { x: v.x / n, y: v.y / n, z: v.z / n };
}

/** The places the field is probed at: both hemispheres, both signs of longitude, near a pole. */
const PLACES: ReadonlyArray<readonly [number, number]> = [
  [10, 20],
  [-35, -120],
  [52, 170],
  [-8, -3],
  [84, 60],
];

/** Footprints, m, with the octaves drawn at most: coarse to fine, and a cheaper reader's cap. */
const FOOTPRINTS: ReadonlyArray<readonly [number, number]> = [
  [400_000, 12],
  [60_000, 12],
  [8_000, 12],
  [600, 12],
  [8_000, 4],
];

/** The probe's cases: the field, the optics and the ground's tap. */
function probeCases(twin: CloudLightInput): CloudProbeCase[] {
  const { shell } = twin;
  const field = PLACES.flatMap(([lat, lon]) =>
    FOOTPRINTS.map(([footprintM, octaveCap]): CloudProbeCase => ({
      kind: "field",
      pointM: onSpheroid(directionAt(lat, lon), shell.equatorialRadiusM, shell.polarRadiusM),
      footprintM,
      octaveCap,
    })),
  );
  const coAlbedos = [0, CONDENSATE.coAlbedo, 1e-5, 1e-3, 0.1];
  const optics: CloudProbeCase[] = [];
  for (const coAlbedo of coAlbedos) {
    for (const tau of [0.01, 1, 10, 50]) {
      for (const asymmetry of [0.75, 0.85]) {
        for (const mu0 of [1, 0.5, 0.02]) {
          optics.push({ kind: "optics", tau, asymmetry, coAlbedo, mu0 });
        }
      }
    }
  }
  // About kμ₀ = 1 at g 0.95 and ϖ₀ 0.9 (k ≈ 1.11): the window's middle and both edges.
  for (const mu0 of [0.9, 0.8955, 0.905, 0.9001]) {
    optics.push({ kind: "optics", tau: 2, asymmetry: 0.95, coAlbedo: 0.1, mu0 });
  }
  const sunRadius = 4.65e-3;
  const ground: CloudProbeCase[] = PLACES.flatMap(([lat, lon]) => {
    const u = directionAt(lat, lon);
    const place = onSpheroid(u, FIGURE.equatorialRadiusM, FIGURE.polarRadiusM);
    const overA = {
      x: place.x / FIGURE.equatorialRadiusM,
      y: place.y / FIGURE.equatorialRadiusM,
      z: place.z / FIGURE.equatorialRadiusM,
    };
    const east = unit({ x: -u.y, y: u.x, z: 0 });
    return [0, 0.6, 2.5].flatMap((tilt) =>
      [sunRadius, 0.1].map((sourceRadiusRad): CloudProbeCase => ({
        kind: "ground",
        placeOverA: overA,
        sun: unit({ x: u.x + tilt * east.x, y: u.y + tilt * east.y, z: u.z + tilt * east.z }),
        sourceRadiusRad,
      })),
    );
  });
  // A place above the shell keeps every part.
  const high = onSpheroid(
    directionAt(20, 40),
    FIGURE.equatorialRadiusM + 9_000,
    FIGURE.polarRadiusM + 9_000,
  );
  ground.push({
    kind: "ground",
    placeOverA: {
      x: high.x / FIGURE.equatorialRadiusM,
      y: high.y / FIGURE.equatorialRadiusM,
      z: high.z / FIGURE.equatorialRadiusM,
    },
    sun: directionAt(20, 40),
    sourceRadiusRad: sunRadius,
  });
  return [...field, ...optics, ...ground];
}

/** The largest of `values`, 0 for none. */
function largest(values: ReadonlyArray<number>): number {
  return values.reduce((most, value) => Math.max(most, value), 0);
}

/** The probe's results and their twins, for one kind of case. */
interface Compared {
  readonly gpu: ReadonlyArray<number>;
  readonly cpu: ReadonlyArray<number>;
  readonly probe: CloudProbeCase;
}

/** R11.T7.b's field, optics and ground's tap against their twins (the module's remarks). */
export async function checkCloudTwins(engine: RenderEngine, checks: Checks): Promise<void> {
  const layers = new CloudLayers(engine, "smoke");
  layers.step(() => performance.now(), Number.POSITIVE_INFINITY);
  const gpu = layers.frame(FRAME);
  if (gpu === null) {
    layers.dispose();
    throw new Error("the clouds' noise tile was not made in one unbounded step");
  }
  const twin = twinOf(cloudNoiseTexels());
  const cases = probeCases(twin);
  const packed = packCloudProbeCases(cases);
  const caseBuffer = engine.createBuffer({
    name: "cloud probe cases",
    bytes: packed.byteLength,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
    category: "other",
  });
  engine.writeBuffer(caseBuffer, 0, packed);
  const target = engine.createRenderTarget({
    name: "cloud probe",
    size: { widthPx: CLOUD_PROBE_OUTPUTS, heightPx: cases.length },
    format: "r32uint",
    mips: 1,
    depth: false,
    category: "render-targets",
  });
  let results: number[][];
  try {
    const material = engine.createMaterial(CLOUD_PROBE_MATERIAL);
    target.render(
      frameOf(
        "cloud probe",
        [
          drawOf(fullScreenMesh(engine, "cloud probe's triangle"), material, [0, 0, 0, 0], {
            uniforms: {},
            textures: {
              [CLOUD_PROBE_NAMES.coverage]: gpu.coverage,
              [CLOUD_PROBE_NAMES.noise]: gpu.noise,
            },
            storageBuffers: {
              [CLOUD_PROBE_NAMES.layer]: gpu.layer,
              [CLOUD_PROBE_NAMES.cases]: caseBuffer,
            },
          }),
        ],
        CLOUD_PROBE_OUTPUTS / cases.length,
      ),
    );
    results = decodeCloudProbe(
      await engine.readTexture(target.colour, 0, undefined, "tolerance"),
      cases.length,
    );
  } finally {
    target.dispose();
    engine.releaseBuffer(caseBuffer);
    layers.dispose();
  }
  const expected = expectedCloudProbe(cases, twin);
  const compared: Compared[] = cases.map((probe, i) => ({
    gpu: results[i] ?? [],
    cpu: expected[i] ?? [],
    probe,
  }));
  checkField(
    checks,
    compared.filter(({ probe }) => probe.kind === "field"),
    twin.params.meanTau,
  );
  checkOptics(
    checks,
    compared.filter(({ probe }) => probe.kind === "optics"),
  );
  checkGround(
    checks,
    compared.filter(({ probe }) => probe.kind === "ground"),
  );
}

/** The difference of a GPU value from its twin's, `NaN` where either is missing. */
function off(a: ReadonlyArray<number>, b: ReadonlyArray<number>, i: number): number {
  return Math.abs((a[i] ?? Number.NaN) - (b[i] ?? Number.NaN));
}

/** Whether every value is finite. */
function allFinite(rows: ReadonlyArray<Compared>): boolean {
  return rows.every(({ gpu }) => gpu.length === CLOUD_PROBE_OUTPUTS && gpu.every(Number.isFinite));
}

function checkField(checks: Checks, rows: ReadonlyArray<Compared>, meanTau: number): void {
  const thresholds = rows.map(({ gpu, cpu }) => off(gpu, cpu, 5));
  const scales = rows.map(({ gpu, cpu }) => off(gpu, cpu, 6));
  const winds = rows.map(({ gpu, cpu }) => off(gpu, cpu, 7));
  checks.check(
    "R11.T7.b the clouds' coverage texture read at a place equals its twin's",
    allFinite(rows) &&
      largest(thresholds) <= COVERAGE_TOLERANCE &&
      largest(scales) <= COVERAGE_TOLERANCE &&
      largest(winds) <= WIND_TOLERANCE_MPS,
    `largest |Δz_c| ${largest(thresholds).toExponential(2)}, |ΔN| ${largest(scales).toExponential(2)} (≤ ${COVERAGE_TOLERANCE}), |Δu| ${largest(winds).toExponential(2)} m/s (≤ ${WIND_TOLERANCE_MPS}) over ${rows.length} cases`,
  );
  const resolved = rows.map(({ gpu, cpu }) => off(gpu, cpu, 3));
  const sd = rows.map(({ gpu, cpu }) => off(gpu, cpu, 4));
  checks.check(
    "R11.T7.b the clouds' field's resolved structure and unresolved spread equal their twin's",
    largest(resolved) <= RESOLVED_TOLERANCE && largest(sd) <= UNRESOLVED_TOLERANCE,
    `largest |Δz_r| ${largest(resolved).toExponential(2)} (≤ ${RESOLVED_TOLERANCE}), |Δs| ${largest(sd).toExponential(2)} (≤ ${UNRESOLVED_TOLERANCE})`,
  );
  // The share and mean τ of the GPU's own z_c, N, z_r and s, in f64.
  const own = rows.map(({ gpu }) => {
    const [, , , zr = 0, s = 1, zc = 0, scale = 0] = gpu;
    const d = (zc - zr) / s;
    const q = normalTail(d);
    return { q, tau: Math.max(meanTau * scale * s * (normalDensity(d) - d * q), 0) };
  });
  const shares = rows.map(({ gpu }, i) => Math.abs((gpu[0] ?? Number.NaN) - (own[i]?.q ?? 0)));
  const taus = rows.map(({ gpu }, i) => {
    const want = own[i]?.tau ?? 0;
    return Math.abs((gpu[1] ?? Number.NaN) - want) / (TAU_RELATIVE_TOLERANCE * want + 1e-6);
  });
  checks.check(
    "R11.T7.b the clouds' field's share and mean τ follow its own structure in f32",
    largest(shares) <= SHARE_TOLERANCE && largest(taus) <= 1,
    `largest |Δf| ${largest(shares).toExponential(2)} (≤ ${SHARE_TOLERANCE}), mean τ ${largest(taus).toFixed(3)} of its tolerance`,
  );
  const coarse = rows.filter(({ cpu }) => (cpu[4] ?? 0) >= COARSE_SD);
  const coarseShares = coarse.map(({ gpu, cpu }) => off(gpu, cpu, 0));
  checks.check(
    "R11.T7.b the clouds' share at a coarse footprint equals its twin's",
    coarse.length >= 5 && largest(coarseShares) <= COARSE_SHARE_TOLERANCE,
    `largest |Δf| ${largest(coarseShares).toExponential(2)} (≤ ${COARSE_SHARE_TOLERANCE}) over ${coarse.length} cases of s ≥ ${COARSE_SD}`,
  );
}

function checkOptics(checks: Checks, rows: ReadonlyArray<Compared>): void {
  const reflectance = rows.map(({ gpu, cpu }) => off(gpu, cpu, 0));
  const diffuse = rows.map(({ gpu, cpu }) => off(gpu, cpu, 2));
  const direct = rows.map(
    ({ gpu, cpu }) => off(gpu, cpu, 1) / (DIRECT_RELATIVE_TOLERANCE * (cpu[1] ?? 0) + 1e-7),
  );
  checks.check(
    "R11.T7.b the f32 δ-Eddington equals its f64 twin",
    allFinite(rows) &&
      largest(reflectance) <= SLAB_TOLERANCE &&
      largest(diffuse) <= SLAB_TOLERANCE &&
      largest(direct) <= 1,
    `largest |ΔR| ${largest(reflectance).toExponential(2)}, |ΔT_diff| ${largest(diffuse).toExponential(2)} (≤ ${SLAB_TOLERANCE}), T_dir ${largest(direct).toFixed(3)} of its tolerance, over ${rows.length} cases`,
  );
  const thick = rows.find(
    ({ probe }) => probe.kind === "optics" && probe.tau === 10 && probe.mu0 === 1,
  );
  checks.check(
    "R11.T7.b a τ = 10 layer under a zenith sun lets through under 10⁻⁴ of the direct beam",
    thick !== undefined && (thick.gpu[1] ?? 1) < 1e-4,
    `T_dir ${(thick?.gpu[1] ?? Number.NaN).toExponential(3)}`,
  );
}

function checkGround(checks: Checks, rows: ReadonlyArray<Compared>): void {
  const below = rows.slice(0, -1);
  const differences = below.map(({ gpu, cpu }) =>
    Math.max(off(gpu, cpu, 0), off(gpu, cpu, 1), off(gpu, cpu, 2)),
  );
  checks.check(
    "R11.T7.b the clouds' light on the ground equals its twin's",
    allFinite(rows) && largest(differences) <= GROUND_TOLERANCE,
    `largest difference ${largest(differences).toExponential(2)} (≤ ${GROUND_TOLERANCE}) over ${below.length} cases`,
  );
  const above = rows.at(-1)?.gpu ?? [];
  checks.check(
    "R11.T7.b a place above the clouds' shell keeps all its light",
    above[0] === 1 && above[1] === 1 && above[2] === 0,
    `(${above.slice(0, 3).join(", ")})`,
  );
}

/** The layer's draws: a camera above the shell looking down past the limb, and one beneath it. */
const LAYER_VIEWS = [
  { heightM: 400_000, pitchRad: -0.35, name: "above" },
  { heightM: 300, pitchRad: 0.6, name: "below" },
] as const;

/** The layer's target's side, px. */
const LAYER_SIDE_PX = 32;

/** R11.T7.b's layer drawn from above and below its shell: every texel finite, its α in [0, 1]. */
export async function checkCloudLayerDraw(engine: RenderEngine, checks: Checks): Promise<void> {
  const layers = new CloudLayers(engine, "smoke layer");
  layers.step(() => performance.now(), Number.POSITIVE_INFINITY);
  const gpu = layers.frame(FRAME);
  const tables = createAirlessTables(engine, "cloud layer: no air");
  const size = { widthPx: LAYER_SIDE_PX, heightPx: LAYER_SIDE_PX };
  const scene = engine.createRenderTarget({
    name: "cloud layer scene",
    size,
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  const target = engine.createRenderTarget({
    name: "cloud layer",
    size,
    format: "rgba16float",
    mips: 1,
    depth: false,
    category: "render-targets",
  });
  try {
    if (gpu === null || scene.depth === null) {
      throw new Error("the clouds' state or the scene's depth was not made");
    }
    scene.render(frameOf("cloud layer scene", []));
    const material = engine.createMaterial(CLOUD_LAYER_MATERIAL);
    const triangle = fullScreenMesh(engine, "cloud layer's triangle");
    const shell = cloudShellOf(FIGURE, cloudLayerHeightM(CONDENSATE));
    for (const view of LAYER_VIEWS) {
      const at = directionAt(15, 25);
      const radius = FIGURE.equatorialRadiusM + view.heightM;
      // Looking east, pitched above or below the horizon.
      const east = unit({ x: -at.y, y: at.x, z: 0 });
      const c = Math.cos(view.pitchRad);
      const s = Math.sin(view.pitchRad);
      const orientation = lookAlong(
        { x: c * east.x + s * at.x, y: c * east.y + s * at.y, z: c * east.z + s * at.z },
        at,
      );
      const uniforms = cloudLayerUniforms({
        camera: {
          positionM: { x: at.x * radius, y: at.y * radius, z: at.z * radius },
          orientation,
          fovXRad: 1.2,
          viewport: size,
        },
        figure: FIGURE,
        shell,
        airTopM: 100_000,
        suns: [
          {
            directionBodyFixed: unit({ x: at.x + 0.3 * east.x, y: at.y + 0.3 * east.y, z: at.z }),
            illuminanceLx: [120_000, 118_000, 110_000],
            angularRadiusRad: 4.65e-3,
            curveSlot: 0,
          },
        ],
        exposureScale: 1e-5,
        octaves: 9,
      });
      target.render(
        frameOf(`cloud layer ${view.name}`, [
          cloudLayerDraw(triangle, material, gpu, scene.depth, tables, uniforms),
        ]),
      );
      // The harness's checks run in order: each reads the GPU back before the next draws.
      // oxlint-disable-next-line no-await-in-loop
      const texels = halfTexels(await engine.readTexture(target.colour));
      let drawn = 0;
      let bad = 0;
      for (let i = 0; i < texels.length; i += 4) {
        const alpha = texels[i + 3] ?? Number.NaN;
        const finite = [0, 1, 2, 3].every((k) => Number.isFinite(texels[i + k] ?? Number.NaN));
        if (!finite || alpha < 0 || alpha > 1) {
          bad += 1;
        } else if (alpha > 0) {
          drawn += 1;
        }
      }
      checks.check(
        `R11.T7.b the cloud layer seen from ${view.name} its shell draws every texel finite`,
        bad === 0 && drawn > 0,
        `${drawn} of ${LAYER_SIDE_PX * LAYER_SIDE_PX} texels drawn, ${bad} not finite or of α outside [0, 1]`,
      );
    }
  } finally {
    target.dispose();
    scene.dispose();
    engine.releaseTexture(tables.body);
    layers.dispose();
  }
}

/** R11.T7.b's kernels compiled: the layer and the probe (`CLOUD_ENTRIES`). */
export async function checkCloudCatalogue(
  engine: RenderEngine,
  status: GraphicsStatusStore,
  checks: Checks,
): Promise<void> {
  await checkCatalogue(engine, status, CLOUD_ENTRIES, checks);
}
