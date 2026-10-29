/**
 * R10.T4.b's counts of the selected patches (plan R10; `signoff-12.md` item 30), rebuilt from its
 * record ("The counts", "The counts' harness is by hand") for R10.T15.b, which takes them again on
 * the fixture re-blessed at generator version 24 (decision-r10-item30 §6).
 *
 * @remarks
 * - **The worlds** are R09's six sample worlds as R10.T4.a measured them
 *   (`fixtures/level-bounds.json`), each level table composed from the fixture as the worker's
 *   `level_table` makes it ({@link sampleWorld}), with σ the fixture's `sigma_m`, each world a
 *   sphere of its radius. `levelBounds.test.ts` reads the same composition.
 * - **The descent** is R05's at its record seed (`RECORD_SEED`, 7), at {@link COUNT_POSE_STEP_S}
 *   apart from 0 s to its end: 124 poses.
 * - **The view** is one 1,920 × 1,080 view of 60° (the presented size, Design note 23) for each
 *   style ({@link COUNT_STYLES}): τ = 1 px (high), 2 px (low) and 4 px (wireframe), with no
 *   contact.
 * - **Two selections a pose**: one held to {@link GUARD_PATCHES} (the guard), whose patches are
 *   counted, and one at the style's pass budget, half its slots (604 high, 187 low, 260
 *   wireframe), whose limit excess gives τ′ = τ × max(1, excess).
 * - **The boxes** are unbaked, each its level's subtree range from the table (the rule's "from the
 *   fixture"), or baked flat at ±1 m, so that τ alone sets the depth, as
 *   `terrainWireSelection.test.ts` bakes them.
 *
 * Renderer code reads no files, so `scripts/levelCounts.mjs` reads the fixture, runs the cells and
 * writes the record this module makes ({@link levelCountsMarkdown}); it is by hand, not a test: an
 * unbudgeted selection on the hard bound reaches 10⁵–10⁶ patches near the ground, so the guard
 * caps it and its sums are lower bounds where it binds.
 */

import {
  type QualitySetting,
  SETTINGS,
  WIREFRAME_TERRAIN_SLOTS,
  WIREFRAME_TERRAIN_TAU_PX,
} from "../quality/qualitySetting";
import { RECORD_SEED } from "../spike/demandRecord";
import { DescentProfile, landingSiteOf } from "../spike/descentProfile";
import { vertexSpacing } from "./cube";
import { LEVEL_SIGMA_LENGTH, type SelectionRule } from "./levelBounds";
import { MAX_LEVEL } from "./patchKey";
import { type BodyFigure, LEVEL_TABLE_STRIDE, type PlanetGeometry, planetGeometry } from "./planet";
import { type HeightRangeLookup, selectPatches, type ViewSelectionInput } from "./select";
import { terrainSlotLayout } from "./slotLayout";

/** One band level's row of a fixture world, as `level_bound_worlds.rs` writes it. */
export interface LevelBoundsLevel {
  readonly level: number;
  readonly bound_m: number;
  readonly sigma_m: number;
  readonly contribution_low_m: number;
  readonly contribution_high_m: number;
}

/** One level's patches as R10.T4.a measured them, below the complete level. */
export interface LevelBoundsMeasured {
  readonly level: number;
  readonly patches: number;
  readonly patch_max_over_bound: number;
  /** The patches whose maximum passes 4σ of their own cells. */
  readonly patches_over_4sigma: number;
  /** The patches whose maximum passes 4σ of the body, the σ selection reads. */
  readonly patches_over_4sigma_body: number;
}

/** One of `fixtures/level-bounds.json`'s worlds, the members the counts read. */
export interface LevelBoundsWorld {
  readonly name: string;
  readonly radius_m: number;
  readonly finest_level: number;
  readonly complete_level: number;
  readonly elevation_lowest_m: number;
  readonly elevation_highest_m: number;
  readonly levels: ReadonlyArray<LevelBoundsLevel>;
  readonly measured: ReadonlyArray<LevelBoundsMeasured>;
}

