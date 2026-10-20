import { afterEach, describe, expect, it } from "vitest";

import type {
  CheckPhaseName,
  TerrainCheckFault,
  TerrainCheckPhaseName,
  TerrainCheckPhaseRecord,
  ViewsCheckStyle,
} from "../../../../../preload/api";
import type { TextureHandle } from "../../../view/engine/types";
import { STYLE_TOGGLE_KEY } from "../../../view/photoreal/style";
import { PHASE_SCENE_NAME } from "../../../view/scenes/phaseScene";
import { TERRAIN_SCENE_NAME } from "../../../view/scenes/terrainScene";
import { runTerrainCheck, type TerrainCheckDeps, type TerrainCheckTiming } from "./terrainCheckRun";
import type { MaskReading, QuietSpan, TerrainMark, WatchedSession } from "./terrainProbe";
import type { ShownView } from "./viewsProbe";

/** Each test's listeners on the page, which outlive its elements, ended after it. */
let listening = new AbortController();

afterEach(() => {
  listening.abort();
  listening = new AbortController();
  document.body.replaceChildren();
});

/** The run's timing on the test's clock: a smoke's, its waits coarser. */
const TIMING: TerrainCheckTiming = {
  settleMs: 100,
  aloneUntilS: null,
  afterTerrainMs: 3_000,
  terrainByS: 90,
  instrumentsMs: 1_000,
  lossByS: 170,
  restoreMs: 10_000,
  afterLossMs: 500,
  waitMs: 5_000,
  pollMs: 100,
  captures: [{ label: "early", scriptS: 10 }],
};

/** When the fake's terrain first draws, s of the script, and its delta's blocks, s. */
const TERRAIN_AT_S = 40;
const DELTA_BLOCKS_AT_S = [121.5, 123, 124.5];

/** `VIEW`'s controls as the script finds them, drawn into the page, and the state they set. */
class FakeView {
  scene = "SERVER";
  /** When the scene was last chosen, ms of the test's clock. */
  chosenAtMs = 0;
  readonly styles = new Map<string, ViewsCheckStyle>([["PRIMARY", "wireframe"]]);
  operated = "PRIMARY";
  auto = false;
  meter = "AVG";
  readonly #clock: () => number;

  constructor(clock: () => number) {
    this.#clock = clock;
    const scenes = this.#fieldset("Scene", ["SERVER", PHASE_SCENE_NAME, TERRAIN_SCENE_NAME]);
    scenes.setAttribute("aria-label", "Scene");
    for (const slot of [1, 2]) {
      const name = `INSTRUMENT ${String(slot)}`;
      this.#fieldset(name, ["OPEN", "CLOSE"]);
    }
    this.#fieldset("CONTROLS", ["PRIMARY", "INSTRUMENT 1", "INSTRUMENT 2"]);
    const exposure = document.createElement("section");
    exposure.className = "view-exposure";
    const enable = document.createElement("button");
    enable.textContent = "ENABLE";
    enable.addEventListener("click", () => {
      this.auto = true;
      enable.setAttribute("aria-disabled", "true");
    });
    exposure.append(enable);
    document.body.append(exposure);
    const meter = document.createElement("section");
    meter.className = "view-meter";
    for (const mode of ["AVG", "LIT", "DARK"]) {
      const button = document.createElement("button");
      button.textContent = mode;
      button.setAttribute("aria-pressed", String(mode === "AVG"));
      button.addEventListener("click", () => {
        this.meter = mode;
        for (const each of meter.querySelectorAll("button")) {
          each.setAttribute("aria-pressed", String(each === button));
        }
      });
      meter.append(button);
    }
    document.body.append(meter);
    document.body.addEventListener(
      "keydown",
      (event) => {
        if (event.key === STYLE_TOGGLE_KEY) {
          const style = this.styles.get(this.operated);
          this.styles.set(this.operated, style === "wireframe" ? "photorealistic" : "wireframe");
          this.#draw();
        }
      },
      { signal: listening.signal },
    );
    this.#draw();
  }

