/**
 * The atmosphere cache (plan R08, R08.T10.a; `decision-r08-t10a-leans.md` items 3, 4, 6, 8–10 and
 * the follow-up to item 6): its requests and their triggers, `ATMOSPHERE: PENDING` and its hold,
 * the answers it cannot draw, failed builds, its size, kept scenes, its views and its disposal.
 * The boundaries are faked: a real `RequestClient` over a recording send, a scripted builder in
 * the optics pool's place, a fake tables factory and fake timers; the last test runs the real pool
 * on in-process workers.
 */
import type { BodyIdHex } from "@hyperion/protocol";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { toBodyDetail } from "../../lib/system/bodiesWire";
import { earthBody, venusClassMedium } from "../../test/atmosphereFixtures";
import { type DetailAnswer, SUN_LIGHT, withRecord } from "../../test/atmosphereRecords";
import { BodyDetailSeam } from "../../test/bodyDetailSeam";
import { InProcessOpticsWorker } from "../../test/inProcessOpticsWorker";
import { toyPart, toySidecar } from "../../test/kDistributionFixtures";
import {
  earthDetail,
  earthDetailOfAnOlderServer,
  FIXTURE_EARTH,
  FIXTURE_SYSTEM,
  FIXTURE_UNIVERSE,
} from "../../test/planetaryFixture";
import { RENDERER_ROOT, runTimeGraph } from "../../test/runTimeGraph";
import { between, seededRandom } from "../../test/seededRandom";
import { heldAppearanceOf } from "../appearance/fromWire";
import type { LitRegime } from "../bodies/regime";
import { EngineUnavailable } from "../engine/resilientEngine";
import { ANNUNCIATION_CLEAR_MS, ANNUNCIATION_ONSET_MS } from "../terrain/annunciation";
import { readKDistribution } from "./absorbers/kDistribution";
import { type AssembledMedium, assembleMedium } from "./assemble";
import {
  ATMOSPHERE_CACHE_BYTES,
  AtmosphereCache,
  type AtmosphereFrameBody,
  type AtmosphereScene,
  type AtmosphereView,
  type AtmosphereViewReport,
  type BuiltAtmosphere,
  type CachedTables,
  type KeptBodyAtmosphere,
  PendingLineHold,
} from "./AtmosphereCache";
import {
  atmosphereKey,
  type BodyAtmosphereInput,
  type BodyLight,
  clampedGroundAlbedo,
} from "./atmosphereInput";
import { R05_EARTH_REFERENCE } from "./earth";
import { ATMOSPHERE_ANSWER_UNREADABLE, type AtmosphereLabel } from "./labels";
import {
  type AtmosphereBuilder,
  type OpticsBuild,
  type OpticsBuildOutcome,
  OpticsWorkerPool,
} from "./opticsPool";
import { perPlanetRowOf, tablesPlanOf } from "./tables";

/** A body's per-planet tables as the fake factory makes them, with the bytes it reports. */
class FakeTables implements CachedTables {
  readonly built: BuiltAtmosphere;
  readonly rowBytes: number;
  disposed = false;

  constructor(built: BuiltAtmosphere, rowBytes: number) {
    this.built = built;
    this.rowBytes = rowBytes;
  }

  dispose(): void {
    this.disposed = true;
  }
}

/** A build the scripted builder holds, landed by the test. */
interface ScriptedBuild {
  readonly input: BodyAtmosphereInput;
  readonly key: string;
  settled: boolean;
  readonly settle: (outcome: OpticsBuildOutcome) => void;
}

/** Takes nothing: a resolver's stand-in until its promise gives it its own. */
function ignore(): void {}

/** A builder in the optics pool's place, whose builds the test lands, fails or leaves running. */
class ScriptedBuilder implements AtmosphereBuilder {
  readonly builds: ScriptedBuild[] = [];
  terminated = false;

  build(input: BodyAtmosphereInput): OpticsBuild {
    let resolve: (outcome: OpticsBuildOutcome) => void = ignore;
    const outcome = new Promise<OpticsBuildOutcome>((settleWith) => {
      resolve = settleWith;
    });
    const build: ScriptedBuild = {
      input,
      key: atmosphereKey(input),
      settled: false,
      settle: (result) => {
        build.settled = true;
        resolve(result);
      },
    };
    this.builds.push(build);
    return {
      outcome,
      cancel: () => {
        build.settle({ kind: "cancelled" });
      },
    };
  }

  terminate(): void {
    this.terminated = true;
    for (const build of this.builds) {
      build.settle({ kind: "cancelled" });
    }
  }

  /** The builds asked for `body`, oldest first. */
  buildsOf(body: BodyIdHex): ScriptedBuild[] {
    return this.builds.filter(
      (build) => build.input.kind === "record" && build.input.detail.record.id === body,
    );
  }

  /**
   * The latest build of `body`.
   *
   * @throws Error where none was asked: a test's bug.
   */
  latestOf(body: BodyIdHex): ScriptedBuild {
    const build = this.buildsOf(body).at(-1);
    if (build === undefined) {
      throw new Error(`no build was asked for ${body}`);
    }
    return build;
  }
}

