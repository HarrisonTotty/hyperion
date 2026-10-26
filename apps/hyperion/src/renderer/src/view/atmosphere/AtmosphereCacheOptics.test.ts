/**
 * The atmosphere cache's optics workers (plan R08, R08.T10.a): a worker's answers
 * (`opticsJobs.ts`), their transfer, and the pool that splits a body's curves of growth between
 * its workers (`opticsPool.ts`; `decision-r08-t4d-kdist-chain.md` §15, ruling 3). The workers here
 * are the real job logic run in process, on toy line tables, or scripted for their failures.
 */
import { describe, expect, it } from "vitest";

import {
  earthBody,
  planckBakeSpectrum,
  r05EarthWithOzoneAbsorber,
} from "../../test/atmosphereFixtures";
import { detailOf, recordInput, SUN_LIGHT } from "../../test/atmosphereRecords";
import { InProcessOpticsWorker } from "../../test/inProcessOpticsWorker";
import { toyPart, toySidecar } from "../../test/kDistributionFixtures";
import {
  earthDetail,
  earthDetailOfAnOlderServer,
  FIXTURE_SYSTEM,
} from "../../test/planetaryFixture";
import { ScriptedOpticsWorker } from "../../test/scriptedOpticsWorker";
import {
  type AbsorberCurve,
  type AbsorberCurveSums,
  absorberCurvesBySun,
  type AbsorberSpec,
} from "./absorbers";
import { type KDistributionTable, readKDistribution } from "./absorbers/kDistribution";
import type { AssembledMedium } from "./assemble";
import type { BodyAtmosphereInput, BodyLight } from "./atmosphereInput";
import { OpticsJobs, type OpticsJobsDeps } from "./opticsJobs";
import { type OpticsRequest, transferablesOf } from "./opticsMessages";
import {
  defaultOpticsWorkers,
  MAX_CONSECUTIVE_OPTICS_FAILURES,
  type OpticsBuildOutcome,
  OpticsWorkerPool,
} from "./opticsPool";

/** The toy's grid: the stored pressures' first span, 4 a decade from 10 Pa to 10 MPa. */
const PRESSURES_PA = Array.from({ length: 25 }, (_, k) => 10 * 10 ** (k / 4));
const TEMPERATURES_K = [150, 200, 250, 300, 350];

/** A toy σ at a node and g-point, m², strong enough to absorb in an Earth's column. */
function toySigma(t: number, p: number, g: number): number {
  return (g === 0 ? 2e-30 : 6e-27) * (t / 250) * (g === 0 ? 1 : 1 / Math.sqrt(1 + p / 1e3));
}

/** A toy line table for `species`: cells 690–694 nm, each a strong and a weak g-point. */
function toyTable(species: string): KDistributionTable {
  const sigmasM2 = TEMPERATURES_K.flatMap((t) =>
    PRESSURES_PA.flatMap((p) => [toySigma(t, p, 0), toySigma(t, p, 1)]),
  );
  const cells = [690, 691, 692, 693, 694].map((nm) => ({ nm, weights: [0.8, 0.2], sigmasM2 }));
  const sidecar = toySidecar(TEMPERATURES_K, PRESSURES_PA, cells.length, {
    species,
    measuredRangeK: [150, 296],
    broadening: { self: 1.5 },
  });
  const part = toyPart(cells, {
    variants: 1,
    temperatures: TEMPERATURES_K.length,
    pressures: PRESSURES_PA.length,
  });
  return readKDistribution(sidecar, [part], `toy ${species}`);
}

/** A worker's k-table reader of toy tables, counting each species it is asked for. */
function toyLoader(asked: string[] = []): OpticsJobsDeps {
  return {
    loadKDistributions: async (species) => {
      asked.push(...species);
      return Promise.resolve(species.map((key) => toyTable(key)));
    },
  };
}

/** A second star, cooler than the Sun. */
const RED_DWARF_LIGHT: BodyLight = {
  star: `${FIXTURE_SYSTEM}.0101`,
  spectrum: planckBakeSpectrum(3_200),
};

