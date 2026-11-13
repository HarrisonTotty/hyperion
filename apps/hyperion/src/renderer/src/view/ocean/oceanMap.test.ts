import { describe, expect, it, vi } from "vitest";

import { countingRenderEngine } from "../../test/countingRenderEngine";
import { oceanAnswerOf, oceanCellCentre, WIND_SPEEDS_MPS } from "../../test/oceanFixtures";
import type { AllocationEvent } from "../engine/memory";
import type { OceanMapAnswer } from "../terrain/workers/messages";
import type { FieldChange } from "../terrain/workers/pool";
import { monthBlend } from "./monthBlend";
import {
  blendedWindAt,
  OCEAN_WIND_WEIGHT_STEP,
  OceanMapStore,
  type OceanMapPool,
  oceanTexelAt,
  packOceanTexels,
} from "./oceanMap";
import { oceanMaskOf } from "./oceanMask";

const MOON = "0200080020000000.0301";

/** Speed code c of 0.01 × 2^((c − 1) ÷ 16) m/s nearest `mps`. */
function speedCode(mps: number): number {
  return Math.round(16 * Math.log2(mps / 0.01)) + 1;
}

/** A sea of three cells: open water with a wind turning from north to east, a lake, and dry land. */
function seaAnswer(): OceanMapAnswer {
  const north: ReadonlyArray<readonly [number, number]> = Array.from({ length: 12 }, (_, m) => [
    m === 1 ? 64 : 0,
    speedCode(m === 1 ? 10 : 5),
  ]);
  return oceanAnswerOf([
    { face: 2, i: 10, j: 10, depthM: 4000, levelM: 0, ice: 0, winds: north },
    { face: 2, i: 11, j: 10, depthM: 35, levelM: 250, ice: 0, winds: north },
    { face: 2, i: 12, j: 10, depthM: 0, levelM: 800, ice: 0, winds: north },
    { face: 2, i: 13, j: 10, depthM: 30, levelM: 0, ice: 255, winds: north },
  ]);
}

describe("the ocean map's wind (R11.T8.b)", () => {
  it("decodes a month's wind towards its azimuth, clockwise from north, at its code's speed", () => {
    const answer = seaAnswer();
    const january = blendedWindAt(answer, 0, { from: 0, to: 1, weight: 0 });
    const speed = WIND_SPEEDS_MPS[speedCode(5)] ?? 0;
    expect(january.northMps).toBeCloseTo(speed, 12);
    expect(january.eastMps).toBeCloseTo(0, 12);
    const february = blendedWindAt(answer, 0, { from: 0, to: 1, weight: 1 });
    // Azimuth code 64 is a quarter turn: towards the east.
    expect(february.eastMps).toBeCloseTo(WIND_SPEEDS_MPS[speedCode(10)] ?? 0, 12);
    expect(february.northMps).toBeCloseTo(0, 12);
  });

  it("blends the mean speeds linearly, along the blended vectors' direction", () => {
    const answer = seaAnswer();
    const wind = blendedWindAt(answer, 0, { from: 0, to: 1, weight: 0.5 });
    const expected =
      0.5 * (WIND_SPEEDS_MPS[speedCode(5)] ?? 0) + 0.5 * (WIND_SPEEDS_MPS[speedCode(10)] ?? 0);
    expect(wind.speedMps).toBeCloseTo(expected, 12);
    expect(Math.hypot(wind.eastMps, wind.northMps)).toBeCloseTo(expected, 12);
    // Between north at 5 m/s and east at 10 m/s: towards atan2(10, 5) east of north.
    expect(Math.atan2(wind.eastMps, wind.northMps)).toBeCloseTo(
      Math.atan2(WIND_SPEEDS_MPS[speedCode(10)] ?? 0, WIND_SPEEDS_MPS[speedCode(5)] ?? 0),
      12,
    );
  });

  it("keeps the from month's direction where the months' winds cancel", () => {
    const opposed: ReadonlyArray<readonly [number, number]> = [
      [0, speedCode(4)],
      [128, speedCode(4)],
    ];
    const answer = oceanAnswerOf([
      { face: 0, i: 0, j: 0, depthM: 10, levelM: 0, ice: 0, winds: opposed },
    ]);
    const wind = blendedWindAt(answer, 0, { from: 0, to: 1, weight: 0.5 });
    expect(wind.speedMps).toBeCloseTo(WIND_SPEEDS_MPS[speedCode(4)] ?? 0, 12);
    expect(wind.northMps).toBeCloseTo(wind.speedMps, 12);
  });
});

describe("the ocean map's texels (R11.T8.b)", () => {
  it("packs open water's depth, level and wind, and nothing on dry, iced or unsurveyed cells", () => {
    const answer = seaAnswer();
    const mask = oceanMaskOf(answer);
    const blend = { from: 0, to: 1, weight: 0 };
    const texels = packOceanTexels(answer, mask, blend);
    expect(texels.data.length).toBe(6 * 32 * 32 * 4);
    const sea = oceanTexelAt(texels, oceanCellCentre(2, 10, 10));
    expect(sea?.depthM).toBe(4000);
    expect(sea?.levelM).toBe(0);
    expect(sea?.windNorthMps).toBeCloseTo(WIND_SPEEDS_MPS[speedCode(5)] ?? 0, 2);
    // A closed basin is drawn at its own level, 250 m above the sea.
    const lake = oceanTexelAt(texels, oceanCellCentre(2, 11, 10));
    expect([lake?.depthM, lake?.levelM]).toEqual([35, 250]);
    expect(oceanTexelAt(texels, oceanCellCentre(2, 12, 10))).toBeNull();
    expect(oceanTexelAt(texels, oceanCellCentre(2, 13, 10))).toBeNull();
    expect(oceanTexelAt(texels, oceanCellCentre(2, 10, 11))).toBeNull();
    // Only the two open cells hold anything.
    const filled = new Set<number>();
    texels.data.forEach((value, at) => {
      if (value !== 0) {
        filled.add(Math.floor(at / 4));
      }
    });
    expect(filled.size).toBe(2);
  });
});

