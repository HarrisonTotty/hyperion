import { afterEach, describe, expect, it, vi } from "vitest";

import type { SpikeSizesMemory } from "../preload/api";
import { type SizesFixtureOptions, sizesReportOf } from "./fixtures/sizesReport";
import { NO_WINDOWS_LOAD_AVERAGE } from "./machineLoad";
import { measured, missing } from "./measured";
import type { SpikeSetting } from "./results";
import {
  buildSizesResults,
  LOAD_INTERVAL_MS,
  LoadSampler,
  sizesResultsProblems,
  sizesSummaryLines,
} from "./sizesResults";

const MIB = 1024 ** 2;

afterEach(() => {
  vi.useRealTimers();
});

/** A load sampler on Linux whose 1-minute load rises by 0.25 at each reading, on the fake clock. */
function linuxSampler(): LoadSampler {
  let load = 0.5;
  return new LoadSampler({
    platform: "linux",
    loadavg: () => {
      const now = load;
      load += 0.25;
      return [now, 0.4, 0.3];
    },
    nowMs: () => Date.now(),
  });
}

describe("the 1-minute load of a measured run (decision-r10-t14b-measure §6(g))", () => {
  it("reads at the start, each minute and the end", () => {
    vi.useFakeTimers();
    const sampler = linuxSampler();
    sampler.start();
    vi.advanceTimersByTime(2 * LOAD_INTERVAL_MS + 30_000);
    expect(sampler.stop()).toEqual(
      measured([
        { tS: 0, oneMinute: 0.5 },
        { tS: 60, oneMinute: 0.75 },
        { tS: 120, oneMinute: 1 },
        { tS: 150, oneMinute: 1.25 },
      ]),
    );
  });

  it("reads none on Windows, which keeps no load average, and says so", () => {
    vi.useFakeTimers();
    const sampler = new LoadSampler({
      platform: "win32",
      loadavg: () => [0, 0, 0],
      nowMs: () => Date.now(),
    });
    sampler.start();
    vi.advanceTimersByTime(LOAD_INTERVAL_MS);
    expect(sampler.stop()).toEqual(missing(NO_WINDOWS_LOAD_AVERAGE));
  });

  it("says the load was never read where it never started", () => {
    expect(linuxSampler().stop()).toEqual(missing("the load was never read"));
  });
});

