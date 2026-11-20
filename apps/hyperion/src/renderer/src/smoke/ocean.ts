/**
 * Plan R11, R11.T8.b's checks on the run's adapter, in smoke part 27: the far ocean's WGSL
 * (`ocean/oceanFar.wgsl`) against its CPU twin (`ocean/oceanFar.ts`), through its probe kernel, and
 * the `ocean` pass that draws it (`ocean/oceanPass.wgsl`) against the same twin through the pass's
 * own rays.
 *
 * @remarks
 * A level-5 water world under a 7 m/s wind, with a dry cell, a cell under ice, a closed basin 250 m
 * above the sea and an unsurveyed cell, is uploaded as an ocean map (`rgba16float`, six layers). The
 * probe meets rays with it: at the glint's specular point of three geometries (the sun behind the
 * camera, oblique, near the terminator) and about each, straight down onto each special cell, past
 * the limb, and over an oblate figure. Each ray's hit and radiance are held to the twin's, run on the
 * probe's own `f32` inputs: the same rays met or missed, the distance to 10⁻⁵, the radiance to 10⁻³
 * of the case's brightest.
 *
 * The pass draws the same world from two radii out into a 48 × 48 target with depth, lit by one
 * source through the airless tables, so that each pixel's light is the twin's at the source's own
 * light: each pixel's ray as the pass forms it (`oceanPassRay`), met or missed as the twin meets
 * it, its radiance to 2 × 10⁻³ of the frame's brightest and its meter class the lit or unlit body's,
 * a pixel at a cell's edge or the limb, where `f32` and `f64` part, let off up to 2% of the pixels.
 * Then a flat wall drawn after it at the sea's median view depth, testing depth and writing none,
 * stands in front of every pixel whose sea lies farther and behind every one whose sea lies nearer:
 * the pass wrote its own depth, reversed-Z, and its test kept the nearer. A moon of 0.3 radii three
 * radii towards the sun eclipses the sunward sea, each pixel's light dimmed as the twin's
 * (`oceanSourceVisible`) dims it, its umbra unlit.
 *
 * R11.T8.c's near sea (`checkOceanNear`) and R11.T8.d's shoreline (`checkOceanShore`) are drawn
 * through the grid over a scene drawn first, its depth copied for the sea to read; with the
 * captures, R11.T8.d's coast from 1 km, 100 m and 2 m (`captureCoast`).
 */
import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../geometry/vec3";
import { createAirlessTables } from "../view/atmosphere/surfaceLighting";
import { type CloudBodyGpu, CLOUD_LAYER_ROWS } from "../view/clouds/cloudLayer";
import { cloudMapFrameOf } from "../view/clouds/cloudMap";
import { CLEAR_GROUND, type CloudGroundLight } from "../view/clouds/cloudShadow";
import {
  type CloudShadowFrame,
  cloudGroundLightAt,
  packCloudShadow,
  type ShadowLevelRead,
} from "../view/clouds/cloudShadowMap";
import { oceanCloudedLight } from "../view/ocean/oceanClouds";
import { toHalfBits } from "../view/sky/half";
import { DISC_ANNULI_HIGH } from "../view/lighting/annuli";
import { SUN_RADIUS_M } from "../view/lighting/hostDisc";
import { hostAnnuli } from "../view/lighting/hostLights";
import { AU_M } from "../view/scenes/kept";
import { aHostDisc } from "../test/litFixtures";
import { lookAlong, rotate } from "../view/camera/quaternion";
import { OCEAN_FIXTURE_LEVEL, oceanCellCentre, waterWorldAnswer } from "../test/oceanFixtures";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../view/engine/gpuFlags";
import type {
  BufferHandle,
  ColourTargetFormat,
  ComputeHandle,
  DrawItem,
  MaterialHandle,
  MeshHandle,
  RenderEngine,
  RenderTarget,
  TextureHandle,
  WgslMaterialSpec,
} from "../view/engine/types";
import FRAME_WGSL from "../view/shaders/frame.wgsl?raw";
import { specularPoint } from "../view/ocean/glint";
import type { OceanLiquid } from "../view/ocean/liquids";
import {
  type OceanFarOptics,
  oceanFarSample,
  oceanLiquidOptics,
  oceanSeaConstants,
} from "../view/ocean/oceanFar";
import {
  OCEAN_FAR_PROBE,
  OCEAN_PROBE_WORKGROUP,
  type OceanProbeRay,
  packOceanFarProbe,
  packOceanRays,
} from "../view/ocean/oceanFarProbe";
import {
  type HeldOceanMap,
  OCEAN_MAP_FORMAT,
  oceanTexelAt,
  type OceanTexels,
  packOceanTexels,
} from "../view/ocean/oceanMap";
import {
  nearSeaAnchor,
  OCEAN_WAVE_ROWS,
  oceanGridMesh,
  oceanGridSize,
  oceanNearHit,
  oceanNearLight,
  oceanThickness,
  packOceanWaves,
  spheroidNormalAt,
} from "../view/ocean/oceanNear";
import {
  OCEAN_DEPTH_COPY_MATERIAL,
  OCEAN_NEAR_PASS_MATERIAL,
  oceanDepthCopyDraw,
  oceanNearDraw,
  oceanNearState,
  oceanNearUniforms,
  type OceanNearView,
} from "../view/ocean/oceanNearPass";
import { type OceanMask, oceanMaskOf } from "../view/ocean/oceanMask";
import { type OceanSea, STANDARD_GRAVITY_MPS2 } from "../view/ocean/oceanBody";
import {
  OCEAN_ECLIPSE_ROWS,
  OCEAN_FAR_CLOUD_PASS_MATERIAL,
  OCEAN_FAR_PASS_MATERIAL,
  OCEAN_LIT_SHARE,
  type OceanDrawView,
  oceanDrawOptics,
  type OceanEclipse,
  oceanPassDraw,
  oceanPassRay,
  oceanPassUniforms,
  oceanSourceVisible,
  packOceanEclipse,
} from "../view/ocean/oceanPass";
import { BLOOM_THRESHOLD_EXPOSED } from "../view/post/bloom";
import type { OceanMapAnswer } from "../view/terrain/workers/messages";
import { EARTH_SEA } from "../view/ocean/seaState";
import {
  setRmsHeightM,
  shoreFade,
  shoreFoamShare,
  shorePixelReachM,
} from "../view/ocean/shoreline";
import { fieldCellIndex } from "../view/terrain/coverage";
import { OCEAN_MAP_WIND_BYTES } from "../view/terrain/workers/messages";
import {
  type Checks,
  drawOf,
  flatSpec,
  frameOf,
  fullScreenMesh,
  halfTexels,
  NEAR_M,
  projection,
} from "./harness";
import type { CapturedImage } from "./atmosphere";
import { hdrCapture } from "./hdrCapture";

/** Earth's mean radius, m. */
const R = 6_371_000;

/** Pure water, red, green, blue. */
const WATER: OceanLiquid = {
  temperatureK: 288,
  densityKgM3: 999,
  refractiveIndex: [1.331, 1.334, 1.338],
  absorptionPerM: [0.35, 0.06, 0.0064],
  scatteringPerM: [0.0012, 0.0019, 0.0045],
  surfaceTensionNPerM: 0.0735,
  viscosityPaS: 1.14e-3,
  basis: "measured",
  missing: [],
  approximate: false,
};

/** The special cells, on face +x: dry, iced, a closed basin, and one left unsurveyed. */
// Away from the face's centre, which the first geometry's specular point lies under.
const DRY = { face: 0, i: 4, j: 4 } as const;
const ICED = { face: 0, i: 5, j: 4 } as const;
const LAKE = { face: 0, i: 6, j: 4 } as const;
const UNSURVEYED = { face: 0, i: 7, j: 4 } as const;

/** The ocean map: a water world, its special cells in place and one cell left unsurveyed. */
function oceanTexels(): OceanTexels {
  return oceanWorld().texels;
}

/** The ocean map's answer, its mask and its texels (see {@link oceanTexels}). */
function oceanWorld(): {
  readonly answer: OceanMapAnswer;
  readonly mask: OceanMask;
  readonly texels: OceanTexels;
} {
  const world = waterWorldAnswer(4000, 64, 152, [
    { ...DRY, depthM: 0, levelM: 300, ice: 0 },
    { ...ICED, depthM: 30, levelM: 0, ice: 255 },
    { ...LAKE, depthM: 40, levelM: 250, ice: 0 },
  ]);
  const left = fieldCellIndex({ ...UNSURVEYED, level: OCEAN_FIXTURE_LEVEL });
  const kept = [...world.cells.keys()].filter((k) => world.cells[k] !== left);
  const answer = {
    ...world,
    cells: Uint32Array.from(kept, (k) => world.cells[k] ?? 0),
    depthM: Float32Array.from(kept, (k) => world.depthM[k] ?? 0),
    levelM: Float32Array.from(kept, (k) => world.levelM[k] ?? 0),
    ice: Uint8Array.from(kept, (k) => world.ice[k] ?? 0),
    winds: keptWinds(world.winds, kept),
  };
  const mask = oceanMaskOf(answer);
  return { answer, mask, texels: packOceanTexels(answer, mask, { from: 0, to: 0, weight: 0 }) };
}

/** The winds of the answer's places `kept`, in their order. */
function keptWinds(winds: Uint8Array, kept: ReadonlyArray<number>): Uint8Array {
  const out = new Uint8Array(OCEAN_MAP_WIND_BYTES * kept.length);
  kept.forEach((k, n) => {
    out.set(
      winds.subarray(OCEAN_MAP_WIND_BYTES * k, OCEAN_MAP_WIND_BYTES * (k + 1)),
      OCEAN_MAP_WIND_BYTES * n,
    );
  });
  return out;
}

