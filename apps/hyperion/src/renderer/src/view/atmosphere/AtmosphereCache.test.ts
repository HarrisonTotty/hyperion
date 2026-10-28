/**
 * The atmosphere cache (plan R08, R08.T10.a; `decision-r08-t10a-leans.md` items 3, 4, 6, 8–10 and
 * the follow-up to item 6): its requests and their triggers, `ATMOSPHERE: PENDING` and its hold,
 * the answers it cannot draw, failed builds, its size, kept scenes, its views and its disposal.
 * The boundaries are faked: a real `RequestClient` over a recording send, a scripted builder in
 * the optics pool's place, a fake tables factory and fake timers; the last test runs the real pool
 * on in-process workers.
 */
import type { BodyIdHex } from "@hyperion/protocol";
import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  type MockInstance,
  onTestFinished,
  vi,
} from "vitest";

import { toBodyDetail } from "../../lib/system/bodiesWire";
import { earthBody, saturnBody, venusClassMedium } from "../../test/atmosphereFixtures";
import { type DetailAnswer, SUN_LIGHT, withRecord } from "../../test/atmosphereRecords";
import { FakeTables } from "../../test/atmosphereCacheFakes";
import { sweepMedium } from "../../test/atmosphereSweep";
import { BodyDetailSeam } from "../../test/bodyDetailSeam";
import { InProcessOpticsWorker } from "../../test/inProcessOpticsWorker";
import { ignore, microtasks, NOT_MODELLED, toyLoader } from "../../test/opticsFakes";
import {
  earthDetail,
  earthDetailOfAnOlderServer,
  FIXTURE_EARTH,
  FIXTURE_SYSTEM,
  FIXTURE_UNIVERSE,
} from "../../test/planetaryFixture";
import { RENDERER_ROOT, runTimeGraph } from "../../test/runTimeGraph";
import { ScriptedOpticsWorker } from "../../test/scriptedOpticsWorker";
import { between, seededRandom } from "../../test/seededRandom";
import { patternedThickBake } from "../../test/thickTables";
import { heldAppearanceOf } from "../appearance/fromWire";
import type { LitRegime } from "../bodies/regime";
import { EngineUnavailable } from "../engine/resilientEngine";
import { builtOfCase } from "../scenes/atmosphereScene";
import { ANNUNCIATION_CLEAR_MS, ANNUNCIATION_ONSET_MS } from "../terrain/annunciation";
import { type AssembledMedium, assembleMedium, mediumTableRadiusM } from "./assemble";
import {
  ATMOSPHERE_CACHE_BYTES,
  AtmosphereCache,
  type AtmosphereFrameBody,
  type AtmosphereScene,
  type AtmosphereView,
  type AtmosphereViewReport,
  type BuiltAtmosphere,
  type KeptBodyAtmosphere,
  PendingLineHold,
} from "./AtmosphereCache";
import {
  atmosphereKey,
  type BodyAtmosphereInput,
  type BodyLight,
  clampedGroundAlbedo,
} from "./atmosphereInput";
import { EARTH_COLUMN, R05_EARTH_REFERENCE } from "./earth";
import { ATMOSPHERE_ANSWER_UNREADABLE, type AtmosphereLabel } from "./labels";
import { keptBakeLightsOf, type OpticsRequest } from "./opticsMessages";
import {
  type AtmosphereBuilder,
  MAX_CONSECUTIVE_OPTICS_FAILURES,
  type OpticsBuild,
  type OpticsBuildOutcome,
  OpticsWorkerPool,
  type ThickBake,
  type ThickBakeAsk,
  type ThickBakeOutcome,
} from "./opticsPool";
import { EARTH_CASE, EARTH_REFERENCE_RADIUS_M, referenceCaseBakeLights } from "./reference/cases";
import { type AtmosphereBody, perPlanetRowOf, tablesPlanOf } from "./tables";
import { type MsSourceTable, thickBakeBands } from "./thick/bake";
import { spectralColumnOf } from "./thick/columnLayout";
import { keptColumnOf, sphereBakeBody } from "./thick/keptBake";
import { type AtmosphereRegime, THICK_REASON } from "./thick/regime";
import { thickBakeRouted } from "./thick/routing";
import { channelSource } from "./thick/spectralColumn";

/** A build the scripted builder holds, landed by the test. */
interface ScriptedBuild {
  readonly input: BodyAtmosphereInput;
  readonly key: string;
  settled: boolean;
  readonly settle: (outcome: OpticsBuildOutcome) => void;
}

/** A thick bake the scripted builder holds, landed by the test (R08.T14.c). */
interface ScriptedBake {
  readonly ask: ThickBakeAsk;
  settled: boolean;
  cancelled: boolean;
  readonly settle: (outcome: ThickBakeOutcome) => void;
}

/**
 * A builder in the optics pool's place, whose builds and thick bakes the test lands, fails or
 * leaves running.
 */