describe("the measured sizes in the results (R10.T14.b)", () => {
  const load = measured([{ tS: 0, oneMinute: 0.5 }]);

  it("adds the driver's figures: the peak less the baseline, and the card at the run's end", () => {
    const sizes = sizesReportOf();
    const results = buildSizesResults(sizes, load, {
      nvidiaBaselineBytes: 168 * MIB,
      peakLessBaselineBytes: measured(600 * MIB),
      gpuProcessPeakBytes: measured(550 * MIB),
      last: { deviceBytes: 760 * MIB, gpuProcessBytes: 540 * MIB },
    });
    expect(results.driver).toEqual({
      preLaunchBaselineBytes: measured(168 * MIB),
      peakLessBaselineBytes: measured(600 * MIB),
      gpuProcessPeakBytes: measured(550 * MIB),
      endDeviceBytes: measured(760 * MIB),
      endGpuProcessBytes: measured(540 * MIB),
      endOthersBytes: measured(220 * MIB),
      othersDriftBytes: measured(52 * MIB),
      itemizedLessDriverBytes: measured(sizes.memory.itemizedGpuBytes - 600 * MIB),
      engineLessDriverBytes: measured(sizes.memory.engine.peak.bytes - 600 * MIB),
    });
    expect([results.load, sizesResultsProblems(results)]).toEqual([load, []]);
  });

  it("states why each driver figure is missing off NVIDIA", () => {
    const results = buildSizesResults(sizesReportOf(), load, {
      nvidiaBaselineBytes: null,
      peakLessBaselineBytes: missing("no nvidia-smi baseline (not NVIDIA)"),
      gpuProcessPeakBytes: missing("no nvidia-smi reading"),
      last: { deviceBytes: null, gpuProcessBytes: null },
    });
    expect(results.driver).toMatchObject({
      endDeviceBytes: missing("no nvidia-smi reading at the run's last memory sample"),
      endOthersBytes: missing("no nvidia-smi reading at the run's last memory sample"),
      itemizedLessDriverBytes: missing("no nvidia-smi baseline (not NVIDIA)"),
      engineLessDriverBytes: missing("no nvidia-smi baseline (not NVIDIA)"),
    });
    expect(sizesResultsProblems(results)).toEqual([]);
  });

  it("refuses a block without the main process's figures, or with malformed ones", () => {
    const sizes = sizesReportOf();
    expect(sizesResultsProblems(sizes)).toEqual([
      "sizes.load is not an object",
      "sizes.driver is not an object",
    ]);
    const results = buildSizesResults(sizes, load, {
      nvidiaBaselineBytes: null,
      peakLessBaselineBytes: missing("off NVIDIA"),
      gpuProcessPeakBytes: missing("off NVIDIA"),
      last: null,
    });
    expect(
      sizesResultsProblems({ ...results, load: measured([{ tS: -1, oneMinute: 0.5 }]) }),
    ).toEqual(["sizes.load.value[0].tS is not a finite number at least 0"]);
  });

  it("summarises each cache's peaks, the rule's forms, the A/B, the shares and the memory", () => {
    const results = buildSizesResults(
      sizesReportOf({
        samples: 3,
        passes: [
          { cache: "lit", views: 1 },
          { cache: "wireframe", views: 1 },
        ],
      }),
      load,
      {
        nvidiaBaselineBytes: 168 * MIB,
        peakLessBaselineBytes: measured(600 * MIB),
        gpuProcessPeakBytes: measured(550 * MIB),
        last: { deviceBytes: 760 * MIB, gpuProcessBytes: 540 * MIB },
      },
    );
    const text = sizesSummaryLines(results, "low").join("\n");
    for (const expected of [
      "## Sizes (R10.T14.b)",
      "### The lit cache: spike (photorealistic, weight 1)",
      "### The wireframe cache: spike (wireframe, weight 1)",
      "| F | 812 (1,104.0 s, slowdown 3, 1,203 m up) | 812 (1,104.0 s, slowdown 3, 1,203 m up) |",
      "| Ruled: Σ max(1.3 AR, 3 F) + forced | 3,000.0 | 930,000,000 B | 2,850.0 | 883,500,000 B | × 2.33 |",
      "the as-built -66,604.0 m",
      "the resident share's peak 87.3 %",
      "resident at 100 % never",
      "touchdown at 1,199.9 s (61 forced, 61 of them resident)",
      "a lead of 7.9 s (touchdown − every forced patch resident)",
      "every forced patch resident first at 1,192.0 s, and at every sample after",
      "the contact by the session's rule at 1,190.0 s (61 forced, 12 of them resident)",
      "- The cascades: 64.0 MiB as the engine allocated them (its shadows category at its peak), T9.a's spec 64.0 MiB as built",
      "- Itemized GPU (the caches, the session's masks and maps, the cascades): 836.4 MiB; with the workers' field copies (72.0 MiB at their peak): 908.4 MiB, against 1 GB (low)",
      "- The engine's tally, the app's own GPU figure: 875.9 MiB at its whole peak (2.0 s); each category at its own peak: render-targets 39.6 MiB (before the flight), height-cache 760.4 MiB (2.0 s), coarse-field-gpu 12.0 MiB (1.0 s), shadows 64.0 MiB (1.0 s)",
      "- Less the driver: the itemized GPU 236.4 MiB, the engine's tally 275.9 MiB",
      "the rest of the card 220.0 MiB",
      "4,920 publishes, 4,800 holding an answer",
      "0 of its guards' calls to fail the session counted and withheld",
      "The 1-minute load: 0.50 at 0 s",
    ]) {
      expect(text).toContain(expected);
    }
  });

  /** The summary of `sizes`' block on `setting`. */
  function summaryOf(
    passes: SizesFixtureOptions["passes"],
    setting: SpikeSetting = "high",
  ): string {
    const results = buildSizesResults(sizesReportOf({ samples: 3, passes }), load, {
      nvidiaBaselineBytes: null,
      peakLessBaselineBytes: missing("off NVIDIA"),
      gpuProcessPeakBytes: missing("off NVIDIA"),
      last: null,
    });
    return sizesSummaryLines(results, setting).join("\n");
  }

  it("states the A/B's price in slots, and that the 10 % test reads the rule's own difference", () => {
    expect(summaryOf([{ cache: "lit", views: 1 }])).toContain(
      "the price in slots, each a share of the sized all-round peak Σ 1.3 AR (3,627.0 slots): the rule's own, rule − rule′, 150.0 slots, 4.1 %, the reading the 10 % test takes; the union's all-round term 1.3 (AR − AR′) 182.0 (2.5 %), its frustum term 3 (F − F′) 33.0 (0.5 %)",
    );
  });

  it("marks a stand-in's pass as not sized, with no rule", () => {
    const text = summaryOf([
      { cache: "lit", views: 2 },
      { cache: "wireframe", views: 0 },
    ]);
    expect(text).toContain(
      "### The wireframe cache: stand-in only, not sized: no view of the wireframe cache's style was asked",
    );
    expect(text).toContain("| Ruled: Σ max(1.3 AR, 3 F) + forced | — | — | — | — | — |");
  });

  it("gives the pass's own counts by segment", () => {
    expect(summaryOf([{ cache: "lit", views: 1 }])).toContain(
      "| orbit coast | 0 | 600 | 2 | 550 | 1,286 | 61 | 480 |",
    );
  });

  it("stands the high setting's itemized total against a discrete card's 2–3 GB", () => {
    expect(summaryOf([{ cache: "lit", views: 2 }], "high")).toContain("against 2–3 GB (discrete)");
  });

  /** The summary's lines of `sizes`' block with its cascades and engine tally changed. */
  function memorySummary(memory: Partial<SpikeSizesMemory>): string {
    const sizes = sizesReportOf({ samples: 3, passes: [{ cache: "lit", views: 1 }] });
    const results = buildSizesResults({ ...sizes, memory: { ...sizes.memory, ...memory } }, load, {
      nvidiaBaselineBytes: null,
      peakLessBaselineBytes: missing("off NVIDIA"),
      gpuProcessPeakBytes: missing("off NVIDIA"),
      last: null,
    });
    return sizesSummaryLines(results, "low").join("\n");
  }

  it("states that no cascades were allocated, and why", () => {
    expect(
      memorySummary({
        cascades: {
          bytes: 0,
          source: "none",
          reason: "the low setting has no shadow cascades",
          specBytes: 67_108_864,
          differsFromSpec: false,
        },
      }),
    ).toContain(
      "- The cascades: none allocated (the low setting has no shadow cascades); T9.a's spec 64.0 MiB",
    );
  });

  it("states cascades unlike T9.a's spec as a finding", () => {
    expect(
      memorySummary({
        cascades: {
          bytes: 16_777_216,
          source: "engine",
          reason: null,
          specBytes: 67_108_864,
          differsFromSpec: true,
        },
      }),
    ).toContain(
      "**not** T9.a's spec 64.0 MiB: 16,777,216 B against 67,108,864 B, a finding for main",
    );
  });

  it("states the destructions whose creations the engine's tally never saw", () => {
    expect(
      memorySummary({
        engine: {
          categories: [{ category: "height-cache", bytes: 1_048_576, atS: 3 }],
          peak: { bytes: 1_048_576, atS: 3 },
          unseenDestroyed: 2,
        },
      }),
    ).toContain(
      "each category at its own peak: height-cache 1.0 MiB (3.0 s); 2 destructions of allocations made before the run's control listened, left out",
    );
  });

  it("marks the session's own figure for the cascades as counted from the engine instead", () => {
    expect(
      memorySummary({
        sessionGpu: [{ category: "shadows", bytes: 67_108_864, atS: 1 }],
      }),
    ).toContain(
      "- The session's shadows: 64.0 MiB at its peak (1.0 s), its own figure for the cascades, which the itemized total counts as the engine allocated them",
    );
  });
});
