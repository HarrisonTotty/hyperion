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
 */
import { add, cross, dot, normalise, scale, sub, type Vec3, vec3 } from "../geometry/vec3";
import { createAirlessTables } from "../view/atmosphere/surfaceLighting";
import { DISC_ANNULI_HIGH } from "../view/lighting/annuli";
import { SUN_RADIUS_M } from "../view/lighting/hostDisc";
import { hostAnnuli } from "../view/lighting/hostLights";
import { AU_M } from "../view/scenes/kept";
import { aHostDisc } from "../test/litFixtures";
import { lookAlong } from "../view/camera/quaternion";
import { OCEAN_FIXTURE_LEVEL, oceanCellCentre, waterWorldAnswer } from "../test/oceanFixtures";
import { BUFFER_USAGE, TEXTURE_USAGE } from "../view/engine/gpuFlags";
import type { ComputeHandle, RenderEngine, TextureHandle } from "../view/engine/types";
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
  type OceanTexels,
  packOceanTexels,
} from "../view/ocean/oceanMap";
import { type OceanMask, oceanMaskOf } from "../view/ocean/oceanMask";
import { type OceanSea, STANDARD_GRAVITY_MPS2 } from "../view/ocean/oceanBody";
import {
  OCEAN_ECLIPSE_ROWS,
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
import { fieldCellIndex } from "../view/terrain/coverage";
import { OCEAN_MAP_WIND_BYTES } from "../view/terrain/workers/messages";
import {
  type Checks,
  drawOf,
  flatSpec,
  frameOf,
  fullScreenMesh,
  halfTexels,
  projection,
} from "./harness";

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
