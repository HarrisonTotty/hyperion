/**
 * The writer of `docs/measurements/rendering/budget.md`'s generated sections (plan R12, Design
 * notes 1, 4, 6 and 7; R12.T1.b): the frame-time and memory tables as the brainstorm lays them
 * out, estimate beside measurement, the streaming and CPU figures, and the runs with their marks.
 *
 * @remarks
 * A pure function of the records: the same records give the same text, which the sync test holds
 * `budget.md` to. A section lies between two `<!-- generated:<name> -->` markers, inside Prettier's
 * ignore markers so that its tables stay as written. A `virtual` record (a virtual display or a
 * hidden window, Design note 11) is left out of every table and listed with the runs; a `forced`
 * one is marked wherever it is shown. A cell takes the latest record of its column with the row,
 * preferring one whose timer is `full`, and shows the median over its repetitions.
 */

import type { MachineRecord, PerfQuality, PerfRepetition, PerfRunRecord } from "./record.ts";
import { BUDGET_ROWS, MEMORY_ITEMS_ORDER } from "./rows.ts";
import type { BudgetRow, MemoryItem } from "./rows.ts";
import { median, monotoneTrend, throttleMarks } from "./stats.ts";

/** The generated sections of `budget.md`, in the order the file holds them. */
export const GENERATED_SECTIONS = ["frame-time", "frames", "memory", "streaming", "runs"] as const;

/** A generated section of `budget.md`. */
export type GeneratedSection = (typeof GENERATED_SECTIONS)[number];

/** The marker that opens and closes a generated section. */
export function sectionMarker(section: GeneratedSection): string {
  return `<!-- generated:${section} -->`;
}

/** One of the brainstorm's estimates for a row, on one of its two columns. */
export interface Estimate {
  /** The figure as the brainstorm states it. */
  readonly text: string;
}

/**
 * A frame-time estimate with its upper end, where the brainstorm states one: the ladder's excess
 * rule (Design note 10) sets a row's measured median against it.
 */
export interface FrameEstimate extends Estimate {
  readonly upperMs: number | null;
}

/** A row of one of the brainstorm's tables, by its name there, with its two estimates. */
export interface BrainstormRow<E extends Estimate = Estimate> {
  /** The row's name in the brainstorm's table, its citation; `null` for a row measured only. */
  readonly brainstorm: string | null;
  /** The row's name in `budget.md`. */
  readonly label: string;
  readonly discrete: E | null;
  readonly uhd620: E | null;
}

function est(text: string): Estimate {
  return { text };
}

function ms(text: string, upperMs: number): FrameEstimate {
  return { text, upperMs };
}

function row<E extends Estimate>(brainstorm: string, discrete: E, uhd620: E): BrainstormRow<E> {
  return { brainstorm, label: brainstorm, discrete, uhd620 };
}

function measuredOnly(label: string): BrainstormRow<never> {
  return { brainstorm: null, label: `${label} (measured only)`, discrete: null, uhd620: null };
}

/**
 * The brainstorm's frame-time table (Performance budget: "Discrete target, 1080p, 16.7 ms budget"
 * and "UHD 620 low setting, 720p, 33 ms budget"), by row, with upper ends: estimates, not
 * measurements.
 */
