import { describe, expect, it } from "vitest";

import { dot, normalise, vec3 } from "../../../geometry/vec3";
import { meshPagesOf, rockLayerFrom, rocksAbout } from "../../../test/rockFixtures";
import {
  createWorkerRockPageBuilder,
  InThreadRockPageBuilder,
  type MeshPageWorker,
} from "../../clusters/meshPageProducer";
import {
  MeshFileReader,
  type MeshPageReply,
  type MeshPageRequest,
} from "../../clusters/meshPageWork";
import {
  CLUSTER_TABLE_BYTES,
  pageClusterCorners,
  PAGE_RECORD_BYTES,
  PAGE_VERTEX_BYTES,
} from "../../clusters/pageFormat";
import { finestLevel, uvToSt, vertexDir, xyzToFaceUv } from "../../terrain/cube";
import type { PatchKey } from "../../terrain/patchKey";
import { type Rock, rockAt } from "./rockLayer";
import { buildRockPages, placeInPatch, ROCK_RECORD_WORDS, type RockPagesInput } from "./rockPages";

const RADIUS_M = 1_737_400;

/** The finest patch holding `dir` on a Moon-sized sphere. */
function finestAt(dir: readonly [number, number, number]): PatchKey {
  const level = finestLevel(RADIUS_M);
  const { face, u, v } = xyzToFaceUv(dir);
  const side = 2 ** level;
  return { face, level, i: Math.floor(uvToSt(u) * side), j: Math.floor(uvToSt(v) * side) };
}

/** A patch of rocks: 40 of them over its middle, its origin the sphere's point at its centre. */
function patchInput(seed: number): RockPagesInput {
  const key = finestAt([0.3, 1, 0.2]);
  const [x, y, z] = vertexDir(key, 32, 32);
  const centre = normalise(vec3(x, y, z));
  const originM = vec3(centre.x * RADIUS_M, centre.y * RADIUS_M, centre.z * RADIUS_M);
  return { key, originM, layer: rockLayerFrom(rocksAbout(seed, 40, centre, 16)) };
}

/** A stand-in for the builder's worker: it answers a structured clone of each request, as a worker. */
class CloningWorker implements MeshPageWorker {
  readonly #events = new EventTarget();
  readonly #reader = new MeshFileReader(() => Promise.reject(new Error("no file is fetched")));

  addEventListener(type: "message", listener: (event: MessageEvent<MeshPageReply>) => void): void;
  addEventListener(type: "error" | "messageerror", listener: (event: Event) => void): void;
  addEventListener(
    type: "message" | "error" | "messageerror",
    listener: ((event: MessageEvent<MeshPageReply>) => void) | ((event: Event) => void),
  ): void {
    this.#events.addEventListener(type, { handleEvent: listener });
  }

  postMessage(message: MeshPageRequest, _transfer: Transferable[]): void {
    void this.#reply(structuredClone(message));
  }

  /** Answers `message` as the worker would, its reply cloned as a post clones it. */
  async #reply(message: MeshPageRequest): Promise<void> {
    const reply = await this.#reader.answer(message);
    if (reply !== null) {
      this.#events.dispatchEvent(new MessageEvent("message", { data: structuredClone(reply) }));
    }
  }

  terminate(): void {}
}

/**
 * The distance of the rock's frame point (`x`, `y`, `z`) from its surface, m, to first order:
 * |F − 1| ÷ |∇F|, F = |x ÷ a|ⁿ + |y ÷ b|ⁿ + |z ÷ c|ⁿ.
 */
function surfaceDistanceM(rock: Rock, x: number, y: number, z: number): number {
  const [a, b, c] = rock.semiAxesM;
  const n = rock.exponent;
  const terms = [x / a, y / b, z / c].map((w) => Math.abs(w));
  const f = terms.reduce((sum, w) => sum + w ** n, 0);
  const g = terms.map((w, k) => (n * w ** (n - 1)) / ([a, b, c][k] ?? 1));
  return Math.abs(f - 1) / Math.hypot(...g);
}

