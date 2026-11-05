import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import { INSPECTION_TIMING_SCHEMA } from "./inspectionTiming";
import {
  batchIn,
  cargoExecutable,
  cpuTimeSeconds,
  isCounted,
  type KeptRun,
  keptRunsIn,
  MAC_PROFILE_DEFAULTS,
  MAC_PROFILE_RUN_SCHEMA,
  type MacProfileMatrixOptions,
  type MacProfileRun,
  type MacProfileRunRecord,
  type MacProfileSample,
  macProfileMatrix,
  macProfileSummaryLines,
  median,
  NO_SKIES,
  pairFigures,
  type ProfiledWindow,
  serverCoresOver,
  skiesBetween,
  skiesInFlight,
  skyEventOf,
  tallied,
  thermalText,
} from "./macProfile";

/** The pod's dry run: one pair, limb rays at low, one round. */
const DRY_RUN: MacProfileMatrixOptions = {
  settings: [],
  ratios: [1],
  levers: ["limb-rays"],
  pairSetting: "low",
  rounds: 1,
  counted: false,
};

/** A sample at `at` (s past a minute) with the server's CPU time so far. */
function sampleAt(atS: number, serverCpuS: number | null): MacProfileSample {
  return {
    at: new Date(Date.UTC(2026, 10, 5, 12, 0, atS)).toISOString(),
    load1: 1.5,
    serverCpuPercent: 12,
    serverCpuS,
    thermal: null,
  };
}

/** Thirty times whose median is `ms` and p95 `p95`, summarised. */
function summary(ms: number, p95: number): ProfiledWindow["frame"]["gpuMs"] {
  return { count: 30, medianMs: ms, p95Ms: p95, meanMs: ms, maxMs: p95 };
}

/** A window whose draws came every `intervalMs` (p95 a fifth more), the GPU's frame `gpuMs`. */
function windowOf(
  name: ProfiledWindow["name"],
  intervalMs: number,
  gpuMs: number,
  passes: Readonly<Record<string, number>>,
): ProfiledWindow {
  return {
    name,
    frame: { intervalMs: summary(intervalMs, 1.2 * intervalMs), gpuMs: summary(gpuMs, gpuMs) },
    passes: Object.entries(passes).map(([label, ms]) => ({
      label,
      medianMs: ms,
      shareOfGpu: ms / gpuMs,
    })),
  };
}

/** The run `id` of `matrix`, kept with `windows`. */
function keptAs(
  matrix: ReadonlyArray<MacProfileRun>,
  id: string,
  windows: ReadonlyArray<ProfiledWindow>,
): KeptRun {
  const run = matrix.find((each) => each.id === id);
  if (run === undefined) {
    throw new Error(`the matrix has no run ${id}`);
  }
  const record: MacProfileRunRecord = {
    schema: MAC_PROFILE_RUN_SCHEMA,
    version: 1,
    id,
    argv: ["--inspection"],
    exit: 0,
    failure: null,
    before: sampleAt(0, 10),
    after: sampleAt(50, 30),
    skies: { started: 1, kept: 1, final: 0, unfinished: 0 },
    timing: `${id}.json`,
    files: [`${id}.json`, "run.json"],
  };
  return { run, record, timing: { run: { timer: "full" }, windows } };
}

/** `kept` as a run whose views check wrote its timing file and then failed with `failure`. */
function failedAfter(kept: KeptRun, failure: string): KeptRun {
  return { ...kept, record: { ...kept.record, exit: 1, failure } };
}