export const FRAME_ESTIMATES: Readonly<Record<BudgetRow, BrainstormRow<FrameEstimate>>> = {
  terrain: row("Terrain geometry and patches", ms("3–5 ms", 5), ms("8–14 ms", 14)),
  "atmosphere-frame": row("Atmosphere, per frame", ms("0.5–1 ms", 1), ms("2–4 ms", 4)),
  "atmosphere-tables": row(
    "Atmosphere, per-planet tables",
    ms("under 0.1 ms, when the atmosphere changes", 0.1),
    ms("about 1 ms, on the same occasions", 1),
  ),
  clouds: row(
    "Volumetric clouds",
    ms("1.5–3 ms at quarter resolution", 3),
    ms("cut: a two-dimensional layer at about 1 ms", 1),
  ),
  ocean: row("Ocean", ms("1–2 ms, Gerstner", 2), ms("2–3 ms", 3)),
  shadows: row("Shadows", ms("1.5–3 ms", 3), ms("0.3–0.6 ms", 0.6)),
  stars: row(
    "Star field and galactic band",
    ms("under 0.2 ms near the Sun", 0.2),
    ms("under 0.5 ms", 0.5),
  ),
  "exposure-histogram": row("Exposure histogram", ms("0.3–0.5 ms", 0.5), ms("about 1 ms", 1)),
  "bloom-tonemap": row("Bloom and tone mapping", ms("under 1 ms", 1), ms("2–3 ms", 3)),
  scatter: row("Scatter instances", ms("1–2 ms", 2), ms("off, or a token density under 1 ms", 1)),
  rings: row("Rings, when in view", ms("0.5–1 ms", 1), ms("under 0.5 ms", 0.5)),
  "station-wireframe": row(
    "Station wireframe view",
    ms("under 3 ms at 1080p", 3),
    ms("4–9 ms at 1080p, against its own 16.7 ms frame", 9),
  ),
  "cockpit-instruments": measuredOnly("Cockpit instruments"),
  compositing: measuredOnly("Consoles' compositing"),
  other: measuredOnly("Other passes"),
};

const MB = 1_000_000;

/**
 * The brainstorm's memory table (Performance budget: "Discrete target" and "UHD 620 low setting"),
 * by item: computed from formats and sizes, not measured. R08's per-view
 * tables are the atmosphere row's "0.43 MB a view".
 */
export const MEMORY_ESTIMATES: Readonly<Record<MemoryItem, BrainstormRow>> = {
  "star-cubemap": row("Star cubemap", est("about 300 MB with mips"), est("34 MB with mips")),
  "height-cache": row("Height-texture cache", est("about 400 MB"), est("about 64 MB")),
  "atmosphere-tables": row(
    "Atmosphere tables",
    est("about 0.4 MB a planet on an Earth, under 2 MB"),
    est("smaller tables, within the discrete figures"),
  ),
  "atmosphere-view": {
    brainstorm: "Atmosphere tables",
    label: "Atmosphere tables, per view",
    discrete: est("0.43 MB a view, summed over suns"),
    uhd620: est("within the discrete figures"),
  },
  "coarse-field-gpu": row(
    "Coarse field, GPU copy",
    est("at most 384 KiB at level 8"),
    est("the same"),
  ),
  "render-targets": row("Render targets", est("about 70 MB at 1080p"), est("about 30 MB at 720p")),
  shadows: row("Shadows", est("four 2,048² cascades, 64 MB"), est("none beyond the patch cache")),
  clouds: row(
    "Clouds",
    est("3D noise volumes, 10–20 MB"),
    est("a two-dimensional layer, under 16 MB"),
  ),
  ocean: measuredOnly("Ocean"),
  rings: measuredOnly("Rings"),
  scatter: measuredOnly("Scatter"),
  stills: measuredOnly("Stills"),
  other: measuredOnly("Other"),
};

/** The brainstorm's GPU total (the memory table's "Total, GPU" row). */
export const MEMORY_TOTAL_ESTIMATE: BrainstormRow = row(
  "Total, GPU",
  est("about 0.75–1.25 GB"),
  est("about 150–300 MB"),
);

/** The brainstorm's coarse-field copies on the CPU (the paragraph under its memory table). */
export const WORKER_FIELD_ESTIMATE: BrainstormRow = {
  brainstorm: "On the CPU side each height worker holds its own copy of the surveyed coarse field",
  label: "Coarse field, the workers' copies (CPU)",
  discrete: est("4–15 MB a worker; about 45 MB for an Earth in three"),
  uhd620: est("the same"),
};

/** How a column recognises its machine: by its GPU's PCI IDs, or by platform and CPU model. */
export type MachineMatch =
  | { readonly kind: "gpu"; readonly vendorId: number; readonly deviceId: number }
  | { readonly kind: "cpu"; readonly platform: MachineRecord["platform"]; readonly model: string };

