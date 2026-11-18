/**
 * Plan R11's authoritative rocks on the run's adapter (R11.T4; plan R14, R14.T5.c): the height
 * worker's bake of a finest patch's rocks and its cost, the procedural builder in its worker
 * against the render thread, and placed rocks drawn through the cluster path.
 *
 * @remarks
 * - **The bake** (R11.T4.a): a height worker of the built app holds the Moon-like fixture field of
 *   `hyperion_surface`'s `tests/golden/worker/moon_like.golden` and bakes {@link BAKED_PATCHES}
 *   finest patches about its first pinned patch, each of which must carry its rock layer; the bakes'
 *   median time is reported against the bake's 40 ms budget and the rocks' 4 ms finding line (plan
 *   R11, T3.d), the whole bake's, not the rocks' alone: a figure, never a gate.
 * - **The builder** (R14.T5.c): the procedural builder's worker (`createWorkerRockPageBuilder`) and
 *   the render thread's (`buildRockPages`) make the same bytes of the same rocks, in the built app's
 *   own module worker, and the worker's build time is reported.
 * - **Placed rocks** (R11.T4.b): twelve rocks on a Moon-sized sphere 20 m below the camera, built,
 *   held in the rocks' pool, culled by the cluster cull at the region's tolerance of 0 and drawn by
 *   the wireframe's rock material into a frame with depth: they cover pixels at their depth, black
 *   inside and stroked at their silhouettes, every texel finite.
 */

import moonFixture from "../../../../../../crates/hyperion-surface/tests/golden/worker/moon_like.golden?raw";
import { vec3 } from "../geometry/vec3";
import { rockLayerFrom, rocksAbout } from "../test/rockFixtures";
import { IDENTITY_QUATERNION } from "../view/camera/quaternion";
import { CLUSTER_CULL_KERNEL } from "../view/clusters/cull";
import { createWorkerRockPageBuilder } from "../view/clusters/meshPageProducer";
import type { RenderEngine } from "../view/engine/types";
import { InThreadRockPageBuilder } from "../view/clusters/meshPageProducer";
import { ResidentRocks } from "../view/surface/scatter/residentRocks";
import { ROCK_CLUSTER_MESH, RockView } from "../view/surface/scatter/rockDraw";
import { rockWireMaterialSpec } from "../view/surface/scatter/rockMaterial";
import { RockMeshes } from "../view/surface/scatter/rockMeshes";
import { buildRockPages } from "../view/surface/scatter/rockPages";
import { finestLevel } from "../view/terrain/cube";
import { frustumOf } from "../view/terrain/cull";
import type { PatchKey } from "../view/terrain/patchKey";
import type {
  BakedPatch,
  HeightWorkerReply,
  HeightWorkerRequest,
} from "../view/terrain/workers/messages";
import { type Checks, halfTexels, IDENTITY, NEAR_M, texel } from "./harness";

/** The finest patches the bake's check bakes about the fixture's first pinned patch: 5 × 5. */
const BAKED_PATCHES = 25;

/** How long a worker's answer may take before the check fails, ms. */
const ANSWER_TIMEOUT_MS = 60_000;

/** The frame of the placed rocks: 256 × 144 px, 60° across. */
const FRAME = { widthPx: 256, heightPx: 144 } as const;

/** A reversed-Z infinite perspective of a 60° horizontal field at `aspect`, column-major. */
function projectionOf(aspect: number): Float32Array {
  const fx = 1 / Math.tan(Math.PI / 6);
  return new Float32Array([fx, 0, 0, 0, 0, fx * aspect, 0, 0, 0, 0, 0, -1, 0, 0, NEAR_M, 0]);
}

/** The bytes of a hexadecimal string. */
function bytesOf(hex: string): ArrayBuffer {
  const bytes = new Uint8Array(hex.length / 2);
  for (let n = 0; n < bytes.length; n += 1) {
    bytes[n] = Number.parseInt(hex.slice(2 * n, 2 * n + 2), 16);
  }
  return bytes.buffer;
}

