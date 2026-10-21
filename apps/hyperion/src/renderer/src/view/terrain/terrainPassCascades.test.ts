/**
 * A lit pass's shadow cascades on the high setting (plan R10, R10.T9.b; Design note 11;
 * decision-r10-t9b-design D2 to D6 and D9): the set made at the first frame that draws them, the
 * placeholder before, the lit draw made again as the bound texture changes, each view's four caster
 * passes of its own records, culled by the cascade's light-space box and the caster reach, the
 * view's snapped centres carried and dropped, the horizon texel from the view's morph bands, the
 * other casters' draws, and no pass at w = 0, over a scripted pool and R01's counting engine.
 */

import { describe, expect, it } from "vitest";

import { instrumentViewId, PRIMARY_VIEW_ID } from "../../displays/view/viewNames";
import { add, cross, dot, norm, normalise, scale, sub, type Vec3 } from "../../geometry/vec3";
import { type CountingRenderEngine, countingRenderEngine } from "../../test/countingRenderEngine";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { lawFor } from "../appearance/phase";
import { lookAlong } from "../camera/quaternion";
import type { ViewId } from "../camera/state";
import { IDENTITY_ROTATION } from "../coords/rotation";
import { MAX_REQUESTED_BUFFER_BYTES } from "../engine/platform";
import { EngineUnavailable } from "../engine/resilientEngine";
import type { DepthLayerPass, DrawItem, MaterialHandle, MeshHandle } from "../engine/types";
import { annulusEdges } from "../lighting/annuli";
import { TERRAIN_SETTINGS } from "../quality/qualitySetting";
import { vertexDir, vertexSpacing } from "./cube";
import { allocationTally } from "./gpu/allocationTally";
import { TERRAIN_SHADOW_PASS_LABEL } from "./gpu/material";
import { normalsAtlasOf } from "./gpu/resources";
import { INSTANCE_RECORD_BYTES } from "./gpu/uniforms";
import { MAX_LEVEL, type PatchKey } from "./patchKey";
import { type PlanetGeometry, planetGeometry, spheroidNormal, surfacePoint } from "./planet";
import type { PatchRequest } from "./select";
import type { CascadeIndex } from "./shadows/cascadeShadow";
import {
  CASCADE_BYTES,
  type CascadeSet,
  discTopDirection,
  fitCascades,
  gridLatticeSpacingM,
} from "./shadows/cascades";
import { SHADOW_PLACEHOLDER_NAME, TerrainShadows } from "./shadows/terrainShadows";
import type { TerrainLighting } from "./terrainMaterial";
import {
  cascadeHorizonTexelM,
  castsInto,
  meanRadiusM,
  morphRangeM,
  TerrainPass,
  type TerrainPassView,
  type TerrainPool,
  type TerrainView,
  type TerrainViewFrame,
  type TerrainViewFrameInput,
} from "./terrainPass";
import {
  type BakedPatch,
  type BakeSettings,
  PALETTE_NONE,
  type TerrainLayers,
} from "./workers/messages";

const PLANET: PlanetGeometry = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));

const PRIMARY = PRIMARY_VIEW_ID;
const INSTRUMENT = instrumentViewId(1);

const RAD = Math.PI / 180;

/** A body's layers, complete and wholly surveyed. */
function layers(): TerrainLayers {
  const palette = new Uint8Array(16).fill(PALETTE_NONE);
  palette.set([1, 0, 2, PALETTE_NONE]);
  const photometry = new Float32Array(40);
  photometry.set([0.1, 0.12, 0.14, 1, 1]);
  return {
    palette,
    weights: new Uint8Array(65 * 65 * 8),
    surveyMask: new Uint8Array(65 * 65).fill(255),
    fieldLevel: 8,
    boundM: 1,
    sigmaM: 0,
    complete: true,
    unsurveyedCells: new Uint32Array(),
    photometry,
    horizonNear: new Uint32Array(65 * 65 * 4),
    horizonFar: new Uint32Array(9 * 9 * 4),
    horizonComplete: true,
    horizonRefusedCells: new Uint32Array(),
  };
}

/** A pool that bakes each demanded patch flat on the datum, at its own place, on the test's call. */
class PlacedLane implements TerrainPool {
  demand: ReadonlyArray<PatchRequest> = [];
  readonly #baked = new Set<(bake: BakedPatch) => void>();

  constructor(readonly settings: BakeSettings) {}

  reprioritise(demand: ReadonlyArray<PatchRequest>): void {
    this.demand = demand;
  }

  onBaked(cb: (bake: BakedPatch) => void): () => void {
    this.#baked.add(cb);
    return () => {
      this.#baked.delete(cb);
    };
  }

  terminate(): void {
    this.#baked.clear();
  }

  /** Answers every demanded patch. */
  bakeDemand(pass: TerrainPass): void {
    for (const request of this.demand) {
      this.#deliver(pass, request.key);
    }
  }