/** A pool that holds each ocean-map query for the test to answer. */
class ScriptedOceanPool implements OceanMapPool {
  readonly asked: Array<{
    readonly body: string;
    readonly resolve: (map: OceanMapAnswer | null) => void;
    readonly reject: (error: Error) => void;
  }> = [];
  readonly #listeners = new Set<(change: FieldChange) => void>();

  oceanMap(body: string): Promise<OceanMapAnswer | null> {
    return new Promise((resolve, reject) => {
      this.asked.push({ body, resolve, reject });
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

  change(body: string, sequence: number, surveyed: ReadonlyArray<number>): void {
    for (const listener of this.#listeners) {
      listener({
        body,
        sequence,
        fieldLevel: 5,
        newlySurveyed: Uint32Array.from(surveyed),
        newlyHeld: new Uint32Array(),
        levels: new Float64Array(100),
        sigmaM: new Float64Array(25),
        chargeM: new Float64Array(25),
      });
    }
  }
}

/** Lets the store's queries resolve, then runs its packing to its end. */
async function settle(store: OceanMapStore): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
  store.step(Number.POSITIVE_INFINITY, () => 0);
}

describe("the ocean-map store (R11.T8.b)", () => {
  it("asks for a body's sea and files its texture, six 32 × 32 layers of rgba16float, under ocean", async () => {
    const engine = await countingRenderEngine();
    const events: AllocationEvent[] = [];
    engine.onAllocation((event) => events.push(event));
    const pool = new ScriptedOceanPool();
    const store = new OceanMapStore(engine, pool);
    store.want(MOON);
    store.want(MOON);
    expect(pool.asked.map((q) => q.body)).toEqual([MOON]);
    expect(store.oceanMapOf(MOON)).toBeNull();
    pool.asked[0]?.resolve(seaAnswer());
    await settle(store);
    expect(engine.textureSpecs.at(-1)).toMatchObject({
      name: `ocean map ${MOON}`,
      size: { width: 32, height: 32, depthOrArrayLayers: 6 },
      format: "rgba16float",
      category: "ocean",
    });
    expect(events.filter((e) => e.kind === "created")).toEqual([
      { kind: "created", name: `ocean map ${MOON}`, bytes: 6 * 32 * 32 * 8, category: "ocean" },
    ]);
    expect(engine.staged.map((s) => s.label)).toEqual(["ocean map upload"]);
    const held = store.oceanMapOf(MOON);
    expect(held?.mask.openCells).toBe(2);
    expect(held?.texels.blend).toEqual(monthBlend(12, 0.0167, 0));
    store.release(MOON);
    expect(engine.live.textures).toBe(0);
    store.dispose();
    expect(pool.listening).toBe(0);
  });

  it("blends the winds again once the season moves a step, and not before", async () => {
    const engine = await countingRenderEngine();
    const pool = new ScriptedOceanPool();
    const store = new OceanMapStore(engine, pool);
    store.want(MOON);
    pool.asked[0]?.resolve(seaAnswer());
    await settle(store);
    const writes = engine.textureWritten.length;
    const first = store.oceanMapOf(MOON)?.texels.blend;
    // Within a step of the weight: the texels stand.
    store.season(MOON, 1e-4);
    expect(store.packing).toBe(false);
    // A month on: packed again and written again.
    store.season(MOON, (2 * Math.PI) / 12 + OCEAN_WIND_WEIGHT_STEP);
    expect(store.packing).toBe(true);
    store.step(Number.POSITIVE_INFINITY, () => 0);
    expect(engine.textureWritten.length).toBe(writes + 1);
    expect(store.oceanMapOf(MOON)?.texels.blend).not.toEqual(first);
    expect(() => {
      store.season(MOON, Number.NaN);
    }).toThrow(RangeError);
    store.dispose();
  });

  it("asks again after a block that surveys cells or starts the field, and remakes its texture on a restore", async () => {
    const engine = await countingRenderEngine();
    const pool = new ScriptedOceanPool();
    const store = new OceanMapStore(engine, pool);
    store.want(MOON);
    pool.asked[0]?.resolve(seaAnswer());
    await settle(store);
    pool.change(MOON, 3, []);
    expect(pool.asked.length).toBe(1);
    pool.change(MOON, 3, [7]);
    expect(pool.asked.length).toBe(2);
    pool.change("0200080020000000.0302", 0, [7]);
    expect(pool.asked.length).toBe(2);
    pool.asked[1]?.resolve(seaAnswer());
    await settle(store);
    const made = engine.textureSpecs.length;
    engine.restore();
    expect(engine.textureSpecs.length).toBe(made + 1);
    expect(store.oceanMapOf(MOON)).not.toBeNull();
    store.dispose();
  });

  it("reports a refused query once, and keeps the body without a map", async () => {
    const engine = await countingRenderEngine();
    const pool = new ScriptedOceanPool();
    const store = new OceanMapStore(engine, pool);
    const error = vi.spyOn(console, "error").mockImplementation(() => undefined);
    store.want(MOON);
    pool.asked[0]?.reject(new Error("refused"));
    await settle(store);
    expect(store.oceanMapOf(MOON)).toBeNull();
    pool.change(MOON, 0, [7]);
    pool.asked[1]?.reject(new Error("refused again"));
    await settle(store);
    expect(error).toHaveBeenCalledTimes(1);
    store.dispose();
  });
});
