/**
 * The descent's demand record (plan R05, T13.a, Design note 19, the patch-demand ruling of
 * 2026-10-03): the fixed-step run over the whole descent, per segment, in four cells, {hard,
 * min(hard, 4σ_n)} × {ridges off, on}, on both settings; and the short windows the unit test pins.
 *
 * @remarks
 * Renderer code reads no files and never loads the surface module (R04.T10.a), so the bakes and
 * the files are the caller's: `scripts/descentDemand.mjs` loads the module, passes a `bake` per
 * ridges value and writes what this returns. The run is cut at a wall-time cap, and says so.
 */

import { type QualitySetting, SETTINGS } from "../quality/qualitySetting";
import { uvToSt, xyzToFaceUv } from "../terrain/cube";
import { type PatchKey, patchKeyString } from "../terrain/patchKey";
import { levelBoundM, type PlanetGeometry, planetGeometry } from "../terrain/planet";
import { type SlotLayout, terrainSlotLayout } from "../terrain/slotLayout";
import { type BoundRule, boundedPlanet, type DemandView } from "./demand";
import { DescentProfile, type DescentTerrain, landingSiteOf } from "./descentProfile";
import { type FixedStepRun, runFixedStep, type SegmentFigures, segmentFigures } from "./fixedStep";
import { TEST_PLANET_FIGURE } from "./testPlanetFigure";

/** The record's schema name. */
export const DEMAND_RECORD_SCHEMA = "hyperion.descent-spike.demand";
/** The record's schema version; bumped with any change to its shape. */
export const DEMAND_RECORD_VERSION = 1;

/** Whether the test planet's ridges are on. */
export type RidgesSetting = "off" | "on";

/** A setting's view, slots and budget for the record (Design notes 21 and 26; the ruling's 4b). */
export interface SettingView {
  readonly setting: QualitySetting;
  readonly view: DemandView & { readonly heightPx: number };
  /** The setting's own slot layout (`terrainSlotLayout`). */
  readonly layout: SlotLayout;
  /** ⌊slots ÷ 2⌋ (the patch-demand ruling, provisional; T18 sets it). */
  readonly maxPatches: number;
}

function viewOf(
  setting: QualitySetting,
  view: DemandView & { readonly heightPx: number },
): SettingView {
  const layout = terrainSlotLayout(SETTINGS[setting].terrain);
  return { setting, view, layout, maxPatches: Math.floor(layout.slotCount / 2) };
}

/** The two settings as the record runs them: high at 1080p and τ = 1 px, low at 720p and τ = 2 px; 60°. */
export const SETTING_VIEWS: ReadonlyArray<SettingView> = [
  viewOf("high", { fovXRad: Math.PI / 3, widthPx: 1920, heightPx: 1080, tauPx: 1 }),
  viewOf("low", { fovXRad: Math.PI / 3, widthPx: 1280, heightPx: 720, tauPx: 2 }),
];

/** The record's seed: the spike's default. */
export const RECORD_SEED = 7n;

/** What a run reads from the surface module for one ridges value. */
export interface SurfaceSource {
  /** `levelTable(ridges)`. */
  readonly levelTable: Float64Array;
  /** `omittedSigmaM(level, ridges)` for levels 0 to 24. */
  readonly omittedSigmaM: ReadonlyArray<number>;
  /** A baked patch's lowest and highest height, metres. */
  readonly rangeOf: (key: PatchKey) => readonly [number, number];
  /**
   * The drawn finest mesh's height at a unit direction, metres above the spheroid: the module's
   * `surfaceHeightM(x, y, z, ridges)`, T4.c's collision interpolant.
   */
  readonly surfaceHeightM: (dir: readonly [number, number, number]) => number;
}