  #deliver(pass: TerrainPass, key: PatchKey): void {
    const n = normalsAtlasOf(pass.layout).samplesPerSide;
    const [x, y, z] = surfacePoint(PLANET.figure, vertexDir(key, 32, 32), 0);
    const bake: BakedPatch = {
      key,
      generation: 1,
      originM: { x, y, z },
      heights: new Float32Array(65 * 65 * 2),
      offsets: this.settings.vertexPath === "baked-offsets" ? new Float32Array(65 * 65 * 6) : null,
      normals: new Float16Array(n * n * 2),
      heightRangeM: [0, 0],
      // Half the patch's diagonal at its largest spacing, and more.
      boundingRadiusM: 64 * vertexSpacing(PLANET.figure.equatorialRadiusM, key.level).maxM,
      originHeightM: 0,
      skirtDepthM: 1,
      layers: layers(),
    };
    for (const listener of this.#baked) {
      listener(bake);
    }
  }
}

/** A body's lit source, its bakes with their weights. */
const BODY = {
  kind: "body",
  body: "0200080020000000.0301",
  detailSeed: "617373656d626c79",
  weights: "baked",
} as const;

/** The place the views stand over: 30° N, 20° E, body-fixed, as a unit direction. */
const OVER: readonly [number, number, number] = [
  Math.cos(30 * RAD) * Math.cos(20 * RAD),
  Math.cos(30 * RAD) * Math.sin(20 * RAD),
  Math.sin(30 * RAD),
];

/** The datum's normal and its local east and north there. */
const FRAME_THERE = (() => {
  const [x, y, z] = spheroidNormal(PLANET.figure, OVER);
  const up: Vec3 = { x, y, z };
  const east = normalise(cross({ x: 0, y: 0, z: 1 }, up));
  return { up, east, north: cross(up, east) };
})();

/** The sun 15° up towards the east, body-fixed. */
const SUN: Vec3 = add(
  scale(FRAME_THERE.east, Math.cos(15 * RAD)),
  scale(FRAME_THERE.up, Math.sin(15 * RAD)),
);

/**
 * A view `heightM` over the place, looking `depressionDeg` below the horizon `headingDeg` east of
 * north; the body does not turn.
 */
function viewOver(heightM: number, headingDeg = 0, depressionDeg = 10): TerrainView {
  const [x, y, z] = surfacePoint(PLANET.figure, OVER, heightM);
  const { up, east, north } = FRAME_THERE;
  const ahead = add(
    scale(north, Math.cos(headingDeg * RAD)),
    scale(east, Math.sin(headingDeg * RAD)),
  );
  const forward = add(
    scale(ahead, Math.cos(depressionDeg * RAD)),
    scale(up, -Math.sin(depressionDeg * RAD)),
  );
  return {
    rotation: IDENTITY_ROTATION,
    cameraM: { x, y, z },
    orientation: lookAlong(forward, up),
    fovXRad: Math.PI / 3,
    viewport: { widthPx: 640, heightPx: 360 },
  };
}

/** One Sun-like star, `SUN`, at 1 au (IAU 2012 B2) with the Sun's radius (IAU 2015 B3). */
const STARLIT: TerrainLighting = (() => {
  const a = PLANET.figure.equatorialRadiusM;
  const annuli = annulusEdges(0.6, 0.2, 2);
  return {
    lights: [
      {
        direction: SUN,
        distance: 1.495978707e11 / a,
        radius: 6.957e8 / a,
        illuminance: [1.3e5, 1.25e5, 1.2e5],
        annuli: [annuli, annuli, annuli],
      },
    ],
    occluders: [],
    secondaries: [],
    elsewhere: lawFor([0.12, 0.12, 0.12], [0.6, 0.6, 0.6], "moon"),
    exposureScale: 1e-4,
  };
})();

/** The star's angular radius from the body's centre, rad: asin(min(1, radius ÷ distance)), R07's. */
const STAR_RHO_RAD = (() => {
  const light = STARLIT.lights[0];
  if (light === undefined) {
    throw new Error("the cases' lighting has a star");
  }
  return Math.asin(Math.min(1, light.radius / light.distance));
})();

/** A view's frame inputs under the star, its body taking the cascades or not. */
function frameOf(view: TerrainView, takesCascades: boolean, nowMs = 0): TerrainViewFrameInput {
  return {
    view,
    sunDirectionBodyFixed: SUN,
    sunIlluminanceLx: [1.3e5, 1.3e5, 1.3e5],
    exposureScale: 1e-4,
    lighting: STARLIT,
    takesCascades,
    nowMs,
  };
}

/** A view of the pass at the high setting's τ. */
function passView(id: ViewId, view: TerrainView): TerrainPassView {
  return { id, view, tauPx: TERRAIN_SETTINGS.high.tauPx, weight: 1 };
}

interface Made {
  readonly pass: TerrainPass;
  readonly engine: CountingRenderEngine;
  readonly shadows: TerrainShadows;
  readonly lane: () => PlacedLane;
}

/** A lit pass on `setting` with a session's cascades of `engine` and the body's `reliefM`, ready. */
async function litPass(
  setting: "high" | "low" = "high",
  given?: CountingRenderEngine,
  reliefM: () => number | null = () => 8_000,
): Promise<Made> {
  const engine =
    given ??
    (await countingRenderEngine({
      maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES,
      maxBufferSize: MAX_REQUESTED_BUFFER_BYTES,
    }));
  const lanes: PlacedLane[] = [];
  const shadows = new TerrainShadows(engine, { cascades: setting === "high" });
  const pass = new TerrainPass({
    engine,
    setting,
    planet: PLANET,
    source: BODY,
    createPool: (bake) => {
      const lane = new PlacedLane(bake);
      lanes.push(lane);
      return lane;
    },
    shadows,
    reliefM,
  });
  await pass.ready();
  return {
    pass,
    engine,
    shadows,
    lane: () => {
      const lane = lanes.at(-1);
      if (lane === undefined) {
        throw new Error("no lane was taken");
      }
      return lane;
    },
  };
}