describe("the Mac batch's matrix", () => {
  it("runs each setting at both ratios, then every pair at high as A B A B", () => {
    const matrix = macProfileMatrix(MAC_PROFILE_DEFAULTS);
    expect(matrix.slice(0, 4).map((run) => run.id)).toEqual([
      "high-dpr1",
      "high-dpr2",
      "low-dpr1",
      "low-dpr2",
    ]);
    expect(matrix.slice(4, 8).map((run) => run.id)).toEqual([
      "limb-rays-high-dpr1-a1",
      "limb-rays-high-dpr1-b1",
      "limb-rays-high-dpr1-a2",
      "limb-rays-high-dpr1-b2",
    ]);
    // Four own runs, seven pairs at DPR 1 and the shading pair at 1 and 2, four runs each; then
    // two counted settings and the occlusion and shading pairs' two counted sides each.
    expect(matrix).toHaveLength(4 + 7 * 4 + 2 * 4 + 2 + 2 * 2);
    expect(new Set(matrix.map((run) => run.id)).size).toBe(matrix.length);
  });

  it("gives each run its ratio and its side's switches", () => {
    const matrix = macProfileMatrix(MAC_PROFILE_DEFAULTS);
    const args = (id: string): ReadonlyArray<string> | undefined =>
      matrix.find((run) => run.id === id)?.args;
    expect(args("high-dpr1")).toEqual([]);
    expect(args("low-dpr2")).toEqual(["--device-pixel-ratio", "2"]);
    expect(args("limb-rays-high-dpr1-a2")).toEqual([]);
    expect(args("limb-rays-high-dpr1-b2")).toEqual(["--limb-rays", "per-texel"]);
    expect(args("occlusion-high-dpr1-b1")).toEqual(["--geometry", "occlusion=hzb"]);
    expect(args("shading-high-dpr2-a1")).toEqual([
      "--device-pixel-ratio",
      "2",
      "--geometry",
      "shading=forward",
    ]);
    expect(args("shading-high-dpr2-b2")).toEqual([
      "--device-pixel-ratio",
      "2",
      "--geometry",
      "shading=visibility",
    ]);
  });

  it("makes the dry run's two runs, the levers in their own order whatever the order given", () => {
    expect(macProfileMatrix(DRY_RUN).map((run) => run.id)).toEqual([
      "limb-rays-low-dpr1-a1",
      "limb-rays-low-dpr1-b1",
    ]);
    const both = macProfileMatrix({ ...DRY_RUN, levers: ["strides", "limb-rays"] });
    expect(both.map((run) => (run.place.kind === "pair" ? run.place.lever : null))).toEqual([
      "limb-rays",
      "limb-rays",
      "strides",
      "strides",
    ]);
  });

  it("counts each setting's own way and each side of a pass-splitting pair last, at DPR 1, apart from the pairs (R14.T2.e)", () => {
    const matrix = macProfileMatrix(MAC_PROFILE_DEFAULTS);
    const counted = matrix.filter(isCounted);
    expect(matrix.slice(-6)).toEqual(counted);
    expect(counted.map((run) => [run.id, run.setting, run.ratio, run.args, run.place])).toEqual([
      ["counted-high-dpr1", "high", 1, ["--count-gpu-calls"], { kind: "counted" }],
      ["counted-low-dpr1", "low", 1, ["--count-gpu-calls"], { kind: "counted" }],
      [
        "counted-occlusion-high-dpr1-a",
        "high",
        1,
        ["--count-gpu-calls"],
        { kind: "counted side", lever: "occlusion", side: "a" },
      ],
      [
        "counted-occlusion-high-dpr1-b",
        "high",
        1,
        ["--count-gpu-calls", "--geometry", "occlusion=hzb"],
        { kind: "counted side", lever: "occlusion", side: "b" },
      ],
      [
        "counted-shading-high-dpr1-a",
        "high",
        1,
        ["--count-gpu-calls", "--geometry", "shading=forward"],
        { kind: "counted side", lever: "shading", side: "a" },
      ],
      [
        "counted-shading-high-dpr1-b",
        "high",
        1,
        ["--count-gpu-calls", "--geometry", "shading=visibility"],
        { kind: "counted side", lever: "shading", side: "b" },
      ],
    ]);
    expect(macProfileMatrix({ ...MAC_PROFILE_DEFAULTS, counted: false }).some(isCounted)).toBe(
      false,
    );
    expect(
      macProfileMatrix({ ...MAC_PROFILE_DEFAULTS, settings: [], levers: ["limb-rays"] }).filter(
        isCounted,
      ),
    ).toEqual([]);
  });

  it("refuses a lever it does not know", () => {
    expect(() => macProfileMatrix({ ...DRY_RUN, levers: ["bloom"] })).toThrow(/bloom/u);
  });
});