/** One cell of the record. */
export interface DemandCell {
  readonly rule: BoundRule;
  readonly ridges: RidgesSetting;
  readonly setting: QualitySetting;
  /** The sampling rate, Hz, and the script span covered, s. */
  readonly rateHz: number;
  readonly coveredS: readonly [number, number];
  readonly frames: number;
  /** Whether the wall-time cap cut the run short. */
  readonly truncated: boolean;
  /**
   * A note beside the figures: on the ridged planet min(hard, 4σ_n) is statistical, not a bound
   * (`bound.rs`'s finding: p99.9 reaches 1.0–1.23 × 4σ_n and patch maxima 1.87 × 4σ_n at levels
   * 4–8).
   */
  readonly note: string | null;
  /** The terrain the descent was flown over, measured from the bakes (`measureTerrain`). */
  readonly terrain: Required<DescentTerrain>;
  readonly segments: ReadonlyArray<SegmentFigures>;
  readonly hash: string;
}

/** The note the ridged 4σ cells carry (the orchestrator's relay of lane A's finding, 2026-10-03). */
export const RIDGED_CALIBRATED_NOTE =
  "statistical, not a bound on the ridged planet (bound.rs finding)";

/** What runs a cell: the settings, the span and the clock. */
export interface CellOptions {
  readonly rule: BoundRule;
  readonly ridges: RidgesSetting;
  readonly settingView: SettingView;
  readonly source: SurfaceSource;
  readonly rateHz: number;
  readonly fromS: number;
  readonly toS: number;
  readonly measureFromS: number;
  readonly nowMs: () => number;
  /** The wall time, on `nowMs`'s clock, after which the run stops. */
  readonly deadlineMs: number;
}

/** Runs one cell of the record. */
export function runCell(options: CellOptions): DemandCell {
  const { settingView, source } = options;
  const hard = planetGeometry(TEST_PLANET_FIGURE, source.levelTable);
  const planet = boundedPlanet(hard, options.rule, source.omittedSigmaM);
  const terrain = measureTerrain(hard, source);
  const run: FixedStepRun = runFixedStep({
    profile: recordProfile(terrain),
    planet,
    setting: settingView.setting,
    view: settingView.view,
    rateHz: options.rateHz,
    fromS: options.fromS,
    toS: options.toS,
    measureFromS: options.measureFromS,
    maxPatches: settingView.maxPatches,
    layout: settingView.layout,
    rangeOf: source.rangeOf,
    nowMs: options.nowMs,
    deadlineMs: options.deadlineMs,
  });
  const last = run.frames.at(-1)?.tS ?? options.fromS;
  return {
    rule: options.rule,
    ridges: options.ridges,
    setting: settingView.setting,
    rateHz: options.rateHz,
    coveredS: [options.fromS, last],
    frames: run.frames.length,
    truncated: run.truncated,
    note: options.rule === "calibrated" && options.ridges === "on" ? RIDGED_CALIBRATED_NOTE : null,
    terrain,
    segments: segmentFigures(run, options.rateHz),
    hash: run.hash,
  };
}

/** The unit test's windows: per segment, 1 s of warm-up, then 1 s measured, ending 2 s before its end. */
export function testWindows(profile: DescentProfile): ReadonlyArray<{
  readonly segment: string;
  readonly fromS: number;
  readonly measureFromS: number;
  readonly toS: number;
}> {
  return profile.segmentSpans().map(({ name, startS, endS }) => {
    const toS = Math.max(startS + 2, endS - 2);
    return { segment: name, fromS: toS - 2, measureFromS: toS - 1, toS };
  });
}

/** What the unit test's fixture holds: the module's level table and σ_n, and the baked ranges. */
export interface RangesFixture {
  readonly levelTable: Float64Array;
  readonly omittedSigmaM: ReadonlyArray<number>;
  /** The collision interpolant's height at the record's landing site, metres. */
  readonly siteHeightM: number;
  readonly ranges: ReadonlyMap<string, readonly [number, number]>;
}

/**
 * The fixture's text: a `levels`, a `sigma` and a `site` line, the module's numbers in full
 * (JavaScript's shortest round-tripping form), then one `face level i j low high` line a key, in
 * key order.
 */