/** The far ocean's optics over a figure of radii `a` and `c`, the sun along `sun`. */
function opticsOf(sun: Vec3, a = R, c = R): OceanFarOptics {
  return {
    equatorialRadiusM: a,
    polarRadiusM: c,
    pixelAngleRad: 2e-4,
    whitecaps: true,
    sunDirection: normalise(sun),
    glintCapCdM2: 5e5,
    sunLux: [1.1e5, 1.2e5, 1.25e5],
    skyLux: [1.5e4, 2e4, 3e4],
    sea: oceanSeaConstants(EARTH_SEA.gravityMps2, EARTH_SEA.liquid, EARTH_SEA.air),
    ...oceanLiquidOptics(WATER),
    bedAlbedo: [0.3, 0.3, 0.25],
  };
}

/** A ray from `origin` towards `target`. */
function rayTo(origin: Vec3, target: Vec3): OceanProbeRay {
  return { originM: origin, direction: normalise(sub(target, origin)) };
}

/** A ray straight down onto the centre of cell (face 0, i, j) from three radii out. */
function downOnto(cell: { readonly i: number; readonly j: number }): OceanProbeRay {
  const d = oceanCellCentre(0, cell.i, cell.j);
  const up = normalise(vec3(d[0], d[1], d[2]));
  return rayTo(scale(up, 3 * R), scale(up, R));
}

/** The specular point's ray of a geometry and eight about it, 10⁻³ rad of the camera's view out. */
function aboutSpecular(camera: Vec3, sun: Vec3): OceanProbeRay[] {
  const point = specularPoint(camera, sun, R);
  if (point === null) {
    return [];
  }
  const centre = normalise(sub(point.pointM, camera));
  const e1 = normalise(cross(centre, vec3(0, 0, 1)));
  const e2 = cross(centre, e1);
  const rays: OceanProbeRay[] = [];
  for (const i of [-1, 0, 1]) {
    for (const j of [-1, 0, 1]) {
      rays.push({
        originM: camera,
        direction: normalise(add(centre, add(scale(e1, i * 1e-3), scale(e2, j * 1e-3)))),
      });
    }
  }
  return rays;
}

/** One case: its optics and rays, and how many of them meet open water. */
interface OceanCase {
  readonly name: string;
  readonly optics: OceanFarOptics;
  readonly rays: ReadonlyArray<OceanProbeRay>;
  readonly met: number;
}

/** The cases (see the module's remarks). */
function oceanCases(): ReadonlyArray<OceanCase> {
  const behind = vec3(3 * R, 0, 0);
  const oblique = vec3(2 * R, 0.5 * R, 0.3 * R);
  const obliqueSun = normalise(vec3(1, -0.6, 0.2));
  const terminator = vec3(1.1 * R, 0.1 * R, 0);
  const terminatorSun = normalise(vec3(Math.cos(1.35), Math.sin(1.35), 0.05));
  const a = 6_378_137;
  const c = 6_356_752;
  return [
    {
      name: "the sun behind the camera",
      optics: opticsOf(vec3(1, 0, 0)),
      rays: [
        ...aboutSpecular(behind, vec3(1, 0, 0)),
        downOnto(DRY),
        downOnto(ICED),
        downOnto(LAKE),
        downOnto(UNSURVEYED),
        { originM: behind, direction: normalise(vec3(-1, 0.5, 0)) },
      ],
      // The nine about the specular point and the lake; not the dry, iced, unsurveyed or limb rays.
      met: 10,
    },
    {
      name: "oblique",
      optics: opticsOf(obliqueSun),
      rays: aboutSpecular(oblique, obliqueSun),
      met: 9,
    },
    {
      name: "near the terminator",
      optics: opticsOf(terminatorSun),
      rays: aboutSpecular(terminator, terminatorSun),
      met: 9,
    },
    {
      name: "an oblate figure",
      optics: opticsOf(obliqueSun, a, c),
      rays: [
        rayTo(vec3(0, 0, 3 * c), vec3(0, 0, c)),
        rayTo(vec3(2 * a, 0.2 * a, 1.5 * c), vec3(0.6 * a, 0, 0.7 * c)),
        rayTo(oblique, vec3(0, 0, 0)),
      ],
      met: 3,
    },
  ];
}

/** `optics` as the probe takes it, every member rounded to an `f32`, for the twin. */
function onProbeInputs(optics: OceanFarOptics): OceanFarOptics {
  const f = Math.fround;
  const v = (x: Vec3): Vec3 => vec3(f(x.x), f(x.y), f(x.z));
  const rgb = (x: readonly [number, number, number]): [number, number, number] => [
    f(x[0]),
    f(x[1]),
    f(x[2]),
  ];
  return {
    ...optics,
    equatorialRadiusM: f(optics.equatorialRadiusM),
    polarRadiusM: f(optics.polarRadiusM),
    pixelAngleRad: f(optics.pixelAngleRad),
    sunDirection: v(optics.sunDirection),
    glintCapCdM2: f(optics.glintCapCdM2),
    sunLux: rgb(optics.sunLux),
    skyLux: rgb(optics.skyLux),
    sea: {
      thresholdMps: f(optics.sea.thresholdMps),
      equivalentPerMps: f(optics.sea.equivalentPerMps),
      coxMunkPerMps: f(optics.sea.coxMunkPerMps),
    },
    refractiveIndex: rgb(optics.refractiveIndex),
    attenuationPerM: optics.attenuationPerM === null ? null : rgb(optics.attenuationPerM),
    deepReflectance: rgb(optics.deepReflectance),
    skyReflectance: f(optics.skyReflectance),
    bedAlbedo: rgb(optics.bedAlbedo),
  };
}

/** Runs one case's rays through the probe and reads them back. */
async function gpuSamples(
  engine: RenderEngine,
  kernel: ComputeHandle,
  oceanMap: TextureHandle,
  oceanCase: OceanCase,
): Promise<Float32Array> {
  const rays = engine.createBuffer({
    name: "smoke ocean rays",
    bytes: 32 * Math.max(oceanCase.rays.length, 1),
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
    category: "other",
  });
  engine.writeBuffer(rays, 0, packOceanRays(oceanCase.rays));
  const samples = engine.createBuffer({
    name: "smoke ocean samples",
    bytes: 32 * Math.max(oceanCase.rays.length, 1),
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_SRC,
    category: "other",
  });
  engine.dispatch(
    kernel,
    {
      uniforms: { params: packOceanFarProbe(oceanCase.optics, oceanCase.rays.length) },
      buffers: { rays, samples },
      sampled: { oceanMap },
      storage: {},
    },
    [Math.ceil(oceanCase.rays.length / OCEAN_PROBE_WORKGROUP), 1, 1],
    "smoke ocean far",
  );
  const read = new Float32Array(await engine.readBuffer(samples));
  engine.releaseBuffer(rays);
  engine.releaseBuffer(samples);
  return read;
}

/** The first ray whose GPU sample differs from its twin's beyond the tolerances, or `null`. */
function firstDifference(
  oceanCase: OceanCase,
  texels: OceanTexels,
  gpu: Float32Array,
): string | null {
  const optics = onProbeInputs(oceanCase.optics);
  const twins = oceanCase.rays.map((ray) => {
    const o = ray.originM;
    const d = ray.direction;
    const f = Math.fround;
    return oceanFarSample(
      optics,
      texels,
      vec3(f(o.x), f(o.y), f(o.z)),
      vec3(f(d.x), f(d.y), f(d.z)),
    );
  });
  const brightest = Math.max(
    1e-30,
    ...twins.flatMap((twin) => (twin === null ? [] : twin.light.radiance)),
  );
  for (let i = 0; i < twins.length; i += 1) {
    const twin = twins[i] ?? null;
    const found = (gpu[8 * i + 3] ?? 0) > 0.5;
    if (found !== (twin !== null)) {
      return `ray ${i}: GPU ${found ? "met" : "missed"} the ocean, the twin ${twin === null ? "missed" : "met"} it`;
    }
    if (twin === null) {
      continue;
    }
    const t = gpu[8 * i + 4] ?? 0;
    if (Math.abs(t - twin.hit.tM) > 1e-5 * twin.hit.tM) {
      return `ray ${i}: t ${t} m against ${twin.hit.tM} m`;
    }
    for (let k = 0; k < 3; k += 1) {
      const value = gpu[8 * i + k] ?? 0;
      const expected = twin.light.radiance[k] ?? 0;
      if (Math.abs(value - expected) > 1e-3 * brightest) {
        return `ray ${i} channel ${k}: ${value} cd/m² against ${expected} (brightest ${brightest})`;
      }
    }
  }
  return null;
}

/** R11.T8.b: the far ocean's kernel equals its twin on every case. */
export async function checkOceanFar(engine: RenderEngine, checks: Checks): Promise<void> {
  const kernel = await engine.createComputeAsync(OCEAN_FAR_PROBE);
  checks.check(
    "R11.T8.b the far ocean's probe runs on the reference path",
    kernel.path === "reference",
    kernel.path,
  );
  const texels = oceanTexels();
  const side = texels.faceTexels;
  const oceanMap = engine.createTexture({
    name: "smoke ocean map",
    size: { width: side, height: side, depthOrArrayLayers: 6 },
    dimension: "2d",
    format: OCEAN_MAP_FORMAT,
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "ocean",
  });
  engine.writeTexture(
    oceanMap,
    { x: 0, y: 0, z: 0 },
    { width: side, height: side, depthOrArrayLayers: 6 },
    texels.data,
  );
  for (const oceanCase of oceanCases()) {
    // Each case reads back before the next dispatches, as the harness's checks do.
    // oxlint-disable-next-line no-await-in-loop
    const gpu = await gpuSamples(engine, kernel, oceanMap, oceanCase);
    const difference = firstDifference(oceanCase, texels, gpu);
    const met = oceanCase.rays.filter((_, i) => (gpu[8 * i + 3] ?? 0) > 0.5).length;
    checks.check(
      `R11.T8.b the far ocean equals its twin: ${oceanCase.name}`,
      difference === null && met === oceanCase.met,
      difference ?? `${met} of ${oceanCase.rays.length} rays met the ocean`,
    );
  }
  engine.releaseTexture(oceanMap);
}