/** The generated Earth (the wire fixture's), its water's and its CO₂'s lines on the toy tables. */
function earthInput(lights: ReadonlyArray<BodyLight> = [SUN_LIGHT]): BodyAtmosphereInput {
  return recordInput(detailOf(earthDetail()), lights);
}

/** The largest difference of two curves' optical depths, relative where a depth passes 1. */
function worstDepth(a: AbsorberCurve, b: AbsorberCurve): number {
  let worst = 0;
  for (const [c, depths] of a.opticalDepth.entries()) {
    for (const [i, value] of depths.entries()) {
      const other = b.opticalDepth[c]?.[i] ?? Number.NaN;
      worst = Math.max(worst, Math.abs(value - other) / Math.max(Math.abs(other), 1));
    }
  }
  return worst;
}

/** A build's assembly, or a test failure. */
function builtOf(outcome: OpticsBuildOutcome): AssembledMedium {
  if (outcome.kind !== "built") {
    throw new Error(
      `the build ended ${outcome.kind}${outcome.kind === "failed" ? `: ${outcome.message}` : ""}`,
    );
  }
  return outcome.assembled;
}

/** A pool of `count` in-process workers on toy tables, logging every request posted. */
function inProcessPool(count: number): {
  readonly pool: OpticsWorkerPool;
  readonly log: OpticsRequest[];
  readonly workers: InProcessOpticsWorker[];
  readonly asked: string[][];
} {
  const log: OpticsRequest[] = [];
  const workers: InProcessOpticsWorker[] = [];
  const asked: string[][] = [];
  const pool = new OpticsWorkerPool({
    workers: count,
    createWorker: () => {
      const species: string[] = [];
      asked.push(species);
      const worker = new InProcessOpticsWorker(toyLoader(species), log);
      workers.push(worker);
      return worker;
    },
  });
  return { pool, log, workers, asked };
}

/** A pool of `count` scripted workers, the replacements among them. */
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
  return { pool, workers };
}

/** The worker at `index`, or a test failure. */
function workerAt(
  workers: ReadonlyArray<ScriptedOpticsWorker>,
  index: number,
): ScriptedOpticsWorker {
  const worker = workers[index];
  if (worker === undefined) {
    throw new Error(`no worker ${index} was started`);
  }
  return worker;
}

/** The request a worker was last posted, or a test failure. */
function lastPosted(worker: ScriptedOpticsWorker): OpticsRequest {
  const request = worker.received.at(-1);
  if (request === undefined) {
    throw new Error("the worker was posted nothing");
  }
  return request;
}

/** Resolves after `hops` turns of the microtask queue. */
async function microtasks(hops: number): Promise<void> {
  return hops === 0 ? Promise.resolve() : Promise.resolve().then(async () => microtasks(hops - 1));
}

const KEPT_UNSET: BodyAtmosphereInput = { kind: "kept", atmosphere: { kind: "unset" } };