describe("the procedural builder's rocks (R14.T5.c)", () => {
  it("builds the same bytes for the same rocks in a worker and on the render thread", async () => {
    const input = patchInput(7);
    const inWorker = await createWorkerRockPageBuilder(() => new CloningWorker()).build(input);
    const inThread = await new InThreadRockPageBuilder().build(input);
    if (
      typeof inWorker === "string" ||
      inWorker === null ||
      typeof inThread !== "object" ||
      inThread === null
    ) {
      throw new Error("both builds make a mesh");
    }
    expect(new Uint8Array(inWorker.file)).toEqual(new Uint8Array(inThread.file));
    expect(new Uint8Array(inWorker.records)).toEqual(new Uint8Array(inThread.records));
    expect(inWorker.rocks).toBe(40);
  });

  it("puts every vertex on its rock's surface to the grid's half step, its rock named in its fourth word", () => {
    const input = patchInput(11);
    const built = buildRockPages(input);
    if (built === null) {
      throw new Error("forty rocks make a mesh");
    }
    let vertices = 0;
    for (const page of meshPagesOf(built.file)) {
      const view = new DataView(page.body);
      // Half a step an axis, to first order in the step.
      const slack = (Math.sqrt(3) / 2) * page.stepM * 1.01;
      for (let c = 0; c < page.clusters; c += 1) {
        const corners = pageClusterCorners(page, c);
        const first = view.getUint32(page.sections.table + CLUSTER_TABLE_BYTES * c, true);
        const n = view.getUint32(page.sections.vertices + PAGE_VERTEX_BYTES * first + 12, true);
        const rock = rockAt(input.layer, n);
        const base = vec3(
          rock.baseCentreM.x - input.originM.x,
          rock.baseCentreM.y - input.originM.y,
          rock.baseCentreM.z - input.originM.z,
        );
        for (let k = 0; k < corners.length; k += 3) {
          const p = vec3(
            page.originM.x + (corners[k] ?? 0) * page.stepM - base.x,
            page.originM.y + (corners[k + 1] ?? 0) * page.stepM - base.y,
            page.originM.z + (corners[k + 2] ?? 0) * page.stepM - base.z,
          );
          const [a, b, up] = rock.axes;
          expect(
            Math.abs(surfaceDistanceM(rock, dot(p, a), dot(p, b), dot(p, up))),
          ).toBeLessThanOrEqual(slack);
          vertices += 1;
        }
      }
    }
    expect(vertices).toBe(3 * built.triangles);
  });

  it("keeps each rock's clusters together, by its sphere and its errors (0, its largest diameter)", () => {
    const input = patchInput(13);
    const built = buildRockPages(input);
    if (built === null) {
      throw new Error("forty rocks make a mesh");
    }
    const seen = new Map<number, string>();
    for (const page of meshPagesOf(built.file)) {
      const view = new DataView(page.body);
      for (let c = 0; c < page.clusters; c += 1) {
        const r = page.sections.records + PAGE_RECORD_BYTES * c;
        const first = view.getUint32(page.sections.table + CLUSTER_TABLE_BYTES * c, true);
        const n = view.getUint32(page.sections.vertices + PAGE_VERTEX_BYTES * first + 12, true);
        const record = [0, 4, 8, 12, 16, 20].map((at) => view.getFloat32(r + at, true)).join(",");
        expect(seen.get(n) ?? record).toBe(record);
        seen.set(n, record);
        const [a, b, cAxis] = rockAt(input.layer, n).semiAxesM;
        expect(view.getFloat32(r + 16, true)).toBe(0);
        expect(view.getFloat32(r + 20, true)).toBe(Math.fround(2 * Math.max(a, b, cAxis)));
        expect(view.getUint32(r + 24, true) >>> 24).toBeLessThanOrEqual(128);
      }
    }
    expect(seen.size).toBe(40);
  });

  it("records each rock's base, ground, datum normal and place in the patch", () => {
    const input = patchInput(17);
    const built = buildRockPages(input);
    if (built === null) {
      throw new Error("forty rocks make a mesh");
    }
    const view = new DataView(built.records);
    for (let n = 0; n < built.rocks; n += 1) {
      const rock = rockAt(input.layer, n);
      const at = 4 * ROCK_RECORD_WORDS * n;
      const f = (k: number): number => view.getFloat32(at + 4 * k, true);
      expect([f(0), f(1), f(2)]).toEqual(
        [
          rock.baseCentreM.x - input.originM.x,
          rock.baseCentreM.y - input.originM.y,
          rock.baseCentreM.z - input.originM.z,
        ].map(Math.fround),
      );
      expect(f(3)).toBe(Math.fround(rock.groundHeightM));
      expect([f(4), f(5), f(6)]).toEqual(
        [rock.axes[2].x, rock.axes[2].y, rock.axes[2].z].map(Math.fround),
      );
      const place = view.getUint32(at + 28, true);
      const [px, py] = placeInPatch(input.key, rock.dir);
      expect(Math.abs((place & 0xffff) / 65_535 - px)).toBeLessThanOrEqual(1 / 65_535);
      expect(Math.abs((place >>> 16) / 65_535 - py)).toBeLessThanOrEqual(1 / 65_535);
      expect(px >= 0 && px <= 1 && py >= 0 && py <= 1).toBe(true);
    }
  });

  it("builds nothing for a layer of no rocks", () => {
    const input = patchInput(19);
    expect(buildRockPages({ ...input, layer: rockLayerFrom([]) })).toBeNull();
  });
});