/** A sample world as selection reads it, with T4.a's per-patch counts. */
export interface SampleWorld {
  readonly name: string;
  readonly figure: BodyFigure;
  readonly finestLevel: number;
  readonly completeLevel: number;
  readonly table: Float64Array;
  readonly sigmaM: Float64Array;
  /**
   * T4.a's per-patch maximum deviation by level, metres: its `patch_max_over_bound` × ε_n, 0 from
   * the complete level on, where nothing is omitted.
   */
  readonly patchMaxM: Float64Array;
  /**
   * T4.a's patches sampled over every level, those whose maximum passed 4σ_body, and those whose
   * maximum passed 4σ of their own cells.
   */
  readonly patches: number;
  readonly pastBodySigma: number;
  readonly pastOwnSigma: number;
}

/** The worker's slack on a held elevation, metres: a micrometre and a part in 10¹². */
function slackM(x: number): number {
  return 1e-6 + 1e-12 * Math.abs(x);
}

/**
 * A fixture world as selection reads it: its level table composed as the worker's `level_table`
 * makes it (`crates/hyperion-surface/src/worker.rs`): the cells' lowest and highest elevation,
 * widened by a micrometre and a part in 10¹², plus the most the level's and its parent's
 * contributions can add, stretched to the datum; the stated bound; the largest vertex spacing on
 * the radius. Its σ is the fixture's `sigma_m`. Levels past the complete level take its row (bound
 * and σ 0). The world is a sphere of its radius: the fixture holds no flattening.
 *
 * @throws Error for a world missing a level row up to its complete level.
 */
export function sampleWorld(world: LevelBoundsWorld): SampleWorld {
  const a = world.radius_m;
  const complete = world.complete_level;
  const row = (level: number): LevelBoundsLevel => {
    const found = world.levels[Math.min(level, complete)];
    if (found === undefined) {
      throw new Error(`${world.name} has no level ${String(level)}`);
    }
    return found;
  };
  const baseLowM = world.elevation_lowest_m - slackM(world.elevation_lowest_m);
  const baseHighM = world.elevation_highest_m + slackM(world.elevation_highest_m);
  const table = new Float64Array(LEVEL_TABLE_STRIDE * (MAX_LEVEL + 1));
  const sigmaM = new Float64Array(LEVEL_SIGMA_LENGTH);
  for (let level = 0; level <= MAX_LEVEL; level += 1) {
    const own = row(level);
    const parent = row(Math.max(0, level - 1));
    const lowM = Math.min(own.contribution_low_m, parent.contribution_low_m);
    const highM = Math.max(own.contribution_high_m, parent.contribution_high_m);
    table.set(
      [
        own.bound_m,
        Math.min(baseLowM + lowM, 0),
        Math.max(baseHighM + highM, 0),
        vertexSpacing(a, level).maxM,
      ],
      LEVEL_TABLE_STRIDE * level,
    );
    sigmaM[level] = own.sigma_m;
  }
  let patches = 0;
  let pastBodySigma = 0;
  let pastOwnSigma = 0;
  const patchMaxM = new Float64Array(LEVEL_SIGMA_LENGTH);
  for (const measured of world.measured) {
    patches += measured.patches;
    pastBodySigma += measured.patches_over_4sigma_body;
    pastOwnSigma += measured.patches_over_4sigma;
    if (measured.level < complete) {
      patchMaxM[measured.level] = measured.patch_max_over_bound * row(measured.level).bound_m;
    }
  }
  return {
    name: world.name,
    figure: { equatorialRadiusM: a, polarRadiusM: a, pole: null },
    finestLevel: world.finest_level,
    completeLevel: complete,
    table,
    sigmaM,
    patchMaxM,
    patches,
    pastBodySigma,
    pastOwnSigma,
  };
}

/** The selection rules counted: the hard bound, and min(hard, 4σ). */
export const COUNT_RULES: ReadonlyArray<SelectionRule> = ["hard", "statistical"];

/** A style as its terrain pass selects: its name, its setting, its τ and its budget, half its slots. */
export interface CountStyle {
  readonly name: "high" | "low" | "wireframe";
  readonly setting: QualitySetting;
  readonly tauPx: number;
  readonly maxPatches: number;
}