export function formatRanges(fixture: RangesFixture): string {
  const ranges = [...fixture.ranges]
    .toSorted(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
    .map(([key, [low, high]]) => `${key.replaceAll("/", " ")} ${low} ${high}`);
  return [
    `levels ${[...fixture.levelTable].join(" ")}`,
    `sigma ${fixture.omittedSigmaM.join(" ")}`,
    `site ${fixture.siteHeightM}`,
    ...ranges,
  ]
    .join("\n")
    .concat("\n");
}

/** The fixture read back. */
export function parseRanges(text: string): RangesFixture {
  const ranges = new Map<string, readonly [number, number]>();
  let levelTable = new Float64Array();
  let omittedSigmaM: number[] = [];
  let siteHeightM = NaN;
  for (const line of text.split("\n")) {
    const [head = "", ...rest] = line.trim().split(" ");
    if (head === "levels") {
      levelTable = Float64Array.from(rest.map(Number));
      continue;
    }
    if (head === "sigma") {
      omittedSigmaM = rest.map(Number);
      continue;
    }
    if (head === "site") {
      siteHeightM = Number(rest[0]);
      continue;
    }
    if (rest.length !== 5) {
      continue;
    }
    const [face, level, i, j, low, high] = [head, ...rest].map(Number);
    ranges.set(`${face}/${level}/${i}/${j}`, [low ?? NaN, high ?? NaN]);
  }
  return { levelTable, omittedSigmaM, siteHeightM, ranges };
}

/** A `rangeOf` that reads a fixture and refuses a key it lacks (regenerate the fixture then). */
export function fixtureRangeOf(
  ranges: ReadonlyMap<string, readonly [number, number]>,
): (key: PatchKey) => readonly [number, number] {
  return (key) => {
    const range = ranges.get(patchKeyString(key));
    if (range === undefined) {
      throw new Error(
        `the ranges fixture lacks ${patchKeyString(key)}: regenerate it with \`just descent-demand --write-fixture\``,
      );
    }
    return range;
  };
}

/** A `rangeOf` that remembers what it baked, for the fixture and to bake each key once. */
export function memoised(rangeOf: (key: PatchKey) => readonly [number, number]): {
  readonly rangeOf: (key: PatchKey) => readonly [number, number];
  readonly ranges: ReadonlyMap<string, readonly [number, number]>;
} {
  const ranges = new Map<string, readonly [number, number]>();
  return {
    ranges,
    rangeOf: (key) => {
      const keyString = patchKeyString(key);
      const known = ranges.get(keyString);
      if (known !== undefined) {
        return known;
      }
      const range = rangeOf(key);
      ranges.set(keyString, range);
      return range;
    },
  };
}

/** The record's Markdown summary. */
export function demandSummary(cells: ReadonlyArray<DemandCell>, startedAt: string): string {
  const lines = [
    `# Descent demand record, ${startedAt.slice(0, 10)}`,
    "",
    `Seed ${RECORD_SEED}. Fixed-step runs of the scripted descent through \`selectPatches\` and a`,
    "simulated cache (R05.T13.a): patches selected, measured demand (first-time-selected keys a",
    "second), the per-level prediction D, the share of frames `limited`, and the selection time.",
    "Timings are provisional unless the machine was quiet (Design note 27).",
    "",
  ];
  for (const cell of cells) {
    lines.push(
      `## ${cell.setting}, ${cell.rule === "hard" ? "hard ε_n" : "min(hard, 4σ_n)"}, ridges ${cell.ridges}`,
      "",
      `${cell.frames} frames at ${cell.rateHz} Hz over ${cell.coveredS[0].toFixed(1)}–${cell.coveredS[1].toFixed(1)} s${cell.truncated ? " (truncated at the wall-time cap)" : ""}; hash \`${cell.hash}\`.${cell.note === null ? "" : ` **${cell.note}.**`}`,
      "",
      "| Segment | Patches (mean, max) | Demand /s | D /s | Demand ÷ D | Limited | Select p50 / p95 / max (ms) |",
      "| --- | --- | --- | --- | --- | --- | --- |",
      ...cell.segments.map(
        (s) =>
          `| ${s.segment} | ${s.meanPatches.toFixed(0)}, ${s.maxPatches} | ${s.demandPerS.toFixed(1)} | ${s.predictedPerS.toFixed(1)} | ${s.predictedPerS > 0 ? (s.demandPerS / s.predictedPerS).toFixed(2) : "—"} | ${(100 * s.limitedFraction).toFixed(0)}% | ${s.selectMsP50.toFixed(1)} / ${s.selectMsP95.toFixed(1)} / ${s.selectMsMax.toFixed(1)} |`,
      ),
      "",
    );
  }
  return lines.join("\n");
}

/** The record's descent: seed {@link RECORD_SEED} over the test planet's figure and `terrain`. */
export function recordProfile(terrain: DescentTerrain = {}): DescentProfile {
  return new DescentProfile(TEST_PLANET_FIGURE, landingSiteOf(RECORD_SEED), terrain);
}

/** The level whose patches the track's clearance is taken over: about 620 m on an Earth. */
export const TRACK_LEVEL = 14;

/** The spacing the ground track is sampled at, metres: under half a level-14 patch. */
const TRACK_STEP_M = 250;

/** The unit direction of a position. */
function unit(p: {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}): readonly [number, number, number] {
  const r = Math.hypot(p.x, p.y, p.z);
  return [p.x / r, p.y / r, p.z / r];
}

/** The patch of `level` holding the unit direction `dir`. */
export function patchKeyAt(dir: readonly [number, number, number], level: number): PatchKey {
  const { face, u, v } = xyzToFaceUv(dir);
  const n = 2 ** level;
  const index = (w: number): number => Math.min(n - 1, Math.max(0, Math.floor(uvToSt(w) * n)));
  return { face, level, i: index(u), j: index(v) };
}

/**
 * The record's terrain: the site's height from the module's collision interpolant at the site's
 * direction (T4.c's finest-mesh height, so the hover's metre is above the ground point itself), and
 * the track's maximum as the highest baked height plus ε_14 over the level-14 patches under the low
 * pass's and the slowdown's ground track, sampled every 250 m: a true upper bound on the terrain
 * there (Design note 15), which covers the slowdown too, since it descends from the low pass's
 * height.
 */
export function measureTerrain(
  hard: PlanetGeometry,
  source: Pick<SurfaceSource, "rangeOf" | "surfaceHeightM">,
): Required<DescentTerrain> {
  const { rangeOf } = source;
  const flat = recordProfile();
  const site = flat.poseAt(flat.durationS).groundPointM;
  const siteHeightM = source.surfaceHeightM(unit(site));
  let trackMaxHeightM = -Infinity;
  const seen = new Set<string>();
  for (const span of flat
    .segmentSpans()
    .filter(({ name }) => name === "low fast pass" || name === "slowdown")) {
    let lastM: { readonly x: number; readonly y: number; readonly z: number } | null = null;
    for (let tS = span.startS; tS <= span.endS; tS += 0.05) {
      const ground = flat.poseAt(tS).groundPointM;
      if (
        lastM !== null &&
        Math.hypot(ground.x - lastM.x, ground.y - lastM.y, ground.z - lastM.z) < TRACK_STEP_M
      ) {
        continue;
      }
      lastM = ground;
      const key = patchKeyAt(unit(ground), TRACK_LEVEL);
      const keyString = patchKeyString(key);
      if (seen.has(keyString)) {
        continue;
      }
      seen.add(keyString);
      trackMaxHeightM = Math.max(trackMaxHeightM, rangeOf(key)[1] + levelBoundM(hard, TRACK_LEVEL));
    }
  }
  return { siteHeightM, trackMaxHeightM: Math.max(trackMaxHeightM, siteHeightM) };
}

/** The test planet's geometry under its hard bound, from the module's level table. */
export function hardPlanet(levelTable: Float64Array): PlanetGeometry {
  return planetGeometry(TEST_PLANET_FIGURE, levelTable);
}