/** A line of the server's log, as its sky handler writes `message`. */
function line(message: string): string {
  return `2026-11-05T07:21:19.065222Z  INFO hyperion_server::requests::sky: ${message} wall_s=1.5`;
}

describe("the server's skies, from its log", () => {
  it("reads each sky line's event and leaves every other line", () => {
    expect(skyEventOf(line("a sky started universe=8160ee7f2970b1b5"))).toBe("started");
    expect(skyEventOf(line("a sky is answered with its kept final reply"))).toBe("kept");
    expect(skyEventOf(line("a sky's final reply is made listed=7089"))).toBe("final");
    expect(skyEventOf(line("a sky ended without its final reply code=Busy"))).toBe("unfinished");
    expect(skyEventOf(line("a sky was given up before its final reply"))).toBe("unfinished");
    expect(skyEventOf(line("a sky's first reply is made listed=7089"))).toBeNull();
    expect(skyEventOf("2026-11-05T07:21:06Z  INFO hyperion_server: listening")).toBeNull();
  });

  it("reads a line a terminal's colours run through", () => {
    expect(
      skyEventOf(
        "\u001b[2m2026-11-05T07:21:19Z\u001b[0m \u001b[32m INFO\u001b[0m " +
          "\u001b[2mhyperion_server::requests::sky\u001b[0m\u001b[2m:\u001b[0m a sky started",
      ),
    ).toBe("started");
  });

  it("counts the skies in flight and those of a span", () => {
    const started = tallied(tallied(NO_SKIES, "started"), "started");
    const later = tallied(started, "final");
    expect(skiesInFlight(started)).toBe(2);
    expect(skiesInFlight(later)).toBe(1);
    expect(skiesBetween(started, tallied(later, "kept"))).toEqual({
      started: 0,
      kept: 1,
      final: 1,
      unfinished: 0,
    });
  });
});

describe("the samples' arithmetic", () => {
  it("reads ps's CPU time in macOS's and procps' forms", () => {
    expect(cpuTimeSeconds("0:01.57")).toBeCloseTo(1.57, 9);
    expect(cpuTimeSeconds("123:03.27")).toBeCloseTo(7383.27, 9);
    expect(cpuTimeSeconds("01:02:03")).toBe(3723);
    expect(cpuTimeSeconds("1-02:03:04")).toBe(93_784);
    expect(cpuTimeSeconds(" 0:00.00\n")).toBe(0);
    expect(cpuTimeSeconds("-")).toBeNull();
  });

  it("gives the server's mean cores over a run from its CPU time", () => {
    expect(serverCoresOver(sampleAt(0, 100), sampleAt(40, 280))).toBeCloseTo(4.5, 12);
    expect(serverCoresOver(sampleAt(0, null), sampleAt(40, 280))).toBeNull();
    expect(serverCoresOver(sampleAt(40, 100), sampleAt(40, 280))).toBeNull();
  });

  it("says nominal for an Apple silicon Mac's notes and gives a limit's lines otherwise", () => {
    expect(
      thermalText(
        "Note: No thermal warning level has been recorded\n" +
          "Note: No performance warning level has been recorded\n" +
          "Note: No CPU power status has been recorded\n",
      ),
    ).toBe("nominal");
    expect(
      thermalText(
        "Note: No thermal warning level has been recorded\n" +
          "CPU_Scheduler_Limit \t= 100\nCPU_Speed_Limit \t= 76\n",
      ),
    ).toBe("CPU_Scheduler_Limit = 100; CPU_Speed_Limit = 76");
    expect(thermalText(null)).toBe("—");
  });

  it("takes the middle of an odd count and the mean of the two middles of an even one", () => {
    expect(median([5, 1, 3])).toBe(3);
    expect(median([44, 40])).toBe(42);
    expect(median([])).toBeNull();
  });
});

