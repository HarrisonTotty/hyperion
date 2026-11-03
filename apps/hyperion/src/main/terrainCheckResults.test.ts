import { describe, expect, it } from "vitest";

import type {
  TerrainCheckFault,
  TerrainCheckRecord,
  TerrainCheckViewRecord,
  TerrainWholeRecord,
} from "../preload/api";
import { DESTROYED_AT_MS, LOSS_WINDOW, TERRAIN_RECORD } from "./fixtures/terrainRecord";
import { measured, type PartialResultsFiles } from "./results";
import {
  buildTerrainCheckResults,
  buildTerrainPhases,
  TERRAIN_DELAY_LIMIT_MS,
  TERRAIN_GAP_LIMIT_MS,
  TERRAIN_PHASES,
  type TerrainCheckInput,
  terrainCheckMarkdown,
  terrainFailures,
  terrainFaultsVerdict,
  type TerrainPhasesResults,
  TerrainResultsWriter,
  writeTerrainResults,
} from "./terrainCheckResults";

/** Files held in `written`, by path, which a removal deletes. */
function memoryFiles(written: Map<string, string>): PartialResultsFiles {
  return {
    mkdir: () => Promise.resolve(),
    exists: (path) => Promise.resolve(written.has(path)),
    writeFile: (path, text) => {
      written.set(path, text);
      return Promise.resolve();
    },
    remove: (path) => {
      written.delete(path);
      return Promise.resolve();
    },
  };
}

function input(overrides: Partial<TerrainCheckInput> = {}): TerrainCheckInput {
  return {
    run: {
      startedAt: new Date("2026-10-19T23:00:00Z"),
      machine: {
        name: "effect",
        cpu: "AMD Ryzen 7 3700X",
        logicalCores: 16,
        memoryBytes: 32e9,
        governor: measured("schedutil"),
        loadAverage: [3.1, 3.4, 3.6],
        gpu: measured({ vendorId: 4318, deviceId: 8710, driverVersion: null, description: null }),
      },
      versions: { app: "0.1.0", electron: "44", chromium: "152", node: "24", v8: "15" },
      platform: "linux",
      launchMode: "vulkan",
      setting: "high",
      smoke: false,
      fixture: "terrain-test",
      switches: [],
      shown: false,
      window: { widthDip: 1926, heightDip: 1086 },
      captures: measured("/repo/target/views-check/terrain/captures"),
    },
    record: TERRAIN_RECORD,
    gpuProcessExits: 0,
    consoleErrors: [],
    ...overrides,
  };
}

/** `TERRAIN_RECORD` with one change. */
function changed(change: Partial<TerrainCheckRecord>): TerrainCheckInput {
  return input({ record: { ...TERRAIN_RECORD, ...change } });
}

/** A device loss at `atMs`, with `reason`. */
function lossAt(atMs: number, reason = "destroyed"): TerrainCheckFault {
  return { atMs, kind: "device-lost", reason, line: `device-lost (${reason}): gone` };
}

