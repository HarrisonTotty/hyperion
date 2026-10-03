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
import { PATCH_QUADS, uvToSt, vertexSpacing, type Xyz, xyzToFaceUv } from "../terrain/cube";
import {
  cornerNeighbours,
  EDGES,
  edgeNeighbour,
  type PatchKey,
  patchKeyString,
} from "../terrain/patchKey";
import { levelBoundM, type PlanetGeometry, planetGeometry } from "../terrain/planet";
import { type SlotLayout, terrainSlotLayout } from "../terrain/slotLayout";
import { type BoundRule, boundedPlanet, type DemandView } from "./demand";
import {
  DESCENT_SEGMENTS,
  DescentProfile,
  type DescentTerrain,
  landingSiteOf,
  type TrackStretch,
  trackStretches,
} from "./descentProfile";
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
   * The drawn finest mesh's height at the unit direction d of the spheroid point M·d (not the
   * geocentric direction; Design note 5), metres above the spheroid: the module's
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
  /** Each segment's boundary altitudes above the table's and its duration as flown (`segmentLifts`). */
  readonly lifts: ReadonlyArray<SegmentLift>;
  /** The least height above a stretch's floor less its clearance, metres (`minFloorMarginM`). */
  readonly minFloorMarginM: number;
  readonly segments: ReadonlyArray<SegmentFigures>;
  readonly hash: string;
}

/** How far the clearance ruling lifted one segment above Design note 19's table. */
export interface SegmentLift {
  readonly segment: string;
  /** Its start and end altitude above the table's, metres. */
  readonly startLiftM: number;
  readonly endLiftM: number;
  /** Its duration as flown, s (the vertical descent's grows when its top rises). */
  readonly durationS: number;
}

