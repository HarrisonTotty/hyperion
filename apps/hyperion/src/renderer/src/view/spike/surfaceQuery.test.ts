import { beforeAll, describe, expect, it } from "vitest";

import {
  bakePatch,
  initSync,
  levelTable,
  omittedSigmaM,
  NormalScale,
  surfaceHeightM,
  Ridges,
  VertexPath,
} from "../../generated/surface/hyperion_surface";
import wasmDataUrl from "../../generated/surface/hyperion_surface_bg.wasm?inline";
import { vertexDir } from "../terrain/cube";
import type { PatchKey } from "../terrain/patchKey";
import {
  answerSurfaceQuery,
  SurfaceQuery,
  type SurfaceQueryModule,
  type SurfaceQueryReply,
  type SurfaceQueryRequest,
  type SurfaceQueryWorker,
} from "./surfaceQuery";

function wasmBytes(): Uint8Array {
  const comma = wasmDataUrl.indexOf(",");
  return Uint8Array.from(atob(wasmDataUrl.slice(comma + 1)), (c) => c.charCodeAt(0));
}

const module: SurfaceQueryModule = { bakePatch, levelTable, omittedSigmaM, surfaceHeightM };

/** The test planet's finest level on its WGS 84 figure. */
const FINEST = 19;

/** Every vertex height of `key`'s bake (own level), metres. */
function vertexHeights(key: PatchKey): number[] {
  const bake = bakePatch(
    key.face,
    key.level,
    key.i,
    key.j,
    VertexPath.FaceDifferences,
    NormalScale.Mesh,
    Ridges.Off,
    0,
  );
  try {
    return Array.from(bake.heights()).filter((_, n) => n % 2 === 0);
  } finally {
    bake.free();
  }
}

beforeAll(() => {
  initSync({ module: wasmBytes() });
});

describe("answerSurfaceQuery", () => {
  it("hands out the module's level table", () => {
    const reply = answerSurfaceQuery(module, { kind: "level-table", id: 3, ridges: "off" });
    expect(reply).toEqual({ kind: "level-table", id: 3, table: levelTable(Ridges.Off) });
  });

  it("answers a vertex's direction with that vertex's height, the collision interpolant", () => {
    const key = { face: 0, level: FINEST, i: 200_000, j: 300_000 } as const;
    const dir = vertexDir(key, 10, 50);
    const reply = answerSurfaceQuery(module, {
      kind: "height",
      id: 1,
      ridges: "off",
      dirs: Float64Array.from(dir),
    });
    if (reply.kind !== "height") {
      throw new Error(`expected a height, got ${reply.kind}`);
    }
    // The bake narrows its heights to f32; the interpolant answers the same vertex in f64.
    expect(Math.fround(reply.heightsM[0] ?? Number.NaN)).toBe(vertexHeights(key)[50 * 65 + 10]);
  });

  it("hands out σ_n of every level, as the module has it", () => {
    const reply = answerSurfaceQuery(module, { kind: "omitted-sigma", id: 8, ridges: "on" });
    expect(reply).toEqual({
      kind: "omitted-sigma",
      id: 8,
      sigmaM: Float64Array.from({ length: 25 }, (_, level) => omittedSigmaM(level, Ridges.On)),
    });
  });

  it("answers each direction it is given", () => {
    const reply = answerSurfaceQuery(
      { ...module, surfaceHeightM: (x, y, z) => x + 10 * y + 100 * z },
      { kind: "height", id: 2, ridges: "off", dirs: Float64Array.of(1, 2, 3, 3, 2, 1) },
    );
    expect(reply).toEqual({ kind: "height", id: 2, heightsM: Float64Array.of(321, 123) });
  });

  it("bounds the surface over each group by its highest vertex plus its level's bound", () => {
    const a = { face: 2, level: 10, i: 300, j: 400 } as const;
    const b = { face: 2, level: 10, i: 301, j: 400 } as const;
    const c = { face: 2, level: 12, i: 1_200, j: 1_600 } as const;
    const reply = answerSurfaceQuery(module, {
      kind: "max-heights",
      id: 4,
      ridges: "off",
      groups: [[a, b], [c]],
    });
    const table = levelTable(Ridges.Off);
    const highest = (key: PatchKey): number => Math.max(...vertexHeights(key));
    expect(reply).toEqual({
      kind: "max-heights",
      id: 4,
      maxesM: Float64Array.of(
        Math.max(highest(a), highest(b)) + (table[40] ?? Number.NaN),
        highest(c) + (table[48] ?? Number.NaN),
      ),
      baked: 3,
    });
  });

  it("bakes a patch the groups share once", () => {
    let bakes = 0;
    const counting: SurfaceQueryModule = {
      ...module,
      bakePatch: (...args) => {
        bakes += 1;
        return module.bakePatch(...args);
      },
    };
    const shared = { face: 1, level: 8, i: 10, j: 20 } as const;
    const other = { face: 1, level: 8, i: 11, j: 20 } as const;
    const reply = answerSurfaceQuery(counting, {
      kind: "max-heights",
      id: 6,
      ridges: "off",
      groups: [[shared, other], [shared], [other, shared]],
    });
    expect(reply.kind).toBe("max-heights");
    expect(bakes).toBe(2);
  });

  it("refuses an empty group", () => {
    const reply = answerSurfaceQuery(module, {
      kind: "max-heights",
      id: 7,
      ridges: "off",
      groups: [[]],
    });
    expect(reply.kind).toBe("failed");
  });

  it("answers a key the module refuses as failed", () => {
    const reply = answerSurfaceQuery(module, {
      kind: "max-heights",
      id: 5,
      ridges: "off",
      groups: [[{ face: 0, level: 2, i: 9, j: 0 }]],
    });
    expect(reply.kind).toBe("failed");
  });
});