/** The lit terrain on each setting, and the wireframe's (R05.T7.c's three tolerances). */
export const COUNT_STYLES: ReadonlyArray<CountStyle> = [
  ...(["high", "low"] as const).map((setting) => ({
    name: setting,
    setting,
    tauPx: SETTINGS[setting].terrain.tauPx,
    maxPatches: Math.floor(terrainSlotLayout(SETTINGS[setting].terrain, "lit").slotCount / 2),
  })),
  {
    name: "wireframe",
    setting: "high",
    tauPx: WIREFRAME_TERRAIN_TAU_PX,
    maxPatches: Math.floor(WIREFRAME_TERRAIN_SLOTS / 2),
  },
];

/** How the boxes are bounded: each its level's subtree range from the table, or flat at ±1 m. */
export type CountBoxes = "unbaked" | "flat";

/** The boxes counted, unbaked first. */
export const COUNT_BOXES: ReadonlyArray<CountBoxes> = ["unbaked", "flat"];

/** Boxes baked flat at ±1 m, so that τ alone sets the depth. */
const FLAT: HeightRangeLookup = { heightRangeM: () => [-1, 1] };

/** The presented view every style is counted on: 1920 × 1080, 60° across (Design note 23). */
export const COUNT_VIEWPORT = { widthPx: 1920, heightPx: 1080 } as const;
export const COUNT_FOV_X_DEG = 60;
export const COUNT_FOV_X_RAD = Math.PI / 3;

/** The most patches the guard selection holds, a pose's count capped where it binds. */
export const GUARD_PATCHES = 16_384;

/** The descent's poses are this far apart, s. */
export const COUNT_POSE_STEP_S = 10;

/** R05's descent over `world` at the record seed. */
export function countProfile(world: SampleWorld): DescentProfile {
  return new DescentProfile(world.figure, landingSiteOf(RECORD_SEED));
}

/** The poses' times, s: every {@link COUNT_POSE_STEP_S} from 0 to the descent's end, 124 of them. */
export function countTimesS(profile: DescentProfile): number[] {
  const times: number[] = [];
  for (let tS = 0; tS <= profile.durationS; tS += COUNT_POSE_STEP_S) {
    times.push(tS);
  }
  return times;
}

/** The style's one view of the descent's pose at `tS`. */
function countView(profile: DescentProfile, tS: number, tauPx: number): ViewSelectionInput {
  const pose = profile.poseAt(tS);
  return {
    camera: { positionM: pose.positionM, orientation: pose.orientation },
    fovXRad: COUNT_FOV_X_RAD,
    viewport: COUNT_VIEWPORT,
    weight: 1,
    tauPx,
  };
}

/** One pose's two selections: the guard's count and the budget's τ′. */
export interface PoseCount {
  readonly tS: number;
  /** The guard selection's patches, at most {@link GUARD_PATCHES}. */
  readonly patches: number;
  /** Whether the guard bound: the pose's count is then a lower bound. */
  readonly capped: boolean;
  /** Whether the style's budget refused a split its τ asked for. */
  readonly limited: boolean;
  /** τ′ = τ × max(1, excess) at the style's budget, px. */
  readonly tauPrimePx: number;
}

/** One world, rule, style and boxes over the descent's poses. */
export interface CountCell {
  readonly world: string;
  readonly rule: SelectionRule;
  readonly style: CountStyle["name"];
  readonly tauPx: number;
  readonly maxPatches: number;
  readonly boxes: CountBoxes;
  readonly poses: ReadonlyArray<PoseCount>;
}

/** The selection's planet of `world` under `rule`. */
function planetOf(world: SampleWorld, rule: SelectionRule): PlanetGeometry {
  return planetGeometry(world.figure, world.table, world.sigmaM, rule);
}

/** What a cell is: its world, rule, style and boxes. */
export interface CountCellInput {
  readonly world: SampleWorld;
  readonly rule: SelectionRule;
  readonly style: CountStyle;
  readonly boxes: CountBoxes;
  /** The poses' times, s; {@link countTimesS} of the world's descent where absent. */
  readonly timesS?: ReadonlyArray<number>;
}