/** The pass check's target's side, px. */
const PASS_SIDE_PX = 48;

/** The share of the pass check's pixels let off at cells' edges and the limb. */
const PASS_EDGE_SHARE = 0.02;

/** The wall's tint's alpha, a meter class no pass writes, by which a pixel the wall drew is told. */
const WALL_ALPHA = 7;

/** The pass check's sun: from the body's centre, body-fixed. */
const PASS_SUN = normalise(vec3(1, 0.4, 0.3));

/** The pass check's eclipse: the Sun at 1 au and a moon of 0.3 radii three radii towards it. */
function passEclipse(): OceanEclipse {
  return {
    lights: [
      {
        direction: PASS_SUN,
        distance: AU_M / R,
        radius: SUN_RADIUS_M / R,
        illuminance: [1.1e5, 1.2e5, 1.25e5],
        annuli: hostAnnuli(aHostDisc(), DISC_ANNULI_HIGH),
      },
    ],
    occluders: [{ centre: scale(PASS_SUN, 3), radius: 0.3 }],
    radiusM: R,
  };
}

/** The pass check's view: the water world from two radii out on +x, lit obliquely, pre-exposed. */
function passView(map: HeldOceanMap): OceanDrawView {
  const sea: OceanSea = {
    body: "0200080020000000.0300",
    map,
    substance: "H2O",
    liquid: WATER,
    air: EARTH_SEA.air,
    equatorialRadiusM: R,
    polarRadiusM: R,
    seaLevelM: 0,
    surfaceGravityMps2: null,
    bedAlbedo: [0.3, 0.3, 0.25],
  };
  return {
    sea,
    camera: { positionM: vec3(2 * R, 0, 0), orientation: lookAlong(vec3(-1, 0, 0), vec3(0, 0, 1)) },
    gravityMps2: STANDARD_GRAVITY_MPS2,
    figureOnDatum: false,
    suns: [
      {
        directionBodyFixed: PASS_SUN,
        illuminanceLx: [1.1e5, 1.2e5, 1.25e5],
        angularRadiusRad: 4.65e-3,
        curveSlot: 0,
      },
    ],
    eclipse: passEclipse(),
    exposureScale: 1e-5,
    thresholdPreExposed: BLOOM_THRESHOLD_EXPOSED,
    pixelAngleRad: 2 / PASS_SIDE_PX,
    whitecaps: true,
  };
}

/**
 * Each pixel's twin: its ray's view length, what it meets of the sea, lit as the pass lights it, its
 * source's light dimmed by its eclipse at the hit, and that light's share.
 */
function passTwins(view: OceanDrawView): ReadonlyArray<{
  readonly viewLength: number;
  readonly sample: ReturnType<typeof oceanFarSample>;
  readonly visible: number;
}> {
  const size = { widthPx: PASS_SIDE_PX, heightPx: PASS_SIDE_PX };
  const sun = view.suns[0];
  const e = view.exposureScale;
  const light = sun?.illuminanceLx ?? [0, 0, 0];
  // Through the airless tables the source reaches the sea whole and no sky lights it; every value
  // pre-exposed, as the pass draws.
  const optics = onProbeInputs({
    ...oceanDrawOptics(view),
    glintCapCdM2: view.thresholdPreExposed,
    sunLux: [light[0] * e, light[1] * e, light[2] * e],
    skyLux: [0, 0, 0],
  });
  const f = Math.fround;
  const o = view.camera.positionM;
  const origin = vec3(f(o.x), f(o.y), f(o.z));
  const twins = [];
  for (let y = 0; y < PASS_SIDE_PX; y += 1) {
    for (let x = 0; x < PASS_SIDE_PX; x += 1) {
      const ray = oceanPassRay(x, y, size, projection(1), view.camera.orientation);
      const d = ray.direction;
      const direction = vec3(f(d.x), f(d.y), f(d.z));
      const whole = oceanFarSample(optics, view.sea.map.texels, origin, direction);
      const shares =
        whole === null
          ? ([1, 1, 1] as const)
          : oceanSourceVisible(view.eclipse, 0, whole.hit.point);
      const dimmed =
        whole === null || shares.every((share) => share === 1)
          ? whole
          : oceanFarSample(
              {
                ...optics,
                sunLux: [
                  optics.sunLux[0] * shares[0],
                  optics.sunLux[1] * shares[1],
                  optics.sunLux[2] * shares[2],
                ],
              },
              view.sea.map.texels,
              origin,
              direction,
            );
      twins.push({ viewLength: ray.viewLength, sample: dimmed, visible: Math.max(...shares) });
    }
  }
  return twins;
}