/** Steps `made` over `views` and bakes its demand, `rounds` times, then steps once more. */
function stream(made: Made, views: ReadonlyArray<TerrainPassView>, rounds = 4): void {
  for (let n = 0; n < rounds; n += 1) {
    made.pass.step({ views, grounded: [] });
    made.lane().bakeDemand(made.pass);
  }
  made.pass.step({ views, grounded: [] });
}

/** A streamed pass with the primary at `view`. */
async function streamedAt(view: TerrainView, setting: "high" | "low" = "high"): Promise<Made> {
  const made = await litPass(setting);
  stream(made, [passView(PRIMARY, view)]);
  return made;
}

/** The primary 50 m over the place, looking north 10° below the horizon. */
const NEAR = viewOver(50);

/**
 * The primary 10⁶ km over the place, looking all but straight down: far enough that cascade 1's
 * texel (some 880 km) is coarser than the coarsest level's spacing (145 km), so w = 0. Nearer, the
 * horizon texel the view's bands give is coarse enough that w > 0 even from 2,000 km at τ = 1 px.
 */
const ORBIT = viewOver(1e9, 0, 89.5);

/** A frame's draw, which it must have. */
function drawOf(frame: TerrainViewFrame): DrawItem {
  if (frame.draw === null) {
    throw new Error("the pass drew nothing");
  }
  return frame.draw;
}

/** A frame's cascades, which it must have. */
function cascadesOf(frame: TerrainViewFrame): NonNullable<TerrainViewFrame["cascades"]> {
  if (frame.cascades === null) {
    throw new Error("the frame has no cascades");
  }
  return frame.cascades;
}

/** The records last written to `buffer`, one hex string a record. */
function recordsIn(engine: CountingRenderEngine, buffer: string): string[] {
  const data = engine.writes.findLast((w) => w.buffer === buffer)?.data ?? new Uint8Array();
  const out: string[] = [];
  for (let at = 0; at + INSTANCE_RECORD_BYTES <= data.byteLength; at += INSTANCE_RECORD_BYTES) {
    out.push(
      Array.from(data.slice(at, at + INSTANCE_RECORD_BYTES), (byte) =>
        byte.toString(16).padStart(2, "0"),
      ).join(""),
    );
  }
  return out;
}

/** Whether `part` is `whole` with some records left out, in order. */
function subsequence(part: ReadonlyArray<string>, whole: ReadonlyArray<string>): boolean {
  let at = 0;
  for (const record of part) {
    at = whole.indexOf(record, at);
    if (at < 0) {
      return false;
    }
    at += 1;
  }
  return true;
}

/** The terrain's caster draw of `pass`, the first of its draws where it has casters. */
function terrainCaster(pass: DepthLayerPass): DrawItem {
  const draw = pass.draws[0];
  if (draw === undefined || !draw.material.name.startsWith("terrain-shadow-")) {
    throw new Error(`pass ${pass.layer} draws no terrain`);
  }
  return draw;
}

/** Whether `texels` is a whole number of texels, to 10⁻⁶ texel. */
function wholeTexels(texels: number): boolean {
  return Math.abs(texels - Math.round(texels)) < 1e-6;
}

/**
 * Whether each of `set`'s cascades is whole texels in x_L and y_L from the body-fixed lattice point
 * nearest the camera: a first frame's snapping, with no centres carried (ruling 7 amended).
 */
function onLattice(set: CascadeSet, cameraM: Vec3): boolean {
  const spacingM = gridLatticeSpacingM(set.cascades[0].radiusM);
  const round = (c: number): number => spacingM * Math.floor(c / spacingM + 0.5);
  const origin = { x: round(cameraM.x), y: round(cameraM.y), z: round(cameraM.z) };
  return set.cascades.every((cascade) => {
    const fromOrigin = sub(cascade.centreBodyFixedM, origin);
    const along = (axis: Vec3): number =>
      (fromOrigin.x * axis.x + fromOrigin.y * axis.y + fromOrigin.z * axis.z) / cascade.texelM;
    return wholeTexels(along(set.basis.x)) && wholeTexels(along(set.basis.y));
  });
}