/** Counts one cell: at each pose, the guard's selection and the style's budget's. */
export function countCell(input: CountCellInput): CountCell {
  const { world, rule, style, boxes } = input;
  const planet = planetOf(world, rule);
  const profile = countProfile(world);
  const ranges = boxes === "flat" ? { heightRanges: FLAT } : {};
  const poses = (input.timesS ?? countTimesS(profile)).map((tS): PoseCount => {
    const view = countView(profile, tS, style.tauPx);
    const select = (maxPatches: number): ReturnType<typeof selectPatches> =>
      selectPatches({
        planet,
        views: [view],
        setting: style.setting,
        grounded: [],
        ...ranges,
        maxPatches,
      });
    const guard = select(GUARD_PATCHES);
    const budget = select(style.maxPatches);
    return {
      tS,
      patches: guard.patches.size,
      capped: guard.limited,
      limited: budget.limited,
      tauPrimePx: style.tauPx * Math.max(1, budget.limitExcess),
    };
  });
  return {
    world: world.name,
    rule,
    style: style.name,
    tauPx: style.tauPx,
    maxPatches: style.maxPatches,
    boxes,
    poses,
  };
}

/** A cell's figures over its poses. */
export interface CellFigures {
  /** The guard's patches summed over the poses. */
  readonly sum: number;
  /** The poses the guard capped: where any is, {@link CellFigures.sum} is a lower bound. */
  readonly capped: number;
  /** The poses the style's budget limited. */
  readonly limited: number;
  /** τ′ over the poses, px: its mean and its largest. */
  readonly tauPrimeMeanPx: number;
  readonly tauPrimeMaxPx: number;
  readonly poses: number;
}

/** A cell's figures over its poses. */
export function cellFigures(cell: CountCell): CellFigures {
  let sum = 0;
  let capped = 0;
  let limited = 0;
  let tauPrimeSumPx = 0;
  let tauPrimeMaxPx = 0;
  for (const pose of cell.poses) {
    sum += pose.patches;
    capped += pose.capped ? 1 : 0;
    limited += pose.limited ? 1 : 0;
    tauPrimeSumPx += pose.tauPrimePx;
    tauPrimeMaxPx = Math.max(tauPrimeMaxPx, pose.tauPrimePx);
  }
  const poses = cell.poses.length;
  return {
    sum,
    capped,
    limited,
    tauPrimeMeanPx: poses > 0 ? tauPrimeSumPx / poses : Number.NaN,
    tauPrimeMaxPx,
    poses,
  };
}

/**
 * The saving of min(hard, 4σ) over the hard bound, the ratio of their sums, and what it bounds:
 * `exact` where the guard capped neither, `at least` where it capped only the hard sum, a lower
 * bound, and `none` where it capped the min(hard, 4σ) sum, a ratio of two capped sums.
 */
export interface Saving {
  readonly ratio: number;
  readonly bounds: "exact" | "at least" | "none";
}

/** The saving of `statistical` over `hard` ({@link Saving}). */
export function savingOf(hard: CellFigures, statistical: CellFigures): Saving {
  return {
    ratio: hard.sum / statistical.sum,
    bounds: statistical.capped > 0 ? "none" : hard.capped > 0 ? "at least" : "exact",
  };
}

/** The record's schema name and version; the version bumped with any change to its shape. */
export const LEVEL_COUNTS_SCHEMA = "hyperion.terrain.level-counts";
export const LEVEL_COUNTS_VERSION = 1;

/** A world's T4.a figures, as the record states them beside its cells. */
export interface CountedWorld {
  readonly name: string;
  readonly patches: number;
  readonly pastBodySigma: number;
  readonly pastOwnSigma: number;
}

/** The counts' record: what was counted, how, and every cell. */
export interface LevelCountsRecord {
  readonly schema: typeof LEVEL_COUNTS_SCHEMA;
  readonly version: typeof LEVEL_COUNTS_VERSION;
  readonly startedAt: string;
  /** The fixture's generator version, at which T4.a measured it. */
  readonly generatorVersion: number;
  readonly seed: string;
  readonly poseStepS: number;
  readonly guardPatches: number;
  readonly viewport: typeof COUNT_VIEWPORT;
  readonly fovXDeg: number;
  readonly notes: ReadonlyArray<string>;
  readonly worlds: ReadonlyArray<CountedWorld>;
  readonly cells: ReadonlyArray<CountCell>;
}

