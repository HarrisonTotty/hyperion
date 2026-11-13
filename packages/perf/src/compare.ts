/**
 * The comparison of a record with the last comparable one (plan R12, Design notes 5 and 7;
 * R12.T6): a change stated as a number with its effect size, a finding and never a failure.
 *
 * @remarks
 * Records compare only when their scene, scene version, machine identity, power profile and
 * setting agree, and their GPU-timing switch (a control run compares with a control); the first
 * field that differs is named. The machine's identity is its hardware and display mode; software
 * versions (driver, kernel, Electron) and the setting's values may differ, and the comparison
 * prints them as the change being measured. Per continuous metric (mean GPU busy time a frame,
 * mean main-thread time a frame, the missed-frame fraction), a change is `REGRESSION` or `BETTER`
 * only when every new repetition lies beyond every old one (complete separation) and the effect
 * exceeds ε; otherwise `SAME`. The percentiles are printed for reading, without a verdict.
 */

import type { PerfRunRecord, PerfRepetition } from "./record.ts";
import { parsePerfRecords, PerfRecordError } from "./record.ts";
import { median, rangeOf } from "./stats.ts";
import type { ValueRange } from "./stats.ts";

/** A comparison's verdict on one metric. */
export type Verdict = "SAME" | "BETTER" | "REGRESSION";

/**
 * ε for the means: a ratio of medians beyond 1 ± 3% (Design note 7; provisional until R12.T8.d
 * sets it to twice the measured within-scene coefficient of variation).
 */
export const RATIO_EPSILON = 0.03;

/** ε for the missed-frame fraction: a difference of medians beyond 1 percentage point (Design note 7). */
export const MISSED_EPSILON = 0.01;

/**
 * The fewest orderings complete separation must be one of two among: C(n + m, n) ≥ 252, so that its
 * two-sided p, 2 / C(n + m, n), is at most five against five's 2/252 ≈ 0.008, the exact
 * Mann–Whitney extreme (Mann and Whitney, _Ann. Math. Statist._ 18, 50–60, 1947). Fewer repetitions
 * than that separate by chance too often to state a change: they read `SAME`.
 */
export const SEPARATION_MIN_ORDERINGS = 252;

/** The metrics compared, each per repetition (Design note 7). */
export const COMPARED_METRICS = [
  "gpuBusyPerFrameMs",
  "mainThreadPerFrameMs",
  "missedFraction",
] as const;

/** A compared metric. */
export type ComparedMetric = (typeof COMPARED_METRICS)[number];

const METRIC_NAMES: Readonly<Record<ComparedMetric, string>> = {
  gpuBusyPerFrameMs: "GPU busy a frame, ms",
  mainThreadPerFrameMs: "main thread a frame, ms",
  missedFraction: "missed-frame fraction",
};

function valueOf(run: PerfRepetition, metric: ComparedMetric): number | null {
  let value: number | null;
  switch (metric) {
    case "gpuBusyPerFrameMs":
      value = run.gpuBusyPerFrameMs;
      break;
    case "mainThreadPerFrameMs":
      value = run.mainThreadPerFrameMs;
      break;
    case "missedFraction":
      value = run.frames.missedFraction;
      break;
  }
  return value;
}

/** One side's values of a metric: their median and range. */
export interface Side {
  readonly median: number;
  readonly range: ValueRange;
  readonly count: number;
}

/** One metric's comparison. */
export interface MetricComparison {
  readonly metric: ComparedMetric;
  /** `null` where a repetition of that side lacks the metric. */
  readonly old: Side | null;
  readonly new: Side | null;
  /** The ratio of medians, new over old, for the means; `null` for the missed-frame fraction. */
  readonly ratio: number | null;
  /** The difference of medians, new less old, for the missed-frame fraction; `null` for the means. */
  readonly difference: number | null;
  /** `worse` or `better` if every new value lies beyond every old one, else `none`. */
  readonly separation: "worse" | "better" | "none";
  /**
   * The two-sided p of complete separation at these counts, 2 / C(n + m, n), printed only where it
   * holds; `null` without both sides.
   */
  readonly p: number | null;
  /** `null` where either side lacks the metric: nothing is stated. */
  readonly verdict: Verdict | null;
}

function sideOf(
  record: PerfRunRecord,
  metric: ComparedMetric,
): { side: Side; values: ReadonlyArray<number> } | null {
  const values: number[] = [];
  for (const run of record.runs) {
    const value = valueOf(run, metric);
    if (value === null) return null;
    values.push(value);
  }
  const middle = median(values);
  const range = rangeOf(values);
  if (middle === null || range === null) return null;
  return { side: { median: middle, range, count: values.length }, values };
}