  /** A fieldset whose legend is `legend`, with a button for each label. */
  #fieldset(legend: string, labels: ReadonlyArray<string>): HTMLFieldSetElement {
    const set = document.createElement("fieldset");
    const title = document.createElement("legend");
    title.textContent = legend;
    set.append(title);
    for (const label of labels) {
      const button = document.createElement("button");
      button.textContent = label;
      button.addEventListener("click", () => {
        this.#pressed(legend, label, button);
      });
      set.append(button);
    }
    document.body.append(set);
    return set;
  }

  #pressed(legend: string, label: string, button: HTMLButtonElement): void {
    if (legend === "Scene") {
      this.scene = label;
      this.chosenAtMs = this.#clock();
      for (const each of button.parentElement?.querySelectorAll("button") ?? []) {
        each.setAttribute("aria-pressed", String(each === button));
      }
    } else if (legend === "CONTROLS") {
      this.operated = label;
    } else if (label === "OPEN") {
      this.styles.set(legend, "wireframe");
    } else if (label === "CLOSE") {
      this.styles.delete(legend);
    }
    this.#draw();
  }

  /** The canvases, one a view, named as `VIEW` names them. */
  #draw(): void {
    for (const canvas of document.querySelectorAll("canvas")) {
      canvas.remove();
    }
    for (const [slot, style] of this.styles) {
      const canvas = document.createElement("canvas");
      canvas.setAttribute("role", "application");
      canvas.setAttribute(
        "aria-label",
        `VIEW, ${style === "wireframe" ? "WIREFRAME" : "PHOTOREALISTIC"}, ${slot}, SEAT`,
      );
      document.body.append(canvas);
    }
  }
}

/** What the fake probe saw the run do. */
interface Seen {
  readonly phases: Array<{
    readonly name: TerrainCheckPhaseName;
    readonly quiet: QuietSpan | null;
  }>;
  destroyedAtMs: number | null;
}

/**
 * The terrain probe's members the run reads, over `view`: a session while `TERRAIN TEST` is
 * chosen, its script the chosen scene's time, its first payload's five blocks sent a second apart
 * from its start, the delta's three from 121.5 s, bakes in flight from the delta's first block;
 * a destruction restored 300 ms later, the terrain drawn from 40 s and again 100 ms after the
 * restore, and the mask's cells by the blocks sent.
 */