/** The fixture's body, seed, blocks and first pinned patch. */
function moonField(): {
  readonly body: string;
  readonly seed: string;
  readonly blocks: ArrayBuffer[];
  readonly anchor: PatchKey;
} {
  let body = "";
  let seed = "";
  let anchor: PatchKey | null = null;
  const blocks: ArrayBuffer[] = [];
  for (const line of moonFixture.split("\n")) {
    const [head, ...rest] = line.split(" ");
    if (head === "body") {
      body = rest[0] ?? "";
    } else if (head === "seed") {
      seed = rest[0] ?? "";
    } else if (head === "block") {
      blocks.push(bytesOf(rest[2] ?? ""));
    } else if (head === "patch" && anchor === null) {
      const [face, level, i, j] = rest.slice(1, 5).map(Number);
      if (face === 0 || face === 1 || face === 2 || face === 3 || face === 4 || face === 5) {
        anchor = { face, level: level ?? 0, i: i ?? 0, j: j ?? 0 };
      }
    }
  }
  if (anchor === null) {
    throw new Error("the fixture pins no patch");
  }
  return { body, seed, blocks, anchor };
}

/** A worker's answer to `request`, within {@link ANSWER_TIMEOUT_MS}. */
function ask(worker: Worker, request: HeightWorkerRequest): Promise<HeightWorkerReply> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(new Error(`no answer within ${String(ANSWER_TIMEOUT_MS)} ms`));
    }, ANSWER_TIMEOUT_MS);
    const onMessage = (event: MessageEvent<HeightWorkerReply>): void => {
      if ("id" in event.data && event.data.id !== request.id) {
        return;
      }
      clearTimeout(timer);
      worker.removeEventListener("message", onMessage);
      resolve(event.data);
    };
    worker.addEventListener("message", onMessage);
    worker.postMessage(request, []);
  });
}

/** The median of `values`. */
function median(values: ReadonlyArray<number>): number {
  const sorted = values.toSorted((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)] ?? Number.NaN;
}

/**
 * The height worker's bake of finest patches with their rocks, its cost, and the procedural
 * builder's worker against the render thread over their rocks (see the module's remarks).
 */