/** What {@link levelCountsRecord} is made of: the fixture's version and worlds, and its cells. */
export interface LevelCountsInput {
  readonly startedAt: string;
  readonly generatorVersion: number;
  readonly worlds: ReadonlyArray<SampleWorld>;
  readonly cells: ReadonlyArray<CountCell>;
  readonly notes?: ReadonlyArray<string>;
}

/** The record of `input`'s cells. */
export function levelCountsRecord(input: LevelCountsInput): LevelCountsRecord {
  return {
    schema: LEVEL_COUNTS_SCHEMA,
    version: LEVEL_COUNTS_VERSION,
    startedAt: input.startedAt,
    generatorVersion: input.generatorVersion,
    seed: String(RECORD_SEED),
    poseStepS: COUNT_POSE_STEP_S,
    guardPatches: GUARD_PATCHES,
    viewport: COUNT_VIEWPORT,
    fovXDeg: COUNT_FOV_X_DEG,
    notes: input.notes ?? [],
    worlds: input.worlds.map(({ name, patches, pastBodySigma, pastOwnSigma }) => ({
      name,
      patches,
      pastBodySigma,
      pastOwnSigma,
    })),
    cells: input.cells,
  };
}

/** The cell of `record` for a world, rule, style and boxes, or `undefined` where none was counted. */
function cellOf(
  record: LevelCountsRecord,
  world: string,
  rule: SelectionRule,
  style: CountStyle["name"],
  boxes: CountBoxes,
): CountCell | undefined {
  return record.cells.find(
    (cell) =>
      cell.world === world && cell.rule === rule && cell.style === style && cell.boxes === boxes,
  );
}

/** A whole number with thousands separated by commas, as the record's tables write them. */
function grouped(value: number): string {
  return Math.round(value).toLocaleString("en-US");
}

/** A ratio to three significant figures. */
function figure(value: number): string {
  return Number.isFinite(value) ? String(Number(value.toPrecision(3))) : "—";
}

/** A sum and its capped poses, `≥` before it where the guard bound. */
function sumCell(figures: CellFigures | null): string {
  if (figures === null) {
    return "—";
  }
  return `${figures.capped > 0 ? "≥ " : ""}${grouped(figures.sum)} (${String(figures.capped)})`;
}

/** A saving, `≥` before it where it is a lower bound and `~` where it bounds nothing. */
function savingCell(saving: Saving | null): string {
  if (saving === null) {
    return "—";
  }
  const mark = saving.bounds === "at least" ? "≥ " : saving.bounds === "none" ? "~" : "";
  return `${mark}${figure(saving.ratio)}`;
}

/** τ′'s mean (largest) in px, and the poses limited where `limited`. */
function tauCell(figures: CellFigures | null, limited: boolean): string {
  if (figures === null) {
    return "—";
  }
  const tau = `${figure(figures.tauPrimeMeanPx)} (${figure(figures.tauPrimeMaxPx)}) px`;
  return limited ? `${tau}, ${String(figures.limited)}` : tau;
}

/** One Markdown table row. */
function tableRow(cells: ReadonlyArray<string>): string {
  return `| ${cells.join(" | ")} |`;
}

/**
 * The record's summary in Markdown: R10.T4.b's two tables, the sums and savings, and τ′ at the high
 * setting's budget, then τ′ for every style, each world a row.
 */