describe("the set of cascades and its placeholder in a lit pass (R10.T9.b; D2)", () => {
  it("makes the set once, at the first frame its body takes the cascades at w > 0, its 67,108,864 B under shadows", async () => {
    const made = await streamedAt(NEAR);
    const tally = allocationTally(made.engine);
    const view = viewOver(50);
    made.pass.drawFor(PRIMARY, frameOf(view, false));
    const before = [tally.liveBytes("shadows"), made.engine.counts.depthLayerTargets];
    const frame = made.pass.drawFor(PRIMARY, frameOf(view, true));
    made.pass.drawFor(PRIMARY, frameOf(view, true, 16));
    expect([
      before,
      cascadesOf(frame).set.weight > 0,
      tally.liveBytes("shadows"),
      made.engine.counts.depthLayerTargets,
    ]).toEqual([[0, 0], true, CASCADE_BYTES, 1]);
  });

  it("binds the placeholder before the set and the set after, making its lit draw again only as that changes", async () => {
    const made = await streamedAt(NEAR);
    const view = viewOver(50);
    const first = drawOf(made.pass.drawFor(PRIMARY, frameOf(view, false)));
    const second = drawOf(made.pass.drawFor(PRIMARY, frameOf(view, false)));
    const withSet = drawOf(made.pass.drawFor(PRIMARY, frameOf(view, true)));
    const again = drawOf(made.pass.drawFor(PRIMARY, frameOf(view, true)));
    const without = drawOf(made.pass.drawFor(PRIMARY, frameOf(view, false)));
    expect([
      first.textures["shadowCascades"]?.name,
      second === first,
      withSet === first,
      withSet.textures["shadowCascades"] === made.shadows.target?.depth,
      again === withSet,
      without === withSet,
      // A body that takes no cascades draws at w = 0, whatever its draw binds.
      without.uniforms["cascadeViewAxis"]?.[3],
    ]).toEqual([SHADOW_PLACEHOLDER_NAME, true, false, true, true, true, 0]);
  });

  it("makes its lit draw again with the set remade after a restore, its centres and its records new", async () => {
    const made = await streamedAt(NEAR);
    const view = viewOver(50);
    const before = drawOf(made.pass.drawFor(PRIMARY, frameOf(view, true)));
    const target = made.shadows.target;
    made.engine.loseDevice();
    made.engine.restore();
    await made.pass.ready();
    stream(made, [passView(PRIMARY, view)]);
    const after = drawOf(made.pass.drawFor(PRIMARY, frameOf(view, true)));
    expect([
      after === before,
      made.shadows.target === target,
      after.textures["shadowCascades"] === made.shadows.target?.depth,
      made.engine.counts.depthLayerTargets,
    ]).toEqual([false, false, true, 2]);
  });

  it("makes no set on the low setting and binds the placeholder, whatever its body takes", async () => {
    const made = await streamedAt(NEAR, "low");
    const frame = made.pass.drawFor(PRIMARY, frameOf(viewOver(50), true));
    expect([
      frame.cascades,
      drawOf(frame).textures["shadowCascades"]?.name,
      made.engine.counts.depthLayerTargets,
      made.engine.depthLayerRenders.length,
    ]).toEqual([null, SHADOW_PLACEHOLDER_NAME, 0, 0]);
  });

  it("makes its own cascades where it is given none, and lets them go with itself", async () => {
    const engine = await countingRenderEngine({
      maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES,
      maxBufferSize: MAX_REQUESTED_BUFFER_BYTES,
    });
    const tally = allocationTally(engine);
    const before = engine.live;
    const lanes: PlacedLane[] = [];
    const pass = new TerrainPass({
      engine,
      setting: "high",
      planet: PLANET,
      source: BODY,
      createPool: (bake) => {
        const lane = new PlacedLane(bake);
        lanes.push(lane);
        return lane;
      },
    });
    await pass.ready();
    const views = [passView(PRIMARY, NEAR)];
    for (let n = 0; n < 4; n += 1) {
      pass.step({ views, grounded: [] });
      lanes.at(-1)?.bakeDemand(pass);
    }
    pass.step({ views, grounded: [] });
    pass.drawFor(PRIMARY, frameOf(NEAR, true));
    const living = tally.liveBytes("shadows");
    pass.dispose();
    expect([living, tally.liveBytes("shadows"), engine.live]).toEqual([CASCADE_BYTES, 0, before]);
  });

  it("lets its own cascades and their restore listener go where its device cannot be made, as during a loss", async () => {
    const engine = await countingRenderEngine({
      maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES,
      maxBufferSize: MAX_REQUESTED_BUFFER_BYTES,
    });
    const listening = engine.restoredListenerCount;
    engine.loseDevice();
    expect(
      () =>
        new TerrainPass({
          engine,
          setting: "high",
          planet: PLANET,
          source: BODY,
          createPool: (bake) => new PlacedLane(bake),
        }),
    ).toThrow(EngineUnavailable);
    expect(engine.restoredListenerCount).toBe(listening);
  });
});

