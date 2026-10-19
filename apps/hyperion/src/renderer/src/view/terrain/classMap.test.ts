import { afterEach, describe, expect, it, vi } from "vitest";

import { countingRenderEngine } from "../../test/countingRenderEngine";
import type { PhotometricLaw } from "../appearance/law";
import { lawFor } from "../appearance/phase";
import { classMapSide, classWeightsAt } from "../bodies/discSurface";
import type { AllocationEvent } from "../engine/memory";
import {
  CLASS_MAP_FACE_TEXELS,
  CLASS_MAP_LEVEL,
  type ClassMapPool,
  ClassMapStore,
  classMapLawsOf,
  classMapTexelsOf,
} from "./classMap";
import { fieldCellIndex } from "./coverage";
import { faceUvToXyz, stToUv } from "./cube";
import type { Face } from "./patchKey";
import { classRowLaw, PHASE_LAW } from "./terrainMaterial";
import { type ClassMapAnswer, PALETTE_NONE } from "./workers/messages";
import type { FieldChange } from "./workers/pool";

const MOON = "0200080020000000.0300";
const SEED = "00000000000000a5";
const ELSEWHERE = lawFor([0.12, 0.12, 0.12], [0.6, 0.6, 0.6], "moon");

/** One texel of a synthetic answer. */
interface TexelSpec {
  readonly face: Face;
  readonly i: number;
  readonly j: number;
  readonly fraction: number;
  /** Bytes over the classes. */
  readonly bytes: ReadonlyArray<number>;
}

/**
 * Three classes: regolith of entry 0 (the Regolith law at L(0) 1), frost of entry 1 (the Ice law at
 * L(0) 0.6) and a form whose palette names no substance.
 */
const CLASSES = [
  [1, 0],
  [4, 1],
  [2, PALETTE_NONE],
] as const;
const PHOTOMETRY = [
  [0.1, 0.11, 0.12, PHASE_LAW.regolith, 1],
  [0.8, 0.85, 0.9, PHASE_LAW.ice, 0.6],
  [0, 0, 0, 0, 0],
] as const;

/** A worker's answer of `texels` over {@link CLASSES}, in rising cell index. */
function answerOf(texels: ReadonlyArray<TexelSpec>, complete = true): ClassMapAnswer {
  const sorted = texels
    .map((t) => ({
      t,
      index: fieldCellIndex({ face: t.face, level: CLASS_MAP_LEVEL, i: t.i, j: t.j }),
    }))
    .toSorted((a, b) => a.index - b.index);
  return {
    level: CLASS_MAP_LEVEL,
    fieldLevel: 7,
    classes: Uint8Array.from(CLASSES.flat()),
    photometry: Float32Array.from(PHOTOMETRY.flat()),
    texels: Uint32Array.from(sorted.map(({ index }) => index)),
    fractions: Float32Array.from(sorted.map(({ t }) => t.fraction)),
    weights: Uint8Array.from(sorted.flatMap(({ t }) => t.bytes)),
    complete,
  };
}

const TEXELS: ReadonlyArray<TexelSpec> = [
  { face: 2, i: 10, j: 11, fraction: 1, bytes: [255, 0, 0] },
  { face: 2, i: 11, j: 11, fraction: 1, bytes: [100, 100, 55] },
  // Three of its four field cells surveyed.
  { face: 2, i: 12, j: 11, fraction: 0.75, bytes: [0, 204, 51] },
  { face: 5, i: 63, j: 0, fraction: 1, bytes: [0, 255, 0] },
];

/** The body-fixed direction of texel (i, j)'s centre on `face`. */
function centreOf(face: Face, i: number, j: number): readonly [number, number, number] {
  const n = CLASS_MAP_FACE_TEXELS;
  return faceUvToXyz(face, stToUv((i + 0.5) / n), stToUv((j + 0.5) / n));
}

/** A pool whose answers the test gives, one query at a time, and whose block changes it raises. */
class ScriptedPool implements ClassMapPool {
  readonly asked: Array<{
    readonly body: string;
    readonly detailSeed: string;
    readonly resolve: (map: ClassMapAnswer | null) => void;
    readonly reject: (error: Error) => void;
  }> = [];
  readonly #listeners = new Set<(change: FieldChange) => void>();

  classMap(body: string, detailSeed: string): Promise<ClassMapAnswer | null> {
    return new Promise((resolve, reject) => {
      this.asked.push({ body, detailSeed, resolve, reject });
    });
  }

  onFieldChange(cb: (change: FieldChange) => void): () => void {
    this.#listeners.add(cb);
    return () => {
      this.#listeners.delete(cb);
    };
  }

  get listening(): number {
    return this.#listeners.size;
  }