describe("a lever's pair", () => {
  const matrix = macProfileMatrix(MAC_PROFILE_DEFAULTS);
  const kept = [
    keptAs(matrix, "march-sources-high-dpr1-a1", [
      windowOf("orbit", 40, 38, { march: 12, bodies: 10 }),
      windowOf("close", 90, 85, { bodies: 50 }),
    ]),
    keptAs(matrix, "march-sources-high-dpr1-b1", [
      windowOf("orbit", 50, 47, { march: 20, bodies: 10 }),
    ]),
    keptAs(matrix, "march-sources-high-dpr1-a2", [
      windowOf("orbit", 44, 40, { march: 14, bodies: 11 }),
    ]),
    keptAs(matrix, "march-sources-high-dpr1-b2", [
      windowOf("orbit", 54, 51, { march: 22, bodies: 11 }),
    ]),
    // Another lever's run, which the pair leaves out.
    failedAfter(
      keptAs(matrix, "occlusion-high-dpr1-b1", [windowOf("orbit", 10, 9, { bodies: 5 })]),
      "views check: the CAMERA line reads no FREE rate",
    ),
  ];

  it("gives each side the median of its runs and B less A by window", () => {
    const [orbit, close] = pairFigures(kept, "march-sources", 1);
    expect(orbit?.window).toBe("orbit");
    expect(orbit?.a.runs).toBe(2);
    expect(orbit?.a.intervalMedianMs).toBe(42);
    expect(orbit?.b.intervalMedianMs).toBe(52);
    expect(orbit?.deltaMs).toBe(10);
    expect(orbit?.deltaShare).toBeCloseTo(10 / 42, 12);
    expect(orbit?.a.intervalP95Ms).toBeCloseTo(1.2 * 42, 12);
    expect(orbit?.gpuDeltaMs).toBe(49 - 39);
    expect(orbit?.a.passMs.get("march")).toBe(13);
    expect(orbit?.b.passMs.get("march")).toBe(21);
    // A window only one side measured has no delta.
    expect(close?.b.runs).toBe(0);
    expect(close?.deltaMs).toBeNull();
  });

  it("has no windows where none of its runs is kept", () => {
    expect(pairFigures(kept, "march-sources", 2)).toEqual([]);
    expect(pairFigures(kept, "strides", 1)).toEqual([]);
  });

  it("writes the runs, their windows, the pairs and the sweep into the summary", () => {
    const lines = macProfileSummaryLines({
      machine: "Apple M5, 16 GiB",
      tree: "e9966e0",
      body: "41ffecb200000002.0400",
      options: MAC_PROFILE_DEFAULTS,
      kept,
      sweep: { table: null, runs: [] },
    });
    const text = lines.join("\n");
    expect(lines[0]).toBe("# The Mac batch: Apple M5, 16 GiB");
    expect(text).toContain("Tree e9966e0, body 41ffecb200000002.0400; 5 of 46 runs kept");
    expect(text).toContain("| high-dpr1 | high | 1 | — | not kept |");
    // A run kept with its windows though its views check failed after them.
    expect(text).toContain(
      "| occlusion-high-dpr1-b1 | high | 1 | occlusion B1 | 1: views check: the CAMERA line reads no FREE rate |",
    );
    // 20 s of the server's CPU over 50 s of the run: 40% of a core; one sky, from the kept reply.
    expect(text).toMatch(
      /\| march-sources-high-dpr1-a1 \| high \| 1 \| march-sources A1 \| 0 \| 50 s \| [^\n]* \| 40% \| 0, 1 \| — → — \|/u,
    );
    expect(text).toContain(
      "| march-sources-high-dpr1-b2 | orbit | 54.0 | 64.8 | 18.5 | 51.0 | march 22.0 (43%), bodies 11.0 (22%) |",
    );
    expect(text).toContain("| orbit | 2, 2 | 42.0 | 52.0 | +10.0 | +23.8% | 23.8 | 19.2 |");
    expect(text).toContain("| orbit | march | 13.0 | 21.0 | +8.0 |");
    expect(text).toContain("### shading, high at DPR 2: R14.T2.e");
    expect(text).toContain("No counted run of the pair is kept.");
    // A timing file before version 7 holds no reads.
    expect(text).toContain("| orbit | not recorded | not recorded |");
    expect(text).toContain("## The height-worker sweep\n\nNot written yet.");
  });
});