/** R11.T8.b: the `ocean` pass draws the far sea as its twin does, and writes its depth. */
export async function checkOceanPass(engine: RenderEngine, checks: Checks): Promise<void> {
  const world = oceanWorld();
  const side = world.texels.faceTexels;
  const oceanMap = engine.createTexture({
    name: "smoke ocean pass map",
    size: { width: side, height: side, depthOrArrayLayers: 6 },
    dimension: "2d",
    format: OCEAN_MAP_FORMAT,
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "ocean",
  });
  engine.writeTexture(
    oceanMap,
    { x: 0, y: 0, z: 0 },
    { width: side, height: side, depthOrArrayLayers: 6 },
    world.texels.data,
  );
  const airless = createAirlessTables(engine, "smoke ocean pass: no air");
  const target = engine.createRenderTarget({
    name: "smoke ocean pass",
    size: { widthPx: PASS_SIDE_PX, heightPx: PASS_SIDE_PX },
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  const material = engine.createMaterial(OCEAN_FAR_PASS_MATERIAL);
  const triangle = fullScreenMesh(engine, "smoke ocean pass triangle");
  const view = passView({ ...world, texture: oceanMap });
  const eclipseRows = engine.createBuffer({
    name: "smoke ocean pass eclipse",
    bytes: 16 * OCEAN_ECLIPSE_ROWS,
    usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
    category: "ocean",
  });
  engine.writeBuffer(eclipseRows, 0, packOceanEclipse(view.eclipse));
  try {
    target.render(
      frameOf("smoke ocean pass", [
        oceanPassDraw(triangle, material, oceanMap, airless, eclipseRows, oceanPassUniforms(view)),
      ]),
    );
    const sea = halfTexels(await engine.readTexture(target.colour));
    const twins = passTwins(view);
    const brightest = Math.max(
      1e-30,
      ...twins.flatMap(({ sample }) => (sample === null ? [] : sample.light.radiance)),
    );
    let met = 0;
    let parted = 0;
    let unlike = 0;
    let umbra = 0;
    let first: string | null = null;
    for (const [p, { sample, visible }] of twins.entries()) {
      const alpha = sea[4 * p + 3] ?? Number.NaN;
      const drawn = alpha === 2 || alpha === 3;
      if (drawn !== (sample !== null)) {
        parted += 1;
        first ??= `pixel ${p}: the pass ${drawn ? "drew" : "left"} it, the twin ${sample === null ? "missed" : "met"} the sea`;
        continue;
      }
      if (sample === null) {
        continue;
      }
      met += 1;
      umbra += visible < OCEAN_LIT_SHARE ? 1 : 0;
      const sun = view.suns[0]?.directionBodyFixed ?? vec3(0, 0, 0);
      const lit = dot(sample.hit.normal, sun) > 0 && visible > OCEAN_LIT_SHARE;
      const off = [0, 1, 2].some((k) => {
        const expected = sample.light.radiance[k] ?? 0;
        return !(
          Math.abs((sea[4 * p + k] ?? Number.NaN) - expected) <=
          2e-3 * brightest + 2e-3 * expected
        );
      });
      if (off || alpha !== (lit ? 2 : 3)) {
        unlike += 1;
        first ??= `pixel ${p}: (${[0, 1, 2, 3].map((k) => sea[4 * p + k]).join(", ")}) against (${sample.light.radiance.join(", ")}, ${lit ? 2 : 3})`;
      }
    }
    const pixels = PASS_SIDE_PX * PASS_SIDE_PX;
    checks.check(
      "R11.T8.b the ocean pass draws the far sea as its twin does, through its own rays",
      met > pixels / 10 && umbra > 0 && parted + unlike <= PASS_EDGE_SHARE * pixels,
      `${met} pixels met the sea, ${umbra} of them in the moon's umbra, ${parted} parted from the twin, ${unlike} unlike it${first === null ? "" : `; first ${first}`}`,
    );
    // A wall at the median view depth of the sea, drawn after it, testing the sea's depth.
    const depths = twins
      .flatMap(({ sample, viewLength }) => (sample === null ? [] : [sample.hit.tM / viewLength]))
      .toSorted((a, b) => a - b);
    const wallM = depths[Math.floor(depths.length / 2)] ?? 2 * R;
    const wall = engine.createMaterial(flatSpec("smoke ocean depth wall", { depthWrite: false }));
    const wallMesh = fullScreenMesh(engine, "smoke ocean depth wall mesh", -wallM);
    try {
      target.render({
        ...frameOf("smoke ocean depth wall", [drawOf(wallMesh, wall, [0, 0, 0, WALL_ALPHA])]),
        colourLoad: "load",
      });
      const walled = halfTexels(await engine.readTexture(target.colour));
      let wrong = 0;
      let nearer = 0;
      twins.forEach(({ sample, viewLength }, p) => {
        const alpha = walled[4 * p + 3] ?? Number.NaN;
        const z = sample === null ? Number.POSITIVE_INFINITY : sample.hit.tM / viewLength;
        if (z < wallM * (1 - 1e-4)) {
          nearer += 1;
          wrong += alpha === WALL_ALPHA ? 1 : 0;
        } else if (z > wallM * (1 + 1e-4)) {
          wrong += alpha === WALL_ALPHA ? 0 : 1;
        }
      });
      checks.check(
        "R11.T8.b the ocean pass writes its sea's depth, which hides what lies behind it",
        nearer > 0 && wrong <= PASS_EDGE_SHARE * pixels,
        `the wall at ${wallM.toExponential(4)} m: ${nearer} pixels of sea nearer, ${wrong} wrong`,
      );
    } finally {
      engine.releaseMaterial(wall);
      engine.releaseMesh(wallMesh);
    }
  } finally {
    target.dispose();
    engine.releaseBuffer(eclipseRows);
    engine.releaseMaterial(material);
    engine.releaseMesh(triangle);
    engine.releaseTexture(airless.body);
    engine.releaseTexture(oceanMap);
  }
}

/** The cloud check's camera's height over the sea, m: level 1 of the shadow map some 20 px across. */
const CLOUD_CHECK_HEIGHT_M = 200_000;

/**
 * The cloud check's shadow: what the volume's map holds wherever it holds the sea, a share of the
 * beam scattered down among it, which the airless tables' sky of none leaves the sky's alone.
 */
const CLOUD_CHECK_SHADOW: CloudGroundLight = { direct: 0.25, sky: 1, diffuse: 0.2 };

/**
 * R11.T7.c: the far sea's cloudy form dims each source's light by the volume's shadow map where it
 * holds the sea, and leaves the clear sea's light off it, as its twin does: the sea from 200 km over
 * +x, its clouds' layer below the sea, so that its tap is the clear sky's, and a shadow map about
 * the camera's nadir that holds {@link CLOUD_CHECK_SHADOW} on both its levels.
 */
export async function checkOceanCloudShadow(engine: RenderEngine, checks: Checks): Promise<void> {
  const world = oceanWorld();
  const side = world.texels.faceTexels;
  const oceanMap = engine.createTexture({
    name: "smoke ocean clouds map",
    size: { width: side, height: side, depthOrArrayLayers: 6 },
    dimension: "2d",
    format: OCEAN_MAP_FORMAT,
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "ocean",
  });
  engine.writeTexture(
    oceanMap,
    { x: 0, y: 0, z: 0 },
    { width: side, height: side, depthOrArrayLayers: 6 },
    world.texels.data,
  );
  const airless = createAirlessTables(engine, "smoke ocean clouds: no air");
  const target = engine.createRenderTarget({
    name: "smoke ocean clouds",
    size: { widthPx: PASS_SIDE_PX, heightPx: PASS_SIDE_PX },
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  const material = engine.createMaterial(OCEAN_FAR_CLOUD_PASS_MATERIAL);
  const triangle = fullScreenMesh(engine, "smoke ocean clouds triangle");
  const passed = passView({ ...world, texture: oceanMap });
  const view: OceanDrawView = {
    ...passed,
    camera: { ...passed.camera, positionM: vec3(R + CLOUD_CHECK_HEIGHT_M, 0, 0) },
    eclipse: null,
  };
  const sun = view.suns[0]?.directionBodyFixed ?? PASS_SUN;
  const shadow: CloudShadowFrame = {
    map: cloudMapFrameOf(vec3(1, 0, 0), view.camera.positionM, R),
    sun,
    datumRadiusM: R,
    axisRatio: 1,
    deck: { baseM: 1_000, topM: 3_000 },
  };
  const buffer = (name: string, data: Float32Array<ArrayBuffer>): BufferHandle => {
    const made = engine.createBuffer({
      name,
      bytes: data.byteLength,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "clouds",
    });
    engine.writeBuffer(made, 0, data);
    return made;
  };
  const texture = (
    name: string,
    format: "rgba16float" | "r8unorm",
    dimension: "2d" | "3d",
    texels: Uint16Array<ArrayBuffer> | Uint8Array<ArrayBuffer>,
  ): TextureHandle => {
    const size = { width: 1, height: 1, depthOrArrayLayers: 1 };
    const made = engine.createTexture({
      name,
      size,
      dimension,
      format,
      mips: 1,
      usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
      category: "clouds",
    });
    engine.writeTexture(made, { x: 0, y: 0, z: 0 }, size, texels);
    return made;
  };
  // The layer's shell half the datum's radius, below the sea, so that its tap is (1, 1, 0) and its
  // field never read; its datum radius R.
  const layerRows = new Float32Array(4 * CLOUD_LAYER_ROWS);
  layerRows.set([0.5 * R, 0.5 * R, 0, 0], 0);
  layerRows.set([0, R, 0, 0], 12);
  const s = CLOUD_CHECK_SHADOW;
  const clouds: CloudBodyGpu = {
    layer: buffer("smoke ocean clouds rows", layerRows),
    coverage: texture(
      "smoke ocean clouds coverage",
      "rgba16float",
      "2d",
      Uint16Array.from([0, 0, 0, 0], (v) => toHalfBits(v)),
    ),
    noise: texture("smoke ocean clouds noise", "r8unorm", "3d", new Uint8Array(1)),
    // One texel of the shadow, which every read of the map's two levels samples alike.
    shadowMap: texture(
      "smoke ocean clouds shadow map",
      "rgba16float",
      "2d",
      Uint16Array.from([s.direct, s.sky, s.diffuse, 1], (v) => toHalfBits(v)),
    ),
    shadowRows: buffer(
      "smoke ocean clouds shadow rows",
      Float32Array.from(packCloudShadow(shadow)),
    ),
  };
  const eclipseRows = buffer("smoke ocean clouds eclipse", packOceanEclipse(null));
  try {
    target.render(
      frameOf("smoke ocean clouds", [
        oceanPassDraw(
          triangle,
          material,
          oceanMap,
          airless,
          eclipseRows,
          oceanPassUniforms(view),
          clouds,
        ),
      ]),
    );
    const sea = halfTexels(await engine.readTexture(target.colour));
    // Each pixel's twin: the plain sea's, its source's light through the cloud's factors at the
    // hit (`oceanCloudedLight`); through the airless tables the sky's light is the cloud's alone.
    const e = view.exposureScale;
    const light = view.suns[0]?.illuminanceLx ?? [0, 0, 0];
    const optics = onProbeInputs({
      ...oceanDrawOptics(view),
      glintCapCdM2: view.thresholdPreExposed,
      sunLux: [light[0] * e, light[1] * e, light[2] * e],
      skyLux: [0, 0, 0],
    });
    const f = Math.fround;
    const o = view.camera.positionM;
    const origin = vec3(f(o.x), f(o.y), f(o.z));
    const size = { widthPx: PASS_SIDE_PX, heightPx: PASS_SIDE_PX };
    const read: ShadowLevelRead = () => s;
    let shaded = 0;
    let clear = 0;
    let unlike = 0;
    let dimmer = 0;
    let first: string | null = null;
    for (let p = 0; p < PASS_SIDE_PX * PASS_SIDE_PX; p += 1) {
      const ray = oceanPassRay(
        p % PASS_SIDE_PX,
        Math.floor(p / PASS_SIDE_PX),
        size,
        projection(1),
        view.camera.orientation,
      );
      const d = ray.direction;
      const direction = vec3(f(d.x), f(d.y), f(d.z));
      const plain = oceanFarSample(optics, view.sea.map.texels, origin, direction);
      if (plain === null) {
        continue;
      }
      const cloud = cloudGroundLightAt(shadow, plain.hit.point, sun, CLEAR_GROUND, read);
      const lit = oceanCloudedLight(optics, cloud, dot(plain.hit.normal, sun));
      const twin = oceanFarSample(
        { ...optics, sunLux: lit.sunLux, skyLux: lit.skyLux },
        view.sea.map.texels,
        origin,
        direction,
      );
      const expected = twin?.light.radiance ?? [0, 0, 0];
      const brightest = Math.max(1e-30, ...plain.light.radiance);
      const off = [0, 1, 2].some(
        (k) =>
          !(
            Math.abs((sea[4 * p + k] ?? Number.NaN) - (expected[k] ?? 0)) <=
            2e-3 * brightest + 2e-3 * (expected[k] ?? 0)
          ),
      );
      if (off) {
        unlike += 1;
        first ??= `pixel ${p}: (${[0, 1, 2].map((k) => sea[4 * p + k]).join(", ")}) against (${expected.join(", ")}), the cloud's direct ${cloud.direct}`;
      }
      if (cloud.direct <= s.direct + 1e-6) {
        shaded += 1;
        dimmer += (sea[4 * p + 1] ?? Number.NaN) < plain.light.radiance[1] ? 1 : 0;
      } else if (cloud.direct >= 1 - 1e-6) {
        clear += 1;
      }
    }
    const pixels = PASS_SIDE_PX * PASS_SIDE_PX;
    checks.check(
      "R11.T7.c the sea's cloudy form dims its sun's light where the volume's shadow map holds the sea, as its twin does",
      shaded > 20 && dimmer === shaded && clear > 20 && unlike <= PASS_EDGE_SHARE * pixels,
      `${shaded} pixels under the map's shadow, ${dimmer} of them dimmer than the plain sea, ${clear} off the map, ${unlike} unlike the twin${first === null ? "" : `; first ${first}`}`,
    );
  } finally {
    target.dispose();
    engine.releaseMaterial(material);
    engine.releaseMesh(triangle);
    engine.releaseTexture(airless.body);
    engine.releaseTexture(oceanMap);
    engine.releaseBuffer(eclipseRows);
    engine.releaseBuffer(clouds.layer);
    engine.releaseBuffer(clouds.shadowRows);
    engine.releaseTexture(clouds.coverage);
    engine.releaseTexture(clouds.noise);
    engine.releaseTexture(clouds.shadowMap);
  }
}

/** The near check's target's side, px. */
const NEAR_SIDE_PX = 48;

/** The near check's camera's height above the sea, m. */
const NEAR_HEIGHT_M = 600;

/** The near check's wall's tint: a blue no sea draws, its alpha a meter class no pass writes. */
const NEAR_WALL_TINT: readonly [number, number, number, number] = [0, 0, 3, WALL_ALPHA];

/** A water world of one wind speed code every month and cell (0 calm), its map's texels. */
function nearWorld(speedCode: number): {
  readonly answer: OceanMapAnswer;
  readonly mask: OceanMask;
  readonly texels: OceanTexels;
} {
  const answer = waterWorldAnswer(30, 64, speedCode);
  const mask = oceanMaskOf(answer);
  return { answer, mask, texels: packOceanTexels(answer, mask, { from: 0, to: 0, weight: 0 }) };
}

/** A camera `heightM` over the sea at the body's +x, looking east `pitchRad` below the horizon. */
function eastwardCamera(heightM: number, pitchRad: number): OceanNearView["camera"] {
  const up = vec3(1, 0, 0);
  const east = vec3(0, 1, 0);
  const forward = add(scale(east, Math.cos(pitchRad)), scale(up, -Math.sin(pitchRad)));
  return { positionM: scale(up, R + heightM), orientation: lookAlong(forward, up) };
}

/**
 * The near check's view: 600 m over the sea of `map`, looking 30° below the horizon to the east,
 * unless `camera` stands elsewhere; a pixel's angle `pixelAngleRad`, the grid's cell 4 of them.
 */
function nearView(
  map: HeldOceanMap,
  refraction: boolean,
  camera = eastwardCamera(NEAR_HEIGHT_M, Math.PI / 6),
  pixelAngleRad = 2 / NEAR_SIDE_PX,
): OceanNearView {
  const view = passView(map);
  return {
    ...view,
    camera,
    eclipse: null,
    // The sun high in the east, over the sea ahead.
    suns: [{ ...(view.suns[0] ?? passSun()), directionBodyFixed: normalise(vec3(1, 0.5, 0.1)) }],
    pixelAngleRad,
    time: { seconds: 1_000, nanos: 0 },
    components: 32,
    cellRad: 4 * pixelAngleRad,
    anchor: nearSeaAnchor(oceanDrawOptics(view), map.texels, camera.positionM, camera.orientation),
    refraction,
  };
}

/** A source for a view that has none, which `passView`'s always has. */
function passSun(): OceanDrawView["suns"][number] {
  return {
    directionBodyFixed: PASS_SUN,
    illuminanceLx: [1.1e5, 1.2e5, 1.25e5],
    angularRadiusRad: 4.65e-3,
    curveSlot: 0,
  };
}

/** What a near draw takes on the run's engine, made for one check and released after it. */
interface NearKit {
  readonly scene: RenderTarget;
  readonly depthBits: RenderTarget;
  readonly colour: RenderTarget;
  readonly material: MaterialHandle;
  readonly copy: MaterialHandle;
  readonly wall: MaterialHandle;
  readonly triangle: MeshHandle;
  readonly grid: MeshHandle;
  readonly airless: ReturnType<typeof createAirlessTables>;
  readonly eclipseRows: BufferHandle;
  readonly waveRows: BufferHandle;
  readonly release: () => void;
}

/** The near check's GPU objects, its targets `size`. */
function nearKit(
  engine: RenderEngine,
  size = { widthPx: NEAR_SIDE_PX, heightPx: NEAR_SIDE_PX },
): NearKit {
  const target = (name: string, format: ColourTargetFormat, depth: boolean): RenderTarget =>
    engine.createRenderTarget({ name, size, format, mips: 1, depth, category: "ocean" });
  const scene = target("smoke ocean near scene", "rgba16float", true);
  const depthBits = target("smoke ocean near depth copy", "r32uint", false);
  const colour = target("smoke ocean near colour copy", "rgba16float", false);
  const { columns, rows } = oceanGridSize(size.widthPx, size.heightPx, 4);
  const { positions, indices } = oceanGridMesh(columns, rows);
  const grid = engine.createMesh({
    name: "smoke ocean near grid",
    positions,
    indices,
    topology: "triangle-list",
    attributes: {},
  });
  const buffer = (name: string, bytes: number): BufferHandle =>
    engine.createBuffer({
      name,
      bytes,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "ocean",
    });
  const kit = {
    scene,
    depthBits,
    colour,
    material: engine.createMaterial(OCEAN_NEAR_PASS_MATERIAL),
    copy: engine.createMaterial(OCEAN_DEPTH_COPY_MATERIAL),
    wall: engine.createMaterial(flatSpec("smoke ocean near wall")),
    triangle: fullScreenMesh(engine, "smoke ocean near triangle"),
    grid,
    airless: createAirlessTables(engine, "smoke ocean near: no air"),
    eclipseRows: buffer("smoke ocean near eclipse", 16 * OCEAN_ECLIPSE_ROWS),
    waveRows: buffer("smoke ocean near waves", 16 * OCEAN_WAVE_ROWS),
  };
  engine.writeBuffer(kit.eclipseRows, 0, packOceanEclipse(null));
  return {
    ...kit,
    release: () => {
      for (const each of [kit.scene, kit.depthBits, kit.colour]) {
        each.dispose();
      }
      for (const each of [kit.material, kit.copy, kit.wall]) {
        engine.releaseMaterial(each);
      }
      engine.releaseMesh(kit.triangle);
      engine.releaseMesh(kit.grid);
      engine.releaseTexture(kit.airless.body);
      engine.releaseBuffer(kit.eclipseRows);
      engine.releaseBuffer(kit.waveRows);
    },
  };
}

/**
 * Draws the near sea of `view` over the scene as it stands, through `clip`: its depth copied, its
 * colour copied, the waves written, the grid drawn loading the scene; then reads the scene's colour
 * back.
 */
async function drawNear(
  engine: RenderEngine,
  kit: NearKit,
  view: OceanNearView,
  oceanMap: TextureHandle,
  clip = projection(1),
): Promise<Float32Array> {
  const state = oceanNearState(view);
  engine.writeBuffer(
    kit.waveRows,
    0,
    packOceanWaves(state.waves, state.set, state.peakWavelengthM),
  );
  const sceneDepth = kit.scene.depth;
  if (sceneDepth === null) {
    throw new Error("the near check's scene has no depth");
  }
  kit.depthBits.render(
    frameOf("smoke ocean near depth copy", [
      oceanDepthCopyDraw(kit.triangle, kit.copy, sceneDepth),
    ]),
  );
  kit.colour.render({
    ...frameOf("smoke ocean near colour copy", []),
    colourFrom: kit.scene.colour,
  });
  kit.scene.render({
    ...frameOf("smoke ocean near", [
      oceanNearDraw(
        kit.grid,
        kit.material,
        {
          oceanMap,
          tables: kit.airless,
          eclipseRows: kit.eclipseRows,
          waveRows: kit.waveRows,
          sceneDepthBits: kit.depthBits.colour,
          sceneColour: kit.colour.colour,
        },
        oceanNearUniforms(view, state.frame),
      ),
    ]),
    projection: clip,
    colourLoad: "load",
  });
  return halfTexels(await engine.readTexture(kit.scene.colour));
}

/** An ocean map's texels uploaded under `name`, six layers. */
function uploadOceanMap(engine: RenderEngine, texels: OceanTexels, name: string): TextureHandle {
  const side = texels.faceTexels;
  const texture = engine.createTexture({
    name,
    size: { width: side, height: side, depthOrArrayLayers: 6 },
    dimension: "2d",
    format: OCEAN_MAP_FORMAT,
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "ocean",
  });
  engine.writeTexture(
    texture,
    { x: 0, y: 0, z: 0 },
    { width: side, height: side, depthOrArrayLayers: 6 },
    texels.data,
  );
  return texture;
}

/** Each pixel's ray and what the near twin makes of a calm sea there, lit as the pass lights it. */
function calmTwins(view: OceanNearView): ReadonlyArray<{
  readonly radiance: readonly [number, number, number] | null;
  readonly viewDistanceM: number;
}> {
  const size = { widthPx: NEAR_SIDE_PX, heightPx: NEAR_SIDE_PX };
  const state = oceanNearState(view);
  const e = view.exposureScale;
  const sun = view.suns[0];
  const lux = sun?.illuminanceLx ?? [0, 0, 0];
  const optics = onProbeInputs({
    ...oceanDrawOptics(view),
    glintCapCdM2: view.thresholdPreExposed,
    sunLux: [lux[0] * e, lux[1] * e, lux[2] * e],
    skyLux: [0, 0, 0],
  });
  const twins = [];
  for (let y = 0; y < NEAR_SIDE_PX; y += 1) {
    for (let x = 0; x < NEAR_SIDE_PX; x += 1) {
      const ray = oceanPassRay(x, y, size, projection(1), view.camera.orientation);
      const hit = oceanNearHit(state.frame, ray.direction);
      const world = add(view.camera.positionM, hit.pointM);
      const texel = oceanTexelAt(view.sea.map.texels, [world.x, world.y, world.z]);
      if (hit.clamped || texel === null) {
        twins.push({ radiance: null, viewDistanceM: Number.POSITIVE_INFINITY });
        continue;
      }
      const toCamera = scale(hit.pointM, -1 / hit.tM);
      const up = spheroidNormalAt(state.frame, world);
      const footprint =
        (hit.tM * view.pixelAngleRad) / Math.sqrt(Math.max(dot(toCamera, up), 0.01));
      const light = oceanNearLight(optics, optics, {
        view: toCamera,
        up,
        normal: up,
        texel,
        resolved: { upwind: 0, crosswind: 0 },
        footprintM: footprint,
        thickness: { depthM: texel.depthM, from: "bathymetry", layer: "sea" },
        shoreFoam: 0,
        bedRadiance: null,
      });
      twins.push({ radiance: light.radiance, viewDistanceM: hit.tM / ray.viewLength });
    }
  }
  return twins;
}

/**
 * R11.T8.c: the `ocean` pass's near sea on the run's adapter. A calm sea 600 m below the camera,
 * drawn through its grid over a cleared scene, equals its twin at each pixel's ray to 2 × 10⁻³ of
 * the frame's brightest, the sky above the horizon left as it was, 2% of the pixels let off at the
 * horizon and the grid's cells; then under a 12 m/s wind its waves move the light. A wall drawn
 * first at the calm sea's median view depth, its depth copied for the sea to read: every pixel of
 * sea nearer than the wall is drawn over it, every one farther left to the wall, as the lower-layer
 * cull orders them; and with refraction the wall's blue shows through the water in front of it.
 */
export async function checkOceanNear(engine: RenderEngine, checks: Checks): Promise<void> {
  const calmWorld = nearWorld(0);
  const windyWorld = nearWorld(165);
  const calmMap = uploadOceanMap(engine, calmWorld.texels, "smoke ocean near calm map");
  const windyMap = uploadOceanMap(engine, windyWorld.texels, "smoke ocean near windy map");
  const kit = nearKit(engine);
  try {
    const pixels = NEAR_SIDE_PX * NEAR_SIDE_PX;
    const calm = nearView({ ...calmWorld, texture: calmMap }, false);
    kit.scene.render(frameOf("smoke ocean near clear", []));
    const calmSea = await drawNear(engine, kit, calm, calmMap);
    const twins = calmTwins(calm);
    const brightest = Math.max(1e-30, ...twins.flatMap(({ radiance }) => radiance ?? []));
    let met = 0;
    let parted = 0;
    let first: string | null = null;
    for (const [p, { radiance }] of twins.entries()) {
      const gpu = [0, 1, 2].map((k) => calmSea[4 * p + k] ?? Number.NaN);
      if (radiance === null) {
        // Above the horizon the scene is left as it was cleared.
        parted += gpu.every((value) => value === 0) ? 0 : 1;
        continue;
      }
      met += 1;
      const off = [0, 1, 2].some(
        (k) =>
          !(
            Math.abs((gpu[k] ?? Number.NaN) - (radiance[k] ?? 0)) <=
            2e-3 * brightest + 2e-3 * (radiance[k] ?? 0)
          ),
      );
      if (off) {
        parted += 1;
        first ??= `pixel ${p}: (${gpu.join(", ")}) against (${radiance.join(", ")})`;
      }
    }
    checks.check(
      "R11.T8.c the near sea draws a calm sea as its twin does, through its grid",
      met > pixels / 10 && parted <= PASS_EDGE_SHARE * pixels,
      `${met} pixels met the sea, ${parted} parted from the twin${first === null ? "" : `; first ${first}`}`,
    );
    // Waves: the same view under a 12 m/s wind spreads the light the calm sea draws evenly.
    const windy = nearView({ ...windyWorld, texture: windyMap }, false);
    kit.scene.render(frameOf("smoke ocean near clear", []));
    const windySea = await drawNear(engine, kit, windy, windyMap);
    let finite = true;
    let moved = 0;
    for (const [p, { radiance }] of twins.entries()) {
      const g = windySea[4 * p + 1] ?? Number.NaN;
      finite &&= Number.isFinite(g);
      const before = calmSea[4 * p + 1] ?? 0;
      if (radiance !== null && Math.abs(g - before) > 0.02 * Math.max(before, 1e-30)) {
        moved += 1;
      }
    }
    const components = oceanNearState(windy).waves.length;
    checks.check(
      "R11.T8.c the near sea's waves move its light, every pixel finite",
      finite && components > 0 && moved > met / 10,
      `${components} components; ${moved} of ${met} pixels moved by over 2% of their own light`,
    );
    // A wall at the calm sea's median view depth, drawn first: the lower layer, read by its copy.
    const depths = twins
      .flatMap(({ viewDistanceM }) => (Number.isFinite(viewDistanceM) ? [viewDistanceM] : []))
      .toSorted((a, b) => a - b);
    const wallM = depths[Math.floor(depths.length / 2)] ?? NEAR_HEIGHT_M;
    const wallMesh = fullScreenMesh(engine, "smoke ocean near wall mesh", -wallM);
    try {
      const walled = async (refraction: boolean): Promise<Float32Array> => {
        kit.scene.render(
          frameOf("smoke ocean near wall", [drawOf(wallMesh, kit.wall, NEAR_WALL_TINT)]),
        );
        return drawNear(engine, kit, { ...calm, refraction }, calmMap);
      };
      const plain = await walled(false);
      let wrong = 0;
      let nearer = 0;
      for (const [p, { viewDistanceM }] of twins.entries()) {
        const isWall = (plain[4 * p + 2] ?? 0) === NEAR_WALL_TINT[2] && (plain[4 * p] ?? 1) === 0;
        if (viewDistanceM < wallM * (1 - 1e-3)) {
          nearer += 1;
          wrong += isWall ? 1 : 0;
        } else if (viewDistanceM > wallM * (1 + 1e-3)) {
          wrong += isWall ? 0 : 1;
        }
      }
      checks.check(
        "R11.T8.c the near sea reads the bed's depth: drawn over a nearer sea, left to a nearer bed",
        nearer > 0 && wrong <= PASS_EDGE_SHARE * pixels,
        `the wall at ${wallM.toFixed(1)} m: ${nearer} pixels of sea nearer, ${wrong} wrong`,
      );
      const refracted = await walled(true);
      let bluer = 0;
      for (const [p, { viewDistanceM }] of twins.entries()) {
        if (viewDistanceM < wallM * 0.99) {
          bluer += (refracted[4 * p + 2] ?? 0) > (plain[4 * p + 2] ?? 0) ? 1 : 0;
        }
      }
      checks.check(
        "R11.T8.c the near sea refracts the bed through its water on the high setting",
        bluer > nearer / 2,
        `${bluer} of ${nearer} pixels over the wall bluer with refraction`,
      );
    } finally {
      engine.releaseMesh(wallMesh);
    }
  } finally {
    kit.release();
    engine.releaseTexture(calmMap);
    engine.releaseTexture(windyMap);
  }
}

/** The coast's beach's slope, tan β: 1 in 20, a sandy beach's (R11.T8.d). */
const COAST_SLOPE = 0.05;

/** The body-fixed up, east and north at the coast's camera's foot, as `eastwardCamera`'s. */
const COAST_UP = vec3(1, 0, 0);
const COAST_EAST = vec3(0, 1, 0);
const COAST_NORTH = vec3(0, 0, 1);

/** The coast check's beach's tint: a sand no sea draws, its alpha the wall's meter class. */
const COAST_SAND_TINT: readonly [number, number, number, number] = [0.4, 0.3, 0.1, WALL_ALPHA];

/** The coast check's bed under its water: dark, so that the surf's foam stands out from it. */
const COAST_DARK_BED: readonly [number, number, number] = [0.02, 0.02, 0.02];

/** The coast check's sun: behind the camera, high in the west, so that no glint is in view. */
const COAST_SUN = normalise(vec3(1, -0.5, 0.1));

/**
 * How far a waterline pixel's alpha may stand from its twin's: the fragment's sea and the scene's
 * depth part by about 0.05% of the distance on SwiftShader, up to 0.047 of alpha at the 48-pixel
 * waterline (2026-11-18).
 */
const COAST_FADE_TOLERANCE = 0.08;

/** The coast check's shallows, m: water this deep and fully drawn, its foam's and waves' pixels. */
const COAST_SHALLOWS_M: readonly [number, number] = [0.2, 2];

/**
 * A coast: a beach rising to the east at {@link COAST_SLOPE} through the sea `shoreM` east of the
 * foot of a camera `heightM` over it, a plane without end.
 */
interface Coast {
  readonly heightM: number;
  readonly shoreM: number;
}

/**
 * The beach's draw: a full-screen triangle whose every pixel meets the beach's plane along its ray,
 * writing the tint and the hit's reversed-Z depth, exact at any distance, which a mesh reaching
 * behind the camera is not on SwiftShader (its depth was off by a third, 2026-11-18).
 */
const BEACH_WGSL = `${FRAME_WGSL}
struct Draw {
  offsetFromCameraM : vec3f,
  tint : vec4f,
  // xyz: the beach's normal in the view's axes; w: its offset, m, the plane n · x = w.
  plane : vec4f,
}
@group(1) @binding(0) var<uniform> draw : Draw;

struct BeachOut {
  @location(0) colour : vec4f,
  @builtin(frag_depth) depth : f32,
}

@vertex fn vertexMain(@location(0) position : vec3f) -> @builtin(position) vec4f {
  return vec4f(position.xy, 1.0, 1.0);
}

@fragment fn fragmentMain(@builtin(position) position : vec4f) -> BeachOut {
  let ndc = vec2f(
    position.x * frame.viewport.z * 2.0 - 1.0,
    1.0 - position.y * frame.viewport.w * 2.0,
  );
  let ray = vec3f(
    (ndc.x + frame.clipProjection[2][0]) / frame.clipProjection[0][0],
    (ndc.y + frame.clipProjection[2][1]) / frame.clipProjection[1][1],
    -1.0,
  );
  // −z of the hit: the ray's own z is −1.
  let t = draw.plane.w / dot(draw.plane.xyz, ray);
  if (!(t > 0.0)) {
    discard;
  }
  return BeachOut(draw.tint, clamp(frame.clipProjection[3][2] / t, 0.0, 1.0));
}
`;

/** The beach's material ({@link BEACH_WGSL}). */
function beachSpec(name: string): WgslMaterialSpec {
  return flatSpec(name, {
    vertexWgsl: BEACH_WGSL,
    fragmentWgsl: BEACH_WGSL,
    uniforms: [
      { name: "tint", type: "vec4f" },
      { name: "plane", type: "vec4f" },
    ],
  });
}

/** The beach's plane, N · x = −tan β s₀ − h from the camera with N = U − tan β E, body-fixed. */
const BEACH_NORMAL = sub(COAST_UP, scale(COAST_EAST, COAST_SLOPE));

/** The beach's offset in {@link BEACH_NORMAL}'s plane, m. */
function beachOffsetM(coast: Coast): number {
  return -COAST_SLOPE * coast.shoreM - coast.heightM;
}

/** The beach's draw of `coast` seen by `camera`, its plane in the view's axes, tinted `tint`. */
function beachDraw(
  mesh: MeshHandle,
  material: MaterialHandle,
  coast: Coast,
  camera: OceanNearView["camera"],
  tint: readonly [number, number, number, number],
): DrawItem {
  const [x, y, z] = [vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1)].map((axis) =>
    dot(BEACH_NORMAL, rotate(camera.orientation, axis)),
  );
  return {
    ...drawOf(mesh, material, tint),
    uniforms: {
      tint: new Float32Array(tint),
      plane: new Float32Array([x ?? 0, y ?? 0, z ?? 0, beachOffsetM(coast)]),
    },
  };
}

/** The beach's distance along the unit body-fixed ray `d` from the camera, m; `null` off it. */
function beachDistanceM(coast: Coast, d: Vec3): number | null {
  const t = beachOffsetM(coast) / dot(BEACH_NORMAL, d);
  return t > 0 && Number.isFinite(t) ? t : null;
}

/** A coast's pixel with no sea: "sky" where nothing is met, "bed" where the beach hides the sea. */
interface CoastOff {
  readonly layer: "sky" | "bed";
  /** The pixel's colour: the cleared scene's, or the beach's tint. */
  readonly expected: readonly [number, number, number];
}

/** A coast's pixel of sea: the calm sea's light there, its fade and depth. */
interface CoastSea {
  readonly layer: "sea";
  /** The pixel's colour: the fade's share of the sea's light over the beach's tint. */
  readonly expected: readonly [number, number, number];
  /** The sea's own light there, before the fade. */
  readonly seaLight: readonly [number, number, number];
  readonly alpha: number;
  readonly depthM: number;
  /** The calm sea's distance along the pixel's ray, and the ray's view length. */
  readonly seaDistanceM: number;
  readonly viewLength: number;
}

/** What the near twin makes of a coast's pixel. */
type CoastTwin = CoastOff | CoastSea;

/**
 * Each pixel's {@link CoastTwin} for the calm sea of `view` over `coast`'s beach, tinted `tint`, the
 * sea where the pass drew it, by `drawn`, the scene's depth after it: its grid's vertices snapped to
 * SwiftShader's sixteenth of a pixel put the sea about 0.15% nearer than the exact hit at the
 * 48-pixel waterline, 0.05 m of water there, a quarter of the fade's span (2026-11-18), which the
 * fade's twin is not to be held to.
 */
function coastTwins(
  view: OceanNearView,
  coast: Coast,
  tint: readonly [number, number, number, number],
  drawn: Float32Array,
): ReadonlyArray<CoastTwin> {
  const size = { widthPx: NEAR_SIDE_PX, heightPx: NEAR_SIDE_PX };
  const state = oceanNearState(view);
  const rmsHeightM = setRmsHeightM(state.set);
  const e = view.exposureScale;
  const lux = view.suns[0]?.illuminanceLx ?? [0, 0, 0];
  const optics = onProbeInputs({
    ...oceanDrawOptics(view),
    glintCapCdM2: view.thresholdPreExposed,
    sunLux: [lux[0] * e, lux[1] * e, lux[2] * e],
    skyLux: [0, 0, 0],
  });
  const bed = [tint[0], tint[1], tint[2]] as const;
  const twins: CoastTwin[] = [];
  for (let y = 0; y < NEAR_SIDE_PX; y += 1) {
    for (let x = 0; x < NEAR_SIDE_PX; x += 1) {
      const ray = oceanPassRay(x, y, size, projection(1), view.camera.orientation);
      const bedM = beachDistanceM(coast, ray.direction);
      const hit = oceanNearHit(state.frame, ray.direction);
      const world = add(view.camera.positionM, hit.pointM);
      const texel = oceanTexelAt(view.sea.map.texels, [world.x, world.y, world.z]);
      if (hit.clamped || texel === null) {
        twins.push(
          bedM === null ? { layer: "sky", expected: [0, 0, 0] } : { layer: "bed", expected: bed },
        );
        continue;
      }
      const toCamera = scale(hit.pointM, -1 / hit.tM);
      const up = spheroidNormalAt(state.frame, world);
      const drawnDepth = drawn[y * NEAR_SIDE_PX + x] ?? 0;
      const seaM = drawnDepth > 0 ? (NEAR_M * ray.viewLength) / drawnDepth : hit.tM;
      const thickness = oceanThickness(seaM, bedM, Math.abs(dot(toCamera, up)), texel.depthM);
      if (thickness.layer === "bed") {
        twins.push({ layer: "bed", expected: bed });
        continue;
      }
      const footprint = (seaM * view.pixelAngleRad) / Math.sqrt(Math.max(dot(toCamera, up), 0.01));
      // The pixel's reach along the beach's slope, which rises to the east.
      const viewCosine = dot(toCamera, up);
      const flat = sub(toCamera, scale(up, viewCosine));
      const along = norm(flat) > 1e-9 ? Math.abs(dot(normalise(flat), COAST_EAST)) : 0;
      const reach = shorePixelReachM(seaM, view.pixelAngleRad, viewCosine, along, 0.01);
      const light = oceanNearLight(optics, optics, {
        view: toCamera,
        up,
        normal: up,
        texel,
        resolved: { upwind: 0, crosswind: 0 },
        footprintM: footprint,
        thickness,
        shoreFoam: shoreFoamShare(thickness.depthM, COAST_SLOPE, rmsHeightM, state.peakWavelengthM),
        bedRadiance: null,
      }).radiance;
      const alpha =
        thickness.from === "depth" ? shoreFade(thickness.depthM, reach, COAST_SLOPE) : 1;
      const mix = (k: 0 | 1 | 2): number => alpha * light[k] + (1 - alpha) * bed[k];
      twins.push({
        layer: "sea",
        expected: [mix(0), mix(1), mix(2)],
        seaLight: light,
        alpha,
        depthM: thickness.depthM,
        seaDistanceM: seaM,
        viewLength: ray.viewLength,
      });
    }
  }
  return twins;
}

/** The scene's depth, copied to its bits and read back, reversed-Z. */
async function sceneDepths(engine: RenderEngine, kit: NearKit): Promise<Float32Array> {
  const sceneDepth = kit.scene.depth;
  if (sceneDepth === null) {
    throw new Error("the near check's scene has no depth");
  }
  kit.depthBits.render(
    frameOf("smoke ocean shore depth", [oceanDepthCopyDraw(kit.triangle, kit.copy, sceneDepth)]),
  );
  return new Float32Array(await engine.readTexture(kit.depthBits.colour));
}

/** How far the sea of `depths`, the scene's, stands along pixel `p`'s ray from its calm twin's, m. */
function heaveM(depths: Float32Array, p: number, twin: CoastSea): number {
  const depth = depths[p] ?? 0;
  return depth > 0 ? Math.abs((NEAR_M * twin.viewLength) / depth - twin.seaDistanceM) : Number.NaN;
}

/** The median of `values`, NaN for none. */
function median(values: ReadonlyArray<number>): number {
  const sorted = values.toSorted((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)] ?? Number.NaN;
}

/**
 * R11.T8.d: the near sea at its shore on the run's adapter. A beach of 1 in 20 rises out of the sea
 * 60 m ahead of a camera 30 m over it, drawn first, its depth copied for the sea to read. A calm sea
 * then fades to the beach's tint as its twin's depth fade does at each pixel's ray, the waterline's
 * pixels between, the beach above the water left as drawn, 2% of the pixels let off at the grid's
 * cells. Under a 12 m/s wind, with the sun behind the camera and a dark bed, the shallows of 0.2 to
 * 2 m are over three times as bright as the same pixels over 30 m of water, the surf's foam, and
 * their waves' heave along the ray under half as much.
 */
export async function checkOceanShore(engine: RenderEngine, checks: Checks): Promise<void> {
  const calmWorld = nearWorld(0);
  const windyWorld = nearWorld(165);
  const calmMap = uploadOceanMap(engine, calmWorld.texels, "smoke ocean shore calm map");
  const windyMap = uploadOceanMap(engine, windyWorld.texels, "smoke ocean shore windy map");
  const kit = nearKit(engine);
  const coast: Coast = { heightM: 30, shoreM: 60 };
  const camera = eastwardCamera(coast.heightM, (20 * Math.PI) / 180);
  const beach = engine.createMaterial(beachSpec("smoke ocean shore beach"));
  const coastView = (world: ReturnType<typeof nearWorld>, map: TextureHandle): OceanNearView => {
    const view = nearView({ ...world, texture: map }, false, camera);
    return {
      ...view,
      sea: { ...view.sea, bedAlbedo: COAST_DARK_BED },
      suns: [{ ...(view.suns[0] ?? passSun()), directionBodyFixed: COAST_SUN }],
    };
  };
  const onBeach = (): void => {
    kit.scene.render(
      frameOf("smoke ocean shore beach", [
        beachDraw(kit.triangle, beach, coast, camera, COAST_SAND_TINT),
      ]),
    );
  };
  try {
    const pixels = NEAR_SIDE_PX * NEAR_SIDE_PX;
    const calm = coastView(calmWorld, calmMap);
    onBeach();
    const calmCoast = await drawNear(engine, kit, calm, calmMap);
    const twins = coastTwins(calm, coast, COAST_SAND_TINT, await sceneDepths(engine, kit));
    const brightest = Math.max(1e-30, ...twins.flatMap(({ expected }) => expected));
    let sea = 0;
    let faded = 0;
    let parted = 0;
    let widest = 0;
    let first: string | null = null;
    for (const [p, twin] of twins.entries()) {
      const gpu = [0, 1, 2].map((k) => calmCoast[4 * p + k] ?? Number.NaN);
      sea += twin.layer === "sea" && twin.alpha > 0 ? 1 : 0;
      let off: boolean;
      if (twin.layer === "sea" && twin.alpha > 0 && twin.alpha < 1) {
        // A waterline's pixel by its alpha, read from the red the beach's tint gives over the sea's.
        faded += 1;
        const sand = COAST_SAND_TINT[0];
        const alpha = (sand - (gpu[0] ?? Number.NaN)) / (sand - twin.seaLight[0]);
        const apart = Math.abs(alpha - twin.alpha);
        widest = Math.max(widest, apart);
        off = !(apart <= COAST_FADE_TOLERANCE);
      } else {
        off = [0, 1, 2].some(
          (k) =>
            !(
              Math.abs((gpu[k] ?? Number.NaN) - (twin.expected[k] ?? 0)) <=
              2e-3 * brightest + 2e-3 * (twin.expected[k] ?? 0)
            ),
        );
      }
      if (off) {
        parted += 1;
        const alpha = twin.layer === "sea" ? `, α ${twin.alpha.toFixed(3)}` : "";
        first ??= `pixel ${p} (${twin.layer}${alpha}): (${gpu.join(", ")}) against (${twin.expected.join(", ")})`;
      }
    }
    checks.check(
      "R11.T8.d the near sea fades to the beach at its waterline as its twin does",
      sea > pixels / 10 && faded > 0 && parted <= PASS_EDGE_SHARE * pixels,
      `${sea} pixels of sea, ${faded} faded, their alpha within ${widest.toFixed(3)} of the twin's; ${parted} parted from it${first === null ? "" : `; first ${first}`}`,
    );
    // The surf: the same view under a 12 m/s wind, over the beach and over open water.
    const windy = coastView(windyWorld, windyMap);
    onBeach();
    const windyCoast = await drawNear(engine, kit, windy, windyMap);
    const coastDepth = await sceneDepths(engine, kit);
    kit.scene.render(frameOf("smoke ocean shore clear", []));
    const windyOpen = await drawNear(engine, kit, windy, windyMap);
    const openDepth = await sceneDepths(engine, kit);
    const [shallowest, deepest] = COAST_SHALLOWS_M;
    const shallows = [...twins.entries()].flatMap(
      ([p, twin]): ReadonlyArray<readonly [number, CoastSea]> =>
        twin.layer === "sea" &&
        twin.alpha === 1 &&
        twin.depthM >= shallowest &&
        twin.depthM <= deepest
          ? [[p, twin]]
          : [],
    );
    const lightOver = (frame: Float32Array): number =>
      median(shallows.map(([p]) => frame[4 * p + 1] ?? Number.NaN));
    const heaveOver = (depths: Float32Array): number =>
      median(shallows.map(([p, twin]) => heaveM(depths, p, twin)));
    const foam = [lightOver(windyCoast), lightOver(windyOpen)] as const;
    const heaves = [heaveOver(coastDepth), heaveOver(openDepth)] as const;
    checks.check(
      "R11.T8.d the surf's foam whitens the shallows, and their waves lower with the depth",
      shallows.length > pixels / 20 && foam[0] > 3 * foam[1] && heaves[0] < 0.5 * heaves[1],
      `${shallows.length} pixels of 0.2 to 2 m: median light ${foam[0].toExponential(3)} over the beach, ${foam[1].toExponential(3)} over 30 m; median heave ${heaves[0].toFixed(3)} m and ${heaves[1].toFixed(3)} m`,
    );
  } finally {
    engine.releaseMaterial(beach);
    kit.release();
    engine.releaseTexture(calmMap);
    engine.releaseTexture(windyMap);
  }
}

/** The coast captures' frame: 640 × 360, 60° high. */
const COAST_CAPTURE_SIZE = { widthPx: 640, heightPx: 360 } as const;

/** The coast captures' sun: 35° high in the south-south-west, behind the camera, its glint out of view. */
const COAST_CAPTURE_SUN = (() => {
  const elevation = (35 * Math.PI) / 180;
  const azimuth = (200 * Math.PI) / 180;
  return normalise(
    add(
      scale(COAST_UP, Math.sin(elevation)),
      add(
        scale(COAST_EAST, Math.cos(elevation) * Math.sin(azimuth)),
        scale(COAST_NORTH, Math.cos(elevation) * Math.cos(azimuth)),
      ),
    ),
  );
})();

/** The sand's albedo in the captures: a quartz beach's, about 0.4 (a lean, 2026-11-18). */
const COAST_CAPTURE_SAND: readonly [number, number, number] = [0.45, 0.4, 0.32];

/** The captures' exposure into the tone curve, the sand about mid-grey. */
const COAST_CAPTURE_EXPOSURE = 4;

/**
 * The pixels in the bottom tenth of a capture's frame `texels` that show the beach's tint `sand`
 * bare, where every one is sea: the grid's edge pulled in by its waves, which its skirt stops.
 */
function bareBottom(texels: Float32Array, sand: ReadonlyArray<number>): number {
  const { widthPx, heightPx } = COAST_CAPTURE_SIZE;
  let bare = 0;
  for (let y = Math.floor(0.9 * heightPx); y < heightPx; y += 1) {
    for (let x = 0; x < widthPx; x += 1) {
      const at = 4 * (y * widthPx + x);
      const isSand = [0, 1, 2].every(
        (k) => Math.abs((texels[at + k] ?? 0) - (sand[k] ?? 0)) <= 1e-3 * (sand[k] ?? 0),
      );
      bare += isSand ? 1 : 0;
    }
  }
  return bare;
}

/** The three coasts the plan names: each camera's height, the waterline ahead and the pitch down. */
const COAST_CAPTURES: ReadonlyArray<{
  readonly name: string;
  readonly coast: Coast;
  readonly pitchRad: number;
}> = [
  { name: "1km", coast: { heightM: 1_000, shoreM: 3_000 }, pitchRad: 0.26 },
  { name: "100m", coast: { heightM: 100, shoreM: 300 }, pitchRad: 0.26 },
  { name: "2m", coast: { heightM: 2, shoreM: 40 }, pitchRad: 0.035 },
];

/**
 * R11.T8.d's by-hand frames: a coast from 1 km, 100 m and 2 m, the plan's, its beach of 1 in 20
 * lit as sand, under a 7 m/s onshore wind, refraction on, through the airless tables (a black sky,
 * which the sea reflects); recorded by eye in the task's record.
 */
export async function captureCoast(engine: RenderEngine, checks: Checks): Promise<CapturedImage[]> {
  const world = nearWorld(152);
  const map = uploadOceanMap(engine, world.texels, "smoke ocean coast map");
  const kit = nearKit(engine, COAST_CAPTURE_SIZE);
  const beach = engine.createMaterial(beachSpec("smoke ocean coast beach"));
  const aspect = COAST_CAPTURE_SIZE.widthPx / COAST_CAPTURE_SIZE.heightPx;
  const focal = 1 / Math.tan(Math.PI / 6);
  const clip = new Float32Array([
    focal / aspect,
    0,
    0,
    0,
    0,
    focal,
    0,
    0,
    0,
    0,
    0,
    -1,
    0,
    0,
    NEAR_M,
    0,
  ]);
  const pixelAngleRad = 2 / (focal * COAST_CAPTURE_SIZE.heightPx);
  const images: CapturedImage[] = [];
  let bare = 0;
  try {
    for (const { name, coast, pitchRad } of COAST_CAPTURES) {
      const camera = eastwardCamera(coast.heightM, pitchRad);
      const base = nearView({ ...world, texture: map }, true, camera, pixelAngleRad);
      const view: OceanNearView = {
        ...base,
        suns: [{ ...(base.suns[0] ?? passSun()), directionBodyFixed: COAST_CAPTURE_SUN }],
      };
      const lux = view.suns[0]?.illuminanceLx ?? [0, 0, 0];
      const lit = Math.max(dot(normalise(BEACH_NORMAL), COAST_CAPTURE_SUN), 0);
      const sand = [0, 1, 2].map(
        (k) => ((COAST_CAPTURE_SAND[k] ?? 0) * (lux[k] ?? 0) * view.exposureScale * lit) / Math.PI,
      );
      kit.scene.render({
        ...frameOf(`smoke ocean coast ${name} beach`, [
          beachDraw(kit.triangle, beach, coast, camera, [
            sand[0] ?? 0,
            sand[1] ?? 0,
            sand[2] ?? 0,
            WALL_ALPHA,
          ]),
        ]),
        projection: clip,
      });
      // Each coast in turn on the one kit, whose targets the next one draws over.
      // oxlint-disable-next-line no-await-in-loop
      const texels = await drawNear(engine, kit, view, map, clip);
      bare += bareBottom(texels, sand);
      images.push(
        hdrCapture(`r11-t8d-coast-${name}`, texels, COAST_CAPTURE_SIZE, COAST_CAPTURE_EXPOSURE, 1),
      );
    }
  } finally {
    engine.releaseMaterial(beach);
    kit.release();
    engine.releaseTexture(map);
  }
  checks.check(
    "R11.T8.d the coast captures, the sea drawn to each frame's bottom edge",
    images.length === COAST_CAPTURES.length && bare === 0,
    `${String(images.length)} frames; ${String(bare)} pixels of bare beach in their bottom tenth`,
  );
  return images;
}