/** A measured column of the tables: one machine at one setting. */
export interface BudgetColumn {
  readonly label: string;
  readonly quality: PerfQuality;
  readonly match: MachineMatch;
  /** The brainstorm's column it is set beside, or `null` for a column with no estimates. */
  readonly estimate: "discrete" | "uhd620" | null;
  /** Shown with no record; otherwise only once a record falls in it. */
  readonly always: boolean;
}

/**
 * The tables' measured columns (Design note 11; the README's Target hardware): the recommended
 * specification's RTX 3080 (GA102, `10de:2206`) at the high setting and the UHD 620 laptop
 * (`8086:3ea0`) at the low setting, each beside its brainstorm column; then, once they have records,
 * the RTX 3080 at the low setting (R12.T9.a) and the owner's M5 Mac at both settings, a third
 * machine (a lean pending the owner, 2026-11-13: remove its two entries to revert it).
 */
export const BUDGET_COLUMNS: ReadonlyArray<BudgetColumn> = [
  {
    label: "RTX 3080, high",
    quality: "high",
    match: { kind: "gpu", vendorId: 0x10de, deviceId: 0x2206 },
    estimate: "discrete",
    always: true,
  },
  {
    label: "UHD 620, low",
    quality: "low",
    match: { kind: "gpu", vendorId: 0x8086, deviceId: 0x3ea0 },
    estimate: "uhd620",
    always: true,
  },
  {
    label: "RTX 3080, low",
    quality: "low",
    match: { kind: "gpu", vendorId: 0x10de, deviceId: 0x2206 },
    estimate: null,
    always: false,
  },
  {
    label: "Apple M5, high (lean)",
    quality: "high",
    match: { kind: "cpu", platform: "darwin", model: "Apple M5" },
    estimate: null,
    always: false,
  },
  {
    label: "Apple M5, low (lean)",
    quality: "low",
    match: { kind: "cpu", platform: "darwin", model: "Apple M5" },
    estimate: null,
    always: false,
  },
];

function matches(machine: MachineRecord, match: MachineMatch): boolean {
  let found: boolean;
  switch (match.kind) {
    case "gpu":
      found =
        machine.gpu !== null &&
        machine.gpu.vendorId === match.vendorId &&
        machine.gpu.deviceId === match.deviceId;
      break;
    case "cpu":
      found = machine.platform === match.platform && machine.cpu.model.includes(match.model);
      break;
  }
  return found;
}

/** Whether a record counts in a column: a physical display, the column's machine and setting. */
export function inColumn(record: PerfRunRecord, column: BudgetColumn): boolean {
  return (
    record.harness.display === "physical" &&
    record.setting.quality === column.quality &&
    matches(record.machine, column.match)
  );
}

interface Indexed {
  readonly record: PerfRunRecord;
  readonly index: number;
}

/** The latest record of `candidates`, by date and then by its place in the store. */
function latest(candidates: ReadonlyArray<Indexed>): Indexed | undefined {
  let best: Indexed | undefined;
  for (const candidate of candidates) {
    if (best === undefined || candidate.record.date > best.record.date) best = candidate;
    else if (candidate.record.date === best.record.date && candidate.index > best.index)
      best = candidate;
  }
  return best;
}

function fixed(value: number | null, digits: number): string {
  return value === null ? "—" : value.toFixed(digits);
}

function megabytes(bytes: number | null): string {
  return bytes === null ? "—" : `${(bytes / MB).toFixed(1)} MB`;
}

function mediansOf(values: ReadonlyArray<number | null>): number | null {
  return median(values.filter((v): v is number => v !== null));
}

function forcedMark(record: PerfRunRecord): string {
  return record.harness.forced ? " (forced)" : "";
}

function shownColumns(records: ReadonlyArray<PerfRunRecord>): ReadonlyArray<BudgetColumn> {
  return BUDGET_COLUMNS.filter((c) => c.always || records.some((r) => inColumn(r, c)));
}

/** A cell's text with the characters that would break its table escaped or joined. */
function cleanCell(cell: string): string {
  return cell.replaceAll("|", "\\|").replaceAll("\n", " ");
}