export function levelCountsMarkdown(record: LevelCountsRecord): string {
  const figuresOf = (
    world: string,
    rule: SelectionRule,
    style: CountStyle["name"],
    boxes: CountBoxes,
  ): CellFigures | null => {
    const cell = cellOf(record, world, rule, style, boxes);
    return cell === undefined ? null : cellFigures(cell);
  };
  const savingAt = (world: string, style: CountStyle["name"], boxes: CountBoxes): Saving | null => {
    const hard = figuresOf(world, "hard", style, boxes);
    const statistical = figuresOf(world, "statistical", style, boxes);
    return hard === null || statistical === null ? null : savingOf(hard, statistical);
  };
  const styles = COUNT_STYLES.map((style) => style.name);
  const lines = [
    `# Level counts, ${record.startedAt}`,
    "",
    `R10.T4.b's counts (plan R10; signoff-12.md item 30) on fixtures/level-bounds.json at generator ` +
      `version ${String(record.generatorVersion)}: R05's descent at seed ${record.seed}, every ` +
      `${String(record.poseStepS)} s; one ${String(record.viewport.widthPx)} × ` +
      `${String(record.viewport.heightPx)} view of ${String(record.fovXDeg)}° a style (high 1 px, ` +
      `low 2 px, wireframe 4 px); no contact; at each pose a selection held to ` +
      `${grouped(record.guardPatches)} patches (the guard), counted, and one at the style's ` +
      `budget, whose excess gives τ′ = τ × max(1, excess). A sum marked ≥ was capped by the guard ` +
      `(its capped poses in brackets), so a saving marked ≥ is a lower bound and one marked ~ a ` +
      `ratio of two capped sums, which bounds nothing.`,
    ...record.notes.map((note) => `- ${note}`),
    "",
    "## The sums and the savings",
    "",
    tableRow([
      "World",
      "Hard, 1 px, unbaked (capped)",
      "min(hard, 4σ), 1 px, unbaked (capped)",
      "Saving, unbaked, 1 / 2 / 4 px",
      "Saving, flat, 1 / 2 / 4 px",
      "Past 4σ_body",
      "Past 4σ, own cells",
    ]),
    tableRow(Array.from({ length: 7 }, () => "---")),
    ...record.worlds.map((world) =>
      tableRow([
        world.name,
        sumCell(figuresOf(world.name, "hard", "high", "unbaked")),
        sumCell(figuresOf(world.name, "statistical", "high", "unbaked")),
        styles.map((style) => savingCell(savingAt(world.name, style, "unbaked"))).join(" / "),
        styles.map((style) => savingCell(savingAt(world.name, style, "flat"))).join(" / "),
        `${String(world.pastBodySigma)} of ${String(world.patches)} ` +
          `(${figure(world.pastBodySigma / world.patches)})`,
        `${String(world.pastOwnSigma)} of ${String(world.patches)} ` +
          `(${figure(world.pastOwnSigma / world.patches)})`,
      ]),
    ),
    "",
    "## τ′ at the high setting's budget, 1 px: mean (max), with the poses limited",
    "",
    tableRow([
      "World",
      "Hard, unbaked",
      "Hard, flat",
      "min(hard, 4σ), unbaked",
      "min(hard, 4σ), flat",
    ]),
    tableRow(Array.from({ length: 5 }, () => "---")),
    ...record.worlds.map((world) =>
      tableRow([
        world.name,
        tauCell(figuresOf(world.name, "hard", "high", "unbaked"), true),
        tauCell(figuresOf(world.name, "hard", "high", "flat"), false),
        tauCell(figuresOf(world.name, "statistical", "high", "unbaked"), true),
        tauCell(figuresOf(world.name, "statistical", "high", "flat"), false),
      ]),
    ),
    "",
    "## τ′ at every style's budget: mean (max), with the poses limited",
    "",
    tableRow(["World", "Style", "Hard, unbaked", "Hard, flat", "min, unbaked", "min, flat"]),
    tableRow(Array.from({ length: 6 }, () => "---")),
    ...record.worlds.flatMap((world) =>
      COUNT_STYLES.map((style) =>
        tableRow([
          world.name,
          `${style.name} (${String(style.tauPx)} px, ${String(style.maxPatches)})`,
          ...COUNT_RULES.flatMap((rule) =>
            COUNT_BOXES.map((boxes) =>
              tauCell(figuresOf(world.name, rule, style.name, boxes), true),
            ),
          ),
        ]),
      ),
    ),
    "",
  ];
  return lines.join("\n");
}