describe("an optics worker's answers", () => {
  it("assembles a body with its line tables, the toy's absorbing", async () => {
    const jobs = new OpticsJobs(toyLoader());
    const { reply } = await jobs.answer({ kind: "assemble", id: 1, input: earthInput() });
    if (reply.kind !== "assembled" || reply.assembled.drawn === undefined) {
      throw new Error(`the Earth was answered ${reply.kind} with no medium`);
    }
    const lined = reply.assembled.drawn.absorbers.filter(
      (spec) => spec.kind === "column" && spec.species.some((s) => s.lines !== undefined),
    );
    expect([lined.length > 0, reply.assembled.drawn.curves]).toEqual([true, []]);
  });

  it.each([
    ["an assembly", { kind: "assemble", id: 1, input: earthInput() }],
    [
      "a share of the curves",
      {
        kind: "curve-sums",
        id: 2,
        input: earthInput(),
        slots: [0],
        share: { index: 1, count: 2 },
      },
    ],
  ] satisfies ReadonlyArray<readonly [string, OpticsRequest]>)(
    "transfers every buffer of %s, keeping its own: a second answer equals the first's copy",
    async (_, request) => {
      const jobs = new OpticsJobs(toyLoader());
      const first = await jobs.answer(request);
      const copy = structuredClone(first.reply, { transfer: first.transfer });
      expect([
        first.transfer.length > 0,
        first.transfer.every((buffer) => buffer.byteLength === 0),
      ]).toEqual([true, true]);
      expect(copy).toEqual((await jobs.answer(request)).reply);
    },
  );

  it("transfers a join's curves, joined from sums that crossed to it transferred", async () => {
    const jobs = new OpticsJobs(toyLoader());
    const input = earthInput();
    const share = await jobs.answer({
      kind: "curve-sums",
      id: 1,
      input,
      slots: [0],
      share: { index: 0, count: 1 },
    });
    if (share.reply.kind !== "curve-sums") {
      throw new Error(`the share was answered ${share.reply.kind}`);
    }
    const sums = [structuredClone(share.reply.sums, { transfer: share.transfer })];
    const join = await jobs.answer({ kind: "curve-join", id: 2, input, slots: [0], sums });
    const copy = structuredClone(join.reply, { transfer: join.transfer });
    expect([copy.kind, join.transfer.every((buffer) => buffer.byteLength === 0)]).toEqual([
      "curves",
      true,
    ]);
  });

  it("loads each line species' table once a worker, however many jobs ask for it", async () => {
    const asked: string[] = [];
    const jobs = new OpticsJobs(toyLoader(asked));
    await jobs.answer({ kind: "assemble", id: 1, input: earthInput() });
    await jobs.answer({ kind: "assemble", id: 2, input: earthInput([SUN_LIGHT, RED_DWARF_LIGHT]) });
    await jobs.answer({
      kind: "curve-sums",
      id: 3,
      input: earthInput(),
      slots: [0],
      share: { index: 0, count: 1 },
    });
    expect([asked.length > 0, asked]).toEqual([true, [...new Set(asked)]]);
  });

  it("answers a job that throws as failed, with its message", async () => {
    const jobs = new OpticsJobs(toyLoader());
    const lights = [SUN_LIGHT, RED_DWARF_LIGHT, SUN_LIGHT];
    const { reply } = await jobs.answer({ kind: "assemble", id: 7, input: earthInput(lights) });
    expect(reply).toEqual({ kind: "failed", id: 7, message: expect.stringMatching(/light/u) });
  });

  it("rejects where a k-table cannot be read, as the worker's error, and reads it afresh after", async () => {
    let failing = true;
    const jobs = new OpticsJobs({
      loadKDistributions: async (species) => {
        if (failing) {
          throw new Error("the part could not be fetched");
        }
        return Promise.resolve(species.map((key) => toyTable(key)));
      },
    });
    const request: OpticsRequest = { kind: "assemble", id: 1, input: earthInput() };
    await expect(jobs.answer(request)).rejects.toThrow(/fetched/u);
    failing = false;
    expect((await jobs.answer(request)).reply.kind).toBe("assembled");
  });
});

describe("transferablesOf", () => {
  it("lists each buffer once, however many arrays view it, and none of a shared buffer", () => {
    const buffer = new ArrayBuffer(16);
    const value = {
      a: new Float64Array(buffer, 0, 1),
      b: [new Float64Array(buffer, 8, 1)],
      c: new Map([["k", new Float32Array(2)]]),
      d: new Int32Array(new SharedArrayBuffer(8)),
    };
    const lengths = transferablesOf(value).map((each) => each.byteLength);
    expect(lengths.toSorted((a, b) => a - b)).toEqual([8, 16]);
  });
});

