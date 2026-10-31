/**
 * The optics pool's bands (plan R08, R08.T14.d's bake time): a bake of several bands asked one job
 * a band over the workers and gathered in the bands' order, on scripted workers; and a disc's table
 * gathered from its bands' shares.
 */
import { describe, expect, it, onTestFinished } from "vitest";

import { saturnBody } from "../../test/atmosphereFixtures";
import { microtasks } from "../../test/opticsFakes";
import { ScriptedOpticsWorker } from "../../test/scriptedOpticsWorker";
import {
  patternedCorrection,
  patternedDeckTables,
  patternedThickBake,
} from "../../test/thickTables";
import type { BodyAtmosphereInput } from "./atmosphereInput";
import {
  DISC_AZIMUTHS,
  DISC_LAYER_VALUES,
  DISC_SUN_COSINES,
  DISC_VIEW_COSINES,
  type DiscBandValues,
  discTableOfBands,
  discTableValuesOf,
  discTexelIndex,
} from "./discTable";
import { R05_EARTH_REFERENCE } from "./earth";
import type { OpticsRequest } from "./opticsMessages";
import {
  type DeckBakeAsk,
  type DiscBakeAsk,
  OpticsWorkerPool,
  type ThickBakeAsk,
} from "./opticsPool";
import { kernelMedium } from "./tables";
import { thickBakeBands } from "./thick/bake";
import { spectralColumnOf } from "./thick/columnLayout";
import { sphereBakeBody } from "./thick/keptBake";
import { channelSource } from "./thick/spectralColumn";

const KEPT_UNSET: BodyAtmosphereInput = { kind: "kept", atmosphere: { kind: "unset" } };

/** A pool of `count` scripted workers, terminated when the test ends however it ends. */
function scriptedPool(count: number): {
  readonly pool: OpticsWorkerPool;
  readonly workers: ScriptedOpticsWorker[];
} {
  const workers: ScriptedOpticsWorker[] = [];
  const pool = new OpticsWorkerPool({
    workers: count,
    createWorker: () => {
      const worker = new ScriptedOpticsWorker();
      workers.push(worker);
      return worker;
    },
  });
  onTestFinished(() => {
    pool.terminate();
  });
  return { pool, workers };
}

/** Each request in flight on the pool's workers, by the band it asks, or a test failure. */
function inFlight(workers: ReadonlyArray<ScriptedOpticsWorker>): Map<number, OpticsRequest> {
  const asked = new Map<number, OpticsRequest>();
  for (const worker of workers) {
    for (const request of worker.pending) {
      if (!("bands" in request)) {
        throw new Error(`a worker was asked ${request.kind}`);
      }
      const band = request.kind === "disc-band-bake" ? request.part : request.bands[0];
      if (band === undefined || (request.kind !== "disc-band-bake" && request.bands.length !== 1)) {
        throw new Error(`a ${request.kind} asked bands ${request.bands.join(", ")}`);
      }
      asked.set(band, request);
    }
  }
  return asked;
}

/** The scripted worker a request was posted to, or a test failure. */
function workerOf(
  workers: ReadonlyArray<ScriptedOpticsWorker>,
  request: OpticsRequest,
): ScriptedOpticsWorker {
  const worker = workers.find((each) => each.received.includes(request));
  if (worker === undefined) {
    throw new Error(`no worker was posted request ${request.id}`);
  }
  return worker;
}

/** An ask of R05's Earth's column, by its channels, on Saturn's four bands. */
function saturnAsk(): ThickBakeAsk {
  const body = { ...saturnBody(), bottomRadiusM: 6_371_008.8 };
  return {
    input: KEPT_UNSET,
    slots: [0],
    bands: thickBakeBands(body).map((_, band) => band),
    column: spectralColumnOf(R05_EARTH_REFERENCE, R05_EARTH_REFERENCE.terms.map(channelSource)),
    body,
  };
}

