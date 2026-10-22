import { afterEach, describe, expect, it, vi } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { type CountingRenderEngine, countingRenderEngine } from "../../test/countingRenderEngine";
import type { ClassMapDiscSurface } from "../appearance/bodyAppearance";
import type { PhotometricLaw } from "../appearance/law";
import { lawFor } from "../appearance/phase";
import {
  classMapPointReflection,
  classMapPointState,
  classMapPointWork,
  classMapReflectionRatio,
  makeClassMapPointNow,
  stepClassMapPoints,
} from "../bodies/classMapPoint";
import { packNormalMap } from "../bodies/discNormals";
import {
  classMapMaturityLayer,
  classMapSide,
  type ClassMapTexels,
  classWeightsAt,
} from "../bodies/discSurface";
import { bodyReflection } from "../bodies/oblate";
import { IDENTITY_ROTATION } from "../coords/rotation";
import type { AllocationEvent, TextureSpec } from "../engine/memory";
import { EngineUnavailable } from "../engine/resilientEngine";
import {
  CLASS_MAP_FACE_TEXELS,
  CLASS_MAP_LEVEL,
  type ClassMapPool,
  ClassMapStore,
  classMapLawsOf,
  classMapTexelsOf,
  normalMapTexelsOf,
} from "./classMap";
import { fieldCellIndex } from "./coverage";
import { faceUvToXyz, stToUv } from "./cube";
import { MATURITY_NODES } from "./maturityNodes";
import type { Face } from "./patchKey";
import type { BodyFigure } from "./planet";
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

/** The synthetic answers' field level, at which their normal maps are. */
const FIELD_LEVEL = 7;

/** One cell of a synthetic answer's normal map, at {@link FIELD_LEVEL}, and its octahedral pair. */
interface NormalSpec {
  readonly face: Face;
  readonly i: number;
  readonly j: number;
  readonly pair: readonly [number, number];
}

/** Three cells' normals, each pair exact in a half float. */
const NORMALS: ReadonlyArray<NormalSpec> = [
  { face: 2, i: 20, j: 22, pair: [0.125, -0.25] },
  { face: 2, i: 21, j: 22, pair: [0, 0] },
  { face: 5, i: 127, j: 0, pair: [0.0625, 0.875] },
];

/**
 * A worker's answer of `texels` over {@link CLASSES}, in rising cell index, with the normals of
 * `normals` (none by default), in rising cell index too.
 */