describe("the optics-worker pool", () => {
  it.each([1, 2, 3])(
    "joins %i shares a light set into each light's whole curves (to 10⁻¹² in τ)",
    async (count) => {
      const { pool } = inProcessPool(count);
      const lights = [SUN_LIGHT, RED_DWARF_LIGHT];
      const assembled = builtOf(await pool.build(earthInput(lights)).outcome);
      pool.terminate();
      if (assembled.drawn === undefined) {
        throw new Error("the Earth drew no medium");
      }
      const { absorbers, spectra, curves } = assembled.drawn;
      const whole = absorbers.map((spec: AbsorberSpec) => absorberCurvesBySun(spec, spectra));
      let worst = 0;
      for (const [j, slot] of curves.entries()) {
        for (const [k, curve] of slot.entries()) {
          const reference = whole[k]?.[j]?.channels;
          worst =
            reference === undefined ? Number.NaN : Math.max(worst, worstDepth(curve, reference));
        }
      }
      expect([curves.length, curves.every((slot) => slot.length === absorbers.length)]).toEqual([
        2,
        true,
      ]);
      expect(worst).toBeLessThan(1e-12);
    },
  );

  it("asks for the brightest light's shares first and alone, then the others', behind the assembly", async () => {
    const { pool, log } = inProcessPool(3);
    builtOf(await pool.build(earthInput([SUN_LIGHT, RED_DWARF_LIGHT])).outcome);
    pool.terminate();
    const asked = log.map((request) =>
      request.kind === "curve-sums"
        ? `sums ${request.slots.join("+")} ${request.share.index}/${request.share.count}`
        : request.kind,
    );
    expect(asked.slice(0, 7)).toEqual([
      "assemble",
      "sums 0 0/3",
      "sums 0 1/3",
      "sums 0 2/3",
      "sums 1 0/3",
      "sums 1 1/3",
      "sums 1 2/3",
    ]);
  });

  it("loads each line species once a worker over many builds", async () => {
    const { pool, asked } = inProcessPool(2);
    builtOf(await pool.build(earthInput()).outcome);
    builtOf(await pool.build(earthInput([SUN_LIGHT, RED_DWARF_LIGHT])).outcome);
    pool.terminate();
    expect([
      asked.some((species) => species.length > 0),
      asked.every((species) => species.length === new Set(species).size),
    ]).toEqual([true, true]);
  });

  it("lands a body that draws no medium with no curves, joining none", async () => {
    const { pool, log } = inProcessPool(2);
    const input = recordInput(detailOf(earthDetailOfAnOlderServer()), [SUN_LIGHT, RED_DWARF_LIGHT]);
    const assembled = builtOf(await pool.build(input).outcome);
    pool.terminate();
    expect([assembled.medium, assembled.labels, log.some((r) => r.kind === "curve-join")]).toEqual([
      undefined,
      ["atmosphereNotYetModelled"],
      false,
    ]);
  });

  it("lands a medium with no absorber with an empty curve list a light", async () => {
    const { pool, workers } = scriptedPool(1);
    const build = pool.build(earthInput());
    const worker = workerAt(workers, 0);
    const { medium } = r05EarthWithOzoneAbsorber();
    worker.reply({
      kind: "assembled",
      id: lastPosted(worker).id,
      assembled: {
        medium: { ...medium, terms: medium.terms.filter((term) => term.absorber === undefined) },
        drawn: {
          body: earthBody(),
          bottomRadiusM: 6_371_008.8,
          absorbers: [],
          spectra: [SUN_LIGHT.spectrum],
          curves: [],
        },
        labels: [],
        reasons: [],
        negligible: [],
      },
    });
    expect(builtOf(await build.outcome).drawn?.curves).toEqual([[]]);
    pool.terminate();
  });

  it("fails a build whose job throws, with the job's message", async () => {
    const { pool } = inProcessPool(2);
    const outcome = await pool.build(earthInput([SUN_LIGHT, RED_DWARF_LIGHT, SUN_LIGHT])).outcome;
    pool.terminate();
    expect(outcome).toEqual({ kind: "failed", message: expect.stringMatching(/light/u) });
  });

  it("replaces a worker that fails and asks its job again", async () => {
    const { pool, workers } = scriptedPool(2);
    const build = pool.build(KEPT_UNSET);
    workerAt(workers, 0).fail();
    const replacement = workerAt(workers, 2);
    const asked = lastPosted(replacement);
    expect([workerAt(workers, 0).terminated, asked.kind, asked.input]).toEqual([
      true,
      "assemble",
      KEPT_UNSET,
    ]);
    replacement.reply({ kind: "assembled", id: asked.id, assembled: NO_MEDIUM });
    expect((await build.outcome).kind).toBe("built");
    pool.terminate();
  });

  it(`gives a worker's place up after ${MAX_CONSECUTIVE_OPTICS_FAILURES} failures in a row, failing every build once none runs`, async () => {
    const { pool, workers } = scriptedPool(1);
    const build = pool.build(KEPT_UNSET);
    for (let n = 0; n < MAX_CONSECUTIVE_OPTICS_FAILURES; n += 1) {
      workerAt(workers, n).fail();
    }
    expect([workers.length, pool.lostWorkers, await build.outcome]).toEqual([
      MAX_CONSECUTIVE_OPTICS_FAILURES,
      1,
      { kind: "failed", message: "every optics worker was given up" },
    ]);
    expect((await pool.build(KEPT_UNSET).outcome).kind).toBe("failed");
    pool.terminate();
  });

  it("asks a light set's shares again when the join's worker fails with their sums", async () => {
    const { pool, workers } = scriptedPool(1);
    const { medium, curves } = r05EarthWithOzoneAbsorber();
    const ozone = medium.terms.find((term) => term.absorber !== undefined)?.absorber?.spec;
    const curve = curves[0]?.[0];
    if (ozone === undefined || curve === undefined) {
      throw new Error("R05's Earth with ozone has its absorber and its curve");
    }
    const build = pool.build(earthInput());
    const first = workerAt(workers, 0);
    first.reply({
      kind: "assembled",
      id: lastPosted(first).id,
      assembled: absorbing(ozone, medium),
    });
    first.reply({ kind: "curve-sums", id: lastPosted(first).id, sums: [toySums()] });
    await microtasks(10);
    expect(lastPosted(first).kind).toBe("curve-join");
    first.fail();
    await microtasks(10);
    const second = workerAt(workers, 1);
    expect(lastPosted(second).kind).toBe("curve-sums");
    second.reply({ kind: "curve-sums", id: lastPosted(second).id, sums: [toySums()] });
    await microtasks(10);
    second.reply({ kind: "curves", id: lastPosted(second).id, curves: [[curve]] });
    expect(builtOf(await build.outcome).drawn?.curves).toEqual([[curve]]);
    pool.terminate();
  });

  it("drops a cancelled build's jobs not yet sent", async () => {
    const { pool } = scriptedPool(1);
    pool.build(KEPT_UNSET);
    const second = pool.build(KEPT_UNSET);
    second.cancel();
    expect([pool.queuedCount, await second.outcome]).toEqual([0, { kind: "cancelled" }]);
    pool.terminate();
  });

  it("terminates every worker, its builds cancelled", async () => {
    const { pool, workers } = scriptedPool(2);
    const build = pool.build(KEPT_UNSET);
    pool.terminate();
    expect([workers.map((worker) => worker.terminated), await build.outcome]).toEqual([
      [true, true],
      { kind: "cancelled" },
    ]);
  });

  it("starts no worker before its first build", () => {
    const { pool, workers } = scriptedPool(3);
    expect([pool.workerCount, workers.length]).toEqual([3, 0]);
    pool.terminate();
  });

  it.each([
    [Number.NaN, 1],
    [1, 1],
    [4, 2],
    [8, 4],
    [16, 4],
    [32, 4],
  ])("runs clamp(⌊%d threads ÷ 2⌋, 1, 4) = %d workers by default", (threads, workers) => {
    expect(defaultOpticsWorkers(threads)).toBe(workers);
  });
});

/** An assembly that draws no medium. */
const NO_MEDIUM: AssembledMedium = {
  medium: undefined,
  drawn: undefined,
  labels: ["atmosphereNotYetModelled"],
  reasons: [],
  negligible: [],
};

/** An assembly of `medium` drawing `spec`, one light. */
function absorbing(spec: AbsorberSpec, medium: AssembledMedium["medium"]): AssembledMedium {
  if (medium === undefined) {
    throw new Error("a medium is needed");
  }
  return {
    medium,
    drawn: {
      body: earthBody(),
      bottomRadiusM: 6_371_008.8,
      absorbers: [spec],
      spectra: [SUN_LIGHT.spectrum],
      curves: [],
    },
    labels: [],
    reasons: [],
    negligible: [],
  };
}

/** One share's sums, as placeholders a scripted worker answers. */
function toySums(): AbsorberCurveSums {
  return {
    share: { index: 0, count: 1 },
    channels: [[new Float64Array(1), new Float64Array(1), new Float64Array(1)]],
    bins: undefined,
    binLogs: undefined,
  };
}