class ScriptedBuilder implements AtmosphereBuilder {
  readonly builds: ScriptedBuild[] = [];
  readonly bakes: ScriptedBake[] = [];
  terminated = false;

  bakeThick(ask: ThickBakeAsk): ThickBake {
    let resolve: (outcome: ThickBakeOutcome) => void = ignore;
    const outcome = new Promise<ThickBakeOutcome>((settleWith) => {
      resolve = settleWith;
    });
    const bake: ScriptedBake = {
      ask,
      settled: false,
      cancelled: false,
      settle: (result) => {
        bake.settled = true;
        resolve(result);
      },
    };
    this.bakes.push(bake);
    return {
      outcome,
      cancel: () => {
        bake.cancelled = true;
        bake.settle({ kind: "cancelled" });
      },
    };
  }

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
    for (const bake of this.bakes) {
      bake.settle({ kind: "cancelled" });
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

/**
 * A cache over the fakes, its tables reporting `rowBytes` each, or their per-planet row's, routing
 * its thick bakes by `routes`, or by the product's routing if absent.
 */
function harness(rowBytes?: number, routes?: (regime: AtmosphereRegime) => boolean): Harness {
  const seam = new BodyDetailSeam();
  const builder = new ScriptedBuilder();
  const made: FakeTables[] = [];
  const factory: { throws: Error | null } = { throws: null };
  const cache = new AtmosphereCache<FakeTables>({
    requests: seam.client,
    builder,
    ...(routes === undefined ? {} : { routes }),
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

/** R05's Earth as an assembly, one light and no absorber, its regime as given. */
function earthAssembly(
  labels: ReadonlyArray<AtmosphereLabel> = [],
  regime: AtmosphereRegime = { kind: "thin" },
): AssembledMedium {
  return {
    medium: R05_EARTH_REFERENCE,
    drawn: {
      body: earthBody(),
      bottomRadiusM: 6_371_008.8,
      absorbers: [],
      spectra: [SUN_LIGHT.spectrum],
      curves: [[]],
      sources: [],
    },
    labels,
    reasons: [],
    negligible: [],
    regime,
  };
}

/** A routing whose `thickScattering` gate has passed, as R08.T14.d's will (R08.T14.c). */
function routedThick(regime: AtmosphereRegime): boolean {
  return thickBakeRouted(regime, new Set(["thickScattering"]));
}

/** An NH₄SH stand-in's reason, which a thick bake does not clear (R08.T14.c). */
const STAND_IN = "NH4SH: drawn with a stated stand-in";

/**
 * R05's Earth routed `thickScattering`, as a server body's assembly under `atmosphereApproximate`
 * with `reasons`, its terms' sources by their channels, on `body`'s figure and lit by `lights`
 * (R08.T14.c).
 */
function thickAssembly(
  reasons: ReadonlyArray<string> = [THICK_REASON],
  body: AtmosphereBody = earthBody(),
  lights: ReadonlyArray<BodyLight> = [SUN_LIGHT],
): AssembledMedium {
  return {
    medium: R05_EARTH_REFERENCE,
    drawn: {
      body,
      bottomRadiusM: mediumTableRadiusM(body.figure),
      absorbers: [],
      spectra: lights.map((light) => light.spectrum),
      curves: lights.map(() => []),
      sources: R05_EARTH_REFERENCE.terms.map(channelSource),
    },
    labels: ["atmosphereApproximate"],
    reasons,
    negligible: [],
    regime: { kind: "thickScattering" },
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

  describe("after a lost link", () => {
    /** A cache whose body's request was lost with the link. */
    async function lostLink(): Promise<Harness> {
      const h = harness();
      note(h, [lit(A)]);
      h.seam.loseLink();
      await microtasks();
      return h;
    }

    it("sends nothing while the link is down", async () => {
      const h = await lostLink();
      note(h, [lit(A)], 0, sceneOf({ linkUp: false }));
      expect(h.seam.askedFor(A)).toBe(1);
    });

    it("asks again once when the link is back up", async () => {
      const h = await lostLink();
      note(h, [lit(A)], 0, sceneOf({ linkUp: false }));
      note(h, [lit(A)]);
      note(h, [lit(A)]);
      expect(h.seam.askedFor(A)).toBe(2);
    });

    it("keeps the body pending while it waits", async () => {
      const h = await lostLink();
      note(h, [lit(A)], 0, sceneOf({ linkUp: false }));
      expect(h.cache.labelsOf(A)).toEqual(["atmospherePending"]);
    });
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

describe("ATMOSPHERE: APPROXIMATE's reasons (R08.T10.c)", () => {
  const WATER = "H2O: its absorption estimated, not measured";

  it("reports each wanted body drawn with the label by its reasons, and none for the others", async () => {
    const h = harness();
    vi.spyOn(console, "error").mockImplementation(ignore);
    const approximate = bodyId(1);
    const computing = bodyId(2);
    const failed = bodyId(3);
    const pending = bodyId(4);
    const frame = [approximate, computing, failed, pending].map((id) => lit(id));
    note(h, frame);
    for (const id of [approximate, computing, failed]) {
      h.seam.answer(h.seam.latestFor(id).id, answerFor(id));
    }
    await microtasks();
    await land(h, approximate, {
      kind: "built",
      assembled: { ...earthAssembly(["atmosphereApproximate"]), reasons: [WATER] },
    });
    // A bake's note in APPROXIMATE's place: the reasons stand with that label alone.
    await land(h, computing, {
      kind: "built",
      assembled: { ...earthAssembly(["atmosphereComputing"]), reasons: [WATER] },
    });
    await land(h, failed, { kind: "failed", message: "a bug" });
    const report = note(h, frame);
    expect([[...report.reasons], report.problems.has(failed), report.pending]).toEqual([
      [[approximate, [WATER]]],
      true,
      true,
    ]);
  });

  it("reports none for a body drawn with the label as a point, which no view wants", async () => {
    const h = harness();
    await drawn(h, [lit(A)], { ...earthAssembly(["atmosphereApproximate"]), reasons: [WATER] });
    expect([...note(h, [lit(A, "point")]).reasons]).toEqual([]);
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

  describe("at the body's next trigger", () => {
    /** A cache whose body's build failed, asked again at a new arrival. */
    async function failedThenAskedAgain(): Promise<{
      readonly h: Harness;
      readonly arrived: AtmosphereScene;
    }> {
      const h = harness();
      vi.spyOn(console, "error").mockImplementation(ignore);
      note(h, [lit(A)]);
      await answer(h, A);
      await land(h, A, { kind: "failed", message: "a bug" });
      const arrived = sceneOf({ arrival: {} });
      note(h, [lit(A)], 0, arrived);
      return { h, arrived };
    }

    it("asks again, its fault standing meanwhile", async () => {
      const { h } = await failedThenAskedAgain();
      expect([h.seam.askedFor(A), h.cache.problemOf(A)]).toEqual([2, { kind: "failed" }]);
    });

    it("clears the fault once a retried build lands", async () => {
      const { h, arrived } = await failedThenAskedAgain();
      await answer(h, A);
      await land(h, A, { kind: "built", assembled: NOT_MODELLED });
      note(h, [lit(A)], 0, arrived);
      expect([h.cache.problemOf(A), h.cache.labelsOf(A)]).toEqual([
        null,
        ["atmosphereNotYetModelled"],
      ]);
    });
  });

  it("holds the body failed, not pending, where taking its build throws", async () => {
    const h = harness();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    note(h, [lit(A)]);
    await answer(h, A);
    // An assembly whose labels cannot be read: a bug in taking the build, not in making it.
    const unreadable: AssembledMedium = {
      ...NOT_MODELLED,
      get labels(): ReadonlyArray<AtmosphereLabel> {
        throw new Error("a bug in taking the build");
      },
    };
    await land(h, A, { kind: "built", assembled: unreadable });
    const report = note(h, [lit(A)]);
    expect([report.problems.get(A), report.pending, errors.mock.calls.length]).toEqual([
      { kind: "failed" },
      false,
      1,
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

describe("a body whose air throws as it is drawn (decision-r08-t10b-leans §2)", () => {
  /** A cache with A drawn, then its draw reported failed by its view and by another. */
  async function drawFailed(): Promise<{
    readonly h: Harness;
    readonly other: AtmosphereView;
    readonly errors: MockInstance<typeof console.error>;
  }> {
    const h = harness();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    await drawn(h, [lit(A)]);
    const other = h.cache.openView();
    h.view.reportDrawFailure(A, new Error("a bug in the draw"), "VIEW 1");
    other.reportDrawFailure(A, new Error("the same bug"), "VIEW 2");
    return { h, other, errors };
  }

  it("holds it failed for every view, its tables let go, with one console.error naming the view", async () => {
    const { h, other, errors } = await drawFailed();
    const report = note(h, [lit(A)]);
    const seen = other.noteFrame({ nowMs: 0, scene: sceneOf(), bodies: [lit(A)] });
    expect([
      h.cache.entryOf(A),
      h.made[0]?.disposed,
      report.problems.get(A),
      seen.problems.get(A),
      report.labels,
      errors.mock.calls.map(([text]) => text),
    ]).toEqual([
      null,
      true,
      { kind: "failed" },
      { kind: "failed" },
      [],
      [`the atmosphere of ${A} could not be drawn in VIEW 1:`],
    ]);
  });

  it("is drawn again once its retry at the next trigger lands", async () => {
    const { h } = await drawFailed();
    const arrived = sceneOf({ arrival: {} });
    note(h, [lit(A)], 0, arrived);
    h.seam.answer(h.seam.latestFor(A).id, answerFor(A));
    await microtasks();
    await land(h, A, { kind: "built", assembled: earthAssembly() });
    note(h, [lit(A)], 0, arrived);
    expect([h.seam.askedFor(A), h.cache.problemOf(A), h.cache.entryOf(A)?.tables]).toEqual([
      2,
      null,
      h.made.at(-1),
    ]);
  });

  it("is drawn by a new cache, VIEW shown again", async () => {
    await drawFailed();
    const again = harness();
    expect((await drawn(again, [lit(A)])).problems.get(A)).toBeUndefined();
  });

  it("takes no report from a closed view, or of a body it does not hold", async () => {
    const h = harness();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    await drawn(h, [lit(A)]);
    const closed = h.cache.openView();
    closed.close();
    closed.reportDrawFailure(A, new Error("a closed view's"), "VIEW 2");
    h.view.reportDrawFailure(bodyId(9), new Error("no such body"), "VIEW 1");
    expect([h.cache.problemOf(A), h.cache.problemOf(bodyId(9)), errors.mock.calls.length]).toEqual([
      null,
      null,
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
      // One body after another, so that the first are evicted, as the size test draws them.
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

  it("makes a scene's medium's tables as given, once, adding atmosphereApproximate to each the router routes thick, R05's Earth among them (R08.T13.d)", () => {
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
    ]).toEqual([
      ["atmosphereApproximate"],
      ["aerosolsNotYetModelled", "atmosphereApproximate"],
      [earth, thick],
      0,
    ]);
  });

  it("gives a scene's medium the router routes thick its reason and regime, and a thin one neither (R08.T10.c, R08.T13.d)", () => {
    const h = harness();
    const venus = venusClassMedium();
    const tenuous: BuiltAtmosphere = {
      medium: sweepMedium({
        singleScatteringAlbedo: 1,
        asymmetry: 0,
        tau: 1 / 64,
        groundAlbedo: 0.3,
      }),
      bottomRadiusM: 6_371_008.8,
      curves: [],
      body: null,
    };
    const thick: BuiltAtmosphere = {
      medium: venus.medium,
      bottomRadiusM: venus.radiusM,
      curves: [],
      body: null,
    };
    const bodies = [
      kept(bodyId(1), { kind: "medium", built: tenuous, labels: [] }),
      kept(bodyId(2), { kind: "medium", built: thick, labels: [] }),
    ];
    note(h, bodies, 0, null);
    const report = note(h, bodies, 0, null);
    expect([
      [...report.reasons],
      h.cache.entryOf(bodyId(1))?.reasons,
      h.cache.entryOf(bodyId(2))?.reasons,
      h.cache.entryOf(bodyId(1))?.regime,
      h.cache.entryOf(bodyId(2))?.regime,
    ]).toEqual([
      [[bodyId(2), [THICK_REASON]]],
      [],
      [THICK_REASON],
      { kind: "thin" },
      { kind: "thickScattering" },
    ]);
  });

  it("routes a kept body again when its scene gives another cover, the same medium kept (R08.T13.d)", () => {
    const h = harness();
    const venus = venusClassMedium();
    const built: BuiltAtmosphere = {
      medium: venus.medium,
      bottomRadiusM: venus.radiusM,
      curves: [],
      body: null,
    };
    const deck = { opticalDepth550: 30, baseAltitudeM: 2_000, topAltitudeM: 5_000 };
    note(h, [kept(A, { kind: "medium", built, labels: [] })], 0, null);
    const clear = h.cache.entryOf(A)?.regime;
    const cover = { cloudFraction: 1, decks: [deck], envelope: false };
    note(h, [kept(A, { kind: "medium", built, labels: [], cover })], 0, null);
    expect([clear, h.cache.entryOf(A)?.regime]).toEqual([
      { kind: "thickScattering" },
      { kind: "cloudDeck", deckAltitudeM: 5_000 },
    ]);
  });

  it("holds a body whose scene is still building its medium pending, then draws it (R08.T10.b)", () => {
    const h = harness();
    const view = h.cache.openView();
    const loading = view.noteFrame({
      nowMs: 0,
      scene: null,
      bodies: [kept(A, { kind: "loading" })],
    });
    const held = view.noteFrame({
      nowMs: ANNUNCIATION_ONSET_MS,
      scene: null,
      bodies: [kept(A, { kind: "loading" })],
    });
    const built = view.noteFrame({
      nowMs: ANNUNCIATION_ONSET_MS + 1,
      scene: null,
      bodies: [kept(A, { kind: "medium", built: keptEarth(), labels: [] })],
    });
    expect([
      loading.pending,
      held.labels,
      built.pending,
      h.cache.entryOf(A)?.tables !== null,
    ]).toEqual([true, ["atmospherePending"], false, true]);
    expect(h.seam.asked.length).toBe(0);
  });

  it("holds a body whose scene's load failed as loadFailed, not pending and unlabelled, logging nothing (R08.T10.b)", () => {
    const h = harness();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    note(h, [kept(A, { kind: "medium", built: keptEarth(), labels: [] })], 0, null);
    const report = note(h, [kept(A, { kind: "failed" })], 0, null);
    expect([
      report.problems.get(A),
      report.pending,
      report.labels,
      h.cache.entryOf(A),
      h.made[0]?.disposed,
      errors.mock.calls.length,
      h.seam.asked.length,
    ]).toEqual([{ kind: "loadFailed" }, false, [], null, true, 0, 0]);
  });

  it("makes a kept body's tables again for another medium, disposing the last", () => {
    const h = harness();
    note(h, [kept(A, { kind: "medium", built: keptEarth(), labels: [] })], 0, null);
    note(h, [kept(A, { kind: "medium", built: keptEarth(), labels: [] })], 0, null);
    expect(h.made.map((tables) => tables.disposed)).toEqual([true, false]);
  });
});

describe("the regime a server body's entry carries (R08.T13.d)", () => {
  it("is the assembly's, as the worker computed it", async () => {
    const h = harness();
    const regime = { kind: "cloudDeck", deckAltitudeM: 4_000 } as const;
    await drawn(h, [lit(A)], earthAssembly(["atmosphereApproximate"], regime));
    expect(h.cache.entryOf(A)?.regime).toEqual(regime);
  });

  it("is null for an assembly that draws no medium", async () => {
    const h = harness();
    await drawn(h, [lit(A)], NOT_MODELLED);
    expect(h.cache.entryOf(A)?.regime).toBeNull();
  });
});

/** R05's Earth as a kept scene builds it, a new object each call. */
function keptEarth(): BuiltAtmosphere {
  return { medium: R05_EARTH_REFERENCE, bottomRadiusM: 6_371_008.8, curves: [], body: null };
}

describe("the cache's disposal", () => {
  /**
   * A cache disposed with a body drawn, one asked and unanswered, and one waiting after a busy
   * answer.
   */
  async function disposed(): Promise<Harness> {
    const h = harness();
    await drawn(h, [lit(A)]);
    note(h, [lit(A), lit(bodyId(1)), lit(bodyId(2))]);
    h.seam.refuse(h.seam.latestFor(bodyId(2)).id, "too_many_requests", "busy");
    await microtasks();
    h.cache.dispose();
    return h;
  }

  it("cancels the requests in flight", async () => {
    const h = await disposed();
    expect(h.seam.cancelled).toEqual([h.seam.latestFor(bodyId(1)).id]);
  });

  it("stops the busy waits", async () => {
    const h = await disposed();
    vi.advanceTimersByTime(60_000);
    expect(h.seam.asked.length).toBe(3);
  });

  it("terminates its builder", async () => {
    const h = await disposed();
    expect(h.builder.terminated).toBe(true);
  });

  it("disposes every table", async () => {
    const h = await disposed();
    expect([h.made.length > 0, h.made.every((tables) => tables.disposed)]).toEqual([true, true]);
  });

  it("reports nothing of a later frame", async () => {
    const h = await disposed();
    expect(note(h, [lit(A)])).toEqual({
      labels: [],
      problems: new Map(),
      reasons: new Map(),
      pending: false,
    });
  });
});

/** The latest bake the cache asked, or a test failure. */
function latestBake(h: Harness): ScriptedBake {
  const bake = h.builder.bakes.at(-1);
  if (bake === undefined) {
    throw new Error("no thick bake was asked");
  }
  return bake;
}

/** The tables made `n`th, or a test failure. */
function madeAt(h: Harness, n: number): FakeTables {
  const tables = h.made[n];
  if (tables === undefined) {
    throw new Error(`no tables were made ${n}th`);
  }
  return tables;
}

describe("a thick body's bake (R08.T14.c)", () => {
  /** A routed cache with A drawn thick from `assembled`, its bake asked and running. */
  async function baking(assembled = thickAssembly(), rowBytes?: number): Promise<Harness> {
    const h = harness(rowBytes, routedThick);
    await drawn(h, [lit(A)], assembled);
    return h;
  }

  /** Lands A's latest bake with a bake of its first tables' plan, one slot, and returns it. */
  async function landBake(h: Harness): Promise<ReadonlyArray<MsSourceTable>> {
    const tables = patternedThickBake(madeAt(h, 0).plan.bands, R05_EARTH_REFERENCE, 1);
    latestBake(h).settle({ kind: "baked", tables });
    await microtasks();
    return tables;
  }

  it("asks no bake by default, a thick body drawn under APPROXIMATE with its reason as before", async () => {
    const h = harness();
    const report = await drawn(h, [lit(A)], thickAssembly());
    expect([h.builder.bakes.length, report.labels, [...report.reasons]]).toEqual([
      0,
      ["atmosphereApproximate"],
      [[A, [THICK_REASON]]],
    ]);
  });

  it("shows COMPUTING in APPROXIMATE's place while the bake runs, never beside it and with no reason, though a stand-in holds it", async () => {
    const h = harness(undefined, routedThick);
    const first = await drawn(h, [lit(A)], thickAssembly([STAND_IN, THICK_REASON]));
    const later = note(h, [lit(A)]);
    expect([
      [first.labels, [...first.reasons]],
      [later.labels, [...later.reasons], later.problems.size],
      h.cache.entryOf(A)?.reasons,
    ]).toEqual([[["atmosphereComputing"], []], [["atmosphereComputing"], [], 0], []]);
  });

  it("asks its bake once its tables are made, every band of their plan and every light slot in one request", async () => {
    const h = harness(undefined, routedThick);
    const second: BodyLight = { star: `${FIXTURE_SYSTEM}.0101`, spectrum: SUN_LIGHT.spectrum };
    const lights = [SUN_LIGHT, second];
    const saturn = saturnBody();
    await drawn(h, [lit(A, "disc", lights)], thickAssembly([THICK_REASON], saturn, lights));
    note(h, [lit(A, "disc", lights)]);
    const { ask } = latestBake(h);
    expect([
      h.builder.bakes.length,
      ask.bands,
      thickBakeBands(ask.body).map((band) => band.gravityRatio),
      ask.slots,
      ask.input === h.builder.latestOf(A).input,
      ask.body,
      ask.column,
      ask.kept,
    ]).toEqual([
      1,
      [0, 1, 2, 3],
      madeAt(h, 0).plan.bands.map((band) => band.gravityRatio),
      [0, 1],
      true,
      { ...saturn, bottomRadiusM: mediumTableRadiusM(saturn.figure) },
      spectralColumnOf(R05_EARTH_REFERENCE, R05_EARTH_REFERENCE.terms.map(channelSource)),
      undefined,
    ]);
  });

  it("gives its tables the bake once it lands, every band in its order, and drops APPROXIMATE", async () => {
    const h = await baking(thickAssembly([THICK_REASON], saturnBody()));
    const tables = patternedThickBake(madeAt(h, 0).plan.bands, R05_EARTH_REFERENCE, 1);
    latestBake(h).settle({ kind: "baked", tables });
    await microtasks();
    const report = note(h, [lit(A)]);
    expect([
      madeAt(h, 0).thick.length,
      madeAt(h, 0).thickHeld === tables,
      tables.map((table) => table.band.gravityRatio),
      report.labels,
      [...report.reasons],
      h.builder.bakes.length,
    ]).toEqual([1, true, madeAt(h, 0).plan.bands.map((band) => band.gravityRatio), [], [], 1]);
  });

  it("keeps APPROXIMATE after the bake where a stand-in still holds it, with that reason alone", async () => {
    const h = await baking(thickAssembly([STAND_IN, THICK_REASON]));
    await landBake(h);
    const report = note(h, [lit(A)]);
    expect([report.labels, [...report.reasons]]).toEqual([
      ["atmosphereApproximate"],
      [[A, [STAND_IN]]],
    ]);
  });

  it("falls back to APPROXIMATE with every reason where the bake fails, raising no fault and asking no bake again", async () => {
    const h = await baking(thickAssembly([STAND_IN, THICK_REASON]));
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    latestBake(h).settle({ kind: "failed", message: "the solver refused a layer" });
    await microtasks();
    const reports = [note(h, [lit(A)]), note(h, [lit(A)])];
    expect([
      reports.map((report) => [report.labels, [...report.reasons], report.problems.size]),
      h.cache.problemOf(A),
      h.cache.entryOf(A)?.tables === h.made[0],
      h.builder.bakes.length,
      errors.mock.calls.length,
    ]).toEqual([
      Array.from({ length: 2 }, () => [
        ["atmosphereApproximate"],
        [[A, [STAND_IN, THICK_REASON]]],
        0,
      ]),
      null,
      true,
      1,
      1,
    ]);
  });

  it("fails the bake where its tables refuse it, with one console.error and no fault", async () => {
    const h = await baking();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    madeAt(h, 0).thickThrows = new RangeError("a thick bake of 2 bands is not the tables' 1");
    await landBake(h);
    const report = note(h, [lit(A)]);
    expect([
      report.labels,
      [...report.reasons],
      report.problems.size,
      errors.mock.calls.length,
    ]).toEqual([["atmosphereApproximate"], [[A, [THICK_REASON]]], 0, 1]);
  });

  it("gives its tables the bake at a later frame where the device was lost as it landed", async () => {
    const h = await baking();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    const tables = madeAt(h, 0);
    tables.thickThrows = new EngineUnavailable("createTexture");
    const baked = await landBake(h);
    note(h, [lit(A)]);
    const during = tables.thick.length;
    tables.thickThrows = null;
    note(h, [lit(A)]);
    expect([
      during,
      tables.thick.length,
      tables.thickHeld === baked,
      errors.mock.calls.length,
    ]).toEqual([0, 1, true, 0]);
  });

  it("gives tables made again after an eviction the bake, asking no bake and no record again", async () => {
    const h = await baking(thickAssembly(), 20_000_000);
    const baked = await landBake(h);
    await drawn(h, [lit(bodyId(1))]);
    const evicted = madeAt(h, 0).disposed;
    const report = note(h, [lit(A)]);
    expect([
      evicted,
      h.made.length,
      madeAt(h, 2).built === madeAt(h, 0).built,
      madeAt(h, 2).thickHeld === baked,
      h.builder.bakes.length,
      h.seam.askedFor(A),
      report.labels,
    ]).toEqual([true, 3, true, true, 1, 1, []]);
  });

  it("cancels the bake when a new key's build lands, asking the new entry's", async () => {
    const h = await baking();
    const first = latestBake(h);
    const second: BodyLight = { star: `${FIXTURE_SYSTEM}.0101`, spectrum: SUN_LIGHT.spectrum };
    const lights = [SUN_LIGHT, second];
    note(h, [lit(A, "disc", lights)]);
    await land(h, A, {
      kind: "built",
      assembled: thickAssembly([THICK_REASON], earthBody(), lights),
    });
    const report = note(h, [lit(A, "disc", lights)]);
    expect([
      first.cancelled,
      h.builder.bakes.length,
      latestBake(h).ask.slots,
      report.labels,
    ]).toEqual([true, 2, [0, 1], ["atmosphereComputing"]]);
  });

  it.each([
    [
      "its draw is reported failed",
      (h: Harness): void => {
        h.view.reportDrawFailure(A, new Error("a bug in the draw"), "VIEW 1");
      },
    ],
    [
      "its record is refused",
      (h: Harness): void => {
        note(h, [lit(A)], 0, sceneOf({ arrival: {} }));
        h.seam.refuse(h.seam.latestFor(A).id, "unknown_body", "no such body");
      },
    ],
    [
      "the cache is disposed",
      (h: Harness): void => {
        h.cache.dispose();
      },
    ],
  ])("cancels the bake when %s", async (_, drop) => {
    const h = await baking();
    vi.spyOn(console, "error").mockImplementation(ignore);
    drop(h);
    await microtasks();
    expect(latestBake(h).cancelled).toBe(true);
  });

  describe("of a kept scene's medium", () => {
    /** A kept body's frame entry, lit by `lights`. */
    function keptBody(
      setting: KeptBodyAtmosphere,
      lights: ReadonlyArray<BodyLight> = [SUN_LIGHT],
    ): AtmosphereFrameBody {
      return { ...lit(A, "disc", lights), kept: setting };
    }

    it("asks TEST EARTH's bake with its own lights, its scene's sun, on its sphere's one band", () => {
      const h = harness(undefined, routedThick);
      const built = builtOfCase(EARTH_CASE, "earth");
      const report = note(h, [keptBody({ kind: "medium", built, labels: [] })], 0, null);
      const { ask } = latestBake(h);
      expect([
        report.labels,
        h.builder.bakes.length,
        ask.input,
        ask.slots,
        ask.bands,
        thickBakeBands(ask.body),
        ask.column,
        ask.kept,
      ]).toEqual([
        ["atmosphereComputing"],
        1,
        { kind: "kept", atmosphere: { kind: "unset" } },
        [0],
        [0],
        thickBakeBands(sphereBakeBody(EARTH_REFERENCE_RADIUS_M, EARTH_COLUMN.referenceGravityMS2)),
        keptColumnOf(EARTH_CASE),
        keptBakeLightsOf(EARTH_CASE.medium, [SUN_LIGHT.spectrum]),
      ]);
    });

    it("asks a kept body lit by no star of its scene's under the reference Sun, as the kept cases' curves were formed", () => {
      const h = harness(undefined, routedThick);
      const built = builtOfCase(EARTH_CASE, "earth");
      note(h, [keptBody({ kind: "medium", built, labels: [] }, [])], 0, null);
      const { ask } = latestBake(h);
      expect([ask.slots, ask.kept]).toEqual([[0], referenceCaseBakeLights(EARTH_CASE)]);
    });

    it("holds a kept medium with no sources as a failed bake, under APPROXIMATE with its reason, warning once", () => {
      const h = harness(undefined, routedThick);
      const warnings = vi.spyOn(console, "warn").mockImplementation(ignore);
      const setting: KeptBodyAtmosphere = { kind: "medium", built: keptEarth(), labels: [] };
      note(h, [keptBody(setting)], 0, null);
      const report = note(h, [keptBody(setting)], 0, null);
      expect([
        h.builder.bakes.length,
        report.labels,
        [...report.reasons],
        report.problems.size,
        warnings.mock.calls.length,
      ]).toEqual([0, ["atmosphereApproximate"], [[A, [THICK_REASON]]], 0, 1]);
    });
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

  it("load none of the thick bake's solver, its column's optics or the reference media, which stay in the optics workers (R08.T14.c)", () => {
    const thick = graph.filter((path) =>
      /\/view\/atmosphere\/(thick\/(bake|binConversion|discreteOrdinates|keptBake|polarisation|spectralColumn)|earth|reference\/[^/]+)\.ts$/u.test(
        path,
      ),
    );
    expect(thick).toEqual([]);
  });
});

describe("the cache over the optics pool", () => {
  it("draws a body assembled and curved in the pool's workers, with its tables", async () => {
    const seam = new BodyDetailSeam();
    const made: FakeTables[] = [];
    const pool = new OpticsWorkerPool({
      workers: 2,
      createWorker: () => new InProcessOpticsWorker(toyLoader()),
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
    // The cache owns the pool: disposing it terminates the workers, however the test ends.
    onTestFinished(() => {
      cache.dispose();
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
  });
});

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

describe("a thick body's bake over the optics pool (R08.T14.c)", () => {
  /** What a test drives: the cache, its view, and the pool's scripted workers. */
  interface OverPool {
    readonly workers: ScriptedOpticsWorker[];
    readonly view: AtmosphereView;
    readonly cache: AtmosphereCache<FakeTables>;
  }

  /**
   * A routed cache over a pool of one scripted worker, with A assembled thick in it and drawn, its
   * bake posted to the worker.
   */
  async function bakingOverPool(): Promise<OverPool> {
    const seam = new BodyDetailSeam();
    const workers: ScriptedOpticsWorker[] = [];
    const pool = new OpticsWorkerPool({
      workers: 1,
      createWorker: () => {
        const worker = new ScriptedOpticsWorker();
        workers.push(worker);
        return worker;
      },
    });
    const cache = new AtmosphereCache<FakeTables>({
      requests: seam.client,
      builder: pool,
      routes: routedThick,
      tables: (built) => new FakeTables(built, 1),
    });
    onTestFinished(() => {
      cache.dispose();
    });
    const view = cache.openView();
    view.noteFrame({ nowMs: 0, scene: sceneOf(), bodies: [lit(A)] });
    seam.answer(seam.latestFor(A).id, answerFor(A));
    await microtasks();
    const worker = workerAt(workers, 0);
    worker.reply({ kind: "assembled", id: lastPosted(worker).id, assembled: thickAssembly() });
    await microtasks();
    // The light's curve share, sent once the assembly freed the worker and dropped by the build,
    // which needs no curves: its late answer frees the worker for the bake.
    worker.reply({ kind: "curve-sums", id: lastPosted(worker).id, sums: [] });
    await microtasks();
    view.noteFrame({ nowMs: 0, scene: sceneOf(), bodies: [lit(A)] });
    return { workers, view, cache };
  }

  /** The view's frame of A, after the replies given have been taken. */
  async function frame(over: OverPool): Promise<AtmosphereViewReport> {
    await microtasks();
    return over.view.noteFrame({ nowMs: 0, scene: sceneOf(), bodies: [lit(A)] });
  }

  it("posts the bake, every band in one thick-bake request, once the body is drawn", async () => {
    const { workers } = await bakingOverPool();
    const worker = workerAt(workers, 0);
    const posted = lastPosted(worker);
    expect([
      worker.received.map((request) => request.kind),
      posted.kind === "thick-bake" ? posted.bands : [],
    ]).toEqual([["assemble", "curve-sums", "thick-bake"], [0]]);
  });

  it("gives APPROXIMATE with THICK_REASON and no fault for a thick-bake-failed answer", async () => {
    const over = await bakingOverPool();
    vi.spyOn(console, "error").mockImplementation(ignore);
    const worker = workerAt(over.workers, 0);
    worker.reply({
      kind: "thick-bake-failed",
      id: lastPosted(worker).id,
      bands: [0],
      message: "the solver refused a layer",
    });
    const report = await frame(over);
    expect([
      report.labels,
      [...report.reasons],
      report.problems.size,
      over.cache.problemOf(A),
    ]).toEqual([["atmosphereApproximate"], [[A, [THICK_REASON]]], 0, null]);
  });

  it("keeps COMPUTING where the bake's worker is lost, the bake asked again of its replacement", async () => {
    const over = await bakingOverPool();
    workerAt(over.workers, 0).fail();
    const report = await frame(over);
    expect([lastPosted(workerAt(over.workers, 1)).kind, report.labels]).toEqual([
      "thick-bake",
      ["atmosphereComputing"],
    ]);
  });

  it("gives APPROXIMATE with THICK_REASON and no fault once every worker is given up", async () => {
    const over = await bakingOverPool();
    const errors = vi.spyOn(console, "error").mockImplementation(ignore);
    for (let n = 0; n < MAX_CONSECUTIVE_OPTICS_FAILURES; n += 1) {
      workerAt(over.workers, n).fail();
    }
    const report = await frame(over);
    expect([
      report.labels,
      [...report.reasons],
      report.problems.size,
      errors.mock.calls.map(([text]) => String(text)),
    ]).toEqual([
      ["atmosphereApproximate"],
      [[A, [THICK_REASON]]],
      0,
      [`the thick bake of ${A} failed: every optics worker was given up`],
    ]);
  });
});