describe("the terrain run's results", () => {
  it("pass a record that holds every check", () => {
    const results = buildTerrainCheckResults(input());
    expect(results.overall).toBe("pass");
    expect(terrainFailures(results)).toBe("");
    expect(results.run.provisional).toMatch(/load average 3\.10 at the start/);
  });

  it("run each setting's three phases, the loss last", () => {
    expect(TERRAIN_PHASES.high.map((phase) => phase.name)).toEqual([
      "terrain-alone",
      "terrain-photoreal-wireframe",
      "terrain-loss",
    ]);
    expect(TERRAIN_PHASES.low.map((phase) => phase.name)).toEqual([
      "terrain-alone",
      "terrain-wireframe",
      "terrain-loss",
    ]);
    expect(TERRAIN_PHASES.low[1]?.views).toEqual(["photorealistic", "wireframe"]);
  });

  it("fail a run whose phases are not its setting's", () => {
    const results = buildTerrainCheckResults({
      ...input(),
      run: { ...input().run, setting: "low" },
    });
    expect(results.findings.phases.verdict).toBe("fail");
  });

  it("fail a phase whose views did not hold their styles", () => {
    const [alone, ...rest] = TERRAIN_RECORD.phases;
    if (alone === undefined) {
      throw new Error("the fixture has no first phase");
    }
    const views = alone.views.map((view) =>
      Object.assign({}, view, { style: "wireframe" as const }),
    );
    const results = buildTerrainCheckResults(changed({ phases: [{ ...alone, views }, ...rest] }));
    expect(results.findings.phases.reasons).toEqual([
      "terrain-alone did not hold its views' styles",
    ]);
  });

  it("fail a view that missed a frame it was due", () => {
    const [alone, ...rest] = TERRAIN_RECORD.phases;
    if (alone === undefined) {
      throw new Error("the fixture has no first phase");
    }
    const views = alone.views.map((view) => Object.assign({}, view, { missedFrames: 2 }));
    const results = buildTerrainCheckResults(changed({ phases: [{ ...alone, views }, ...rest] }));
    expect(results.findings.framesDrawn.reasons).toEqual([
      "terrain-alone: PRIMARY missed 2 frames it was due",
    ]);
    expect(results.overall).toBe("fail");
  });

  describe("no view stopped (R10.T14.d; decision-r10-t16-leans §2)", () => {
    /** The record with the first phase's primary's longest gap `gapMs`, ending at 68.1 s. */
    function gapOf(gapMs: number): TerrainCheckInput {
      const [alone, ...rest] = TERRAIN_RECORD.phases;
      if (alone === undefined) {
        throw new Error("the fixture has no first phase");
      }
      const views = alone.views.map((view) =>
        Object.assign({}, view, { longestGapMs: gapMs, longestGapEndScriptS: 68.1 }),
      );
      return changed({ phases: [{ ...alone, views }, ...rest] });
    }

    it("passes a view whose longest gap is the limit", () => {
      expect(buildTerrainCheckResults(gapOf(TERRAIN_GAP_LIMIT_MS)).findings.noStall).toEqual({
        verdict: "pass",
        reasons: [],
        limitMs: TERRAIN_GAP_LIMIT_MS,
      });
    });

    it("fails a view whose longest gap is over the limit, saying where it ended", () => {
      const results = buildTerrainCheckResults(gapOf(1_317));
      expect([results.findings.noStall.reasons, results.overall]).toEqual([
        [
          `terrain-alone: PRIMARY drew nothing for 1317 ms to 68.1 s of the script, over ${String(TERRAIN_GAP_LIMIT_MS)} ms`,
        ],
        "fail",
      ]);
    });

    it("tells a stall from a missed frame: a stall misses none", () => {
      const results = buildTerrainCheckResults(gapOf(1_317));
      expect([results.findings.framesDrawn.verdict, results.findings.noStall.verdict]).toEqual([
        "pass",
        "fail",
      ]);
    });

    it("fails a run whose page drew no frame for longer than the limit between its phases", () => {
      const results = buildTerrainCheckResults(
        changed({
          span: {
            ...TERRAIN_RECORD.span,
            longestFrameGapMs: 1_128,
            longestGapEndMs: 275_000,
            longestGapEndScriptS: 3.1,
          },
        }),
      );
      expect([results.findings.noStall.reasons, results.overall]).toEqual([
        [
          `the page drew no animation frame for 1128 ms between the phases, to 3.1 s of the script, over ${String(TERRAIN_GAP_LIMIT_MS)} ms`,
        ],
        "fail",
      ]);
    });

    it("places the page's longest gap in the phase whose window holds it", () => {
      const results = buildTerrainCheckResults(
        changed({
          span: { ...TERRAIN_RECORD.span, longestFrameGapMs: 650, longestGapEndScriptS: null },
        }),
      );
      expect(results.findings.noStall.reasons).toEqual([
        `the page drew no animation frame for 650 ms in terrain-photoreal-wireframe, over ${String(TERRAIN_GAP_LIMIT_MS)} ms`,
      ]);
    });

    it("judges nothing of a session whose compile had not ended", () => {
      const results = buildTerrainCheckResults(
        changed({ sessions: [{ madeAtMs: 10, compile: { kind: "pending" } }] }),
      );
      expect(results.findings.noStall.verdict).toBe("pass");
      expect(terrainCheckMarkdown(results)).toContain(
        "(judged by the same limit): session 1 not compiled by the run's end.",
      );
    });

    it("records VIEW's opening, judging nothing by it", () => {
      const results = buildTerrainCheckResults(input());
      expect([results.opening, results.findings.noStall.verdict]).toEqual([
        { preparedMs: 900, preparations: 3, longestFrameGapMs: 1_100 },
        "pass",
      ]);
      expect(terrainCheckMarkdown(results)).toContain(
        "VIEW's opening (recorded, not judged): its views' 3 preparations settled 900 ms after the run's start, the longest time between animation frames meanwhile 1100 ms.",
      );
    });

    it("states the limit, the page's longest gap and each view's in the summary", () => {
      const text = terrainCheckMarkdown(buildTerrainCheckResults(gapOf(1_317)));
      expect(text).toContain(
        `| No view stopped (no time without a frame or a view's draw over ${String(TERRAIN_GAP_LIMIT_MS)} ms from VIEW's opening to the teardown, outside the restore) | fail | the page's longest time between animation frames 67 ms (in terrain-photoreal-wireframe); the longest time between draws: terrain-alone PRIMARY 1317 ms to 68.1 s of the script;`,
      );
    });

    it("fails a session whose compile window held no frame for longer than the limit", () => {
      const stalled = {
        madeAtMs: 10,
        compile: { kind: "ended", compileMs: 1_450, compiled: 3, longestFrameGapMs: 1_317 },
      } as const;
      const results = buildTerrainCheckResults(changed({ sessions: [stalled] }));
      expect([results.findings.noStall.reasons, results.sessions, results.overall]).toEqual([
        [
          `session 1: no animation frame for 1317 ms while its materials compiled, over ${String(TERRAIN_GAP_LIMIT_MS)} ms`,
        ],
        [stalled],
        "fail",
      ]);
      expect(terrainCheckMarkdown(results)).toContain(
        "The sessions' materials, compiled as each is made, before its phases (judged by the same limit): session 1 3 compiled in 1450 ms, the longest time between animation frames meanwhile 1317 ms.",
      );
    });
  });

  it("fail a texel that is not finite, and a photorealistic view never read back", () => {
    const [alone, instruments, loss] = TERRAIN_RECORD.phases;
    if (alone === undefined || instruments === undefined || loss === undefined) {
      throw new Error("the fixture has three phases");
    }
    const results = buildTerrainCheckResults(
      changed({
        phases: [
          {
            ...alone,
            views: alone.views.map((view) => Object.assign({}, view, { nonFiniteTexels: 3 })),
          },
          {
            ...instruments,
            views: instruments.views.map((view) => Object.assign({}, view, { readBacks: 0 })),
          },
          loss,
        ],
      }),
    );
    expect(results.findings.framesFinite.reasons).toEqual([
      "terrain-alone: PRIMARY had 3 texels not finite",
      "terrain-photoreal-wireframe: PRIMARY's scene target was never read back",
      "terrain-photoreal-wireframe: INSTRUMENT 1's scene target was never read back",
    ]);
  });

  it("times the terrain from the hand-over it was first drawn in (R10.T14.d)", () => {
    const [alone, ...rest] = TERRAIN_RECORD.phases;
    if (alone === undefined) {
      throw new Error("the fixture has no first phase");
    }
    // A lone handed-over draw 50 s before the hand-over the terrain was drawn in.
    const views = alone.views.map((view) =>
      Object.assign({}, view, {
        firstHandoverMs: (view.firstTerrainMs ?? 0) - 50_000,
        terrainHandoverMs: (view.firstTerrainMs ?? 0) - 100,
      }),
    );
    const results = buildTerrainCheckResults(changed({ phases: [{ ...alone, views }, ...rest] }));
    expect([
      results.phases[0]?.views[0]?.terrainDelayMs,
      results.findings.terrainDrawn.verdict,
    ]).toEqual([100, "pass"]);
  });

  it("fail a first phase whose primary never handed over", () => {
    const [alone, ...rest] = TERRAIN_RECORD.phases;
    if (alone === undefined) {
      throw new Error("the fixture has no first phase");
    }
    const views = alone.views.map((view) => ({
      ...view,
      handedOverDraws: 0,
      terrainDraws: 0,
      firstHandoverMs: null,
      firstHandoverScriptS: null,
      firstTerrainMs: null,
    }));
    const results = buildTerrainCheckResults(changed({ phases: [{ ...alone, views }, ...rest] }));
    expect(results.findings.terrainDrawn.reasons).toEqual([
      "terrain-alone: the primary never handed TEST MOON over",
    ]);
  });

  it.each([
    ["never drew the terrain", { firstTerrainMs: null }, /never drew the terrain/],
    [
      "drew it too late",
      { firstTerrainMs: 3_000 + 1_200 + TERRAIN_DELAY_LIMIT_MS + 1 },
      /after its hand-over, over/,
    ],
    [
      "lost it after its first",
      { terrainGaps: 4, terrainGapSpansS: [[120.5, 120.6]] },
      /4 handed-over frames without the terrain once drawn \(120\.5–120\.6 s of the script\)/,
    ],
  ] as const)("fail a view that handed over and %s", (_what, change, reason) => {
    const [alone, ...rest] = TERRAIN_RECORD.phases;
    if (alone === undefined) {
      throw new Error("the fixture has no first phase");
    }
    const views = alone.views.map((view) => ({ ...view, ...change }));
    const results = buildTerrainCheckResults(changed({ phases: [{ ...alone, views }, ...rest] }));
    expect(results.findings.terrainDrawn.verdict).toBe("fail");
    expect(results.findings.terrainDrawn.reasons.join("; ")).toMatch(reason);
  });

  describe("a wireframe view handing over to its wireframe pass (R10.T11)", () => {
    /** The fixture with its wireframe instrument's figures changed by `change`. */
    function wireframeChanged(change: Partial<TerrainCheckViewRecord>): TerrainCheckInput {
      return changed({
        phases: TERRAIN_RECORD.phases.map((phase) => ({
          ...phase,
          views: phase.views.map((view) =>
            view.style === "wireframe" ? { ...view, ...change } : view,
          ),
        })),
      });
    }

    it("passes where it handed over and drew its terrain", () => {
      const wireframe = TERRAIN_RECORD.phases
        .flatMap((phase) => phase.views)
        .filter((view) => view.style === "wireframe");
      if (!wireframe.some((view) => view.handedOverDraws > 0 && view.firstTerrainMs !== null)) {
        throw new Error("the fixture's wireframe view hands over and draws");
      }
      expect(buildTerrainCheckResults(input()).findings.terrainDrawn).toMatchObject({
        verdict: "pass",
        reasons: [],
      });
    });

    it("fails where it handed over and never drew its terrain, for that reason alone", () => {
      const results = buildTerrainCheckResults(
        wireframeChanged({ terrainDraws: 0, firstTerrainMs: null }),
      );
      expect(results.findings.terrainDrawn.reasons).toEqual([
        "terrain-photoreal-wireframe: INSTRUMENT 2 handed over and never drew the terrain",
      ]);
    });

    it("fails where it drew its terrain too late after its hand-over, as a photorealistic view does", () => {
      const view = TERRAIN_RECORD.phases[1]?.views.find((each) => each.style === "wireframe");
      const handoverMs = view?.firstHandoverMs ?? 0;
      const results = buildTerrainCheckResults(
        wireframeChanged({ firstTerrainMs: handoverMs + TERRAIN_DELAY_LIMIT_MS + 1 }),
      );
      expect(results.findings.terrainDrawn.reasons.join("; ")).toMatch(
        /INSTRUMENT 2 drew the terrain \d+ ms after its hand-over, over/,
      );
    });
  });

  it.each([
    ["a destruction with no block to come", { blocksToCome: 0 }, /no block was still to come/],
    ["a destruction with no bake in flight", { bakesInFlight: 0 }, /no bake was in flight/],
    ["no terrain after the restore", { terrainAgainAtMs: null }, /not drawn again/],
    [
      "a field asked again",
      { after: { ...TERRAIN_RECORD.loss.after, fieldAsks: 1 } },
      /1 surface_field asks/,
    ],
    [
      "a field started again",
      { after: { ...TERRAIN_RECORD.loss.after, fieldInits: 3 } },
      /3 field-init posts/,
    ],
    [
      "a block posted again",
      { after: { ...TERRAIN_RECORD.loss.after, fieldChunks: 9 } },
      /9 field-chunk posts after the loss, where the 2 blocks then sent to 3 workers make 6/,
    ],
    [
      "a cell lost at the restore",
      { masks: { ...TERRAIN_RECORD.loss.masks, beforeKept: false } },
      /lost a cell/,
    ],
    [
      "a mask unlike the coverage",
      { masks: { ...TERRAIN_RECORD.loss.masks, end: 47, endAsExpected: false } },
      /47 cells at the end, not the coverage's 48/,
    ],
    [
      "a mask unread",
      { masks: { ...TERRAIN_RECORD.loss.masks, afterRestore: null } },
      /not read back/,
    ],
    [
      "no destruction",
      { destroyedAtMs: null, notDestroyed: "the delta did not stream" },
      /not destroyed: the delta did not stream/,
    ],
  ] as const)("fail the loss with %s", (_what, change, reason) => {
    const results = buildTerrainCheckResults(
      changed({ loss: { ...TERRAIN_RECORD.loss, ...change } }),
    );
    expect(results.findings.loss.verdict).toBe("fail");
    expect(results.findings.loss.reasons.join("; ")).toMatch(reason);
  });

  it.each([
    ["the session left undisposed", { disposed: false }, /not disposed/],
    ["a worker left", { workersLive: 2 }, /2 height workers left/],
    ["the session's textures left", { sessionGpuBytes: 4096 }, /4096 B of the session's textures/],
    [
      "the terrain's bytes left",
      { liveBytes: [{ category: "height-cache", bytes: 1024 }] },
      /1024 B of the terrain's height-cache left/,
    ],
  ] as const)("fail the teardown with %s", (_what, change, reason) => {
    const results = buildTerrainCheckResults(
      changed({ teardown: { ...TERRAIN_RECORD.teardown, ...change } }),
    );
    expect(results.findings.teardown.verdict).toBe("fail");
    expect(results.findings.teardown.reasons.join("; ")).toMatch(reason);
  });

  it("record the hand-over expected and seen, and where the both-surfaces check is held", () => {
    const { findings } = buildTerrainCheckResults(input());
    expect(findings.handover).toEqual({
      rangeM: 2.08e7,
      expectedScriptS: 38.2,
      observedScriptS: 38.5,
    });
    expect(findings.bothSurfaces).toMatch(/part-6/);
  });

  it.each([
    [
      "its height workers lost",
      "TERRAIN NOT AVAILABLE: height computation failed, reopen VIEW to retry",
    ],
    [
      "a computation failing on",
      "TERRAIN NOT AVAILABLE: terrain computation failed, reopen VIEW to retry",
    ],
  ])(
    "fail a run whose terrain session showed the terrain's fault, for %s (R10.T16.e)",
    (_cause, text) => {
      const results = buildTerrainCheckResults(
        changed({ terrainFaults: [{ atMs: 41_000, text, views: ["view"] }] }),
      );
      expect([
        results.findings.terrainFaults.verdict,
        results.overall,
        terrainFailures(results),
      ]).toEqual(["fail", "fail", `the terrain's fault at 41000 ms on view: ${text}`]);
    },
  );

  it("pass a run whose terrain session never showed the terrain's fault", () => {
    const { findings } = buildTerrainCheckResults(input());
    expect(findings.terrainFaults).toEqual({ verdict: "pass", reasons: [], record: [] });
  });

  it("list the console's first errors", () => {
    const errors = Array.from({ length: 7 }, (_, i) => `error ${String(i)}`);
    const { findings } = buildTerrainCheckResults(input({ consoleErrors: errors }));
    expect(findings.consoleErrors).toEqual({ count: 7, first: errors.slice(0, 5) });
  });
});

/** The faults verdict over the fixture's loss window. */
function verdict(
  faults: ReadonlyArray<TerrainCheckFault>,
  restoresAtMs: ReadonlyArray<number> = [DESTROYED_AT_MS + 400],
  gpuProcessExits = 0,
): ReturnType<typeof terrainFaultsVerdict> {
  return terrainFaultsVerdict({
    faults,
    restoresAtMs,
    lossWindow: LOSS_WINDOW,
    destroyedAtMs: DESTROYED_AT_MS,
    gpuProcessExits,
  });
}

/** A whole-body run's readings that hold every check (R10.T14.c). */
const WHOLE: TerrainWholeRecord = {
  cells: 24_576,
  heldCells: 24_576,
  textSamples: 400,
  notSurveyedSamples: 0,
  surveyEdgeSamples: 0,
  decodeMs: 3_100,
  decodeLongestFrameGapMs: 140,
  cacheBytes: 400_000_000,
  selectionBudget: 604,
  looks: (["orbit", "cube-corner", "antipode"] as const).map((name) => ({
    kind: "read" as const,
    name,
    settled: true,
    discPixels: 90_000,
    skyPixels: 0,
    outsidePixels: 40_000,
    litOutsidePixels: 0,
    finite: true,
  })),
};

/** {@link TERRAIN_RECORD} as a whole-body run's, its readings `whole` and its mask whole. */
function wholeInput(whole: TerrainWholeRecord = WHOLE): TerrainCheckInput {
  return input({
    run: { ...input().run, fixture: "terrain-test-whole" },
    record: {
      ...TERRAIN_RECORD,
      loss: {
        ...TERRAIN_RECORD.loss,
        masks: { ...TERRAIN_RECORD.loss.masks, end: 24_576, expected: 24_576 },
      },
      whole,
    },
  });
}

describe("the whole-body run's results (R10.T14.c)", () => {
  it("pass a run that held every cell, read no off-survey state and drew no sky in a disc", () => {
    const results = buildTerrainCheckResults(wholeInput());
    expect([results.findings.whole?.verdict, results.overall, terrainFailures(results)]).toEqual([
      "pass",
      "pass",
      "",
    ]);
    expect([
      results.findings.whole?.peakCacheBytes,
      results.findings.whole?.mostPatchesSelected,
      results.findings.whole?.mostPatchesDrawn,
    ]).toEqual([20_000_000, 55, 61]);
  });

  it("pass a run whose draw passes the selection's budget by a coarsen's held patches, its selection within it (R10.T16.i)", () => {
    const results = buildTerrainCheckResults(wholeInput({ ...WHOLE, selectionBudget: 58 }));
    expect([results.findings.whole?.verdict, results.findings.whole?.reasons]).toEqual([
      "pass",
      [],
    ]);
  });

  it.each([
    [{ heldCells: 48 }, "the coverage held 48 of 24576 cells as the first phase ended"],
    [{ notSurveyedSamples: 3 }, "NOT SURVEYED read in 3 of 400 samples"],
    [{ surveyEdgeSamples: 2 }, "a SURVEY EDGE named in 2 of 400 samples"],
    [{ textSamples: 0 }, "the page's text was never sampled"],
    [{ selectionBudget: 54 }, "55 patches selected at once, over the selection's budget of 54"],
    [
      { selectionBudget: 30 },
      "61 patches drawn at once, over twice the selection's budget, 60, the cache's slots",
    ],
    [{ selectionBudget: null }, "no lit pass's selection budget was read"],
  ] as const)("fail on %j", (change, reason) => {
    const results = buildTerrainCheckResults(wholeInput({ ...WHOLE, ...change }));
    expect([results.findings.whole?.verdict, results.overall]).toEqual(["fail", "fail"]);
    expect(results.findings.whole?.reasons).toContain(reason);
  });

  it.each([
    [{ skyPixels: 12 }, "the orbit look drew 12 of its disc's 90000 pixels as the sky"],
    [{ litOutsidePixels: 5 }, "the orbit look drew 5 of 40000 pixels lit beyond the limb"],
    [{ settled: false }, "the orbit look's terrain did not settle"],
    [{ finite: false }, "the orbit look read a texel that is not finite"],
    [{ discPixels: 0 }, "the orbit look read no pixel of its disc"],
  ] as const)("fail a look with %j", (change, reason) => {
    const [orbit, ...rest] = WHOLE.looks;
    if (orbit === undefined) {
      throw new Error("the readings have an orbit look");
    }
    const results = buildTerrainCheckResults(
      wholeInput({ ...WHOLE, looks: [{ ...orbit, ...change }, ...rest] }),
    );
    expect(results.findings.whole?.reasons).toContain(reason);
  });

  it("fail a look that was not read, naming why", () => {
    const [, ...rest] = WHOLE.looks;
    const failed = {
      kind: "failed",
      name: "orbit",
      settled: false,
      failure: "no scene target",
    } as const;
    const results = buildTerrainCheckResults(wholeInput({ ...WHOLE, looks: [failed, ...rest] }));
    expect(results.findings.whole?.reasons).toContain(
      "the orbit look was not read: no scene target",
    );
  });

  it("fail where the coverage mask did not end whole, a reference sphere past the terrain", () => {
    const base = wholeInput();
    const results = buildTerrainCheckResults({
      ...base,
      record: {
        ...base.record,
        loss: { ...base.record.loss, masks: { ...base.record.loss.masks, end: 24_000 } },
      },
    });
    expect(results.findings.whole?.reasons).toContain(
      "the coverage mask held 24000 of 24576 cells at the loss phase's end",
    );
  });

  it("judge no whole body on any other run, and name the whole body's files and summary", () => {
    expect(buildTerrainCheckResults(input()).findings.whole).toBeUndefined();
    const results = buildTerrainCheckResults(wholeInput());
    const markdown = terrainCheckMarkdown(results);
    expect(markdown).toContain("TEST MOON's whole body, every cell surveyed");
    expect(markdown).toContain("| The whole body held and drawn");
    expect(results.run.fixture).toBe("terrain-test-whole");
  });
});

describe("the terrain run's faults verdict", () => {
  it("fails with no loss: the forced loss is expected once", () => {
    const result = verdict([], []);
    expect(result.verdict).toBe("fail");
    expect(result.reasons).toEqual(["no device loss was seen: the forced loss is expected once"]);
  });

  it("passes one loss inside the loss phase's window, then one restore, and excludes it alone", () => {
    const loss = lossAt(DESTROYED_AT_MS);
    const result = verdict([loss]);
    expect(result.verdict).toBe("pass");
    expect(result.expected).toBe(loss);
    expect(result.restores).toBe(1);
  });

  it("fails one loss outside the window", () => {
    const result = verdict([lossAt(LOSS_WINDOW.startMs - 10)], [LOSS_WINDOW.startMs - 5]);
    expect(result.verdict).toBe("fail");
    expect(result.expected).toBeNull();
    expect(result.reasons).toEqual(["the device loss fell outside the loss phase's window"]);
  });

  it("fails a loss inside the window but before the harness destroyed the device", () => {
    const result = verdict([lossAt(DESTROYED_AT_MS - 1_000)], [DESTROYED_AT_MS - 600]);
    expect([result.expected, result.reasons]).toEqual([
      null,
      ["the device loss came before the harness destroyed the device"],
    ]);
  });

  it("fails two losses", () => {
    const result = verdict(
      [lossAt(DESTROYED_AT_MS), lossAt(DESTROYED_AT_MS + 2_000)],
      [DESTROYED_AT_MS + 400, DESTROYED_AT_MS + 2_400],
    );
    expect(result.verdict).toBe("fail");
    expect(result.reasons).toEqual(["2 device losses: the forced loss is expected once"]);
  });

  it("fails a loss for another reason than its destruction", () => {
    const result = verdict([lossAt(DESTROYED_AT_MS, "unknown")]);
    expect(result.reasons).toEqual(["the device was lost with reason unknown, not destroyed"]);
  });

  it("fails an exit of the GPU process", () => {
    const result = verdict([lossAt(DESTROYED_AT_MS)], [DESTROYED_AT_MS + 400], 1);
    expect(result.verdict).toBe("fail");
    expect(result.reasons).toEqual(["1 exits of the GPU process"]);
  });

  it("fails any other fault beside the forced loss", () => {
    const refused: TerrainCheckFault = {
      atMs: DESTROYED_AT_MS + 600,
      kind: "view-refused",
      reason: null,
      line: "view-refused: instrument-1",
    };
    const result = verdict([lossAt(DESTROYED_AT_MS), refused]);
    expect(result.others).toEqual([refused]);
    expect(result.reasons).toEqual(["a fault: view-refused: instrument-1"]);
  });

  it("fails a loss with no restore, or a second restore", () => {
    expect(verdict([lossAt(DESTROYED_AT_MS)], []).reasons).toEqual([
      "0 restores after the forced loss, where one is expected",
    ]);
    expect(
      verdict([lossAt(DESTROYED_AT_MS)], [DESTROYED_AT_MS + 400, DESTROYED_AT_MS + 900]).reasons,
    ).toEqual(["2 restores after the forced loss, where one is expected"]);
  });
});

describe("the terrain run's summary and its files", () => {
  it("states each check's verdict and the figures, provisional", () => {
    const text = terrainCheckMarkdown(buildTerrainCheckResults(input()));
    expect(text).toContain("# Terrain run: effect, high, 2026-10-19");
    expect(text).toContain("- **Overall:** pass");
    expect(text).toMatch(/\| One forced loss, then one restore, and no other fault \| pass \|/);
    expect(text).toContain(
      "| No terrain fault (the session's terrainFaults never held a view) | pass | none |",
    );
    expect(text).toContain("## Phases (provisional)");
    expect(text).toContain("| terrain-loss |");
  });

  it("states each phase's triangles a frame, lit and casting, over the draws that counted them (plan R14)", () => {
    const text = terrainCheckMarkdown(buildTerrainCheckResults(input()));
    expect(text).toContain("## Triangles a frame (plan R14)");
    expect(text).toContain("| terrain-alone | 0.444 M, 0.531 M; 3 | 0.968 M, 1.032 M; 2 |");
  });

  it("gives each phase's GPU time by pass, the dearest first, on a run given --hyperion-gpu-timing (R14's Mac measurements)", () => {
    const [alone, ...rest] = TERRAIN_RECORD.phases;
    if (alone === undefined) {
      throw new Error("the fixture has no first phase");
    }
    const passes = [
      { label: "symbology", ms: [0.5, 0.5], kernels: [], workgroups: [] },
      { label: "bodies", ms: [2, 2.2], kernels: [], workgroups: [] },
    ];
    const results = buildTerrainCheckResults(changed({ phases: [{ ...alone, passes }, ...rest] }));
    // The fixture's frames' GPU time, 2.5 and 2.6 ms, has a mean of 2.55 ms.
    expect(results.phases.map((phase) => phase.gpuPasses)).toEqual([
      [
        {
          label: "bodies",
          gpuMs: { count: 2, p50Ms: 2, p95Ms: 2.2, p99Ms: 2.2, maxMs: 2.2, meanMs: 2.1 },
          shareOfGpu: 2.1 / 2.55,
          kernels: [],
        },
        {
          label: "symbology",
          gpuMs: { count: 2, p50Ms: 0.5, p95Ms: 0.5, p99Ms: 0.5, maxMs: 0.5, meanMs: 0.5 },
          shareOfGpu: 0.5 / 2.55,
          kernels: [],
        },
      ],
      null,
      null,
    ]);
    const text = terrainCheckMarkdown(results);
    expect(text).toContain("## GPU time by pass (provisional; R14's Mac measurements)");
    expect(text).toContain("| terrain-alone | bodies | 2 | 2.00 / 2.20 / 2.20 | 2.10 | 82.4% |");
  });

  it("reads no GPU time by pass where the run was not given --hyperion-gpu-timing", () => {
    const results = buildTerrainCheckResults(input());
    expect(results.phases.map((phase) => phase.gpuPasses)).toEqual([null, null, null]);
    expect(terrainCheckMarkdown(results)).toContain(
      "None recorded: the run was not given `--hyperion-gpu-timing`.",
    );
  });

  it("states the machine's CPU governor, as T20's summary does", () => {
    expect(terrainCheckMarkdown(buildTerrainCheckResults(input()))).toContain("governor schedutil");
  });

  it("names each file for its date, machine and setting, a smoke's and a repeat's apart", async () => {
    const written = new Map<string, string>();
    const files = memoryFiles(written);
    const results = buildTerrainCheckResults(input());
    const first = await writeTerrainResults("/out", results, files);
    const second = await writeTerrainResults("/out", results, files);
    const smoke = await writeTerrainResults(
      "/out",
      buildTerrainCheckResults({ ...input(), run: { ...input().run, smoke: true } }),
      files,
    );
    expect(first.json).toBe("/out/2026-10-19-effect-high-terrain.json");
    expect(second.json).toBe("/out/2026-10-19-effect-high-terrain-2.json");
    expect(smoke.markdown).toBe("/out/2026-10-19-effect-high-terrain-smoke.md");
    expect(JSON.parse(written.get(first.json) ?? "null")).toMatchObject({
      schema: "hyperion.views-check.terrain-results",
      version: 4,
      overall: "pass",
      findings: { terrainFaults: { verdict: "pass", reasons: [], record: [] } },
    });
  });

  it("writes the finished phases as each ends, then the results under the same name in their place (R14's Mac measurements)", async () => {
    const written = new Map<string, string>();
    const files = memoryFiles(written);
    // A stopped run's finished phases, whose name a later run of the day never takes.
    written.set("/out/2026-10-19-effect-high-terrain-partial.json", "{}");
    const run = input().run;
    const writer = new TerrainResultsWriter("/out", files);
    const [alone, wireframe] = TERRAIN_RECORD.phases;
    if (alone === undefined || wireframe === undefined) {
      throw new Error("the fixture has no first two phases");
    }
    const phases = (records: TerrainCheckRecord["phases"]): TerrainPhasesResults =>
      buildTerrainPhases({ run, timer: "quantized", devicePixelRatio: 2, phases: records });
    const first = await writer.writePhases(phases([alone]));
    const second = await writer.writePhases(phases([alone, wireframe]));
    expect([first, second.json]).toEqual([
      {
        json: "/out/2026-10-19-effect-high-terrain-2-partial.json",
        markdown: "/out/2026-10-19-effect-high-terrain-2-partial.md",
      },
      "/out/2026-10-19-effect-high-terrain-2-partial.json",
    ]);
    expect(JSON.parse(written.get(second.json) ?? "null")).toMatchObject({
      schema: "hyperion.views-check.terrain-phases",
      version: 1,
      run: { timer: "quantized", devicePixelRatio: 2 },
      phases: [{ name: "terrain-alone" }, { name: "terrain-photoreal-wireframe" }],
    });
    expect(written.get(second.markdown)).toContain(
      "No check is judged here; the results file, written as the run ends, takes this file's place.",
    );
    const results = await writer.writeResults(buildTerrainCheckResults(input()));
    expect([results.json, [...written.keys()].toSorted()]).toEqual([
      "/out/2026-10-19-effect-high-terrain-2.json",
      [
        "/out/2026-10-19-effect-high-terrain-2.json",
        "/out/2026-10-19-effect-high-terrain-2.md",
        "/out/2026-10-19-effect-high-terrain-partial.json",
      ],
    ]);
  });

  it("names a weathered run's files -terrain-weathered, its run block and summary naming its field (R10.T1.c)", async () => {
    const written = new Map<string, string>();
    const files = memoryFiles(written);
    const weathered = (smoke: boolean): TerrainCheckInput => ({
      ...input(),
      run: { ...input().run, smoke, fixture: "terrain-test-weathered" },
    });
    const full = await writeTerrainResults(
      "/out",
      buildTerrainCheckResults(weathered(false)),
      files,
    );
    const smoke = await writeTerrainResults(
      "/out",
      buildTerrainCheckResults(weathered(true)),
      files,
    );
    expect([full.json, smoke.markdown]).toEqual([
      "/out/2026-10-19-effect-high-terrain-weathered.json",
      "/out/2026-10-19-effect-high-terrain-weathered-smoke.md",
    ]);
    expect(JSON.parse(written.get(full.json) ?? "null")).toMatchObject({
      run: { smoke: false, fixture: "terrain-test-weathered" },
    });
    expect(written.get(full.markdown)).toContain(
      "- **Field:** `terrain-test-weathered`, TEST MOON weathered (`terrain_test_weathered.golden`), served in TERRAIN TEST's place",
    );
  });

  it("keeps TERRAIN TEST's run block, and its summary, as before the option (R10.T1.c)", () => {
    const results = buildTerrainCheckResults(input());
    expect([
      Object.keys(results.run),
      terrainCheckMarkdown(results).includes("**Field:**"),
    ]).toEqual([
      [
        "startedAt",
        "machine",
        "versions",
        "platform",
        "launchMode",
        "setting",
        "smoke",
        "switches",
        "shown",
        "window",
        "devicePixelRatio",
        "timer",
        "captures",
        "provisional",
      ],
      false,
    ]);
  });
});