/** A Markdown table, its columns padded to one width so that it reads as text too. */
function table(head: ReadonlyArray<string>, body: ReadonlyArray<ReadonlyArray<string>>): string {
  const cleanHead = head.map(cleanCell);
  const cleanBody = body.map((cells) => cells.map(cleanCell));
  const widths = cleanHead.map((cell, i) =>
    Math.max(3, cell.length, ...cleanBody.map((cells) => (cells[i] ?? "").length)),
  );
  const line = (cells: ReadonlyArray<string>): string =>
    `| ${widths.map((w, i) => (cells[i] ?? "").padEnd(w)).join(" | ")} |`;
  return [
    line(cleanHead),
    `| ${widths.map((w) => "-".repeat(w)).join(" | ")} |`,
    ...cleanBody.map(line),
  ].join("\n");
}

function estimateOf(brainstorm: BrainstormRow, column: "discrete" | "uhd620"): string {
  return (column === "discrete" ? brainstorm.discrete : brainstorm.uhd620)?.text ?? "—";
}

/** The header of a table with each estimate column set before its measured column. */
function header(
  first: string,
  columns: ReadonlyArray<BudgetColumn>,
  unit: string,
): ReadonlyArray<string> {
  return [
    first,
    ...columns.flatMap((c) =>
      c.estimate === null
        ? [`${c.label}, measured ${unit}`]
        : [
            `${c.estimate === "discrete" ? "Discrete" : "UHD 620"} estimate`,
            `${c.label}, measured ${unit}`,
          ],
    ),
  ];
}

function cellsOf(
  brainstorm: BrainstormRow,
  columns: ReadonlyArray<BudgetColumn>,
  measuredCell: (column: BudgetColumn) => string,
): ReadonlyArray<string> {
  return [
    brainstorm.label,
    ...columns.flatMap((c) =>
      c.estimate === null
        ? [measuredCell(c)]
        : [estimateOf(brainstorm, c.estimate), measuredCell(c)],
    ),
  ];
}

function rowFigures(record: PerfRunRecord, budgetRow: BudgetRow): string {
  const found = record.runs.flatMap((run) => run.rows.filter((r) => r.row === budgetRow));
  const bracketed = record.runs.some((run) =>
    run.passes.some((p) => p.row === budgetRow && record.harness.bracketedPasses.includes(p.key)),
  );
  const figures = [
    mediansOf(found.map((r) => r.p50Ms)),
    mediansOf(found.map((r) => r.p95Ms)),
    mediansOf(found.map((r) => r.p99Ms)),
  ]
    .map((v) => fixed(v, 2))
    .join(" / ");
  return `${bracketed ? "≤ " : ""}${figures}`;
}

function frameCell(
  records: ReadonlyArray<PerfRunRecord>,
  column: BudgetColumn,
  budgetRow: BudgetRow,
): string {
  const candidates = records
    .map((record, index) => ({ record, index }))
    .filter(
      ({ record }) =>
        inColumn(record, column) &&
        record.runs.some((run) => run.rows.some((r) => r.row === budgetRow)),
    );
  // A station run whose server shared its machine is shown, never counted as the station's
  // figure (Design note 8): it fills the station's cell only where no other record does, marked.
  const counted =
    budgetRow === "station-wireframe"
      ? candidates.filter(({ record }) => record.harness.server.placement !== "co-resident")
      : candidates;
  const pool = counted.length > 0 ? counted : candidates;
  const pick = latest(pool.filter(({ record }) => record.harness.timer === "full")) ?? latest(pool);
  if (pick === undefined) return "—";
  const { record } = pick;
  const coResident =
    budgetRow === "station-wireframe" && record.harness.server.placement === "co-resident"
      ? " (co-resident: not the station's figure)"
      : "";
  let cell: string;
  switch (record.harness.timer) {
    case "quantized":
      cell = `QUANTISED (${record.id})`;
      break;
    case "absent":
      cell = `NOT AVAILABLE (${record.id})`;
      break;
    case "full":
      cell = `${rowFigures(record, budgetRow)} (${record.id})${forcedMark(record)}`;
      break;
  }
  return `${cell}${coResident}`;
}