describe("each view's caster passes (R10.T9.b; D1 and D4)", () => {
  it("draws four passes a view-frame, each into its own layer with its cascade's matrices, under the view's name", async () => {
    const made = await streamedAt(NEAR);
    const frame = made.pass.drawFor(PRIMARY, frameOf(viewOver(50), true));
    const { set, passes } = cascadesOf(frame);
    const render = made.engine.depthLayerRenders.at(-1);
    expect([
      made.engine.depthLayerRenders.length,
      render?.owner,
      render?.target,
      render?.passes === passes,
      passes.map((each) => [each.label, each.layer]),
      ([0, 1, 2, 3] as const).every(
        (n) =>
          passes[n]?.viewRotation === set.cascades[n].viewRotation &&
          passes[n].projection === set.cascades[n].projection,
      ),
    ]).toEqual([
      1,
      PRIMARY,
      "terrain shadow cascades",
      true,
      [0, 1, 2, 3].map((layer) => [TERRAIN_SHADOW_PASS_LABEL, layer]),
      true,
    ]);
  });

  it("draws the terrain's casters with the view's own contacts, storage, Draw values and lit mesh, through the caster, a direct count", async () => {
    const made = await streamedAt(NEAR);
    const frame = made.pass.drawFor(PRIMARY, frameOf(viewOver(50), true));
    const lit = drawOf(frame);
    const caster = terrainCaster(cascadesOf(frame).passes[0] ?? fail());
    const storage = caster.storageBuffers ?? {};
    const litStorage = lit.storageBuffers ?? {};
    expect({
      material: caster.material.name,
      mesh: caster.mesh === lit.mesh,
      shared: (["heights", "slots", "contacts", "surveyMask", "offsets"] as const).map(
        (name) => storage[name] === litStorage[name],
      ),
      instances: storage["instances"]?.name,
      bound: Object.keys(storage).toSorted(),
      uniforms: (["bodyRotation", "cameraBodyFixedM", "atlas"] as const).map(
        (name) => caster.uniforms[name] === lit.uniforms[name],
      ),
      textures: caster.textures,
      indirect: caster.indirect,
      counted: (caster.instanceCount ?? 0) > 0,
    }).toEqual({
      material: `terrain-shadow-${made.pass.layout.vertexPath}`,
      mesh: true,
      shared: [true, true, true, true, true],
      instances: `terrain casters (${PRIMARY}) 1`,
      bound: (made.pass.layout.vertexPath === "baked-offsets"
        ? ["contacts", "heights", "instances", "offsets", "slots", "surveyMask"]
        : ["contacts", "heights", "instances", "slots", "surveyMask"]
      ).toSorted(),
      uniforms: [true, true, true],
      textures: {},
      indirect: undefined,
      counted: true,
    });
  });

  it("writes each cascade's casters as a byte-identical subset of the view's own records, in order", async () => {
    const made = await streamedAt(NEAR);
    const frame = made.pass.drawFor(PRIMARY, frameOf(viewOver(50), true));
    const own = recordsIn(made.engine, `terrain instances (${PRIMARY})`);
    const cast = cascadesOf(frame).passes.flatMap((each) => {
      const draw = each.draws[0];
      if (draw === undefined || !draw.material.name.startsWith("terrain-shadow-")) {
        return [];
      }
      const records = recordsIn(
        made.engine,
        `terrain casters (${PRIMARY}) ${String(each.layer + 1)}`,
      );
      return [{ records, count: draw.instanceCount }];
    });
    expect(cast.length).toBeGreaterThan(0);
    for (const { records, count } of cast) {
      expect([records.length, subsequence(records, own)]).toEqual([count, true]);
    }
  });

  it("gives two views of one pass each their own four passes, of their own records", async () => {
    const made = await litPass();
    const primary = viewOver(50, 0);
    const instrument = viewOver(80, 90);
    stream(made, [passView(PRIMARY, primary), passView(INSTRUMENT, instrument)]);
    const first = made.pass.drawFor(PRIMARY, frameOf(primary, true));
    const second = made.pass.drawFor(INSTRUMENT, frameOf(instrument, true));
    const renders = made.engine.depthLayerRenders;
    const instancesOf = (frame: TerrainViewFrame): string | undefined =>
      terrainCaster(cascadesOf(frame).passes[0] ?? fail()).storageBuffers?.["instances"]?.name;
    expect([
      renders.map(({ owner, passes }) => [owner, passes.length]),
      instancesOf(first),
      instancesOf(second),
      subsequence(
        recordsIn(made.engine, `terrain casters (${INSTRUMENT}) 1`),
        recordsIn(made.engine, `terrain instances (${INSTRUMENT})`),
      ),
    ]).toEqual([
      [
        [PRIMARY, 4],
        [INSTRUMENT, 4],
      ],
      `terrain casters (${PRIMARY}) 1`,
      `terrain casters (${INSTRUMENT}) 1`,
      true,
    ]);
  });

  it("releases a leaving view's caster buffers with its own", async () => {
    const made = await litPass();
    const primary = viewOver(50, 0);
    const instrument = viewOver(80, 90);
    stream(made, [passView(PRIMARY, primary), passView(INSTRUMENT, instrument)]);
    made.pass.drawFor(INSTRUMENT, frameOf(instrument, true));
    const destroyed: string[] = [];
    made.engine.onAllocation((event) => {
      if (event.kind === "destroyed") {
        destroyed.push(event.name);
      }
    });
    made.pass.step({ views: [passView(PRIMARY, primary)], grounded: [] });
    expect(destroyed.filter((name) => name.startsWith("terrain casters")).toSorted()).toEqual(
      [1, 2, 3, 4].map((n) => `terrain casters (${INSTRUMENT}) ${String(n)}`),
    );
  });

  it("appends the other casters' draws after the terrain's in each pass, until they are removed", async () => {
    const made = await streamedAt(NEAR);
    const asked: Array<readonly [number, string]> = [];
    const other: DrawItem = {
      mesh: { kind: "mesh", name: "craft" } satisfies MeshHandle,
      material: { kind: "material", name: "craft caster" } satisfies MaterialHandle,
      offsetFromCameraM: new Float32Array(3),
      uniforms: {},
      textures: {},
    };
    const remove = made.shadows.casters.add((_set, index, view) => {
      asked.push([index, view]);
      return [other];
    });
    const view = viewOver(50);
    const withOther = cascadesOf(made.pass.drawFor(PRIMARY, frameOf(view, true))).passes;
    remove();
    const without = cascadesOf(made.pass.drawFor(PRIMARY, frameOf(view, true))).passes;
    expect([
      asked,
      withOther.map((each) => each.draws.at(-1) === other),
      withOther.map((each) => each.draws.length - 1),
      without.map((each) => each.draws.length),
    ]).toEqual([
      [0, 1, 2, 3].map((index) => [index, PRIMARY]),
      [true, true, true, true],
      without.map((each) => each.draws.length),
      without.map((each) => each.draws.length),
    ]);
  });

  it("skips every pass at w = 0, far out, and still fits", async () => {
    const made = await streamedAt(ORBIT);
    const frame = made.pass.drawFor(PRIMARY, frameOf(ORBIT, true));
    expect([
      cascadesOf(frame).set.weight,
      cascadesOf(frame).passes,
      made.engine.depthLayerRenders.length,
      made.engine.counts.depthLayerTargets,
      drawOf(frame).uniforms["cascadeViewAxis"]?.[3],
    ]).toEqual([0, [], 0, 0, 0]);
  });

  it("fits nothing for a body that takes no cascades, or under no star", async () => {
    const made = await streamedAt(NEAR);
    const view = viewOver(50);
    const unlit = made.pass.drawFor(PRIMARY, {
      ...frameOf(view, true),
      lighting: { ...STARLIT, lights: [] },
    });
    const other = made.pass.drawFor(PRIMARY, frameOf(view, false));
    expect([unlit.cascades, other.cascades, made.engine.depthLayerRenders.length]).toEqual([
      null,
      null,
      0,
    ]);
  });
});