  /** Raises a block's change of `body`. */
  change(
    body: string,
    sequence: number,
    surveyed: ReadonlyArray<number>,
    held: ReadonlyArray<number> = [],
  ): void {
    for (const listener of this.#listeners) {
      listener({
        body,
        sequence,
        fieldLevel: 7,
        newlySurveyed: Uint32Array.from(surveyed),
        newlyHeld: Uint32Array.from(held),
        levels: new Float64Array(100),
      });
    }
  }
}

/** Lets the store's answered promises run: a few turns of the microtask queue. */
async function settle(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
}

/** A class's law as the map should take it: its photometry on its law's row law. */
function lawOf(code: 1 | 2 | 3, l0: number, a: readonly [number, number, number]): PhotometricLaw {
  return { ...classRowLaw(code, l0), a };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("a class map from a worker's answer", () => {
  it("is at level 6, 64 texels a face, each texel's weights taken at band level 0", () => {
    expect([CLASS_MAP_LEVEL, CLASS_MAP_FACE_TEXELS, classMapSide(CLASS_MAP_FACE_TEXELS)]).toEqual([
      6, 64, 66,
    ]);
  });

  it("holds no texel for an unsurveyed cell, and each answered texel's weights times its surveyed share", () => {
    const map = classMapTexelsOf(answerOf(TEXELS));
    if (map === null) {
      throw new Error("no map");
    }
    expect([map.faceTexels, map.classes]).toEqual([64, 3]);
    const n = CLASS_MAP_FACE_TEXELS;
    const answered = new Set(TEXELS.map((t) => `${String(t.face)},${String(t.i)},${String(t.j)}`));
    let unsurveyed = 0;
    for (const face of [0, 1, 2, 3, 4, 5] as const) {
      for (let j = 0; j < n; j += 1) {
        for (let i = 0; i < n; i += 1) {
          if (answered.has(`${String(face)},${String(i)},${String(j)}`)) {
            continue;
          }
          const weights = classWeightsAt(map, centreOf(face, i, j));
          if (weights.some((w) => w !== 0)) {
            throw new Error(`texel (${String(face)}, ${String(i)}, ${String(j)}) holds weight`);
          }
          unsurveyed += 1;
        }
      }
    }
    expect(unsurveyed).toBe(6 * 64 * 64 - TEXELS.length);
    const bytesAt = (t: TexelSpec): number[] =>
      classWeightsAt(map, centreOf(t.face, t.i, t.j)).map((w) => Math.round(w * 255));
    expect(TEXELS.map(bytesAt)).toEqual([
      [255, 0, 0],
      [100, 100, 55],
      // 0.75 of 204 and 51: 153 and 38.25, kept to their sum's 191.
      [0, 153, 38],
      [0, 255, 0],
    ]);
  });

  it("is none for a map of no class", () => {
    const none: ClassMapAnswer = {
      ...answerOf([]),
      classes: new Uint8Array(0),
      photometry: new Float32Array(0),
    };
    expect(classMapTexelsOf(none)).toBeNull();
  });

  it.each([
    { name: "of another level", change: { level: 5 }, message: /level 6, not 5/u },
    {
      name: "whose shares are not its texels'",
      change: { fractions: new Float32Array(3) },
      message: /4 texels have 3 shares/u,
    },
  ])("refuses an answer $name", ({ change, message }) => {
    expect(() => classMapTexelsOf({ ...answerOf(TEXELS), ...change })).toThrow(message);
  });

  it("refuses an answer of more classes than a texel packs", () => {
    expect(() => classMapTexelsOf({ ...answerOf([]), classes: new Uint8Array(2 * 17) })).toThrow(
      /at most 16 classes/u,
    );
  });

  it("takes a law a class: its photometry on its law's row law, a distinct object, none for an unnamed substance", () => {
    const laws = classMapLawsOf(answerOf(TEXELS));
    expect(laws).toHaveLength(3);
    const [regolith, frost, unnamed] = laws;
    // A in display order: R, V, B as r, g, b.
    expect(regolith).toEqual(
      lawOf(PHASE_LAW.regolith, 1, [Math.fround(0.12), Math.fround(0.11), Math.fround(0.1)]),
    );
    expect(frost).toEqual(
      lawOf(PHASE_LAW.ice, Math.fround(0.6), [
        Math.fround(0.9),
        Math.fround(0.85),
        Math.fround(0.8),
      ]),
    );
    expect(regolith?.lommelSeeligerCurve).toBe("mcewen-1996");
    expect(unnamed).toBeNull();
    expect(regolith).not.toBe(frost);
    expect(() => classMapLawsOf({ ...answerOf(TEXELS), photometry: new Float32Array(14) })).toThrow(
      /3 classes has 14 photometry values/u,
    );
  });
});

/** The allocations' kinds but uploads. */
function kindsOf(events: ReadonlyArray<AllocationEvent>): string[] {
  return events.filter((e) => e.kind !== "uploaded").map((e) => e.kind);
}

describe("the class-map store", () => {
  it("asks for a body's map, and files its texture, 66 × 66 texels a layer, under coarse-field-gpu", async () => {
    const engine = await countingRenderEngine();
    const events: AllocationEvent[] = [];
    engine.onAllocation((event) => events.push(event));
    const pool = new ScriptedPool();
    const store = new ClassMapStore(engine, pool);
    store.want(MOON, SEED);
    store.want(MOON, SEED);
    expect(pool.asked.map((q) => [q.body, q.detailSeed])).toEqual([[MOON, SEED]]);
    expect(store.classMapOf(MOON, ELSEWHERE)).toBeNull();
    pool.asked[0]?.resolve(answerOf(TEXELS));
    await settle();
    expect(engine.textureSpecs.at(-1)).toMatchObject({
      name: `class map ${MOON}`,
      size: { width: 66, height: 66, depthOrArrayLayers: 6 },
      format: "rgba8unorm",
      category: "coarse-field-gpu",
    });
    expect(events.filter((e) => e.kind === "created")).toEqual([
      {
        kind: "created",
        name: `class map ${MOON}`,
        bytes: 66 * 66 * 6 * 4,
        category: "coarse-field-gpu",
      },
    ]);
    const surface = store.classMapOf(MOON, ELSEWHERE);
    expect(surface?.kind).toBe("class-map");
    expect(surface?.elsewhere).toBe(ELSEWHERE);
    // Its texels travel with it, the texture's CPU copy, which the point regime integrates (R10.T10.f).
    expect(surface?.texels).toEqual(classMapTexelsOf(answerOf(TEXELS)));
    // The unnamed substance's class shades with `elsewhere`, the others with their own laws.
    expect(surface?.laws[2]).toBe(ELSEWHERE);
    expect(surface?.laws.slice(0, 2)).toEqual(classMapLawsOf(answerOf(TEXELS)).slice(0, 2));
    // The same surface, its laws the same objects, frame after frame; another for another law.
    expect(store.classMapOf(MOON, ELSEWHERE)).toBe(surface);
    const other = lawFor([0.3, 0.3, 0.3], [1, 1, 1], "moon");
    expect(store.classMapOf(MOON, other)?.elsewhere).toBe(other);
    expect(store.classMapOf(MOON, other)?.texels).toBe(surface?.texels);
    expect(store.classMapOf("0200080020000000.0301", ELSEWHERE)).toBeNull();
    store.dispose();
  });

  it("asks again after a block that surveys cells, once the query out is answered, and keeps the latest map", async () => {
    const engine = await countingRenderEngine();
    const pool = new ScriptedPool();
    const store = new ClassMapStore(engine, pool);
    store.want(MOON, SEED);
    // A block before the answer: asked again once it is in, not twice at once.
    pool.change(MOON, 1, [5, 6]);
    expect(pool.asked).toHaveLength(1);
    pool.asked[0]?.resolve(answerOf(TEXELS.slice(0, 1)));
    await settle();
    expect(pool.asked).toHaveLength(2);
    pool.asked[1]?.resolve(answerOf(TEXELS));
    await settle();
    const before = store.classMapOf(MOON, ELSEWHERE);
    // A block that surveys nothing over a complete map asks nothing.
    pool.change(MOON, 2, [], [40, 41]);
    expect(pool.asked).toHaveLength(2);
    // Another body's block asks nothing of this one.
    pool.change("0200080020000000.0301", 0, [1, 2]);
    expect(pool.asked).toHaveLength(2);
    pool.change(MOON, 3, [7, 8]);
    expect(pool.asked).toHaveLength(3);
    pool.asked[2]?.resolve(answerOf(TEXELS.slice(1)));
    await settle();
    const after = store.classMapOf(MOON, ELSEWHERE);
    expect(after).not.toBe(before);
    expect(engine.textureSpecs.filter((s) => s.category === "coarse-field-gpu")).toHaveLength(3);
    store.dispose();
  });

  it("asks again for an incomplete map at a block that brings cells, and for a field started afresh", async () => {
    const engine = await countingRenderEngine();
    const pool = new ScriptedPool();
    const store = new ClassMapStore(engine, pool);
    store.want(MOON, SEED);
    pool.asked[0]?.resolve(answerOf(TEXELS, false));
    await settle();
    pool.change(MOON, 4, [], [90]);
    expect(pool.asked).toHaveLength(2);
    pool.asked[1]?.resolve(answerOf(TEXELS));
    await settle();
    pool.change(MOON, 0, []);
    expect(pool.asked).toHaveLength(3);
    store.dispose();
  });

  it("leaves a body uniform before its field is held, and asks at the field's first block", async () => {
    const engine = await countingRenderEngine();
    const pool = new ScriptedPool();
    const store = new ClassMapStore(engine, pool);
    store.want(MOON, SEED);
    pool.asked[0]?.resolve(null);
    await settle();
    expect(store.classMapOf(MOON, ELSEWHERE)).toBeNull();
    pool.change(MOON, 0, [3]);
    expect(pool.asked).toHaveLength(2);
    store.dispose();
  });

  it("drops an answer for a body released, or asked for again with another seed, meanwhile", async () => {
    const engine = await countingRenderEngine();
    const pool = new ScriptedPool();
    const store = new ClassMapStore(engine, pool);
    store.want(MOON, SEED);
    store.release(MOON);
    pool.asked[0]?.resolve(answerOf(TEXELS));
    await settle();
    expect(store.classMapOf(MOON, ELSEWHERE)).toBeNull();
    store.want(MOON, SEED);
    store.want(MOON, "00000000000000a6");
    expect(pool.asked.map((q) => q.detailSeed)).toEqual([SEED, SEED, "00000000000000a6"]);
    pool.asked[1]?.resolve(answerOf(TEXELS));
    await settle();
    expect(store.classMapOf(MOON, ELSEWHERE)).toBeNull();
    pool.asked[2]?.resolve(answerOf(TEXELS));
    await settle();
    expect(store.classMapOf(MOON, ELSEWHERE)).not.toBeNull();
    expect(engine.textureSpecs.filter((s) => s.category === "coarse-field-gpu")).toHaveLength(1);
    store.dispose();
  });

  /** A store holding the Moon's map, its engine's allocations and its pool. */
  async function holding(): Promise<{
    readonly engine: Awaited<ReturnType<typeof countingRenderEngine>>;
    readonly events: AllocationEvent[];
    readonly pool: ScriptedPool;
    readonly store: ClassMapStore;
  }> {
    const engine = await countingRenderEngine();
    const events: AllocationEvent[] = [];
    engine.onAllocation((event) => events.push(event));
    const pool = new ScriptedPool();
    const store = new ClassMapStore(engine, pool);
    store.want(MOON, SEED);
    pool.asked[0]?.resolve(answerOf(TEXELS));
    await settle();
    return { engine, events, pool, store };
  }

  it("remakes the texture after a device loss, with the texels it held", async () => {
    const { engine, store } = await holding();
    const before = store.classMapOf(MOON, ELSEWHERE);
    engine.restore();
    const after = store.classMapOf(MOON, ELSEWHERE);
    expect([after?.weights !== before?.weights, engine.textureWritten.at(-1)?.data]).toEqual([
      true,
      engine.textureWritten.at(-2)?.data,
    ]);
    store.dispose();
  });

  it("releases the texture with the body", async () => {
    const { events, store } = await holding();
    store.release(MOON);
    expect([store.classMapOf(MOON, ELSEWHERE), kindsOf(events)]).toEqual([
      null,
      ["created", "destroyed"],
    ]);
    store.dispose();
  });

  it("keeps one listener on the pool as a body is released and asked for again", async () => {
    const { pool, store } = await holding();
    store.release(MOON);
    store.want(MOON, SEED);
    pool.asked[1]?.resolve(answerOf(TEXELS));
    await settle();
    expect(pool.listening).toBe(1);
    store.dispose();
  });

  it("releases its textures and stops listening to the pool at its disposal", async () => {
    const { events, pool, store } = await holding();
    store.dispose();
    expect([kindsOf(events), pool.listening]).toEqual([["created", "destroyed"], 0]);
  });

  it("refuses a body once disposed", async () => {
    const { store } = await holding();
    store.dispose();
    expect(() => {
      store.want(MOON, SEED);
    }).toThrow(/disposed/u);
  });

  it("reports a refused query once, and asks again at the next block", async () => {
    const engine = await countingRenderEngine();
    const pool = new ScriptedPool();
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    const store = new ClassMapStore(engine, pool);
    store.want(MOON, SEED);
    pool.asked[0]?.reject(new Error("refused"));
    await settle();
    pool.change(MOON, 1, [2]);
    pool.asked[1]?.reject(new Error("refused again"));
    await settle();
    expect(error).toHaveBeenCalledTimes(1);
    expect(pool.asked).toHaveLength(2);
    expect(store.classMapOf(MOON, ELSEWHERE)).toBeNull();
    store.dispose();
  });
});