function frameTimeSection(records: ReadonlyArray<PerfRunRecord>): string {
  const columns = shownColumns(records);
  const body = BUDGET_ROWS.map((r) =>
    cellsOf(FRAME_ESTIMATES[r], columns, (c) => frameCell(records, c, r)),
  );
  return [
    "GPU time a frame by row: measured p50 / p95 / p99 in ms, the median over the record's",
    "repetitions, with the record's ID; `—` where no record has the row yet.",
    "",
    table(header("Pass (the brainstorm's row)", columns, "p50 / p95 / p99, ms"), body),
  ].join("\n");
}

function spread(values: ReadonlyArray<number | null>, digits: number): string {
  const known = values.filter((v): v is number => v !== null);
  const middle = median(known);
  if (middle === null) return "—";
  return `${middle.toFixed(digits)} [${Math.min(...known).toFixed(digits)}, ${Math.max(...known).toFixed(digits)}]`;
}

function framesSection(records: ReadonlyArray<PerfRunRecord>): string {
  const body: string[][] = [];
  for (const column of shownColumns(records)) {
    const inIt = records
      .map((record, index) => ({ record, index }))
      .filter(({ record }) => inColumn(record, column));
    for (const scene of new Set(inIt.map(({ record }) => record.scene.id))) {
      const pick = latest(inIt.filter(({ record }) => record.scene.id === scene));
      if (pick === undefined) continue;
      const { record } = pick;
      body.push([
        column.label,
        `${record.id}${forcedMark(record)}`,
        String(record.pooledFrames.count),
        spread(
          record.runs.map((run) => run.frames.p50Ms),
          2,
        ),
        spread(
          record.runs.map((run) => run.frames.p95Ms),
          2,
        ),
        spread(
          record.runs.map((run) => run.frames.p99Ms),
          2,
        ),
        fixed(record.pooledFrames.p99Ms, 2),
        spread(
          record.runs.map((run) => run.frames.missedFraction),
          4,
        ),
      ]);
    }
  }
  return [
    "Frame intervals, R05's percentiles (Design note 7): the median over repetitions of each",
    "repetition's p50, p95 and p99 with their range, the frames pooled, and p99 over the pooled",
    "frames beside the median of the repetitions' p99s; the latest record of each scene a column holds.",
    "",
    body.length === 0
      ? "No record yet."
      : table(
          [
            "Column",
            "Record",
            "Frames",
            "p50, ms [range]",
            "p95, ms [range]",
            "p99, ms [range]",
            "Pooled p99, ms",
            "Missed fraction [range]",
          ],
          body,
        ),
  ].join("\n");
}

function latestWith(
  records: ReadonlyArray<PerfRunRecord>,
  column: BudgetColumn,
  has: (run: PerfRepetition) => boolean,
): PerfRunRecord | undefined {
  const candidates = records
    .map((record, index) => ({ record, index }))
    .filter(({ record }) => inColumn(record, column) && record.runs.some(has));
  return latest(candidates)?.record;
}

function memoryCell(
  records: ReadonlyArray<PerfRunRecord>,
  column: BudgetColumn,
  item: MemoryItem,
): string {
  const record = latestWith(records, column, (run) => run.memory.itemPeakBytes[item] !== undefined);
  if (record === undefined) return "—";
  const peak = mediansOf(record.runs.map((run) => run.memory.itemPeakBytes[item] ?? null));
  return `${megabytes(peak)} (${record.id})${forcedMark(record)}`;
}

function totalCell(records: ReadonlyArray<PerfRunRecord>, column: BudgetColumn): string {
  const record = latestWith(records, column, (run) => run.memory.gpu !== null);
  if (record === undefined) return "—";
  const gpu = record.runs.flatMap((run) => (run.memory.gpu === null ? [] : [run.memory.gpu]));
  const resident = mediansOf(gpu.map((g) => g.residentPeakBytes));
  const lower = mediansOf(gpu.map((g) => g.residentLowerBoundBytes));
  const sources = [...new Set(gpu.map((g) => g.source))].join(", ");
  const bound = lower === null ? "" : `, at least ${megabytes(lower)}`;
  const tally = sources === "adapter-tally" ? ", a lower bound" : "";
  return `${megabytes(resident)} resident (${sources}${tally}${bound}) (${record.id})${forcedMark(record)}`;
}