describe("culling a caster by its cascade's light-space box (R10.T9.b; D4)", () => {
  /** A hand-made set: 2 m over a Moon-sized body, the sun 20° up, its reach for 10 km of relief. */
  const SET: CascadeSet = fitCascades({
    cameraBodyFixedM: { x: 0, y: 0, z: 1_737_402 },
    orientationBodyFixed: lookAlong({ x: 1, y: 0, z: -0.2 }, { x: 0, y: 0, z: 1 }),
    rotation: IDENTITY_ROTATION,
    fovXRad: Math.PI / 3,
    aspect: 16 / 9,
    heightAboveGroundM: 2,
    sunDirectionBodyFixed: { x: Math.cos(20 * RAD), y: 0, z: Math.sin(20 * RAD) },
    datumNormalBodyFixed: { x: 0, y: 0, z: 1 },
    bodyRadiusM: 1_737_400,
    reliefRangeM: 10_000,
    horizonTexelM: 1,
    depthClipControl: false,
  });

  /** The place `v` from cascade `index`'s snapped centre, in the light's axes, less the camera. */
  function at(index: CascadeIndex, v: Vec3): Vec3 {
    const o = SET.cascades[index].centreFromCameraLightM;
    const { x, y, z } = SET.basis;
    return add(add(scale(x, o.x + v.x), scale(y, o.y + v.y)), scale(z, o.z + v.z));
  }

  it.each([0, 1, 2, 3] as const)(
    "keeps a patch inside cascade %i's box, or within the reach towards the sun, and drops one beside, behind or beyond it",
    (index) => {
      const r = SET.cascades[index].radiusM;
      const reach = SET.casterReachM;
      const rho = 5;
      const cases: ReadonlyArray<readonly [Vec3, boolean]> = [
        [{ x: 0, y: 0, z: 0 }, true],
        [{ x: r + rho - 0.5, y: 0, z: 0 }, true],
        [{ x: r + rho + 0.5, y: 0, z: 0 }, false],
        [{ x: 0, y: -(r + rho + 0.5), z: 0 }, false],
        [{ x: 0, y: 0, z: -(r + rho - 0.5) }, true],
        [{ x: 0, y: 0, z: -(r + rho + 0.5) }, false],
        [{ x: 0, y: 0, z: r + reach + rho - 0.5 }, true],
        [{ x: 0, y: 0, z: r + reach + rho + 0.5 }, false],
      ];
      expect(cases.map(([v]) => castsInto(SET, index, at(index, v), rho))).toEqual(
        cases.map(([, kept]) => kept),
      );
    },
  );

  it("takes a reach of thousands of metres towards a low sun, so that a far caster still casts", () => {
    expect(SET.casterReachM).toBeGreaterThan(10_000);
  });
});

/**
 * The camera's height in frame `n` of the centres' cases, m: above 100 m the shadowed range follows
 * h, so the texel changes as the camera climbs.
 */
function heightOf(n: number): number {
  return 150 + n;
}