function fakeProbe(view: FakeView, clock: () => number, seen: Seen): TerrainCheckDeps["terrain"] {
  const session: WatchedSession = {
    pass: () => null,
    wirePass: () => null,
    coverageMask: () => null,
    survey: () => null,
    memory: () => ({ workerFieldBytes: 0, keptFieldBytes: 0, gpuBytes: new Map() }),
    workerCount: 3,
  };
  const terrain = (): boolean => view.scene === TERRAIN_SCENE_NAME;
  const scriptS = (): number | null => (terrain() ? (clock() - view.chosenAtMs) / 1000 : null);
  /** The blocks the session's fixture link has sent: each choice of TERRAIN TEST makes one anew. */
  const sent = (): number => {
    const tS = scriptS() ?? 0;
    const first = Math.min(5, Math.floor(tS));
    return first + DELTA_BLOCKS_AT_S.filter((at) => tS >= at).length;
  };
  const restoredAtMs = (): number | null =>
    seen.destroyedAtMs === null ? null : seen.destroyedAtMs + 300;
  const texture = (): TextureHandle => ({
    kind: "texture",
    name: restoredAtMs() !== null && clock() >= (restoredAtMs() ?? 0) ? "after" : "before",
  });
  return {
    get session() {
      return terrain() ? session : null;
    },
    moon: "5e5e000000000000.0003",
    get scriptS() {
      return scriptS();
    },
    workers: () => ({
      made: 0,
      live: 0,
      bakesPosted: (scriptS() ?? 0) >= 121.5 ? 10 : 0,
      baked: (scriptS() ?? 0) >= 125 ? 10 : 0,
      refused: 0,
      inits: 0,
      chunks: 0,
      releases: 0,
    }),
    mark: (): TerrainMark => {
      const tS = scriptS() ?? 0;
      const blocks = sent();
      return {
        atMs: clock(),
        scriptS: scriptS(),
        workers: {
          made: 3,
          live: terrain() ? 3 : 0,
          bakesPosted: tS >= 121.5 ? 10 : 0,
          baked: tS >= 125 ? 10 : 0,
          refused: 0,
          inits: Math.min(blocks, 1) * 3,
          chunks: Math.max(0, blocks - 1) * 3,
          releases: 0,
        },
        fixture: {
          asks: tS >= 120 ? 2 : 1,
          fieldAsks: tS >= 120 ? 2 : 1,
          blocksSent: blocks,
          blocksToCome: Math.max(0, (tS >= 120 ? 8 : 5) - blocks),
        },
      };
    },
    resetPeaks: () => undefined,
    destroyDevice: () => {
      seen.destroyedAtMs = clock();
      return true;
    },
    firstTerrainSince: (_view, sinceMs) => {
      const at = view.chosenAtMs + TERRAIN_AT_S * 1000;
      const restored = restoredAtMs();
      const first = Math.max(
        sinceMs,
        restored === null || sinceMs < restored ? at : restored + 100,
      );
      return terrain() && clock() >= first ? first : null;
    },
    readMask: (): Promise<MaskReading | null> =>
      Promise.resolve(
        terrain()
          ? {
              texture: texture(),
              cells: new Set(Array.from({ length: 24 + 8 * Math.max(0, sent() - 5) }, (_, i) => i)),
            }
          : null,
      ),
    coverageCells: () => new Set(Array.from({ length: 48 }, (_, i) => i)),
    faults: (): TerrainCheckFault[] =>
      seen.destroyedAtMs === null
        ? []
        : [{ atMs: seen.destroyedAtMs, kind: "device-lost", reason: "destroyed", line: "lost" }],
    teardown: () => ({
      disposed: !terrain(),
      workersMade: 6,
      workersLive: terrain() ? 3 : 0,
      sessionGpuBytes: 0,
      liveBytes: [],
    }),
    phase: (
      name: TerrainCheckPhaseName,
      start: TerrainMark,
      end: TerrainMark,
      shown: ReadonlyArray<ShownView>,
      quiet: QuietSpan | null,
    ): TerrainCheckPhaseRecord => {
      seen.phases.push({ name, quiet });
      return {
        name,
        startMs: start.atMs,
        endMs: end.atMs,
        scriptStartS: start.scriptS ?? 0,
        scriptEndS: end.scriptS ?? 0,
        views: shown.map((each) => ({
          name: each.name,
          style: each.style,
          canvas: { widthPx: 1126, heightPx: 906 },
          draws: 1,
          gpuMs: [],
          submitMs: [],
          missedFrames: 0,
          longestGapMs: 0,
          handedOverDraws: 0,
          offSurveyDraws: 0,
          terrainDraws: 0,
          terrainGaps: 0,
          terrainGapSpansS: [],
          firstHandoverMs: null,
          firstHandoverScriptS: null,
          firstTerrainMs: null,
          readBacks: 0,
          nonFiniteTexels: 0,
        })),
        frameIntervalsMs: [],
        mainThreadMs: [],
        frameGpuMs: [],
        patchesDrawn: [],
        bakes: { posted: 0, baked: 0, refused: 0 },
        fieldPosts: { inits: 0, chunks: 0, releases: 0 },
        fieldAsks: 0,
        blocksSent: 0,
        memory: null,
      };
    },
  };
}