function workerFieldCell(records: ReadonlyArray<PerfRunRecord>, column: BudgetColumn): string {
  const record = latestWith(records, column, (run) => run.memory.workerFieldBytes.length > 0);
  if (record === undefined) return "—";
  const runs = record.runs.filter((run) => run.memory.workerFieldBytes.length > 0);
  const total = median(runs.map((run) => run.memory.workerFieldBytes.reduce((a, b) => a + b, 0)));
  const workers = median(runs.map((run) => run.memory.workerFieldBytes.length));
  return `${megabytes(total)} in ${fixed(workers, 0)} workers (${record.id})${forcedMark(record)}`;
}

function memorySection(records: ReadonlyArray<PerfRunRecord>): string {
  const columns = shownColumns(records);
  const body = [
    ...MEMORY_ITEMS_ORDER.map((item) =>
      cellsOf(MEMORY_ESTIMATES[item], columns, (c) => memoryCell(records, c, item)),
    ),
    cellsOf(MEMORY_TOTAL_ESTIMATE, columns, (c) => totalCell(records, c)),
    cellsOf(WORKER_FIELD_ESTIMATE, columns, (c) => workerFieldCell(records, c)),
  ];
  return [
    "Peak bytes by item from the allocation tally, then the kernel's resident total (Design note 6):",
    "the median over the record's repetitions, in MB of 10⁶ bytes.",
    "",
    table(header("Item (the brainstorm's row)", columns, "peak"), body),
  ].join("\n");
}

function streamingSection(records: ReadonlyArray<PerfRunRecord>): string {
  const columns = shownColumns(records);
  const streamingRows: string[][] = [];
  const cpuRows: string[][] = [];
  for (const column of columns) {
    const streamed = latestWith(records, column, (run) => run.streaming.length > 0);
    if (streamed !== undefined) {
      const segments = streamed.runs[0]?.streaming.map((s) => s.segment) ?? [];
      for (const segment of segments) {
        const each = streamed.runs.flatMap((run) =>
          run.streaming.filter((s) => s.segment === segment),
        );
        streamingRows.push([
          column.label,
          streamed.id,
          segment,
          fixed(mediansOf(each.map((s) => s.patchesPerS)), 1),
          fixed(mediansOf(each.map((s) => s.demandPerS)), 1),
          fixed(mediansOf(each.map((s) => s.streamingSeconds)), 1),
        ]);
      }
    }
    const cpu = latestWith(
      records,
      column,
      (run) => run.mainThreadPerFrameMs !== null || run.heightPointUs !== null,
    );
    if (cpu !== undefined) {
      cpuRows.push([
        column.label,
        cpu.id,
        fixed(mediansOf(cpu.runs.map((run) => run.mainThreadPerFrameMs)), 2),
        fixed(mediansOf(cpu.runs.map((run) => run.heightPointUs)), 1),
        fixed(median(cpu.runs.map((run) => run.latePipelines)), 0),
      ]);
    }
  }
  return [
    "Patches a second sustained against the demand, by segment, and the time spent under",
    "`TERRAIN: STREAMING` (Design note 9); then the CPU's figures: the renderer's main thread a",
    "frame, the height workers' µs a point with its gradient (the budget's 10 µs) and the",
    "pipelines created late.",
    "",
    streamingRows.length === 0
      ? "No record has streaming figures yet."
      : table(
          ["Column", "Record", "Segment", "Patches a second", "Demand a second", "Streaming, s"],
          streamingRows,
        ),
    "",
    cpuRows.length === 0
      ? "No record has CPU figures yet."
      : table(
          [
            "Column",
            "Record",
            "Main thread a frame, ms",
            "Height workers, µs a point",
            "Late pipelines",
          ],
          cpuRows,
        ),
  ].join("\n");
}