/** A worker that answers with the real module, on a microtask, as a message would arrive. */
class InlineWorker implements SurfaceQueryWorker {
  readonly #message: ((event: MessageEvent<SurfaceQueryReply>) => void)[] = [];
  readonly #error: ((event: ErrorEvent) => void)[] = [];
  terminated = false;
  /** Whether it answers nothing, as a worker still loading its module does. */
  silent = false;

  postMessage(message: SurfaceQueryRequest, _transfer: Transferable[]): void {
    if (this.silent) {
      return;
    }
    const reply = answerSurfaceQuery(module, message);
    queueMicrotask(() => {
      for (const cb of this.#message) {
        cb(new MessageEvent("message", { data: reply }));
      }
    });
  }

  addEventListener(type: "message" | "error", cb: never): void {
    if (type === "message") {
      this.#message.push(cb);
    } else {
      this.#error.push(cb);
    }
  }

  fail(message: string): void {
    for (const cb of this.#error) {
      cb(new ErrorEvent("error", { message }));
    }
  }

  terminate(): void {
    this.terminated = true;
  }
}

describe("SurfaceQuery", () => {
  it("answers each question by its own id", async () => {
    const query = new SurfaceQuery(new InlineWorker(), "off");
    const [table, bound] = await Promise.all([
      query.levelTable(),
      query.maxHeightsM([[{ face: 1, level: 8, i: 10, j: 20 }]]),
    ]);
    expect(table).toHaveLength(100);
    expect(bound).toHaveLength(1);
    expect(bound[0]).toBeGreaterThan(-30_000);
  });

  it("fails a question outstanding when the worker fails", async () => {
    const worker = new InlineWorker();
    worker.silent = true;
    const query = new SurfaceQuery(worker, "off");
    const outstanding = query.levelTable();
    worker.fail("trap");
    await expect(outstanding).rejects.toThrow(/trap/);
  });

  it("fails every question after the worker failed", async () => {
    const worker = new InlineWorker();
    const query = new SurfaceQuery(worker, "off");
    worker.fail("trap");
    await expect(query.levelTable()).rejects.toThrow(/trap/);
  });

  it("stops its worker when disposed", () => {
    const worker = new InlineWorker();
    new SurfaceQuery(worker, "off").dispose();
    expect(worker.terminated).toBe(true);
  });
});