describe("the counted runs (R14.T2.e)", () => {
  const matrix = macProfileMatrix(MAC_PROFILE_DEFAULTS);
  /** A window counted, its passes' bytes a draw, its air thick. */
  const counted = (
    name: ProfiledWindow["name"],
    passes: ReadonlyArray<readonly [string, number, number]>,
  ): ProfiledWindow => ({
    ...windowOf(name, 400, 380, {}),
    atmosphereReads: [
      {
        air: 1,
        read: "thick",
        thermal: false,
        frames: 30,
        kernels: ["atmosphere sky view, thick"],
      },
    ],
    attachments: {
      bytesLoadedPerDraw: passes.reduce((sum, [, loaded]) => sum + loaded, 0),
      bytesStoredPerDraw: passes.reduce((sum, [, , stored]) => sum + stored, 0),
      passes: passes.map(([label, bytesLoadedPerDraw, bytesStoredPerDraw]) => ({
        label,
        passesPerDraw: 1,
        bytesLoadedPerDraw,
        bytesStoredPerDraw,
        attachments: [
          {
            label,
            attachment: "colour 0",
            view: { format: "rgba16float", widthPx: 1280, heightPx: 720, sampleCount: 1 },
            loadOp: bytesLoadedPerDraw > 0 ? "load" : "clear",
            storeOp: "store",
            passes: 30,
            bytesLoaded: 30 * bytesLoadedPerDraw,
            bytesStored: 30 * bytesStoredPerDraw,
          },
        ],
      })),
    },
  });
  const MIB = 1_048_576;
  const kept = [
    keptAs(matrix, "counted-shading-high-dpr1-a", [counted("orbit", [["bodies", 0, 7 * MIB]])]),
    keptAs(matrix, "counted-shading-high-dpr1-b", [
      counted("orbit", [
        ["terrain id raster", 0, 3.5 * MIB],
        ["terrain resolve", 3.5 * MIB, 7 * MIB],
      ]),
    ]),
    keptAs(matrix, "shading-high-dpr1-a1", [windowOf("orbit", 40, 38, { bodies: 10 })]),
  ];
  const text = macProfileSummaryLines({
    machine: "Apple M5, 16 GiB",
    tree: "e9966e0",
    body: "41ffecb200000002.0400",
    options: MAC_PROFILE_DEFAULTS,
    kept,
    sweep: { table: null, runs: [] },
  }).join("\n");

  it("reads a pair side's runs drawn alike as one read, whatever their frames", () => {
    const thick = (frames: number): ProfiledWindow => ({
      ...windowOf("orbit", 40, 38, {}),
      atmosphereReads: [{ air: 1, read: "thick", thermal: false, frames, kernels: [] }],
    });
    const [orbit] = pairFigures(
      [
        keptAs(matrix, "shading-high-dpr1-a1", [thick(118)]),
        keptAs(matrix, "shading-high-dpr1-a2", [thick(120)]),
      ],
      "shading",
      1,
    );
    expect([orbit?.a.reads, orbit?.b.reads]).toEqual([["air 1: thick"], []]);
  });

  it("gives the shading pair's counted sides' bytes by pass beside its deltas", () => {
    const pair = text.slice(text.indexOf("### shading, high at DPR 1"));
    expect(pair).toContain("| orbit | terrain resolve | — | — | 3.50 | 7.00 |");
    expect(pair).toContain("| orbit | bodies | 0.00 | 7.00 | — | — |");
    expect(pair.indexOf("terrain resolve")).toBeLessThan(
      pair.indexOf("### shading, high at DPR 2"),
    );
  });

  it("lists each counted run's windows by their bytes and reads, and says which are not kept", () => {
    expect(text).toContain("| counted-shading-high-dpr1-b | high | 1 | counted, shading B | 0 |");
    expect(text).toContain(
      "- **orbit:** 3.50 MiB loaded, 10.50 MiB stored a draw; atmosphere read air 1: thick, 30 frames",
    );
    expect(text).toContain(
      "| terrain resolve | 1.0 | 3.50 | 7.00 | colour 0 rgba16float 1280 × 720 load/store |",
    );
    expect(text).toContain("### counted-low-dpr1 (counted)\n\nNot kept.");
  });
});