function answerOf(
  texels: ReadonlyArray<TexelSpec>,
  complete = true,
  normals: ReadonlyArray<NormalSpec> = [],
  normalsComplete = true,
): ClassMapAnswer {
  const sorted = texels
    .map((t) => ({
      t,
      index: fieldCellIndex({ face: t.face, level: CLASS_MAP_LEVEL, i: t.i, j: t.j }),
    }))
    .toSorted((a, b) => a.index - b.index);
  const cells = normals
    .map((n) => ({
      n,
      index: fieldCellIndex({ face: n.face, level: FIELD_LEVEL, i: n.i, j: n.j }),
    }))
    .toSorted((a, b) => a.index - b.index);
  return {
    level: CLASS_MAP_LEVEL,
    fieldLevel: FIELD_LEVEL,
    classes: Uint8Array.from(CLASSES.flat()),
    photometry: Float32Array.from(PHOTOMETRY.flat()),
    texels: Uint32Array.from(sorted.map(({ index }) => index)),
    fractions: Float32Array.from(sorted.map(({ t }) => t.fraction)),
    weights: Uint8Array.from(sorted.flatMap(({ t }) => t.bytes)),
    complete,
    normalLevel: FIELD_LEVEL,
    normalCells: Uint32Array.from(cells.map(({ index }) => index)),
    normalPairs: Float16Array.from(cells.flatMap(({ n }) => n.pair)),
    normalsComplete,
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

/**
 * Whether two arrays hold the same values, compared by a loop: a deep equality over a level-7
 * normal map's 400 KB takes about a second.
 */
function sameValues(a: ArrayLike<number> | undefined, b: ArrayLike<number>): boolean {
  if (a === undefined || a.length !== b.length) {
    return false;
  }
  for (let k = 0; k < b.length; k += 1) {
    if (!Object.is(a[k], b[k])) {
      return false;
    }
  }
  return true;
}

/** The bytes of a typed array's view. */
function bytesOf(view: ArrayBufferView): Uint8Array {
  return new Uint8Array(view.buffer, view.byteOffset, view.byteLength);
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

  it("packs its normal map at the field's level, each answered cell its pair", () => {
    const answer = answerOf(TEXELS, true, NORMALS);
    const normals = normalMapTexelsOf(answer);
    const packed = packNormalMap(FIELD_LEVEL, answer.normalCells, answer.normalPairs);
    expect([sameValues(normals?.data, packed.data), normals?.faceTexels]).toEqual([
      true,
      2 ** FIELD_LEVEL,
    ]);
  });

  it("packs no normal map for an answer of no normal", () => {
    expect(normalMapTexelsOf(answerOf(TEXELS))).toBeNull();
  });

  it("refuses a normal map whose pairs are not two a cell", () => {
    const answer = answerOf(TEXELS, true, NORMALS);
    expect(() =>
      normalMapTexelsOf({ ...answer, normalPairs: answer.normalPairs.subarray(1) }),
    ).toThrow(/3 cells has 5 pair values/u);
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
    // An answer of no normal makes no normal map: the datum's normal shades it.
    expect(surface?.normals).toBeNull();
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

  /** A store holding the Moon's map with {@link NORMALS}, its engine's allocations and its pool. */
  async function holdingNormals(normalsComplete = true): Promise<{
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
    pool.asked[0]?.resolve(answerOf(TEXELS, true, NORMALS, normalsComplete));
    await settle();
    return { engine, events, pool, store };
  }

  it("files the normal map beside the class map, 130 × 130 texels a layer at the field's level 7, under coarse-field-gpu", async () => {
    const { engine, events, store } = await holdingNormals();
    const name = `class map normals ${MOON}`;
    expect(engine.textureSpecs.map((s) => s.name)).toEqual([`class map ${MOON}`, name]);
    expect(engine.textureSpecs.at(-1)).toMatchObject({
      name,
      size: { width: 130, height: 130, depthOrArrayLayers: 6 },
      dimension: "2d",
      format: "rg16float",
      category: "coarse-field-gpu",
    });
    expect(events.filter((e) => e.kind === "created")).toEqual([
      {
        kind: "created",
        name: `class map ${MOON}`,
        bytes: 66 * 66 * 6 * 4,
        category: "coarse-field-gpu",
      },
      { kind: "created", name, bytes: 130 * 130 * 6 * 4, category: "coarse-field-gpu" },
    ]);
    // The texels uploaded are the packed answer's, half floats as bytes.
    const texels = normalMapTexelsOf(answerOf(TEXELS, true, NORMALS));
    if (texels === null) {
      throw new Error("the answer has normals");
    }
    const written = engine.textureWritten.at(-1);
    expect(written?.texture).toBe(name);
    expect(sameValues(written?.data, bytesOf(texels.data))).toBe(true);
    // The surface hands both out together, the normals with their CPU copy.
    const surface = store.classMapOf(MOON, ELSEWHERE);
    expect(surface?.normals?.texture.name).toBe(name);
    expect(surface?.normals?.texels.faceTexels).toBe(texels.faceTexels);
    expect(sameValues(surface?.normals?.texels.data, texels.data)).toBe(true);
    expect(store.classMapOf(MOON, ELSEWHERE)).toBe(surface);
    store.dispose();
  });

  it("asks again while the normals are incomplete, at a block that brings cells", async () => {
    const { pool, store } = await holdingNormals(false);
    pool.change(MOON, 4, [], [90]);
    expect(pool.asked).toHaveLength(2);
    pool.asked[1]?.resolve(answerOf(TEXELS, true, NORMALS));
    await settle();
    // Complete in both: a block that surveys nothing asks nothing more.
    pool.change(MOON, 5, [], [91]);
    expect(pool.asked).toHaveLength(2);
    store.dispose();
  });

  it("remakes both textures after a device loss, with the texels they held", async () => {
    const { engine, store } = await holdingNormals();
    const before = store.classMapOf(MOON, ELSEWHERE);
    const firstWrites = engine.textureWritten.slice();
    engine.restore();
    const after = store.classMapOf(MOON, ELSEWHERE);
    expect(after?.weights).not.toBe(before?.weights);
    expect(after?.normals?.texture).not.toBe(before?.normals?.texture);
    expect(after?.normals?.texels).toBe(before?.normals?.texels);
    const again = engine.textureWritten.slice(firstWrites.length);
    expect(again.map((w) => w.texture)).toEqual(firstWrites.map((w) => w.texture));
    expect(again.every((w, k) => sameValues(w.data, firstWrites[k]?.data ?? []))).toBe(true);
    store.dispose();
  });

  it("releases both textures with the body", async () => {
    const released = await holdingNormals();
    released.store.release(MOON);
    expect([released.store.classMapOf(MOON, ELSEWHERE), kindsOf(released.events)]).toEqual([
      null,
      ["created", "created", "destroyed", "destroyed"],
    ]);
    released.store.dispose();
  });

  it("releases both textures at its disposal", async () => {
    const disposed = await holdingNormals();
    disposed.store.dispose();
    expect(kindsOf(disposed.events)).toEqual(["created", "created", "destroyed", "destroyed"]);
  });

  it("makes both textures or neither: the class map's released where the normal map's is refused, both at the restore", async () => {
    const engine = await countingRenderEngine();
    const events: AllocationEvent[] = [];
    engine.onAllocation((event) => events.push(event));
    // The device lost between the two creations: the normal map's refused.
    let refuse = true;
    const refusing: ConstructorParameters<typeof ClassMapStore>[0] = {
      createTexture: (spec: TextureSpec) => {
        if (refuse && spec.format === "rg16float") {
          throw new EngineUnavailable("createTexture");
        }
        return engine.createTexture(spec);
      },
      writeTexture: engine.writeTexture.bind(engine),
      releaseTexture: engine.releaseTexture.bind(engine),
      onRestored: engine.onRestored.bind(engine),
    };
    const pool = new ScriptedPool();
    const store = new ClassMapStore(refusing, pool);
    store.want(MOON, SEED);
    pool.asked[0]?.resolve(answerOf(TEXELS, true, NORMALS));
    await settle();
    expect([store.classMapOf(MOON, ELSEWHERE), kindsOf(events)]).toEqual([
      null,
      ["created", "destroyed"],
    ]);
    refuse = false;
    engine.restore();
    expect(store.classMapOf(MOON, ELSEWHERE)?.normals?.texture.name).toBe(
      `class map normals ${MOON}`,
    );
    store.dispose();
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

/** Runs the class-mapped points' slices until no work remains, whatever the time. */
function drain(): void {
  stepClassMapPoints(Number.POSITIVE_INFINITY, () => 0);
}

describe("the class-map store's points (R10.T14.f)", () => {
  afterEach(() => {
    // Each test's store, disposed, cancels what it queued, so that none steps another's.
    const { builds, refreshes } = classMapPointWork();
    if (builds !== 0 || refreshes !== 0) {
      throw new Error(`a test left ${String(builds)} builds and ${String(refreshes)} refreshes`);
    }
  });

  /** A sphere: the point is drawn about the body-fixed pole. */
  const SPHERE: BodyFigure = { equatorialRadiusM: 7.35e5, polarRadiusM: 7.35e5, pole: null };
  /** Face 2's centre, the body-fixed pole, over which the answers' texels lie. */
  const POLE = vec3(0, 0, 1);

  /** The Moon's surface as the store hands it out, which must exist. */
  function surfaceOf(store: ClassMapStore): ClassMapDiscSurface {
    const surface = store.classMapOf(MOON, ELSEWHERE);
    if (surface === null) {
      throw new Error("the store holds no map of the Moon");
    }
    return surface;
  }

  /** The Moon's point under `surface`, lit and seen from over the pole. */
  function pointOf(surface: ClassMapDiscSurface): ReadonlyArray<number> {
    return classMapPointReflection(SPHERE, surface, IDENTITY_ROTATION, POLE, POLE);
  }

  /**
   * A store holding the Moon's map with {@link NORMALS}, its point's elements made and its point
   * drawn, and the surface it was drawn under.
   */
  async function drawnMoon(): Promise<{
    readonly engine: CountingRenderEngine;
    readonly pool: ScriptedPool;
    readonly store: ClassMapStore;
    readonly first: ClassMapDiscSurface;
    readonly drawnFirst: ReadonlyArray<number>;
  }> {
    const engine = await countingRenderEngine();
    const pool = new ScriptedPool();
    const store = new ClassMapStore(engine, pool);
    store.want(MOON, SEED);
    pool.asked[0]?.resolve(answerOf(TEXELS, true, NORMALS));
    await settle();
    const first = surfaceOf(store);
    makeClassMapPointNow(first);
    return { engine, pool, store, first, drawnFirst: pointOf(first) };
  }

  /** A block that surveys cells, and the new map it brings, answered as the store's second. */
  async function remade(pool: ScriptedPool): Promise<void> {
    pool.change(MOON, 1, [7, 8]);
    pool.asked[1]?.resolve(answerOf(TEXELS.slice(1), true, NORMALS));
    await settle();
  }

  it("queues a map's point elements as it takes the map", async () => {
    const engine = await countingRenderEngine();
    const pool = new ScriptedPool();
    const store = new ClassMapStore(engine, pool);
    store.want(MOON, SEED);
    pool.asked[0]?.resolve(answerOf(TEXELS));
    await settle();
    expect(classMapPointState(surfaceOf(store))).toBe("queued");
    store.dispose();
  });

  it("cancels a map's unfinished point elements with its body", async () => {
    const engine = await countingRenderEngine();
    const pool = new ScriptedPool();
    const store = new ClassMapStore(engine, pool);
    store.want(MOON, SEED);
    pool.asked[0]?.resolve(answerOf(TEXELS));
    await settle();
    const surface = surfaceOf(store);
    store.release(MOON);
    expect(classMapPointState(surface)).toBe("unmade");
    store.dispose();
  });

  it("makes the elements anew for a surveying block's map, the last made map's point standing in meanwhile", async () => {
    const { pool, store, first, drawnFirst } = await drawnMoon();
    // A block that surveys cells brings a new map, its texels and normals new objects (R10.T10.g's
    // remake), whose elements are made again.
    await remade(pool);
    const second = surfaceOf(store);
    expect([
      second.texels === first.texels,
      second.normals?.texels === first.normals?.texels,
      classMapPointState(second),
      pointOf(second),
    ]).toEqual([false, false, "queued", drawnFirst]);
    store.dispose();
  });

  it("draws the new map's own point once the slices have made its elements and found its light", async () => {
    const { pool, store, drawnFirst } = await drawnMoon();
    await remade(pool);
    const second = surfaceOf(store);
    drain();
    // The first frame after draws what it was drawn with, and queues its own light.
    pointOf(second);
    drain();
    const ratio = classMapReflectionRatio(SPHERE, second, POLE, POLE);
    const uniform = bodyReflection({ ...SPHERE, pole: POLE }, ELSEWHERE, POLE, POLE);
    const own = uniform.map((u, c) => u * (ratio[c] ?? Number.NaN));
    expect([pointOf(second), own[1] === drawnFirst[1]]).toEqual([own, false]);
    store.dispose();
  });

  it("stands the old map in for a new one that comes before a frame asks again after a device loss", async () => {
    const { engine, pool, store, drawnFirst } = await drawnMoon();
    // The restore makes the textures again and forgets the surface handed out.
    engine.restore();
    await remade(pool);
    expect(pointOf(surfaceOf(store))).toEqual(drawnFirst);
    store.dispose();
  });
});

// --- The map's maturity (plan R10, R10.T1.c's client half) ---------------------------------------

/**
 * `answer` with a maturity, as a worker answers it: each texel's code `codeOf` its cell index, and
 * each named class's 16 nodes in B, V and R, node j's band b 0.1 (c + 1) + 0.01 b + 0.001 j; the
 * unnamed class's 0.
 */
function maturedAnswer(
  answer: ClassMapAnswer,
  codeOf: (index: number) => number = (index) => index % 65_536,
): ClassMapAnswer {
  const classes = answer.classes.length / 2;
  const nodes = new Float32Array(3 * MATURITY_NODES * classes);
  for (let c = 0; c < classes; c += 1) {
    if (answer.classes[2 * c + 1] === PALETTE_NONE) {
      continue;
    }
    for (let j = 0; j < MATURITY_NODES; j += 1) {
      for (let b = 0; b < 3; b += 1) {
        nodes[3 * (MATURITY_NODES * c + j) + b] = 0.1 * (c + 1) + 0.01 * b + 0.001 * j;
      }
    }
  }
  return {
    ...answer,
    maturity: { codes: Uint16Array.from(answer.texels, codeOf), nodes },
  };
}

/** Class c's node K − 1 in band b of {@link maturedAnswer}'s nodes, as `f32` holds it. */
function background(c: number, b: number): number {
  return Math.fround(0.1 * (c + 1) + 0.01 * b + 0.001 * (MATURITY_NODES - 1));
}

/** The code at face texel (i, j) of `face` of a packed map, as the layout documents it. */
function codeAt(map: ClassMapTexels, face: Face, i: number, j: number): number {
  const side = classMapSide(map.faceTexels);
  const at = ((classMapMaturityLayer(face, map.classes) * side + j + 1) * side + i + 1) * 4;
  return (map.data[at] ?? Number.NaN) * 256 + (map.data[at + 1] ?? Number.NaN);
}

describe("a class map's maturity from a worker's answer (R10.T1.c)", () => {
  it("packs each answered texel's code, and none elsewhere", () => {
    const answer = maturedAnswer(answerOf(TEXELS), (index) => (index * 37) % 65_536);
    const map = classMapTexelsOf(answer);
    if (map === null) {
      throw new Error("no map");
    }
    expect(TEXELS.map((t) => codeAt(map, t.face, t.i, t.j))).toEqual(
      TEXELS.map(
        (t) =>
          (fieldCellIndex({ face: t.face, level: CLASS_MAP_LEVEL, i: t.i, j: t.j }) * 37) % 65_536,
      ),
    );
    expect([codeAt(map, 2, 13, 11), codeAt(map, 0, 0, 0)]).toEqual([0, 0]);
  });

  it("carries the classes' nodes in the display channels, each named class drawn from them", () => {
    const answer = maturedAnswer(answerOf(TEXELS));
    const map = classMapTexelsOf(answer);
    const answered = answer.maturity?.nodes ?? new Float32Array();
    const expected = Float32Array.from(
      answered,
      (_, at) => answered[at - (at % 3) + 2 - (at % 3)] ?? 0,
    );
    expect([sameValues(map?.maturity?.nodes, expected), map?.maturity?.drawn]).toEqual([
      true,
      [true, true, false],
    ]);
  });

  it("packs no maturity for an answer without one, as before", () => {
    const map = classMapTexelsOf(answerOf(TEXELS));
    expect(map === null ? null : Object.hasOwn(map, "maturity")).toBe(false);
  });

  it.each([
    {
      name: "whose codes are not one a texel",
      change: (answer: ClassMapAnswer): ClassMapAnswer => ({
        ...answer,
        maturity: {
          codes: new Uint16Array(3),
          nodes: answer.maturity?.nodes ?? new Float32Array(),
        },
      }),
      message: /4 texels have 3 maturity codes/u,
    },
    {
      name: "whose nodes are not K × 3 a class",
      change: (answer: ClassMapAnswer): ClassMapAnswer => ({
        ...answer,
        maturity: {
          codes: answer.maturity?.codes ?? new Uint16Array(),
          nodes: new Float32Array(3 * MATURITY_NODES * 2),
        },
      }),
      message: /3 classes has 144 node values, not 96/u,
    },
  ])("refuses an answer $name", ({ change, message }) => {
    expect(() => classMapTexelsOf(change(maturedAnswer(answerOf(TEXELS))))).toThrow(message);
  });

  it("takes no law of nodes that are not K × 3 a class", () => {
    const answer = maturedAnswer(answerOf(TEXELS));
    expect(() =>
      classMapLawsOf({
        ...answer,
        maturity: {
          codes: answer.maturity?.codes ?? new Uint16Array(),
          nodes: new Float32Array(3 * MATURITY_NODES * 2),
        },
      }),
    ).toThrow(/3 classes has 144 node values, not 96/u);
  });

  it("takes a named class's A as its node K − 1, in the display channels, its row and L(0) its photometry's", () => {
    const [regolith, frost, unnamed] = classMapLawsOf(maturedAnswer(answerOf(TEXELS)));
    expect(regolith).toEqual(
      lawOf(PHASE_LAW.regolith, 1, [background(0, 2), background(0, 1), background(0, 0)]),
    );
    expect(frost).toEqual(
      lawOf(PHASE_LAW.ice, Math.fround(0.6), [
        background(1, 2),
        background(1, 1),
        background(1, 0),
      ]),
    );
    expect(unnamed).toBeNull();
  });

  it("files a matured map's texture with six more layers, and hands out its texels with their maturity", async () => {
    const engine = await countingRenderEngine();
    const pool = new ScriptedPool();
    const store = new ClassMapStore(engine, pool);
    // Disposed however the test ends, so that the point build it queues never outlives it.
    try {
      store.want(MOON, SEED);
      const answer = maturedAnswer(answerOf(TEXELS));
      pool.asked[0]?.resolve(answer);
      await settle();
      expect(engine.textureSpecs.at(-1)?.size).toEqual({
        width: 66,
        height: 66,
        depthOrArrayLayers: 12,
      });
      const surface = store.classMapOf(MOON, ELSEWHERE);
      expect(sameValues(surface?.texels.data, classMapTexelsOf(answer)?.data ?? [])).toBe(true);
      expect(surface?.texels.maturity?.drawn).toEqual([true, true, false]);
      expect(surface?.laws.slice(0, 2)).toEqual(classMapLawsOf(answer).slice(0, 2));
      // Its point's elements queued as any map's (R10.T14.f), and cancelled with the store.
      expect(surface === null ? null : classMapPointState(surface)).toBe("queued");
    } finally {
      store.dispose();
    }
    expect(classMapPointWork()).toEqual({ builds: 0, refreshes: 0 });
  });
});