describe("the view's snapped centres (R10.T9.b; D6(d); ruling 7 amended)", () => {
  it("carries them from frame to frame, and drops them after a frame without the cascades", async () => {
    const made = await streamedAt(viewOver(heightOf(0)));
    const fit = (n: number, takes = true): TerrainViewFrame =>
      made.pass.drawFor(PRIMARY, frameOf(viewOver(heightOf(n)), takes, n));
    const lattice = (frame: TerrainViewFrame, n: number): boolean =>
      onLattice(cascadesOf(frame).set, viewOver(heightOf(n)).cameraM);
    const first = fit(0);
    const carried = fit(1);
    fit(2, false);
    const dropped = fit(3);
    expect([lattice(first, 0), lattice(carried, 1), lattice(dropped, 3)]).toEqual([
      true,
      false,
      true,
    ]);
  });

  it("keeps them through a frame at w = 0, which still fits", async () => {
    const made = await streamedAt(viewOver(heightOf(0)));
    made.pass.drawFor(PRIMARY, frameOf(viewOver(heightOf(0)), true));
    const orbit = made.pass.drawFor(PRIMARY, frameOf(ORBIT, true));
    const back = made.pass.drawFor(PRIMARY, frameOf(viewOver(heightOf(1)), true));
    expect([
      cascadesOf(orbit).set.weight,
      onLattice(cascadesOf(back).set, viewOver(heightOf(1)).cameraM),
    ]).toEqual([0, false]);
  });

  it("drops them for a view new to the pass, and when the pass is remade after a loss", async () => {
    const made = await streamedAt(viewOver(heightOf(0)));
    const primary = passView(PRIMARY, viewOver(heightOf(0)));
    made.pass.drawFor(PRIMARY, frameOf(viewOver(heightOf(0)), true));
    // The view leaves and comes back: a new view to the pass.
    made.pass.step({ views: [], grounded: [] });
    made.pass.step({ views: [primary], grounded: [] });
    const returned = made.pass.drawFor(PRIMARY, frameOf(viewOver(heightOf(1)), true));
    made.pass.drawFor(PRIMARY, frameOf(viewOver(heightOf(2)), true));
    made.engine.loseDevice();
    made.engine.restore();
    await made.pass.ready();
    stream(made, [primary]);
    const rebuilt = made.pass.drawFor(PRIMARY, frameOf(viewOver(heightOf(3)), true));
    expect([
      onLattice(cascadesOf(returned).set, viewOver(heightOf(1)).cameraM),
      onLattice(cascadesOf(rebuilt).set, viewOver(heightOf(3)).cameraM),
    ]).toEqual([true, true]);
  });
});