describe("the batch's files", () => {
  const dirs: string[] = [];
  afterEach(() => {
    for (const dir of dirs.splice(0)) {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("reads a kept run with its record and timing file, and none half written", () => {
    const runs = mkdtempSync(join(tmpdir(), "mac-profile-"));
    dirs.push(runs);
    const matrix = macProfileMatrix(DRY_RUN);
    const [a, b] = matrix;
    if (a === undefined || b === undefined) {
      throw new Error("the dry run's matrix has two runs");
    }
    const { record } = keptAs(matrix, a.id, []);
    mkdirSync(join(runs, a.id));
    writeFileSync(join(runs, a.id, "run.json"), JSON.stringify(record));
    writeFileSync(
      join(runs, a.id, record.timing),
      JSON.stringify({ schema: INSPECTION_TIMING_SCHEMA, run: { timer: "full" }, windows: [] }),
    );
    // B's directory, as a stop part way through moving its files leaves it: no record yet.
    mkdirSync(join(runs, b.id));
    writeFileSync(join(runs, b.id, `${b.id}.json`), "{");
    const kept = keptRunsIn(runs, matrix);
    expect(kept.map((each) => each.run.id)).toEqual([a.id]);
    expect(kept[0]?.record.after.serverCpuS).toBe(30);
  });

  it("reads the batch's commit and body, a batch that recorded no body the default's", () => {
    const out = mkdtempSync(join(tmpdir(), "mac-profile-"));
    dirs.push(out);
    expect(batchIn(out)).toBeNull();
    writeFileSync(join(out, "batch.json"), JSON.stringify({ tree: "24aef3e" }));
    expect(batchIn(out)).toEqual({ tree: "24aef3e", body: null });
    writeFileSync(
      join(out, "batch.json"),
      JSON.stringify({ tree: "24aef3e", body: "41ffecb200000002.0300" }),
    );
    expect(batchIn(out)).toEqual({ tree: "24aef3e", body: "41ffecb200000002.0300" });
  });

  it("finds the binary cargo built among its messages", () => {
    const messages = [
      "",
      JSON.stringify({ reason: "build-script-executed", package_id: "x" }),
      JSON.stringify({
        reason: "compiler-artifact",
        target: { name: "hyperion_server", kind: ["lib"] },
        executable: null,
      }),
      JSON.stringify({
        reason: "compiler-artifact",
        target: { name: "hyperion-server", kind: ["bin"] },
        executable: "/repo/target/release/hyperion-server",
      }),
      JSON.stringify({ reason: "build-finished", success: true }),
    ].join("\n");
    expect(cargoExecutable(messages, "hyperion-server")).toBe(
      "/repo/target/release/hyperion-server",
    );
    expect(cargoExecutable("", "hyperion-server")).toBeNull();
  });
});