/** C(n, k), exactly while it is below 2⁵³. */
function choose(n: number, k: number): number {
  let out = 1;
  for (let i = 1; i <= k; i += 1) out = (out * (n - k + i)) / i;
  return Math.round(out);
}

/** Compares one metric of two records' repetitions by Design note 7's rule. */
export function compareMetric(
  older: PerfRunRecord,
  newer: PerfRunRecord,
  metric: ComparedMetric,
): MetricComparison {
  const before = sideOf(older, metric);
  const after = sideOf(newer, metric);
  const absolute = metric === "missedFraction";
  if (before === null || after === null) {
    return {
      metric,
      old: before?.side ?? null,
      new: after?.side ?? null,
      ratio: null,
      difference: null,
      separation: "none",
      p: null,
      verdict: null,
    };
  }
  const separation =
    after.side.range.min > before.side.range.max
      ? "worse"
      : after.side.range.max < before.side.range.min
        ? "better"
        : "none";
  const orderings = choose(before.side.count + after.side.count, before.side.count);
  const ratio =
    absolute || before.side.median === 0 ? null : after.side.median / before.side.median;
  const difference = absolute ? after.side.median - before.side.median : null;
  const beyond =
    separation === "worse"
      ? absolute
        ? (difference ?? 0) > MISSED_EPSILON
        : (ratio ?? 1) > 1 + RATIO_EPSILON
      : separation === "better"
        ? absolute
          ? (difference ?? 0) < -MISSED_EPSILON
          : (ratio ?? 1) < 1 - RATIO_EPSILON
        : false;
  const stated = beyond && orderings >= SEPARATION_MIN_ORDERINGS;
  return {
    metric,
    old: before.side,
    new: after.side,
    ratio,
    difference,
    separation,
    p: 2 / orderings,
    verdict: !stated ? "SAME" : separation === "worse" ? "REGRESSION" : "BETTER",
  };
}

/** The fields two records must agree on to compare, each with its path and how it is read. */
const IDENTITY: ReadonlyArray<readonly [string, (r: PerfRunRecord) => unknown]> = [
  ["scene.id", (r) => r.scene.id],
  ["scene.version", (r) => r.scene.version],
  ["machine.name", (r) => r.machine.name],
  ["machine.platform", (r) => r.machine.platform],
  ["machine.cpu.model", (r) => r.machine.cpu.model],
  ["machine.cpu.logicalCores", (r) => r.machine.cpu.logicalCores],
  ["machine.memoryBytes", (r) => r.machine.memoryBytes],
  ["machine.gpu.vendorId", (r) => r.machine.gpu?.vendorId ?? null],
  ["machine.gpu.deviceId", (r) => r.machine.gpu?.deviceId ?? null],
  ["machine.display.widthPx", (r) => r.machine.display?.widthPx ?? null],
  ["machine.display.heightPx", (r) => r.machine.display?.heightPx ?? null],
  ["machine.display.refreshHz", (r) => r.machine.display?.refreshHz ?? null],
  ["machine.display.devicePixelRatio", (r) => r.machine.display?.devicePixelRatio ?? null],
  ["harness.display", (r) => r.harness.display],
  ["state.supply", (r) => r.state.supply],
  ["state.profile.tlpMode", (r) => r.state.profile.tlpMode],
  ["state.profile.governor", (r) => r.state.profile.governor],
  ["state.profile.epp", (r) => r.state.profile.epp],
  ["state.profile.noTurbo", (r) => r.state.profile.noTurbo],
  ["state.profile.pl1Watts", (r) => r.state.profile.pl1Watts],
  ["state.profile.pl2Watts", (r) => r.state.profile.pl2Watts],
  ["state.profile.tauSeconds", (r) => r.state.profile.tauSeconds],
  ["state.profile.raplZone", (r) => r.state.profile.raplZone],
  ["state.profile.lowPowerMode", (r) => r.state.profile.lowPowerMode],
  ["setting.quality", (r) => r.setting.quality],
  ["harness.gpuTiming", (r) => r.harness.gpuTiming],
];

/** A field two records disagree on, with both values as JSON. */
export interface FieldDifference {
  readonly field: string;
  readonly old: string;
  readonly new: string;
}