/** The marks a record's line carries in the runs table. */
export function recordMarks(record: PerfRunRecord): ReadonlyArray<string> {
  const { harness } = record;
  const marks: string[] = [];
  if (harness.forced) marks.push(`FORCED: ${harness.forcedReasons.join("; ")}`);
  if (harness.warmUp.steady === false) marks.push("NOT STEADY");
  if (!harness.gpuTiming) marks.push("control: no GPU-timing switch");
  if (harness.timer === "quantized") marks.push("QUANTISED");
  if (harness.timer === "absent") marks.push("timer NOT AVAILABLE");
  if (harness.display === "virtual") marks.push("virtual: left out of the tables");
  if (harness.server.placement === "co-resident") marks.push("server co-resident");
  if (
    harness.server.placement !== "none" &&
    harness.repetitions > 1 &&
    !harness.server.restartedPerRepetition
  ) {
    marks.push("server not restarted per repetition");
  }
  const means = record.runs.map((run) => run.frames.meanMs);
  if (means.every((m): m is number => m !== null) && monotoneTrend(means)) marks.push("TREND");
  const throttled = throttleMarks(record.runs.map((run) => run.throttle.throttleFraction));
  if (throttled.length > 0) marks.push(`throttling apart: repetitions ${throttled.join(", ")}`);
  return marks;
}

function runsSection(records: ReadonlyArray<PerfRunRecord>): string {
  if (records.length === 0) return "No record yet: `just perf-record <file>` adds one.";
  const body = records.map((r) => {
    const marks = recordMarks(r);
    return [
      r.id,
      r.source,
      `${r.scene.id} v${String(r.scene.version)}`,
      r.machine.name,
      r.setting.quality,
      String(r.harness.repetitions),
      marks.length > 0 ? marks.join("; ") : "—",
      r.runs.map((run) => fixed(run.frames.meanMs, 2)).join(", "),
    ];
  });
  return [
    "Every record, in the store's order, with its marks and its repetitions' mean frame intervals",
    "(Design note 7: the trend screen's input, for reading beside it).",
    "",
    table(
      [
        "Record",
        "Source",
        "Scene",
        "Machine",
        "Setting",
        "Repetitions",
        "Marks",
        "Mean frame by repetition, ms",
      ],
      body,
    ),
  ].join("\n");
}

/** `budget.md` has lost a generated section's marker, or holds one twice. */
export class BudgetMarkerError extends Error {
  /** The section whose markers are wrong. */
  readonly section: GeneratedSection;

  constructor(section: GeneratedSection, found: number) {
    super(
      `budget.md holds ${String(found)} ${sectionMarker(section)} markers, not the 2 that bound its section`,
    );
    this.name = "BudgetMarkerError";
    this.section = section;
  }
}

/** Every generated section's text from the records. */
export function budgetSections(
  records: ReadonlyArray<PerfRunRecord>,
): Readonly<Record<GeneratedSection, string>> {
  return {
    "frame-time": frameTimeSection(records),
    frames: framesSection(records),
    memory: memorySection(records),
    streaming: streamingSection(records),
    runs: runsSection(records),
  };
}

/**
 * `budget.md` with every generated section written from the records and the prose around them
 * kept as it is.
 *
 * @throws {@link BudgetMarkerError} if a section's two markers are not in the text, each once: a
 *   hand edit broke the file.
 */
export function writeBudget(markdown: string, records: ReadonlyArray<PerfRunRecord>): string {
  const sections = budgetSections(records);
  let out = markdown;
  for (const section of GENERATED_SECTIONS) {
    const marker = sectionMarker(section);
    const parts = out.split(marker);
    if (parts.length !== 3) throw new BudgetMarkerError(section, parts.length - 1);
    const [before, , after] = parts;
    out = [
      before,
      marker,
      "\n<!-- prettier-ignore-start -->\n",
      sections[section],
      "\n<!-- prettier-ignore-end -->\n",
      marker,
      after,
    ].join("");
  }
  return out;
}