export async function checkRockBake(checks: Checks): Promise<void> {
  const field = moonField();
  const worker = new Worker(new URL("../view/terrain/workers/height.worker.ts", import.meta.url), {
    type: "module",
  });
  const builder = createWorkerRockPageBuilder();
  try {
    let id = 0;
    for (const [n, block] of field.blocks.entries()) {
      id += 1;
      // In order: each block is taken after the one before it, as the pool posts them.
      // oxlint-disable-next-line no-await-in-loop
      const reply = await ask(
        worker,
        n === 0
          ? { kind: "field-init", id, body: field.body, block, form: "payload", sigma: "skipped" }
          : { kind: "field-chunk", id, body: field.body, block },
      );
      if (reply.kind !== "field-held") {
        throw new Error(`block ${String(n)} was refused: ${reply.kind}`);
      }
    }
    const finest = finestLevel(1_737_400);
    const down = finest - field.anchor.level;
    const bakes: BakedPatch[] = [];
    const times: number[] = [];
    for (let k = 0; k < BAKED_PATCHES; k += 1) {
      id += 1;
      const key: PatchKey = {
        face: field.anchor.face,
        level: finest,
        i: field.anchor.i * 2 ** down + Math.floor(2 ** down / 2) + (k % 5) - 2,
        j: field.anchor.j * 2 ** down + Math.floor(2 ** down / 2) + Math.floor(k / 5) - 2,
      };
      const start = performance.now();
      // One bake at a time, so that each one's time is its own.
      // oxlint-disable-next-line no-await-in-loop
      const reply = await ask(worker, {
        kind: "bake",
        id,
        key,
        generation: 1,
        settings: {
          vertexPath: "face-differences",
          normals: "mesh",
          source: {
            kind: "body",
            body: field.body,
            detailSeed: field.seed,
            weights: "baked",
          },
        },
      });
      times.push(performance.now() - start);
      if (reply.kind === "baked") {
        bakes.push(reply.bake);
      }
    }
    const withRocks = bakes.filter((bake) => bake.layers?.rocks !== undefined);
    const rocks = withRocks.reduce((sum, bake) => sum + (bake.layers?.rocks?.count ?? 0), 0);
    checks.check(
      "R11.T4.a the height worker bakes each finest patch with its rock layer",
      bakes.length === BAKED_PATCHES && withRocks.length === BAKED_PATCHES,
      `${String(withRocks.length)} of ${String(bakes.length)} bakes of ${String(BAKED_PATCHES)} carry rocks, ${String(rocks)} rocks in all`,
    );
    checks.check(
      "R11.T4.a a finest patch's bake with its rocks, its median time (a figure: the bake's budget 40 ms, the rocks' finding line 4 ms of it)",
      Number.isFinite(median(times)),
      `median ${median(times).toFixed(1)} ms, most ${Math.max(...times).toFixed(1)} ms over ${String(times.length)} bakes, the run's adapter's machine`,
    );
    // The procedural builder in its worker against the render thread, over the bakes' rocks.
    let same = 0;
    let built = 0;
    const buildTimes: number[] = [];
    for (const bake of withRocks) {
      const layer = bake.layers?.rocks;
      if (layer === undefined || layer.count === 0) {
        continue;
      }
      const input = { key: bake.key, originM: bake.originM, layer };
      const start = performance.now();
      // One build at a time, so that each one's time is its own.
      // oxlint-disable-next-line no-await-in-loop
      const inWorker = await builder.build(input);
      buildTimes.push(performance.now() - start);
      const here = buildRockPages(input);
      built += 1;
      if (
        typeof inWorker === "object" &&
        inWorker !== null &&
        here !== null &&
        equalBytes(inWorker.file, here.file) &&
        equalBytes(inWorker.records, here.records)
      ) {
        same += 1;
      }
    }
    checks.check(
      "R14.T5.c the procedural builder's pages are the same bytes in its worker and on the render thread",
      built > 0 && same === built,
      `${String(same)} of ${String(built)} patches' meshes alike; the worker's build median ${median(buildTimes).toFixed(1)} ms`,
    );
  } finally {
    builder.dispose();
    worker.terminate();
  }
}

/** Whether `a` and `b` hold the same bytes. */
function equalBytes(a: ArrayBuffer, b: ArrayBuffer): boolean {
  if (a.byteLength !== b.byteLength) {
    return false;
  }
  const [x, y] = [new Uint8Array(a), new Uint8Array(b)];
  return x.every((byte, n) => byte === y[n]);
}

