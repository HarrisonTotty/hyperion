import { describe, expect, it } from "vitest";

import type { TerrainCheckFault, TerrainCheckRecord, TerrainCheckViewRecord } from "../preload/api";
import { DESTROYED_AT_MS, LOSS_WINDOW, TERRAIN_RECORD } from "./fixtures/terrainRecord";
import { measured, type ResultsFiles } from "./results";
import {
  buildTerrainCheckResults,
  TERRAIN_DELAY_LIMIT_MS,
  TERRAIN_GAP_LIMIT_MS,
  TERRAIN_PHASES,
  type TerrainCheckInput,
  terrainCheckMarkdown,
  terrainFailures,
  terrainFaultsVerdict,
  writeTerrainResults,
} from "./terrainCheckResults";

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
      /4 handed-over frames without the terrain after its first \(120\.5–120\.6 s of the script\)/,
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

  it("states the machine's CPU governor, as T20's summary does", () => {
    expect(terrainCheckMarkdown(buildTerrainCheckResults(input()))).toContain("governor schedutil");
  });

  it("names each file for its date, machine and setting, a smoke's and a repeat's apart", async () => {
    const written = new Map<string, string>();
    const files: ResultsFiles = {
      mkdir: () => Promise.resolve(),
      exists: (path) => Promise.resolve(written.has(path)),
      writeFile: (path, text) => {
        written.set(path, text);
        return Promise.resolve();
      },
    };
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
      version: 3,
      overall: "pass",
      findings: { terrainFaults: { verdict: "pass", reasons: [], record: [] } },
    });
  });
});