/** The run over a fake `VIEW` and probe on the test's clock, to its end. */
async function run(): Promise<{
  readonly record: Awaited<ReturnType<typeof runTerrainCheck>>;
  readonly calls: ReadonlyArray<string>;
  readonly seen: Seen;
  readonly view: FakeView;
}> {
  let now = 0;
  const clock = (): number => now;
  const view = new FakeView(clock);
  const seen: Seen = { phases: [], destroyedAtMs: null };
  const calls: string[] = [];
  const terrain = fakeProbe(view, clock, seen);
  const record = await runTerrainCheck({
    api: {
      startPhase: (name: CheckPhaseName) => {
        calls.push(`start ${name}`);
        return Promise.resolve();
      },
      endPhase: (name: CheckPhaseName) => {
        calls.push(`end ${name}`);
        return Promise.resolve();
      },
      capture: (label) => {
        calls.push(`capture ${label} at ${(terrain.scriptS ?? 0).toFixed(1)} s`);
        return Promise.resolve();
      },
    },
    views: {
      canvasOf: () => ({ widthPx: 1126, heightPx: 906 }),
      timer: () => "full",
      restoresAtMs: () =>
        seen.destroyedAtMs !== null && now >= seen.destroyedAtMs + 300
          ? [seen.destroyedAtMs + 300]
          : [],
    },
    terrain,
    setting: "high",
    document,
    timing: TIMING,
    nowMs: clock,
    markWindow: () => undefined,
    sleep: (ms) => {
      now += ms;
      return Promise.resolve();
    },
    signal: new AbortController().signal,
    devicePixelRatio: 1,
  });
  return { record, calls, seen, view };
}

describe("the terrain run's script (R10.T16.d)", () => {
  it("runs its setting's phases in order, its looks in the first, and ends on a kept scene with no terrain", async () => {
    const { calls, view, seen } = await run();
    expect(calls.filter((call) => call.startsWith("start") || call.startsWith("end"))).toEqual([
      "start terrain-alone",
      "end terrain-alone",
      "start terrain-photoreal-wireframe",
      "end terrain-photoreal-wireframe",
      "start terrain-loss",
      "end terrain-loss",
    ]);
    expect(calls.filter((call) => call.startsWith("capture"))).toEqual([
      expect.stringMatching(/^capture early at 1\d\.\d s$/),
      expect.stringMatching(/^capture first-terrain at 4\d\.\d s$/),
      expect.stringMatching(/^capture restored at 12\d\.\d s$/),
    ]);
    expect([view.scene, view.auto, view.meter]).toEqual([PHASE_SCENE_NAME, true, "LIT"]);
    expect(seen.phases.map((phase) => phase.name)).toEqual([
      "terrain-alone",
      "terrain-photoreal-wireframe",
      "terrain-loss",
    ]);
  });

  it("destroys the device once, mid-stream, after the delta's first block, and leaves the restore out", async () => {
    const { record, seen } = await run();
    expect(record.loss).toMatchObject({
      blocksToCome: 2,
      bakesInFlight: 10,
      notDestroyed: null,
    });
    expect(record.loss.destroyedAtScriptS).toBeGreaterThanOrEqual(121.5);
    expect(record.loss.destroyedAtScriptS).toBeLessThan(123);
    expect(record.loss.terrainAgainAtMs).toBe((seen.destroyedAtMs ?? 0) + 400);
    expect(seen.phases.at(-1)?.quiet).toEqual({
      fromMs: seen.destroyedAtMs,
      toMs: (seen.destroyedAtMs ?? 0) + 400,
    });
  });

  it("reads the mask before the loss, after the restore and at the end, against the coverage", async () => {
    const { record } = await run();
    expect(record.loss.masks).toEqual({
      before: 32,
      afterRestore: 32,
      end: 48,
      expected: 48,
      beforeKept: true,
      endAsExpected: true,
    });
    expect(record.loss.after).toMatchObject({ fieldAsks: 0, blocksSent: 2, fieldChunks: 6 });
  });

  it("records the faults, the teardown and the hand-over", async () => {
    const { record } = await run();
    expect(record.faults).toHaveLength(1);
    expect(record.teardown).toMatchObject({ disposed: true, workersLive: 0 });
    expect(record.handover?.scriptS).toBeGreaterThan(0);
  });
});