/** Each segment's lift above the table (decision-r05-descent-clearance.md's record). */
export function segmentLifts(profile: DescentProfile): SegmentLift[] {
  return profile.segments.map((segment) => {
    const table = DESCENT_SEGMENTS.find(({ name }) => name === segment.name);
    return {
      segment: segment.name,
      startLiftM: segment.startAltitudeM - (table?.startAltitudeM ?? NaN),
      endLiftM: segment.endAltitudeM - (table?.endAltitudeM ?? NaN),
      durationS: segment.durationS,
    };
  });
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
  const profile = recordProfile(terrain);
  const run: FixedStepRun = runFixedStep({
    profile,
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
    lifts: segmentLifts(profile),
    minFloorMarginM: profile.minFloorMarginM,
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

/** The segments the ruling lifted, as text, or "none". */
function liftsText(lifts: ReadonlyArray<SegmentLift>): string {
  const lifted = lifts.filter(({ startLiftM, endLiftM }) => startLiftM !== 0 || endLiftM !== 0);
  return lifted.length === 0
    ? "none"
    : lifted
        .map(
          (l) =>
            `${l.segment} +${l.startLiftM.toFixed(1)} → +${l.endLiftM.toFixed(1)} m (${l.durationS.toFixed(1)} s)`,
        )
        .join(", ");
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
      `Site ${cell.terrain.siteHeightM.toFixed(1)} m; least margin above the stretches' floors and clearances ${cell.minFloorMarginM.toFixed(1)} m; lifts above the table: ${liftsText(cell.lifts)}.`,
      "",
      "| Segment | Patches (mean, max) | Demand /s | D /s | Demand ÷ D | Limited | Height above floor (m) | Select p50 / p95 / max (ms) |",
      "| --- | --- | --- | --- | --- | --- | --- | --- |",
      ...cell.segments.map(
        (s) =>
          `| ${s.segment} | ${s.meanPatches.toFixed(0)}, ${s.maxPatches} | ${s.demandPerS.toFixed(1)} | ${s.predictedPerS.toFixed(1)} | ${s.predictedPerS > 0 ? (s.demandPerS / s.predictedPerS).toFixed(2) : "—"} | ${(100 * s.limitedFraction).toFixed(0)}% | ${s.minHeightAboveFloorM.toFixed(0)}–${s.maxHeightAboveFloorM.toFixed(0)} | ${s.selectMsP50.toFixed(1)} / ${s.selectMsP95.toFixed(1)} / ${s.selectMsMax.toFixed(1)} |`,
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

/** A unit direction as the module's and the cube's triples take it. */
function triple(d: { readonly x: number; readonly y: number; readonly z: number }): Xyz {
  return [d.x, d.y, d.z];
}

/** The patch of `level` holding the unit direction `dir` (d of the spheroid point M·d). */
export function patchKeyAt(dir: Xyz, level: number): PatchKey {
  const { face, u, v } = xyzToFaceUv(dir);
  const n = 2 ** level;
  const index = (w: number): number => Math.min(n - 1, Math.max(0, Math.floor(uvToSt(w) * n)));
  return { face, level, i: index(u), j: index(v) };
}

/**
 * The patches of a stretch's bound level under its ground track, with their eight neighbours
 * (seven at a cube corner), as decision-r05-descent-clearance.md has lane C's `trackPatchKeys`
 * take them: the track sampled at most half the level's shortest patch edge apart along the ground,
 * from the stretch's fastest point (with 1% for the arc's radius against the ground's), both ends
 * included, so no patch it crosses is missed and the neighbours add one edge to either side.
 */
export function stretchKeys(profile: DescentProfile, stretch: TrackStretch): PatchKey[] {
  const { level } = stretch;
  const edgeM = PATCH_QUADS * vertexSpacing(profile.figure.polarRadiusM, level).minM;
  const fastestMps = Math.max(
    profile.poseAt(stretch.startS).horizontalSpeedMps,
    profile.poseAt(stretch.endS).horizontalSpeedMps,
    1,
  );
  const stepS = edgeM / 2 / (1.01 * fastestMps);
  const steps = Math.ceil((stretch.endS - stretch.startS) / stepS);
  const keys = new Map<string, PatchKey>();
  const take = (key: PatchKey | null): void => {
    if (key !== null) {
      keys.set(patchKeyString(key), key);
    }
  };
  for (let n = 0; n <= steps; n += 1) {
    const tS = Math.min(stretch.startS + n * stepS, stretch.endS);
    const key = patchKeyAt(triple(profile.groundDirAt(tS)), level);
    take(key);
    for (const edge of EDGES) {
      take(edgeNeighbour(key, edge));
    }
    for (const corner of cornerNeighbours(key)) {
      take(corner);
    }
  }
  return [...keys.values()];
}

/**
 * The landing site's height, metres above the spheroid: the collision interpolant along the site's
 * direction d of the spheroid point M·d (Design note 5), which the profile carries. The geocentric
 * direction of the same point lands 0.036° (4 km) away at seed 7's site, on ground 107 m lower.
 */
export function siteHeightOf(
  profile: DescentProfile,
  source: Pick<SurfaceSource, "surfaceHeightM">,
): number {
  return source.surfaceHeightM(triple(profile.siteDir));
}

/**
 * The record's terrain (decision-r05-descent-clearance.md): the site's height from the module's
 * collision interpolant along the site's direction d (T4.c's finest-mesh height, bit-exact, so the
 * hover's metre is above the ground point itself), and each stretch's floor, the highest baked
 * height plus ε_n over {@link stretchKeys} at its level: a true upper bound on the finest mesh
 * there (Design note 15), since the mesh interpolates its vertices. `trackMaxHeightM` is the low
 * pass's floor, for the readout.
 */
export function measureTerrain(
  hard: PlanetGeometry,
  source: Pick<SurfaceSource, "rangeOf" | "surfaceHeightM">,
): Required<DescentTerrain> {
  const flat = recordProfile();
  const siteHeightM = siteHeightOf(flat, source);
  const stretches = trackStretches(flat);
  const stretchMaxHeightsM = stretches.map(
    (stretch) =>
      stretchKeys(flat, stretch).reduce(
        (highest, key) => Math.max(highest, source.rangeOf(key)[1]),
        -Infinity,
      ) + levelBoundM(hard, stretch.level),
  );
  const lowPass = stretches.findIndex((s) => s.segment === "low fast pass");
  return {
    siteHeightM,
    trackMaxHeightM: stretchMaxHeightsM[lowPass] ?? siteHeightM,
    stretchMaxHeightsM,
  };
}

/** The test planet's geometry under its hard bound, from the module's level table. */
export function hardPlanet(levelTable: Float64Array): PlanetGeometry {
  return planetGeometry(TEST_PLANET_FIGURE, levelTable);
}
