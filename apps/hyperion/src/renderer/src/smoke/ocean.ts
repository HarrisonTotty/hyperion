/**
 * Plan R11, R11.T8.b's check on the run's adapter: the far ocean's WGSL (`ocean/oceanFar.wgsl`)
 * against its CPU twin (`ocean/oceanFar.ts`), through its probe kernel, in smoke part 27.
 *
 * @remarks
 * A level-5 water world under a 7 m/s wind, with a dry cell, a cell under ice, a closed basin 250 m
 * above the sea and an unsurveyed cell, is uploaded as an ocean map (`rgba16float`, six layers). The
 * probe meets rays with it: at the glint's specular point of three geometries (the sun behind the
 * camera, oblique, near the terminator) and about each, straight down onto each special cell, past
 * the limb, and over an oblate figure. Each ray's hit and radiance are held to the twin's, run on the
 * probe's own `f32` inputs: the same rays met or missed, the distance to 10⁻⁵, the radiance to 10⁻³
 * of the case's brightest.
 */
import { add, cross, normalise, scale, sub, type Vec3, vec3 } from "../geometry/vec3";
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
import { OCEAN_MAP_FORMAT, type OceanTexels, packOceanTexels } from "../view/ocean/oceanMap";
import { oceanMaskOf } from "../view/ocean/oceanMask";
import { EARTH_SEA } from "../view/ocean/seaState";
import { fieldCellIndex } from "../view/terrain/coverage";
import { OCEAN_MAP_WIND_BYTES } from "../view/terrain/workers/messages";
import type { Checks } from "./harness";

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
  return packOceanTexels(answer, oceanMaskOf(answer), { from: 0, to: 0, weight: 0 });
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