/** An ask of R05's Earth's column on a sphere's one band. */
function sphereAsk(): ThickBakeAsk {
  const body = sphereBakeBody(6_371_008.8, 9.806_65);
  return { ...saturnAsk(), bands: [0], body };
}

/** What a disc of R05's Earth's medium marches with, no routed bake landed. */
const AIR: DiscBakeAsk["air"] = {
  medium: R05_EARTH_REFERENCE,
  kernelMedium: kernelMedium(R05_EARTH_REFERENCE),
  curves: [],
};

/** The kinds of every request the pool's workers were posted, worker by worker. */
function askedKinds(workers: ReadonlyArray<ScriptedOpticsWorker>): ReadonlyArray<string> {
  return workers.flatMap((worker) => worker.received.map((request) => request.kind));
}

/** The order the tests answer Saturn's four bands in: none in the bands' own. */
const LANDING = [3, 1, 0, 2] as const;

/** A disc's band's share, its values patterned by band so that each lands where it should. */
function bandValues(band: number, glow: boolean): DiscBandValues {
  return {
    slots: 1,
    sunFloor: -0.25,
    lit: discTableValuesOf(
      Float32Array.from({ length: DISC_LAYER_VALUES }, (_, i) => band + i / DISC_LAYER_VALUES),
    ),
    glow: glow
      ? Float64Array.from(
          { length: DISC_VIEW_COSINES * 3 },
          (_, i) => (i % 3 === 1 ? -1 : 1) * (band + 1) * (i + 1),
        )
      : null,
  };
}