/** Placed rocks drawn through the cluster path in the wireframe (see the module's remarks). */
export async function checkPlacedRocks(engine: RenderEngine, checks: Checks): Promise<void> {
  const target = engine.createRenderTarget({
    name: "R11 placed rocks",
    size: FRAME,
    format: "rgba16float",
    mips: 1,
    depth: true,
    category: "render-targets",
  });
  const material = engine.createMaterial(rockWireMaterialSpec());
  const kernel = engine.createCompute(CLUSTER_CULL_KERNEL);
  const mesh = engine.createMesh(ROCK_CLUSTER_MESH.mesh);
  const meshes = new RockMeshes(engine, new InThreadRockPageBuilder(), 32 * 1024 * 1024);
  const view = new RockView(engine, "R11 placed rocks");
  try {
    // A patch of rocks on the top of a Moon-sized sphere, the camera 20 m above it, looking down.
    const radiusM = 1_737_400;
    const centre = vec3(0, 0, radiusM);
    const rocks = rocksAbout(11, 12, vec3(0, 0, 1), 24, radiusM);
    const resident = new ResidentRocks();
    const key: PatchKey = { face: 4, level: 17, i: 1, j: 1 };
    const bake: BakedPatch = {
      key,
      generation: 1,
      originM: centre,
      heights: new Float32Array(),
      offsets: null,
      normals: new Float16Array(),
      heightRangeM: [0, 0],
      boundingRadiusM: 20,
      originHeightM: 0,
      skirtDepthM: 0,
      layers: {
        palette: new Uint8Array(16),
        weights: new Uint8Array(),
        surveyMask: new Uint8Array(),
        fieldLevel: 6,
        boundM: 0,
        sigmaM: 0,
        complete: true,
        unsurveyedCells: new Uint32Array(),
        horizonNear: new Uint32Array(),
        horizonFar: new Uint32Array(),
        horizonComplete: true,
        horizonRefusedCells: new Uint32Array(),
        photometry: new Float32Array(40),
        rocks: rockLayerFrom(rocks),
      },
    };
    resident.take("4/17/1/1", bake);
    meshes.sync(resident);
    await Promise.resolve();
    await Promise.resolve();
    const frustum = frustumOf(
      { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3, viewport: FRAME },
      0.01,
    );
    const contours = new Float32Array(4).fill(0);
    const draws = view.frame({
      patches: [{ keyString: "4/17/1/1", terrainSlot: 0, rule: "region" }],
      meshes,
      cameraBodyFixedM: vec3(0, 0, radiusM + 20),
      rotationRows: [vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1)],
      planes: frustum.planes,
      horizon: null,
      toleranceRad: 1e-3,
      viewAxis: vec3(0, 0, -1),
      kernel,
      mesh,
      base: {
        material,
        offsetFromCameraM: new Float32Array(3),
        uniforms: {
          bodyRotation: IDENTITY,
          atlas: contours,
          contour: contours,
          contourWidthsPx: Float32Array.of(1, 2, 2, 0),
          graticule: contours,
          muted: Float32Array.of(0.5, 0.5, 0.5, 1),
          text: Float32Array.of(1, 1, 1, 1),
          casing: Float32Array.of(0, 0, 0, 1),
          occluderSlopePx: Float32Array.of(0),
        },
        textures: {},
      },
    });
    target.render({
      label: "R11 placed rocks",
      viewRotation: IDENTITY,
      projection: projectionOf(FRAME.widthPx / FRAME.heightPx),
      draws,
      postProcesses: [],
    });
    if (target.depth === null) {
      throw new Error(`${target.name} was made with depth and has none`);
    }
    const colour = halfTexels(await engine.readTexture(target.colour));
    const depth = new Float32Array(await engine.readTexture(target.depth));
    let covered = 0;
    let stroked = 0;
    let finite = true;
    for (let y = 0; y < FRAME.heightPx; y += 1) {
      for (let x = 0; x < FRAME.widthPx; x += 1) {
        const d = depth[y * FRAME.widthPx + x] ?? 0;
        const [r, g, b] = texel(colour, FRAME.widthPx, x, y);
        finite &&= Number.isFinite(r) && Number.isFinite(g) && Number.isFinite(b);
        if (d > 0) {
          covered += 1;
          stroked += r > 0.5 ? 1 : 0;
        }
      }
    }
    const nearest = Math.max(...depth);
    checks.check(
      "R11.T4.b placed rocks draw through the cluster path: covered at their depth, black inside, stroked at their silhouettes",
      draws.length === 2 &&
        view.counts.rocks === 12 &&
        covered > 50 &&
        stroked > 0 &&
        stroked < covered &&
        finite &&
        nearest < NEAR_M / 2,
      `${String(view.counts.rocks)} rocks in ${String(view.counts.instances)} instances, ${String(draws.length)} draws; ${String(covered)} px covered, ${String(stroked)} stroked; the nearest depth ${nearest.toExponential(3)}`,
    );
  } finally {
    view.dispose();
    meshes.dispose();
    engine.releaseMesh(mesh);
    engine.releaseMaterial(material);
    engine.releaseCompute(kernel);
    target.dispose();
  }
}