/** The first field of the comparison's identity two records disagree on, or `null` if they compare. */
export function incomparability(
  older: PerfRunRecord,
  newer: PerfRunRecord,
): FieldDifference | null {
  for (const [field, read] of IDENTITY) {
    const a = JSON.stringify(read(older));
    const b = JSON.stringify(read(newer));
    if (a !== b) return { field, old: a, new: b };
  }
  return null;
}

/** What may differ between comparable records, and does: the change being measured. */
export function changes(
  older: PerfRunRecord,
  newer: PerfRunRecord,
): ReadonlyArray<FieldDifference> {
  const out: FieldDifference[] = [];
  const add = (field: string, a: unknown, b: unknown): void => {
    const x = JSON.stringify(a);
    const y = JSON.stringify(b);
    if (x !== y) out.push({ field, old: x, new: y });
  };
  add("commit", older.commit, newer.commit);
  add("dirty", older.dirty, newer.dirty);
  add("generatorVersion", older.generatorVersion, newer.generatorVersion);
  const keys = [
    ...new Set([...Object.keys(older.setting.values), ...Object.keys(newer.setting.values)]),
  ].toSorted();
  for (const key of keys)
    add(
      `setting.values.${key}`,
      older.setting.values[key] ?? null,
      newer.setting.values[key] ?? null,
    );
  add(
    "machine.gpu.driverVersion",
    older.machine.gpu?.driverVersion ?? null,
    newer.machine.gpu?.driverVersion ?? null,
  );
  add("machine.kernel", older.machine.kernel, newer.machine.kernel);
  add(
    "machine.versions.electron",
    older.machine.versions.electron,
    newer.machine.versions.electron,
  );
  add(
    "machine.versions.chromium",
    older.machine.versions.chromium,
    newer.machine.versions.chromium,
  );
  add("harness.switches", older.harness.switches, newer.harness.switches);
  add("harness.server", older.harness.server, newer.harness.server);
  add("harness.heightWorkers", older.harness.heightWorkers, newer.harness.heightWorkers);
  return out;
}

/** Two records compared. */
export interface Comparison {
  readonly old: PerfRunRecord;
  readonly new: PerfRunRecord;
  readonly changes: ReadonlyArray<FieldDifference>;
  readonly metrics: ReadonlyArray<MetricComparison>;
}

/**
 * Compares two records metric by metric.
 *
 * @throws Error naming the first field they disagree on, if they do not compare.
 */
export function compareRecords(older: PerfRunRecord, newer: PerfRunRecord): Comparison {
  const differs = incomparability(older, newer);
  if (differs !== null) {
    throw new Error(
      `${older.id} and ${newer.id} differ in ${differs.field}: ${differs.old} against ${differs.new}`,
    );
  }
  return {
    old: older,
    new: newer,
    changes: changes(older, newer),
    metrics: COMPARED_METRICS.map((m) => compareMetric(older, newer, m)),
  };
}

/** What the search for a record's comparison found. */
export type Previous =
  | { readonly kind: "comparable"; readonly old: PerfRunRecord }
  | { readonly kind: "refused"; readonly nearest: PerfRunRecord; readonly differs: FieldDifference }
  | { readonly kind: "none" };

/**
 * The last record before `newer` in the store that compares with it; or, where none does, the last
 * earlier record of its scene and the first field it differs in; or nothing, for a scene's first.
 */
export function previousComparable(
  records: ReadonlyArray<PerfRunRecord>,
  newer: PerfRunRecord,
): Previous {
  const at = records.findIndex((r) => r.id === newer.id);
  const earlier = (at < 0 ? records : records.slice(0, at)).toReversed();
  const old = earlier.find((r) => incomparability(r, newer) === null);
  if (old !== undefined) return { kind: "comparable", old };
  const nearest = earlier.find((r) => r.scene.id === newer.scene.id);
  if (nearest === undefined) return { kind: "none" };
  const differs = incomparability(nearest, newer);
  return differs === null
    ? { kind: "comparable", old: nearest }
    : { kind: "refused", nearest, differs };
}

function fixed(value: number | null, digits: number): string {
  return value === null ? "—" : value.toFixed(digits);
}

function sideText(side: Side | null, digits: number): string {
  if (side === null) return "NOT AVAILABLE";
  return `${side.median.toFixed(digits)} [${side.range.min.toFixed(digits)}, ${side.range.max.toFixed(digits)}]`;
}

function columns(rows: ReadonlyArray<ReadonlyArray<string>>): string {
  const widths = (rows[0] ?? []).map((_, i) => Math.max(...rows.map((r) => (r[i] ?? "").length)));
  return rows
    .map((r) =>
      r
        .map((cell, i) => cell.padEnd(widths[i] ?? 0))
        .join("  ")
        .trimEnd(),
    )
    .join("\n");
}