/** What a test drives. */
interface Harness {
  readonly seam: BodyDetailSeam;
  readonly builder: ScriptedBuilder;
  readonly made: FakeTables[];
  readonly cache: AtmosphereCache<FakeTables>;
  readonly view: AtmosphereView;
  /** The fake factory's behaviour: what it throws while set. */
  readonly factory: { throws: Error | null };
}

/** A cache over the fakes, its tables reporting `rowBytes` each, or their per-planet row's. */
function harness(rowBytes?: number): Harness {
  const seam = new BodyDetailSeam();
  const builder = new ScriptedBuilder();
  const made: FakeTables[] = [];
  const factory: { throws: Error | null } = { throws: null };
  const cache = new AtmosphereCache<FakeTables>({
    requests: seam.client,
    builder,
    tables: (built) => {
      if (factory.throws !== null) {
        throw factory.throws;
      }
      const row = perPlanetRowOf(
        built.medium,
        built.curves,
        tablesPlanOf(built.body, built.bottomRadiusM),
      );
      const tables = new FakeTables(built, rowBytes ?? row.baseBytes);
      made.push(tables);
      return tables;
    },
  });
  return { seam, builder, made, cache, view: cache.openView(), factory };
}

/** The scene's designation of record. */
const DESIGNATION = "ATMOSPHERE TEST SYSTEM";

/** The scene's arrival, by identity. */
const ARRIVAL = {};

/** The fixture's scene, its link up. */
function sceneOf(change: Partial<AtmosphereScene> = {}): AtmosphereScene {
  return {
    universe: FIXTURE_UNIVERSE,
    system: FIXTURE_SYSTEM,
    designation: DESIGNATION,
    time: earthDetail().time,
    arrival: ARRIVAL,
    grants: new Map(),
    linkUp: true,
    ...change,
  };
}

/** A body of the fixture's system other than its Earth. */
function bodyId(n: number): BodyIdHex {
  return `${FIXTURE_SYSTEM}.${(0x0400 + n).toString(16).padStart(4, "0")}`;
}

/** A frame's body in `regime`, lit by the Sun. */
function lit(
  id: BodyIdHex,
  regime: LitRegime = "disc",
  lights: ReadonlyArray<BodyLight> = [SUN_LIGHT],
): AtmosphereFrameBody {
  return { id, regime, lights };
}

/** A view's frame of `bodies`, at `nowMs`, in `scene`. */
function note(
  h: Harness,
  bodies: ReadonlyArray<AtmosphereFrameBody>,
  nowMs = 0,
  scene: AtmosphereScene | null = sceneOf(),
): AtmosphereViewReport {
  return h.view.noteFrame({ nowMs, scene, bodies });
}

/** The fixture's answer for `body`: an older server's Earth, its surface not modelled. */
function answerFor(
  body: BodyIdHex,
  base: DetailAnswer = earthDetailOfAnOlderServer(),
): DetailAnswer {
  return withRecord(base, (record) => ({ ...record, id: body }));
}

/** Resolves after `hops` turns of the microtask queue. */
async function microtasks(hops = 20): Promise<void> {
  return hops === 0 ? Promise.resolve() : Promise.resolve().then(async () => microtasks(hops - 1));
}

/** Answers `body`'s latest request and lets the answer be taken. */
async function answer(h: Harness, body: BodyIdHex, answered = answerFor(body)): Promise<void> {
  h.seam.answer(h.seam.latestFor(body).id, answered);
  await microtasks();
}

/** Lands `body`'s latest build and lets it be taken. */
async function land(h: Harness, body: BodyIdHex, outcome: OpticsBuildOutcome): Promise<void> {
  h.builder.latestOf(body).settle(outcome);
  await microtasks();
}

/** An assembly that draws no medium, as a generated body's is until P14.T24.a. */
const NOT_MODELLED: AssembledMedium = {
  medium: undefined,
  drawn: undefined,
  labels: ["atmosphereNotYetModelled"],
  reasons: [],
  negligible: [],
};

/** R05's Earth as an assembly, one light and no absorber. */
function earthAssembly(labels: ReadonlyArray<AtmosphereLabel> = []): AssembledMedium {
  return {
    medium: R05_EARTH_REFERENCE,
    drawn: {
      body: earthBody(),
      bottomRadiusM: 6_371_008.8,
      absorbers: [],
      spectra: [SUN_LIGHT.spectrum],
      curves: [[]],
    },
    labels,
    reasons: [],
    negligible: [],
  };
}