describe("the fit's inputs (R10.T9.b; D6)", () => {
  /** A view's morph bands, as the pass makes them at τ. */
  function bands(view: TerrainView): Float64Array {
    const morph = new Float64Array(2 * (MAX_LEVEL + 1));
    for (let level = 0; level <= MAX_LEVEL; level += 1) {
      const [start, end] = morphRangeM(PLANET, level, {
        camera: { positionM: view.cameraM, orientation: view.orientation },
        fovXRad: view.fovXRad,
        viewport: view.viewport,
        weight: 1,
        tauPx: TERRAIN_SETTINGS.high.tauPx,
      });
      morph[2 * level] = start;
      morph[2 * level + 1] = end;
    }
    return morph;
  }

  it("takes the horizon texel continuous in h through each band and each split, from the level drawn at h", () => {
    const morph = bands(viewOver(50));
    const finest = PLANET.finestLevel;
    // From inside the finest level's reach out past level 3's band.
    const fromM = 0.5 * (morph[2 * finest + 1] ?? 0);
    const toM = 1.5 * (morph[2 * 3 + 1] ?? 0);
    let worst = 0;
    let previous = cascadeHorizonTexelM(PLANET, morph, fromM);
    for (let n = 1; n <= 20_000; n += 1) {
      const h = fromM * (toM / fromM) ** (n / 20_000);
      const texel = cascadeHorizonTexelM(PLANET, morph, h);
      worst = Math.max(worst, Math.abs(texel / previous - 1));
      previous = texel;
    }
    // Each step multiplies h by under 1.001 here, so the texel changes by under a few parts in a
    // thousand within a band, and never by a split's factor of 2.
    expect(worst).toBeLessThan(1e-2);
    // Each level's spacing doubled to its parent's at the band's end, and its own at its start.
    for (let level = 4; level <= finest; level += 1) {
      const start = morph[2 * level] ?? 0;
      const end = morph[2 * level + 1] ?? 0;
      const spacing = vertexSpacing(PLANET.figure.equatorialRadiusM, level).meanM;
      expect(cascadeHorizonTexelM(PLANET, morph, start) / spacing).toBeCloseTo(1, 12);
      expect(cascadeHorizonTexelM(PLANET, morph, end * (1 - 1e-12)) / spacing).toBeCloseTo(2, 6);
      expect(cascadeHorizonTexelM(PLANET, morph, end) / spacing).toBeCloseTo(2, 12);
    }
  });

  it("takes the level selection's traversal reaches from the root, past a fine level whose band end bumps beyond h", () => {
    const morph = bands(viewOver(50));
    const end = (level: number): number => morph[2 * level + 1] ?? Number.NaN;
    // Inside level 4's band, beyond level 5's end.
    const h = Math.sqrt(end(5) * end(4));
    const bumped = Float64Array.from(morph);
    const finest = PLANET.finestLevel;
    bumped[2 * finest + 1] = 10 * h;
    const spacing = vertexSpacing(PLANET.figure.equatorialRadiusM, 4).meanM;
    const texel = cascadeHorizonTexelM(PLANET, bumped, h);
    expect([finest > 5, texel, texel / spacing >= 1 && texel / spacing <= 2]).toEqual([
      true,
      cascadeHorizonTexelM(PLANET, morph, h),
      true,
    ]);
  });

  it("takes the coarsest level's spacing at any height beyond every band, and 0 for one below the ground", () => {
    const morph = bands(viewOver(50));
    const a = PLANET.figure.equatorialRadiusM;
    expect(cascadeHorizonTexelM(PLANET, morph, 1e9)).toBe(vertexSpacing(a, 0).meanM);
    expect(cascadeHorizonTexelM(PLANET, morph, -5)).toBe(cascadeHorizonTexelM(PLANET, morph, 0));
  });

  it("takes R as the figure's mean radius, (2a + c) ÷ 3", () => {
    const { equatorialRadiusM: a, polarRadiusM: c } = WGS84_FIGURE;
    expect(meanRadiusM(WGS84_FIGURE)).toBeCloseTo((2 * a + c) / 3, 6);
    // IUGG's R₁ for GRS 80, 6,371,008.7714 m (Moritz 1980), which WGS 84 shares to 0.1 mm.
    expect(meanRadiusM(WGS84_FIGURE)).toBeCloseTo(6_371_008.77, 1);
  });

  it("fits to the top edge of star 0's disc, its angular radius from the centre towards the zenith (decision-r10-t9b-penumbra)", async () => {
    const made = await streamedAt(NEAR);
    const { set } = cascadesOf(made.pass.drawFor(PRIMARY, frameOf(viewOver(50), true)));
    const rho = STAR_RHO_RAD;
    expect([
      angleRad(set.basis.z, SUN),
      dot(set.basis.z, discTopDirection(SUN, FRAME_THERE.up, rho)),
      set.sunElevationRad,
    ]).toEqual([expect.closeTo(rho, 9), expect.closeTo(1, 12), expect.closeTo(15 * RAD + rho, 6)]);
    // The Sun's from 1 au: some 0.27°.
    expect(rho).toBeCloseTo(4.65e-3, 5);
  });

  it("fits as T9.a's fitCascades of the pass's own inputs, the device's depth-clip-control and the body's relief", async () => {
    const made = await streamedAt(NEAR);
    const view = viewOver(50);
    const { set } = cascadesOf(made.pass.drawFor(PRIMARY, frameOf(view, true)));
    // The ground is the datum's here: every bake is flat on it.
    const under = 50;
    const expected = fitCascades({
      cameraBodyFixedM: view.cameraM,
      orientationBodyFixed: view.orientation,
      rotation: IDENTITY_ROTATION,
      fovXRad: view.fovXRad,
      aspect: 640 / 360,
      heightAboveGroundM: under,
      sunDirectionBodyFixed: discTopDirection(SUN, FRAME_THERE.up, STAR_RHO_RAD),
      datumNormalBodyFixed: FRAME_THERE.up,
      bodyRadiusM: meanRadiusM(PLANET.figure),
      reliefRangeM: 8_000,
      horizonTexelM: 1,
      depthClipControl: made.engine.capabilities.depthClipControl,
    });
    expect([
      set.rangeM,
      set.sunElevationRad,
      set.casterReachM / expected.casterReachM,
      set.unclippedDepth,
      set.cascades[0].radiusM,
    ]).toEqual([
      expected.rangeM,
      expect.closeTo(expected.sunElevationRad, 6),
      expect.closeTo(1, 6),
      false,
      expect.closeTo(expected.cascades[0].radiusM, 6),
    ]);
  });

  it("takes no cascades, throwing nothing, for a viewport of no height, and drops the view's centres", async () => {
    const made = await streamedAt(viewOver(heightOf(0)));
    const fit = (n: number, heightPx = 360): TerrainViewFrame => {
      const view = viewOver(heightOf(n));
      return made.pass.drawFor(
        PRIMARY,
        frameOf({ ...view, viewport: { widthPx: 640, heightPx } }, true, n),
      );
    };
    fit(0);
    const flat = fit(1, 0);
    const after = fit(2);
    expect([
      flat.cascades,
      flat.draw !== null,
      onLattice(cascadesOf(after).set, viewOver(heightOf(2)).cameraM),
    ]).toEqual([null, true, true]);
  });

  it("takes no cascades, throwing nothing, under a star of no direction", async () => {
    const made = await streamedAt(NEAR);
    const light = STARLIT.lights[0];
    if (light === undefined) {
      throw new Error("the cases' lighting has a star");
    }
    const frame = made.pass.drawFor(PRIMARY, {
      ...frameOf(viewOver(50), true),
      lighting: { ...STARLIT, lights: [{ ...light, direction: { x: 0, y: 0, z: 0 } }] },
    });
    expect([frame.cascades, made.engine.depthLayerRenders.length]).toEqual([null, 0]);
  });

  it("takes a relief that is not finite as 0, a caster reach of 0", async () => {
    const made = await litPass("high", undefined, () => Number.NaN);
    stream(made, [passView(PRIMARY, NEAR)]);
    const { set } = cascadesOf(made.pass.drawFor(PRIMARY, frameOf(viewOver(50), true)));
    expect([set.casterReachM, set.weight > 0]).toEqual([0, true]);
  });
});

/** The angle between two directions, rad, by atan2 so that it keeps its precision when small. */
function angleRad(a: Vec3, b: Vec3): number {
  return Math.atan2(norm(cross(a, b)), dot(a, b));
}

/** Fails a test where a pass it needs is missing. */
function fail(): never {
  throw new Error("a pass is missing");
}