describe("the optics pool's bands (R08.T14.d)", () => {
  it("asks a body's four bands one job a band, every worker busy, and gathers the tables in the bands' order whatever order they land in", async () => {
    const { pool, workers } = scriptedPool(4);
    const ask = saturnAsk();
    const tables = patternedThickBake(thickBakeBands(ask.body), R05_EARTH_REFERENCE, 1);
    const bake = pool.bakeThick(ask);
    const asked = inFlight(workers);
    expect([...asked.keys()].toSorted((p, q) => p - q)).toEqual([0, 1, 2, 3]);
    for (const band of LANDING) {
      const request = asked.get(band);
      const table = tables[band];
      if (request === undefined || table === undefined) {
        throw new Error(`band ${band} was not asked`);
      }
      workerOf(workers, request).reply({ kind: "thick-baked", id: request.id, tables: [table] });
    }
    const outcome = await bake.outcome;
    expect(
      outcome.kind === "baked" && outcome.tables.every((table, b) => table === tables[b]),
    ).toBe(true);
  });

  it("gathers a correction's bands in the bands' order whatever order they land in", async () => {
    const { pool, workers } = scriptedPool(4);
    const ask = saturnAsk();
    const corrections = patternedCorrection(thickBakeBands(ask.body), R05_EARTH_REFERENCE, 1);
    const correction = pool.bakePolarisation(ask);
    const asked = inFlight(workers);
    for (const band of LANDING) {
      const request = asked.get(band);
      const table = corrections[band];
      if (request === undefined || table === undefined) {
        throw new Error(`band ${band} was not asked`);
      }
      workerOf(workers, request).reply({
        kind: "polarisation-baked",
        id: request.id,
        tables: [table],
      });
    }
    const outcome = await correction.outcome;
    expect(outcome.kind === "baked" ? outcome.tables.map((table) => table.band) : outcome).toEqual(
      corrections.map((table) => table.band),
    );
  });

  it("gathers a deck's bands with the correction over each in the bands' order whatever order they land in", async () => {
    const { pool, workers } = scriptedPool(4);
    const ask: DeckBakeAsk = { ...saturnAsk(), split: { altitudeM: 3_000, floor: "ground" } };
    const bands = thickBakeBands(ask.body);
    const decks = patternedDeckTables(bands, R05_EARTH_REFERENCE, 1, 3_000);
    const corrections = patternedCorrection(bands, R05_EARTH_REFERENCE, 1);
    const deck = pool.bakeDeck(ask);
    const asked = inFlight(workers);
    for (const band of LANDING) {
      const request = asked.get(band);
      const tables = decks[band];
      const over = corrections[band];
      if (request === undefined || tables === undefined || over === undefined) {
        throw new Error(`band ${band} was not asked`);
      }
      workerOf(workers, request).reply({
        kind: "deck-baked",
        id: request.id,
        tables: [tables],
        correction: [over],
      });
    }
    const outcome = await deck.outcome;
    expect(
      outcome.kind === "baked"
        ? [
            outcome.tables.map((table) => decks.indexOf(table)),
            outcome.correction?.map((table) => corrections.indexOf(table)),
          ]
        : outcome,
    ).toEqual([
      [0, 1, 2, 3],
      [0, 1, 2, 3],
    ]);
  });

  it("fails a body's bake with its first band to fail, its other bands' jobs given up, and lands another body's", async () => {
    const { pool, workers } = scriptedPool(4);
    const failing = pool.bakeThick(saturnAsk());
    const landing = pool.bakeThick(saturnAsk());
    expect(pool.queuedCount).toBe(4);
    const asked = inFlight(workers);
    const broken = asked.get(1);
    if (broken === undefined) {
      throw new Error("band 1 was not asked");
    }
    workerOf(workers, broken).reply({
      kind: "thick-bake-failed",
      id: broken.id,
      bands: [1],
      message: "the solver refused a layer",
    });
    expect(await failing.outcome).toEqual({
      kind: "failed",
      message: "the solver refused a layer",
    });
    const tables = patternedThickBake(thickBakeBands(saturnAsk().body), R05_EARTH_REFERENCE, 1);
    // The failed bake's other bands hold their workers until their late answers, which no bake
    // takes; the other body's bands go to the workers as they free.
    for (const [band, request] of asked) {
      if (band !== 1) {
        workerOf(workers, request).reply({
          kind: "thick-baked",
          id: request.id,
          tables: tables.slice(band, band + 1),
        });
      }
    }
    for (const [band, request] of inFlight(workers)) {
      workerOf(workers, request).reply({
        kind: "thick-baked",
        id: request.id,
        tables: tables.slice(band, band + 1),
      });
    }
    const landed = await landing.outcome;
    expect([
      landed.kind === "baked" && landed.tables.every((table, b) => table === tables[b]),
      pool.queuedCount,
    ]).toEqual([true, 0]);
  });

  it("settles a bake cancelled with its bands queued and sent, dropping the queued", async () => {
    const { pool } = scriptedPool(2);
    const bake = pool.bakeThick(saturnAsk());
    expect([pool.inFlightCount, pool.queuedCount]).toEqual([2, 2]);
    bake.cancel();
    expect([await bake.outcome, pool.queuedCount]).toEqual([{ kind: "cancelled" }, 0]);
  });

  it("asks a disc's bands one disc-band-bake a band with the landed bake, and gathers its table as discTableOfBands does", async () => {
    const { pool, workers } = scriptedPool(4);
    const thick = saturnAsk();
    const bake = patternedThickBake(thickBakeBands(thick.body), R05_EARTH_REFERENCE, 1);
    const ask: DiscBakeAsk = {
      ...thick,
      air: {
        medium: R05_EARTH_REFERENCE,
        kernelMedium: kernelMedium(R05_EARTH_REFERENCE),
        curves: [],
        bake,
      },
    };
    const disc = pool.bakeDisc(ask);
    const asked = inFlight(workers);
    for (const band of LANDING) {
      const request = asked.get(band);
      if (request?.kind !== "disc-band-bake") {
        throw new Error(`band ${band} was not asked a disc's band`);
      }
      expect([request.bands, request.air.bake === bake]).toEqual([thick.bands, true]);
      workerOf(workers, request).reply({
        kind: "disc-band-baked",
        id: request.id,
        values: bandValues(band, true),
      });
    }
    const outcome = await disc.outcome;
    expect(outcome).toEqual({
      kind: "baked",
      table: discTableOfBands([0, 1, 2, 3].map((band) => bandValues(band, true))),
    });
  });

  it("bakes a disc's J one thick-bake a band where no routed bake has landed, then gives it to every band's job", async () => {
    const { pool, workers } = scriptedPool(4);
    const thick = saturnAsk();
    const bake = patternedThickBake(thickBakeBands(thick.body), R05_EARTH_REFERENCE, 1);
    const air = {
      medium: R05_EARTH_REFERENCE,
      kernelMedium: kernelMedium(R05_EARTH_REFERENCE),
      curves: [],
    };
    const disc = pool.bakeDisc({ ...thick, air });
    for (const band of LANDING) {
      const request = inFlight(workers).get(band);
      const table = bake[band];
      if (request?.kind !== "thick-bake" || table === undefined) {
        throw new Error(`band ${band}'s J was not asked`);
      }
      workerOf(workers, request).reply({ kind: "thick-baked", id: request.id, tables: [table] });
    }
    await microtasks();
    const parts = inFlight(workers);
    for (const [band, request] of parts) {
      if (request.kind !== "disc-band-bake") {
        throw new Error(`band ${band} was asked ${request.kind}`);
      }
      expect([
        request.air.medium,
        request.air.bake?.every((table, b) => table === bake[b]),
      ]).toEqual([air.medium, true]);
      workerOf(workers, request).reply({
        kind: "disc-band-baked",
        id: request.id,
        values: bandValues(band, false),
      });
    }
    const outcome = await disc.outcome;
    expect([parts.size, outcome.kind]).toEqual([4, "baked"]);
  });

  it("fails a disc whose J fails, asking none of its bands", async () => {
    const { pool, workers } = scriptedPool(4);
    const disc = pool.bakeDisc({ ...saturnAsk(), air: AIR });
    const broken = inFlight(workers).get(2);
    if (broken === undefined) {
      throw new Error("band 2's J was not asked");
    }
    workerOf(workers, broken).reply({
      kind: "thick-bake-failed",
      id: broken.id,
      bands: [2],
      message: "the solver refused a layer",
    });
    expect(await disc.outcome).toEqual({ kind: "failed", message: "the solver refused a layer" });
    await microtasks();
    expect(askedKinds(workers)).not.toContain("disc-band-bake");
  });

  it("settles a disc cancelled while its J bakes, asking none of its bands when J's late answers land", async () => {
    const { pool, workers } = scriptedPool(2);
    const thick = saturnAsk();
    const bake = patternedThickBake(thickBakeBands(thick.body), R05_EARTH_REFERENCE, 1);
    const disc = pool.bakeDisc({ ...thick, air: AIR });
    expect(pool.queuedCount).toBe(2);
    disc.cancel();
    for (const [band, request] of inFlight(workers)) {
      workerOf(workers, request).reply({
        kind: "thick-baked",
        id: request.id,
        tables: bake.slice(band, band + 1),
      });
    }
    await microtasks();
    expect([await disc.outcome, pool.queuedCount, askedKinds(workers)]).toEqual([
      { kind: "cancelled" },
      0,
      ["thick-bake", "thick-bake"],
    ]);
  });

  it("settles a disc cancelled once its bands are asked, dropping those queued", async () => {
    const { pool, workers } = scriptedPool(2);
    const thick = saturnAsk();
    const bake = patternedThickBake(thickBakeBands(thick.body), R05_EARTH_REFERENCE, 1);
    const disc = pool.bakeDisc({ ...thick, air: { ...AIR, bake } });
    expect([askedKinds(workers), pool.queuedCount]).toEqual([
      ["disc-band-bake", "disc-band-bake"],
      2,
    ]);
    disc.cancel();
    expect([await disc.outcome, pool.queuedCount]).toEqual([{ kind: "cancelled" }, 0]);
  });

  it("asks a disc of one band whose routed bake has landed in one disc-bake, as a whole table", () => {
    const { pool, workers } = scriptedPool(2);
    const thick = sphereAsk();
    const bake = patternedThickBake(thickBakeBands(thick.body), R05_EARTH_REFERENCE, 1);
    pool.bakeDisc({ ...thick, air: { ...AIR, bake } });
    expect(workers.map((worker) => worker.received.map((request) => request.kind))).toEqual([
      ["disc-bake"],
      [],
    ]);
  });

  it("spreads the J of a disc of one band whose routed bake has not landed over its bins first", async () => {
    const { pool, workers } = scriptedPool(2);
    const thick = sphereAsk();
    const disc = pool.bakeDisc({ ...thick, air: AIR });
    expect(askedKinds(workers)).toEqual(["thick-bake-bins", "thick-bake-bins"]);
    disc.cancel();
    expect(await disc.outcome).toEqual({ kind: "cancelled" });
  });
});