/** Asks for, answers, builds and draws `bodies` in one view's frames, at `nowMs`. */
async function drawn(
  h: Harness,
  bodies: ReadonlyArray<AtmosphereFrameBody>,
  assembled: AssembledMedium = earthAssembly(),
  nowMs = 0,
): Promise<AtmosphereViewReport> {
  note(h, bodies, nowMs);
  for (const { id } of bodies) {
    h.seam.answer(h.seam.latestFor(id).id, answerFor(id));
  }
  await microtasks();
  for (const { id } of bodies) {
    h.builder.latestOf(id).settle({ kind: "built", assembled });
  }
  await microtasks();
  return note(h, bodies, nowMs);
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

const A = FIXTURE_EARTH;

describe("the atmosphere cache's requests", () => {
  it("sends one body_detail for each body first wanted in the disc, mesh or terrain regime, and none for a point", () => {
    const h = harness();
    const bodies = [lit(bodyId(1)), lit(bodyId(2), "mesh"), lit(bodyId(3), "terrain")];
    const point = lit(bodyId(4), "point");
    note(h, [...bodies, point]);
    note(h, [...bodies, point]);
    expect([1, 2, 3, 4].map((n) => h.seam.askedFor(bodyId(n)))).toEqual([1, 1, 1, 0]);
  });

  it("asks again once when the scene's arrival changes", async () => {
    const h = harness();
    await drawn(h, [lit(A)]);
    const arrival = sceneOf({ arrival: {} });
    note(h, [lit(A)], 0, arrival);
    note(h, [lit(A)], 0, arrival);
    expect(h.seam.askedFor(A)).toBe(2);
  });

  it("asks again once when the body's granted detail changes", async () => {
    const h = harness();
    note(h, [lit(A)], 0, sceneOf({ grants: new Map([[A, "surface"]]) }));
    await answer(h, A);
    const granted = sceneOf({ grants: new Map([[A, "full"]]) });
    note(h, [lit(A)], 0, granted);
    note(h, [lit(A)], 0, granted);
    expect(h.seam.askedFor(A)).toBe(2);
  });

  it("asks again once at the next frame after an event on the body", async () => {
    const h = harness();
    await drawn(h, [lit(A)]);
    h.cache.noteBodyEvent(A);
    note(h, [lit(A)]);
    note(h, [lit(A)]);
    expect(h.seam.askedFor(A)).toBe(2);
  });

  it("waits while the link is down and asks again once it is back up after a lost link", async () => {
    const h = harness();
    note(h, [lit(A)]);
    h.seam.loseLink();
    await microtasks();
    note(h, [lit(A)], 0, sceneOf({ linkUp: false }));
    const whileDown = h.seam.askedFor(A);
    note(h, [lit(A)]);
    note(h, [lit(A)]);
    expect([whileDown, h.seam.askedFor(A), h.cache.labelsOf(A)]).toEqual([
      1,
      2,
      ["atmospherePending"],
    ]);
  });

  it("rebuilds from the reply held, with no request, when the body's lights change", async () => {
    const h = harness();
    await drawn(h, [lit(A)]);
    const second: BodyLight = { star: `${FIXTURE_SYSTEM}.0101`, spectrum: SUN_LIGHT.spectrum };
    note(h, [lit(A, "disc", [SUN_LIGHT, second])]);
    const rebuilt = h.builder.latestOf(A).input;
    expect([
      h.seam.askedFor(A),
      h.builder.buildsOf(A).length,
      rebuilt.kind === "record" ? rebuilt.lights : [],
    ]).toEqual([1, 2, [SUN_LIGHT, second]]);
  });

  it("keeps its entry and builds nothing when a re-request's answer has the drawn entry's key", async () => {
    const h = harness();
    await drawn(h, [lit(A)]);
    const entry = h.cache.entryOf(A);
    note(h, [lit(A)], 0, sceneOf({ arrival: {} }));
    await answer(h, A);
    expect([h.builder.buildsOf(A).length, h.made.length, h.cache.entryOf(A)]).toEqual([
      1,
      1,
      entry,
    ]);
  });

  it("rebuilds when a re-request's answer changes the body's inventory, so its key", async () => {
    const h = harness();
    await drawn(h, [lit(A)]);
    note(h, [lit(A)], 0, sceneOf({ arrival: {} }));
    await answer(h, A, answerFor(A, earthDetail()));
    expect(h.builder.buildsOf(A).map((build) => build.key)).toEqual([
      atmosphereKey(inputOf(answerFor(A))),
      atmosphereKey(inputOf(answerFor(A, earthDetail()))),
    ]);
  });

  it("forms a record's input with its ground's albedo p q per channel, clamped to [0, 1]", async () => {
    const h = harness();
    note(h, [lit(A)]);
    await answer(h, A, earthDetail());
    expect(h.builder.latestOf(A).input).toEqual(inputOf(earthDetail()));
  });
});

/** The input the cache forms from an answer, lit by the Sun, as a test states it. */
function inputOf(answered: DetailAnswer): BodyAtmosphereInput {
  const converted = toBodyDetail(answered, FIXTURE_SYSTEM, DESIGNATION);
  if (converted.kind !== "ok") {
    throw new Error(`the answer does not convert: ${converted.fault}`);
  }
  const { detail } = converted;
  const { geometricAlbedo: p, phaseIntegral: q } = heldAppearanceOf(detail.record).photometry;
  return {
    kind: "record",
    detail,
    lights: [SUN_LIGHT],
    groundAlbedo: clampedGroundAlbedo([p[0] * q[0], p[1] * q[1], p[2] * q[2]]),
  };
}

describe("ATMOSPHERE: PENDING", () => {
  it("is a body's while its request is in flight and while it builds, and its labels once drawn", async () => {
    const h = harness();
    note(h, [lit(A)]);
    const asked = h.cache.labelsOf(A);
    await answer(h, A);
    const building = h.cache.labelsOf(A);
    await land(h, A, { kind: "built", assembled: NOT_MODELLED });
    note(h, [lit(A)]);
    expect([asked, building, h.cache.labelsOf(A)]).toEqual([
      ["atmospherePending"],
      ["atmospherePending"],
      ["atmosphereNotYetModelled"],
    ]);
  });

  it("is none of a body's on a re-request while its previous state is drawn", async () => {
    const h = harness();
    await drawn(h, [lit(A)], NOT_MODELLED);
    const report = note(h, [lit(A)], 0, sceneOf({ arrival: {} }));
    expect([h.seam.askedFor(A), h.cache.labelsOf(A), report.pending]).toEqual([
      2,
      ["atmosphereNotYetModelled"],
      false,
    ]);
  });

  it("shows no line for a body drawn within 250 ms", async () => {
    const h = harness();
    const shown: boolean[] = [];
    shown.push(note(h, [lit(A)], 0).labels.includes("atmospherePending"));
    await answer(h, A);
    await land(h, A, { kind: "built", assembled: NOT_MODELLED });
    for (const nowMs of [200, 249, 400, 2_000]) {
      shown.push(note(h, [lit(A)], nowMs).labels.includes("atmospherePending"));
    }
    expect(shown).toEqual([false, false, false, false, false]);
  });

  it("shows the line once a body has been pending 250 ms, and clears it 1 s after it is drawn", async () => {
    const h = harness();
    const at = (nowMs: number): boolean =>
      note(h, [lit(A)], nowMs).labels.includes("atmospherePending");
    const before = [at(0), at(ANNUNCIATION_ONSET_MS - 1), at(ANNUNCIATION_ONSET_MS)];
    await answer(h, A);
    await land(h, A, { kind: "built", assembled: NOT_MODELLED });
    const drawnAt = 400;
    const after = [
      at(drawnAt),
      at(drawnAt + ANNUNCIATION_CLEAR_MS - 1),
      at(drawnAt + ANNUNCIATION_CLEAR_MS),
    ];
    expect([before, after]).toEqual([
      [false, false, true],
      [true, true, false],
    ]);
  });

  it("comes on at most once in any 1.25 s through a flyby, however the bodies and replies are timed", async () => {
    const h = harness();
    const random = seededRandom(0x0a7e_5f1b);
    const count = 48;
    let t = 0;
    // Bodies come into the disc in bursts and lulls, and most replies are quick, so that a body
    // pending comes and goes many times a second, as a line drawn from it unheld would.
    const flyby = Array.from({ length: count }, (_, n) => {
      t += random() < 0.25 ? between(random, 600, 2_500) : between(random, 10, 300);
      return {
        id: bodyId(n + 1),
        entersMs: t,
        leavesMs: t + between(random, 150, 4_000),
        replyAfterMs: random() < 0.8 ? between(random, 0, 300) : between(random, 300, 2_000),
      };
    });
    const askedAt = new Map<number, number>();
    const answered = new Set<number>();
    const onsets: number[] = [];
    let pendingOnsets = 0;
    let wasPending = false;
    let shown = false;
    const frame = async (nowMs: number): Promise<void> => {
      for (const asked of h.seam.asked) {
        if (!askedAt.has(asked.id)) {
          askedAt.set(asked.id, nowMs);
        }
        const body = flyby.find((each) => each.id === asked.body);
        const since = askedAt.get(asked.id) ?? nowMs;
        if (body !== undefined && !answered.has(asked.id) && nowMs - since >= body.replyAfterMs) {
          answered.add(asked.id);
          h.seam.answer(asked.id, answerFor(asked.body));
        }
      }
      await microtasks();
      for (const build of h.builder.builds) {
        if (!build.settled) {
          build.settle({ kind: "built", assembled: NOT_MODELLED });
        }
      }
      await microtasks();
      const inDisc = flyby.filter((body) => nowMs >= body.entersMs && nowMs < body.leavesMs);
      const report = note(
        h,
        inDisc.map((body) => lit(body.id)),
        nowMs,
      );
      const on = report.labels.includes("atmospherePending");
      if (on && !shown) {
        onsets.push(nowMs);
      }
      if (report.pending && !wasPending) {
        pendingOnsets += 1;
      }
      shown = on;
      wasPending = report.pending;
    };
    for (let nowMs = 0; nowMs < t + 6_000; nowMs += 1_000 / 60) {
      // Each frame waits on the last's answers, as a view's loop does.
      // oxlint-disable-next-line no-await-in-loop
      await frame(nowMs);
    }
    const gaps = onsets.slice(1).map((at, k) => at - (onsets[k] ?? Number.NaN));
    expect([
      onsets.length > 1,
      pendingOnsets > 2 * onsets.length,
      gaps.filter((gap) => !(gap >= ANNUNCIATION_ONSET_MS + ANNUNCIATION_CLEAR_MS)),
    ]).toEqual([true, true, []]);
  });

  it("is held per view, each its own hold, over one request and one entry", async () => {
    const h = harness();
    const other = h.cache.openView();
    note(h, [lit(A)], 0);
    other.noteFrame({ nowMs: 200, scene: sceneOf(), bodies: [lit(A)] });
    const first = note(h, [lit(A)], 300).labels.includes("atmospherePending");
    const second = other
      .noteFrame({ nowMs: 300, scene: sceneOf(), bodies: [lit(A)] })
      .labels.includes("atmospherePending");
    expect([h.seam.askedFor(A), first, second]).toEqual([1, true, false]);
  });
});

describe("PendingLineHold", () => {
  it("shows after 250 ms held and goes after 1 s clear, a short clearing changing nothing", () => {
    const hold = new PendingLineHold();
    const shown = [
      hold.update(true, 0),
      hold.update(true, 249),
      hold.update(true, 250),
      hold.update(false, 300),
      hold.update(true, 900),
      hold.update(false, 1_000),
      hold.update(false, 1_999),
      hold.update(false, 2_000),
    ];
    expect(shown).toEqual([false, false, true, true, true, true, true, false]);
  });
});

describe("a body_detail the cache cannot draw", () => {
  it.each([
    ["unknown_body", true],
    ["internal", false],
  ] as const)(
    "refused %s gives no medium, no label and the rejected problem (refusal %s), with no log",
    async (code, refusal) => {
      const h = harness();
      const errors = vi.spyOn(console, "error").mockImplementation(ignore);
      note(h, [lit(A)]);
      h.seam.refuse(h.seam.latestFor(A).id, code, " the server's reason ");
      await microtasks();
      const report = note(h, [lit(A)]);
      expect([
        h.cache.entryOf(A),
        report.labels,
        report.problems.get(A),
        errors.mock.calls.length,
      ]).toEqual([
        null,
        [],
        { kind: "rejected", code, message: " the server's reason ", refusal },
        0,
      ]);
    },
  );

  it("drops what was drawn when a re-request is refused, disposing its tables", async () => {
    const h = harness();
    await drawn(h, [lit(A)]);
    note(h, [lit(A)], 0, sceneOf({ arrival: {} }));
    h.seam.refuse(h.seam.latestFor(A).id, "unknown_body", "no such body");
    await microtasks();
    expect([h.cache.entryOf(A), h.made[0]?.disposed]).toEqual([null, true]);
  });

  it("holds an answer toBodyDetail cannot take as invalid, logging it once", async () => {
    const h = harness();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    note(h, [lit(A)]);
    await answer(h, A, answerFor("0300080020000000.0300"));
    note(h, [lit(A)]);
    note(h, [lit(A)]);
    expect([h.cache.problemOf(A), h.cache.labelsOf(A), errors.mock.calls.length]).toEqual([
      { kind: "invalid", check: expect.stringMatching(/another system/u) },
      [],
      1,
    ]);
  });

  it("holds an answer of another kind as invalid, its answer unreadable", async () => {
    const h = harness();
    vi.spyOn(console, "error").mockImplementation(ignore);
    note(h, [lit(A)]);
    h.seam.answerWith(h.seam.latestFor(A).id, { kind: "unsubscribe" });
    await microtasks();
    expect(h.cache.problemOf(A)).toEqual({ kind: "invalid", check: ATMOSPHERE_ANSWER_UNREADABLE });
  });

  it.each(["too_many_requests", "queue_full", "cancelled"] as const)(
    "answered %s, raises nothing, keeps PENDING and asks again after 2 s, doubling to 32 s",
    async (code) => {
      const h = harness();
      const errors = vi.spyOn(console, "error").mockImplementation(ignore);
      note(h, [lit(A)]);
      const waits: number[] = [];
      for (let n = 0; n < 6; n += 1) {
        h.seam.refuse(h.seam.latestFor(A).id, code, "busy");
        // Each wait follows the last's answer.
        // oxlint-disable-next-line no-await-in-loop
        await microtasks();
        const before = h.seam.asked.length;
        let waited = 0;
        while (h.seam.asked.length === before && waited < 40_000) {
          vi.advanceTimersByTime(1_000);
          waited += 1_000;
        }
        waits.push(waited);
      }
      expect([waits, h.cache.labelsOf(A), h.cache.problemOf(A), errors.mock.calls.length]).toEqual([
        [2, 4, 8, 16, 32, 32].map((s) => s * 1_000),
        ["atmospherePending"],
        null,
        0,
      ]);
    },
  );

  it("starts its busy waits afresh after an answer", async () => {
    const h = harness();
    note(h, [lit(A)]);
    h.seam.refuse(h.seam.latestFor(A).id, "too_many_requests", "busy");
    await microtasks();
    vi.advanceTimersByTime(2_000);
    h.seam.refuse(h.seam.latestFor(A).id, "too_many_requests", "busy");
    await microtasks();
    vi.advanceTimersByTime(4_000);
    await answer(h, A);
    note(h, [lit(A)], 0, sceneOf({ arrival: {} }));
    h.seam.refuse(h.seam.latestFor(A).id, "too_many_requests", "busy");
    await microtasks();
    const before = h.seam.askedFor(A);
    vi.advanceTimersByTime(2_000);
    expect(h.seam.askedFor(A) - before).toBe(1);
  });

  it("stops a busy wait while no view wants the body, and asks once it is wanted again", async () => {
    const h = harness();
    note(h, [lit(A)]);
    h.seam.refuse(h.seam.latestFor(A).id, "queue_full", "busy");
    await microtasks();
    note(h, []);
    vi.advanceTimersByTime(60_000);
    const whileUnwanted = h.seam.askedFor(A);
    note(h, [lit(A)]);
    expect([whileUnwanted, h.seam.askedFor(A)]).toEqual([1, 2]);
  });

  it("holds a refusal through a re-request at the next trigger, its line standing with no PENDING, until an answer clears it", async () => {
    const h = harness();
    note(h, [lit(A)]);
    h.seam.refuse(h.seam.latestFor(A).id, "internal", "down");
    await microtasks();
    const report = note(h, [lit(A)], 0, sceneOf({ arrival: {} }));
    const during = [h.seam.askedFor(A), report.problems.has(A), report.pending];
    await answer(h, A);
    expect([during, h.cache.problemOf(A), h.cache.labelsOf(A)]).toEqual([
      [2, true, false],
      null,
      ["atmospherePending"],
    ]);
  });

  it("asks afresh once failures are forgotten", async () => {
    const h = harness();
    note(h, [lit(A)]);
    h.seam.refuse(h.seam.latestFor(A).id, "unknown_body", "gone");
    await microtasks();
    note(h, [lit(A)]);
    h.cache.forgetFailures();
    note(h, [lit(A)]);
    expect([h.seam.askedFor(A), h.cache.problemOf(A)]).toEqual([2, null]);
  });
});

describe("a build that fails", () => {
  it("gives no medium, no label, one console.error and the failed problem, standing frame after frame", async () => {
    const h = harness();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    note(h, [lit(A)]);
    await answer(h, A);
    await land(h, A, { kind: "failed", message: "a bug" });
    const reports = [note(h, [lit(A)]), note(h, [lit(A)]), note(h, [lit(A)])];
    expect([
      h.cache.entryOf(A),
      reports.map((report) => [report.labels, report.problems.get(A), report.pending]),
      errors.mock.calls.length,
    ]).toEqual([null, Array.from({ length: 3 }, () => [[], { kind: "failed" }, false]), 1]);
  });

  it("is built again at the body's next trigger, a build that lands clearing it", async () => {
    const h = harness();
    vi.spyOn(console, "error").mockImplementation(ignore);
    note(h, [lit(A)]);
    await answer(h, A);
    await land(h, A, { kind: "failed", message: "a bug" });
    const arrived = sceneOf({ arrival: {} });
    note(h, [lit(A)], 0, arrived);
    const during = h.cache.problemOf(A);
    await answer(h, A);
    await land(h, A, { kind: "built", assembled: NOT_MODELLED });
    note(h, [lit(A)], 0, arrived);
    expect([during, h.cache.problemOf(A), h.cache.labelsOf(A), h.seam.askedFor(A)]).toEqual([
      { kind: "failed" },
      null,
      ["atmosphereNotYetModelled"],
      2,
    ]);
  });

  it("is asked afresh once failures are forgotten", async () => {
    const h = harness();
    vi.spyOn(console, "error").mockImplementation(ignore);
    note(h, [lit(A)]);
    await answer(h, A);
    await land(h, A, { kind: "failed", message: "a bug" });
    h.cache.forgetFailures();
    note(h, [lit(A)]);
    expect([h.seam.askedFor(A), h.cache.labelsOf(A)]).toEqual([2, ["atmospherePending"]]);
  });

  it("fails the same way where the tables' factory throws, once", async () => {
    const h = harness();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    h.factory.throws = new RangeError("more slices than the tables hold");
    await drawn(h, [lit(A)]);
    note(h, [lit(A)]);
    expect([h.cache.problemOf(A), h.made.length, errors.mock.calls.length]).toEqual([
      { kind: "failed" },
      0,
      1,
    ]);
  });

  it("raises nothing where the device is lost, and makes the tables once it is back", async () => {
    const h = harness();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    h.factory.throws = new EngineUnavailable("createTexture");
    await drawn(h, [lit(A)]);
    note(h, [lit(A)]);
    const during = [h.cache.problemOf(A), h.cache.entryOf(A), h.cache.labelsOf(A)];
    h.factory.throws = null;
    note(h, [lit(A)]);
    expect([during, h.made.length, h.cache.entryOf(A)?.tables, errors.mock.calls.length]).toEqual([
      [null, null, ["atmospherePending"]],
      1,
      h.made[0],
      0,
    ]);
  });
});

describe("the cache's size", () => {
  const ROW_BYTES = 2_000_000;
  const BODIES = Array.from({ length: 20 }, (_, n) => bodyId(n + 1));

  it("is its tables' rowBytes, evicting the least recently wanted past 32,000,000 B", async () => {
    const h = harness(ROW_BYTES);
    const sizes: number[] = [];
    for (const id of BODIES) {
      // Each body is drawn in its own frames, so the last drawn are the most recently wanted.
      // oxlint-disable-next-line no-await-in-loop
      await drawn(h, [lit(id)]);
      sizes.push(h.cache.bytes);
    }
    const disposed = h.made.flatMap((tables, n) => (tables.disposed ? [BODIES[n]] : []));
    expect([ATMOSPHERE_CACHE_BYTES, Math.max(...sizes), disposed]).toEqual([
      32_000_000,
      32_000_000,
      BODIES.slice(0, 4),
    ]);
  });

  it("never evicts a body wanted this frame, though the wanted pass its size", async () => {
    const h = harness(ROW_BYTES);
    await drawn(
      h,
      BODIES.map((id) => lit(id)),
    );
    expect([h.cache.bytes, h.made.filter((tables) => tables.disposed).length]).toEqual([
      BODIES.length * ROW_BYTES,
      0,
    ]);
  });

  it("makes an evicted body's tables again when it is wanted, sending no request", async () => {
    const h = harness(ROW_BYTES);
    for (const id of BODIES) {
      // oxlint-disable-next-line no-await-in-loop
      await drawn(h, [lit(id)]);
    }
    const first = BODIES[0] ?? A;
    note(h, [lit(first)]);
    expect([
      h.seam.askedFor(first),
      h.builder.buildsOf(first).length,
      h.made.length,
      h.cache.entryOf(first)?.tables === h.made.at(-1),
    ]).toEqual([1, 1, BODIES.length + 1, true]);
  });

  it("lets go every body no view wants when the arrival changes", async () => {
    const h = harness();
    await drawn(h, [lit(A)]);
    note(h, [lit(bodyId(1))], 0, sceneOf({ arrival: {} }));
    const size = h.cache.size;
    note(h, [lit(A)], 0, sceneOf({ arrival: {} }));
    expect([size, h.made[0]?.disposed, h.seam.askedFor(A)]).toEqual([1, true, 2]);
  });

  it("lets a closed view's bodies be evicted", async () => {
    const h = harness(20_000_000);
    const other = h.cache.openView();
    await drawn(h, [lit(A)]);
    other.noteFrame({ nowMs: 0, scene: sceneOf(), bodies: [lit(bodyId(1))] });
    await answer(h, bodyId(1));
    await land(h, bodyId(1), { kind: "built", assembled: earthAssembly() });
    other.noteFrame({ nowMs: 0, scene: sceneOf(), bodies: [lit(bodyId(1))] });
    const both = h.cache.bytes;
    other.close();
    note(h, [lit(A)]);
    expect([both, h.cache.bytes]).toEqual([40_000_000, 20_000_000]);
  });
});

describe("a kept scene's bodies", () => {
  /** A kept body's frame entry. */
  function kept(id: BodyIdHex, setting: KeptBodyAtmosphere): AtmosphereFrameBody {
    return { ...lit(id), kept: setting };
  }

  it("carries the assembly's labels for a body left unset or set airless, asking for neither", () => {
    const h = harness();
    note(h, [kept(bodyId(1), { kind: "unset" }), kept(bodyId(2), { kind: "airless" })], 0, null);
    const assembled = (["unset", "airless"] as const).map(
      (kind) => assembleMedium({ kind: "kept", atmosphere: { kind } }).labels,
    );
    expect([
      [h.cache.labelsOf(bodyId(1)), h.cache.labelsOf(bodyId(2))],
      h.seam.asked.length,
    ]).toEqual([assembled, 0]);
  });

  it("makes a scene's medium's tables as given, once, adding atmosphereApproximate to a thick one", () => {
    const h = harness();
    const venus = venusClassMedium();
    const earth: BuiltAtmosphere = {
      medium: R05_EARTH_REFERENCE,
      bottomRadiusM: 6_371_008.8,
      curves: [],
      body: earthBody(),
    };
    const thick: BuiltAtmosphere = {
      medium: venus.medium,
      bottomRadiusM: venus.radiusM,
      curves: [],
      body: null,
    };
    const bodies = [
      kept(bodyId(1), { kind: "medium", built: earth, labels: [] }),
      kept(bodyId(2), { kind: "medium", built: thick, labels: ["aerosolsNotYetModelled"] }),
    ];
    note(h, bodies, 0, null);
    note(h, bodies, 0, null);
    expect([
      h.cache.labelsOf(bodyId(1)),
      h.cache.labelsOf(bodyId(2)),
      h.made.map((tables) => tables.built),
      h.seam.asked.length,
    ]).toEqual([[], ["aerosolsNotYetModelled", "atmosphereApproximate"], [earth, thick], 0]);
  });

  it("makes a kept body's tables again for another medium, disposing the last", () => {
    const h = harness();
    note(h, [kept(A, { kind: "medium", built: keptEarth(), labels: [] })], 0, null);
    note(h, [kept(A, { kind: "medium", built: keptEarth(), labels: [] })], 0, null);
    expect(h.made.map((tables) => tables.disposed)).toEqual([true, false]);
  });
});

/** R05's Earth as a kept scene builds it, a new object each call. */
function keptEarth(): BuiltAtmosphere {
  return { medium: R05_EARTH_REFERENCE, bottomRadiusM: 6_371_008.8, curves: [], body: null };
}

describe("the cache's disposal", () => {
  it("cancels its requests and waits, terminates its builder and disposes every table", async () => {
    const h = harness();
    await drawn(h, [lit(A)]);
    note(h, [lit(A), lit(bodyId(1)), lit(bodyId(2))]);
    h.seam.refuse(h.seam.latestFor(bodyId(2)).id, "too_many_requests", "busy");
    await microtasks();
    h.cache.dispose();
    vi.advanceTimersByTime(60_000);
    const report = note(h, [lit(A)]);
    expect([
      h.seam.cancelled,
      h.builder.terminated,
      h.made.every((tables) => tables.disposed),
      h.seam.asked.length,
      report,
    ]).toEqual([
      [h.seam.latestFor(bodyId(1)).id],
      true,
      true,
      3,
      { labels: [], problems: new Map(), pending: false },
    ]);
  });
});

describe("the cache and the pool on the render thread", () => {
  const graph = [
    ...runTimeGraph(`${RENDERER_ROOT}view/atmosphere/AtmosphereCache.ts`),
    ...runTimeGraph(`${RENDERER_ROOT}view/atmosphere/opticsPool.ts`),
  ];

  it("load the tables' module, whose factory the app's cache calls", () => {
    expect(graph).toContain(`${RENDERER_ROOT}view/atmosphere/tables.ts`);
  });

  it("load none of the optics' assembly, data or worker, which stay in the optics workers", () => {
    const optics = graph.filter(
      (path) =>
        /\/view\/atmosphere\/(absorbers|assemble|aerosol|mie|negligible|opticsJobs|optics\.worker)\.ts$/u.test(
          path,
        ) || /\/view\/atmosphere\/(absorbers|materials)\//u.test(path),
    );
    expect(optics).toEqual([]);
  });
});

describe("the cache over the optics pool", () => {
  it("draws a body assembled and curved in the pool's workers, with its tables", async () => {
    vi.useRealTimers();
    const seam = new BodyDetailSeam();
    const made: FakeTables[] = [];
    const pool = new OpticsWorkerPool({
      workers: 2,
      createWorker: () =>
        new InProcessOpticsWorker({
          loadKDistributions: async (species) =>
            Promise.resolve(species.map((key) => toyTable(key))),
        }),
    });
    const cache = new AtmosphereCache<FakeTables>({
      requests: seam.client,
      builder: pool,
      tables: (built) => {
        const tables = new FakeTables(built, 1);
        made.push(tables);
        return tables;
      },
    });
    const view = cache.openView();
    view.noteFrame({ nowMs: 0, scene: sceneOf(), bodies: [lit(A)] });
    seam.answer(seam.latestFor(A).id, earthDetail());
    await vi.waitFor(
      () => {
        view.noteFrame({ nowMs: 0, scene: sceneOf(), bodies: [lit(A)] });
        expect(made.length).toBe(1);
      },
      { timeout: 10_000, interval: 5 },
    );
    const built = made[0]?.built;
    const absorbing = built?.medium.terms.filter((term) => term.absorber !== undefined).length;
    expect([
      (absorbing ?? 0) > 0,
      built?.curves.length,
      built?.curves[0]?.length,
      cache.entryOf(A)?.tables === made[0],
    ]).toEqual([true, 1, absorbing, true]);
    cache.dispose();
  });
});

/** The toy's grid: the stored pressures' first span, 4 a decade from 10 Pa to 10 MPa. */
const PRESSURES_PA = Array.from({ length: 25 }, (_, k) => 10 * 10 ** (k / 4));
const TEMPERATURES_K = [150, 200, 250, 300, 350];

/** A toy line table for `species`: cells 690–692 nm, a strong and a weak g-point each. */
function toyTable(species: string): ReturnType<typeof readKDistribution> {
  const sigmasM2 = TEMPERATURES_K.flatMap((t) =>
    PRESSURES_PA.flatMap((p) => [2e-30 * (t / 250), (6e-27 * (t / 250)) / Math.sqrt(1 + p / 1e3)]),
  );
  const cells = [690, 691, 692].map((nm) => ({ nm, weights: [0.8, 0.2], sigmasM2 }));
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