function percentiles(record: PerfRunRecord): string {
  const of = (pick: (run: PerfRepetition) => number | null): number | null =>
    median(record.runs.map(pick).filter((v): v is number => v !== null));
  return `${fixed(
    of((r) => r.frames.p50Ms),
    2,
  )} / ${fixed(
    of((r) => r.frames.p95Ms),
    2,
  )} / ${fixed(
    of((r) => r.frames.p99Ms),
    2,
  )}`;
}

/** A comparison as the tool prints it. */
export function comparisonText(comparison: Comparison): string {
  const { old: older, new: newer } = comparison;
  const lines = [
    `${newer.id} against ${older.id}`,
    `${newer.scene.id} v${String(newer.scene.version)} on ${newer.machine.name} at ${newer.setting.quality}: ${String(older.runs.length)} and ${String(newer.runs.length)} repetitions`,
    ...comparison.changes.map((c) => `changed: ${c.field} ${c.old} → ${c.new}`),
    "",
    columns([
      ["metric", "old median [range]", "new median [range]", "effect", "p", "verdict"],
      ...comparison.metrics.map((m) => {
        const digits = m.metric === "missedFraction" ? 4 : 3;
        const effect =
          m.ratio !== null
            ? `ratio ${m.ratio.toFixed(3)}`
            : m.difference !== null
              ? `difference ${m.difference >= 0 ? "+" : ""}${m.difference.toFixed(4)}`
              : "—";
        return [
          METRIC_NAMES[m.metric],
          sideText(m.old, digits),
          sideText(m.new, digits),
          effect,
          m.p === null || m.separation === "none" ? "—" : m.p.toPrecision(2),
          m.verdict ?? "NOT AVAILABLE",
        ];
      }),
    ]),
    "",
    "For reading, without a verdict:",
    columns([
      ["", "old", "new"],
      ["frame p50 / p95 / p99, ms (median of repetitions)", percentiles(older), percentiles(newer)],
      [
        "frame p99 over the pooled frames, ms",
        fixed(older.pooledFrames.p99Ms, 2),
        fixed(newer.pooledFrames.p99Ms, 2),
      ],
      ["pooled frames", String(older.pooledFrames.count), String(newer.pooledFrames.count)],
    ]),
  ];
  return `${lines.join("\n")}\n`;
}

/** The outcome of `just perf-compare`: its exit code and its text. */
export interface CompareOutcome {
  /** 0 for a comparison, whatever its verdicts, and for a scene's first record; 1 for a refusal. */
  readonly exitCode: 0 | 1;
  readonly text: string;
}

/**
 * `just perf-compare <id> [--against <id>]` over the store's text: the record against the last
 * comparable one, or against the record named. It never fails on a regression.
 */
export function perfCompare(store: string, id: string, against: string | null): CompareOutcome {
  let records: ReadonlyArray<PerfRunRecord>;
  try {
    records = parsePerfRecords(store);
  } catch (error) {
    if (error instanceof PerfRecordError)
      return { exitCode: 1, text: `the store is not valid: ${error.message}\n` };
    throw error;
  }
  const newer = records.find((r) => r.id === id);
  if (newer === undefined) return { exitCode: 1, text: `no record ${id} in the store\n` };
  if (against !== null) {
    const older = records.find((r) => r.id === against);
    if (older === undefined) return { exitCode: 1, text: `no record ${against} in the store\n` };
    const differs = incomparability(older, newer);
    if (differs !== null) {
      return {
        exitCode: 1,
        text: `refused: ${older.id} differs in ${differs.field}: ${differs.old} against ${differs.new}\n`,
      };
    }
    return { exitCode: 0, text: comparisonText(compareRecords(older, newer)) };
  }
  const previous = previousComparable(records, newer);
  let outcome: CompareOutcome;
  switch (previous.kind) {
    case "none":
      outcome = {
        exitCode: 0,
        text: `${newer.id}: no earlier record of ${newer.scene.id}, nothing to compare\n`,
      };
      break;
    case "refused":
      outcome = {
        exitCode: 1,
        text: `refused: no earlier record compares with ${newer.id}; the last of ${newer.scene.id}, ${previous.nearest.id}, differs in ${previous.differs.field}: ${previous.differs.old} against ${previous.differs.new}\n`,
      };
      break;
    case "comparable":
      outcome = { exitCode: 0, text: comparisonText(compareRecords(previous.old, newer)) };
      break;
  }
  return outcome;
}