describe("a disc's table of its bands' shares", () => {
  it("lays each band's layers in the bands' order", () => {
    const table = discTableOfBands([bandValues(0, false), bandValues(1, false)]);
    expect([
      table.bands,
      table.slots,
      table.sunFloor,
      table.thermalScale,
      table.values.length,
    ]).toEqual([2, 1, -0.25, undefined, 2 * DISC_LAYER_VALUES]);
    expect([
      table.values.subarray(0, DISC_LAYER_VALUES),
      table.values.subarray(DISC_LAYER_VALUES),
    ]).toEqual([bandValues(0, false).lit, bandValues(1, false).lit]);
  });

  it("holds every band's glow over the largest magnitude of all, the same at every sun row and azimuth, signed", () => {
    const parts = [bandValues(0, true), bandValues(1, true)];
    const table = discTableOfBands(parts);
    // Band 1's glow at v = 15, c = 2 is 2 × 48: the largest.
    const scale = 2 * DISC_VIEW_COSINES * 3;
    const texel = (b: number, s: number, v: number, k: number): number[] =>
      Array.from(
        table.values.subarray(discTexelIndex(2 + b, s, v, k), discTexelIndex(2 + b, s, v, k) + 4),
      );
    const expected = (b: number, v: number): number[] =>
      Array.from(
        discTableValuesOf(
          Float32Array.of(
            (parts[b]?.glow?.[v * 3] ?? Number.NaN) / scale,
            (parts[b]?.glow?.[v * 3 + 1] ?? Number.NaN) / scale,
            (parts[b]?.glow?.[v * 3 + 2] ?? Number.NaN) / scale,
            0,
          ),
        ),
      );
    expect([table.thermalScale, table.values.length]).toEqual([scale, 4 * DISC_LAYER_VALUES]);
    expect([
      texel(1, DISC_SUN_COSINES - 1, DISC_VIEW_COSINES - 1, DISC_AZIMUTHS - 1),
      texel(0, 0, 3, 0),
      texel(0, 5, 3, 7),
    ]).toEqual([expected(1, DISC_VIEW_COSINES - 1), expected(0, 3), expected(0, 3)]);
    expect(texel(0, 0, 3, 0)[1]).toBeLessThan(0);
  });

  it("holds no thermal layer where every band's glow is 0", () => {
    const dark = { ...bandValues(0, false), glow: new Float64Array(DISC_VIEW_COSINES * 3) };
    const table = discTableOfBands([dark, dark]);
    expect([table.thermalScale, table.values.length]).toEqual([undefined, 2 * DISC_LAYER_VALUES]);
  });

  it.each([
    ["no band", []],
    ["bands of other light slots", [bandValues(0, false), { ...bandValues(1, false), slots: 2 }]],
    [
      "bands of other sun floors",
      [bandValues(0, false), { ...bandValues(1, false), sunFloor: -0.5 }],
    ],
    ["a band with a glow and one without", [bandValues(0, true), bandValues(1, false)]],
  ] satisfies ReadonlyArray<readonly [string, ReadonlyArray<DiscBandValues>]>)(
    "refuses %s",
    (_, parts) => {
      expect(() => discTableOfBands(parts)).toThrow(RangeError);
    },
  );
});
