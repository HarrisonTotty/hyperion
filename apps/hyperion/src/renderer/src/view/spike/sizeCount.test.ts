import { describe, expect, it } from "vitest";

import { normalise, type Vec3, vec3 } from "../../geometry/vec3";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { goldenLevelTable, selectionOf, WGS84_FIGURE } from "../../test/terrainFixtures";
import type { Quaternion } from "../camera/pose";
import { lookAlong } from "../camera/quaternion";
import { IDENTITY_ROTATION } from "../coords/rotation";
import { MAX_REQUESTED_BUFFER_BYTES } from "../engine/platform";
import { PatchCache } from "../terrain/cache";
import {
  COUNT_MODEL_SITE,
  frustumRuleSlots,
  REFERENCE_PX_PER_RAD,
  sizingRuleSlots,
  slotBudgetBytes,
} from "../terrain/cacheBudget";
import { PATCH_QUADS, vertexDir, vertexSpacing } from "../terrain/cube";
import { normalsAtlasOf } from "../terrain/gpu/resources";
import { parentKey, type PatchKey, patchKeyString } from "../terrain/patchKey";
import {
  levelBoundM,
  lowestHeightM,
  planetGeometry,
  subtreeHeightRangeM,
  surfacePoint,
} from "../terrain/planet";
import {
  type HeightRangeLookup,
  type PatchRequest,
  type Selection,
  type SelectionInput,
  selectPatches,
  type ViewSelectionInput,
} from "../terrain/select";
import { slotLayout } from "../terrain/slotLayout";
import { type TerrainFrameInput, TerrainPass, type TerrainPool } from "../terrain/terrainPass";
import type { BakedPatch, BakeSettings } from "../terrain/workers/messages";
import { PassSizeCounter, type PassSizeCounterOptions, type SizeCountContext } from "./sizeCount";

/** `select.test.ts`' planet: WGS 84's figure under R05's golden level table, without ridges. */
const PLANET = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
const HD = { widthPx: 1920, heightPx: 1080 } as const;
const FOV_X_RAD = Math.PI / 3;

/** The point `altitudeM` above the count model's site, `select.test.ts`' own, body-fixed metres. */
function above(altitudeM: number): Vec3 {
  const centre = PATCH_QUADS / 2;
  const [x, y, z] = surfacePoint(
    WGS84_FIGURE,
    vertexDir(COUNT_MODEL_SITE, centre, centre),
    altitudeM,
  );
  return vec3(x, y, z);
}

/** A camera at `positionM` looking down, tilted `tiltRad` towards the east. */
function lookingDown(positionM: Vec3, tiltRad = 0): Quaternion {
  const up = normalise(positionM);
  const east = normalise(vec3(-positionM.y, positionM.x, 0));
  const c = Math.cos(tiltRad);
  const s = Math.sin(tiltRad);
  const forward = vec3(-up.x * c + east.x * s, -up.y * c + east.y * s, -up.z * c + east.z * s);
  return lookAlong(forward, up);
}

function view(
  positionM: Vec3,
  orientation: Quaternion,
  overrides: Partial<ViewSelectionInput> = {},
): ViewSelectionInput {
  return {
    camera: { positionM, orientation },
    fovXRad: FOV_X_RAD,
    viewport: HD,
    weight: 1,
    tauPx: 1,
    ...overrides,
  };
}

/** A lookup in which every patch down to `depth` is baked at ±100 m. */
function bakedToDepth(depth: number): HeightRangeLookup {
  return { heightRangeM: (key) => (key.level <= depth ? [-100, 100] : undefined) };
}

/** A grounded body 5 m above the site, with a 20 m radius, under a camera above the site. */
const CONTACT = { positionM: above(5), radiusM: 20 } as const;

/**
 * Two views 1.5 km above the site, one looking straight down at the contact, the other tilted
 * 1.2 rad east, both with frustum margins, over bakes to level 7 and a 300-patch budget.
 */
function descentInput(altitudeM = 1_500, extra: Partial<SelectionInput> = {}): SelectionInput {
  const positionM = above(altitudeM);
  return {
    planet: PLANET,
    views: [
      view(positionM, lookingDown(positionM), { frustumMarginRad: 0.05 }),
      view(positionM, lookingDown(positionM, 1.2), { frustumMarginRad: 0.05, weight: 0.25 }),
    ],
    setting: "high",
    grounded: [CONTACT],
    heightRanges: bakedToDepth(7),
    maxPatches: 300,
    ...extra,
  };
}

/** The script's moment, set by each test. */
interface Script {
  context: SizeCountContext;
}

/** A counter over two photorealistic views, at `script`'s moment, with `options` over the rest. */
function counterOf(script: Script, options: Partial<PassSizeCounterOptions> = {}): PassSizeCounter {
  return new PassSizeCounter({
    cache: "lit",
    views: [
      { id: "main", style: "photorealistic" },
      { id: "instrument", style: "photorealistic" },
    ],
    bytesPerSlot: 310_000,
    context: () => script.context,
    sampledLowestM: () => null,
    nowMs: () => 0,
    ...options,
  });
}

/** The script at `scriptTimeS`, in the descent's final segment, 1.5 km up. */
function at(scriptTimeS: number): SizeCountContext {
  return { scriptTimeS, segment: "final", heightAboveGroundM: 1_500 };
}

/** Each view's frustum count and margin at `counter`'s first sample. */
function frustumOf(counter: PassSizeCounter): Array<ReadonlyArray<number | null | undefined>> {
  return counter.series().views.map((v) => [v.frustum[0], v.marginRad[0]]);
}

/** The pass's selection of `input`, told to `counter` as the pass's observer would be. */
function passSelects(counter: PassSizeCounter, input: SelectionInput): Selection {
  const selection = selectPatches(input);
  counter.onSelect(input, selection);
  return selection;
}

/** `input` with no budget: what the counts select over. */
function unbudgeted(input: SelectionInput): SelectionInput {
  const { maxPatches, ...rest } = input;
  void maxPatches;
  return rest;
}

/** Everything a selection returns but its bounds' objects, in order. */
function digest(selection: Selection): unknown {
  return {
    patches: [...selection.patches.entries()].map(([k, p]) => [k, p.views, p.seen, p.forced]),
    demand: selection.demand.map((r) => [patchKeyString(r.key), r.priority, r.forced]),
    limited: selection.limited,
    limitExcess: selection.limitExcess,
    hiddenBaked: selection.hiddenBaked.map(patchKeyString),
  };
}

/** The non-forced leaves of `selection` with view `bit`'s bit, and the forced leaves. */
function leavesOf(
  selection: Selection,
  bit: number,
): { readonly seen: number; readonly forced: number } {
  const patches = [...selection.patches.values()];
  return {
    seen: patches.filter((p) => !p.forced && (p.views & (1 << bit)) !== 0).length,
    forced: patches.filter((p) => p.forced).length,
  };
}

describe("the five counts (decision-r10-t14b-measure §2)", () => {
  it("counts each view's leaves with the forced ones left out, and the forced ones once", () => {
    const script: Script = { context: at(0) };
    const counter = counterOf(script);
    const input = descentInput();
    passSelects(counter, input);
    const series = counter.series();
    const expected = selectPatches(unbudgeted(input));
    const withForced = [0, 1].map(
      (bit) => [...expected.patches.values()].filter((p) => (p.views & (1 << bit)) !== 0).length,
    );
    expect({
      views: series.views.map((v) => v.frustum[0]),
      forced: series.forced[0],
      forcedSeen: series.forcedSeen[0],
    }).toEqual({
      views: [leavesOf(expected, 0).seen, leavesOf(expected, 1).seen],
      forced: leavesOf(expected, 0).forced,
      forcedSeen: [...expected.patches.values()].filter((p) => p.forced && p.seen).length,
    });
    // The contact is under the first view, so its forced leaves are seen, and left out of its count.
    expect(series.forcedSeen[0]).toBeGreaterThan(0);
    expect(series.views[0]?.frustum[0]).toBeLessThan(withForced[0] ?? 0);
  });

  it("finds every view's frustum within its all-round count, its margin's within the frustum, and fewer with a shallower occluder", () => {
    const script: Script = { context: at(0) };
    // The golden table's lowest is far below 1 km down, so this occluder is shallower.
    const shallowM = -1_000;
    expect(lowestHeightM(PLANET)).toBeLessThan(shallowM);
    const counter = counterOf(script, { sampledLowestM: () => shallowM });
    passSelects(counter, descentInput());
    const series = counter.series();
    const counts = series.views.map((v) => ({
      f: v.frustum[0] ?? 0,
      f0: v.frustumNoMargin[0] ?? 0,
      ar: v.allRound[0] ?? 0,
      fPrime: v.frustumDiagnostic[0] ?? 0,
      arPrime: v.allRoundDiagnostic[0] ?? 0,
    }));
    for (const c of counts) {
      expect(c.ar).toBeGreaterThanOrEqual(c.f);
      expect(c.f0).toBeLessThanOrEqual(c.f);
      expect(c.arPrime).toBeLessThanOrEqual(c.ar);
      expect(c.fPrime).toBeLessThanOrEqual(c.f);
    }
    const union = series.union;
    expect(union.allRound[0] ?? 0).toBeGreaterThan(union.frustum[0] ?? 0);
    expect(union.allRoundDiagnostic[0] ?? 0).toBeLessThan(union.allRound[0] ?? 0);
    expect(series.occluderM).toEqual([shallowM]);
  });

  it("takes the occluder's depth from the resident leaves where they are deeper than the header", () => {
    const script: Script = { context: at(0) };
    const counter = counterOf(script, { sampledLowestM: () => -50 });
    passSelects(counter, descentInput());
    script.context = at(1);
    const deeper = counterOf(script, { sampledLowestM: () => -500 });
    passSelects(deeper, descentInput());
    // Every leaf baked is baked at ±100 m: the header's −50 m is not the deepest, its −500 m is.
    expect([counter.series().occluderM, deeper.series().occluderM]).toEqual([[-100], [-500]]);
    expect(counter.summary().deepestOccluderM).toBe(-100);
  });

  it("makes no diagnostic count before a depth is known", () => {
    const script: Script = { context: at(0) };
    const counter = counterOf(script);
    passSelects(counter, { ...descentInput(), heightRanges: { heightRangeM: () => undefined } });
    const summary = counter.summary();
    expect({
      occluder: counter.series().occluderM,
      primed: counter.series().union.allRoundDiagnostic,
      rule: summary.ruleDiagnostic,
      price: summary.occluderPrice,
    }).toEqual({ occluder: [null], primed: [null], rule: null, price: null });
  });
});

describe("the sampling (decision-r10-t14b-measure §2)", () => {
  /** A cheap pass input: one view from orbit, nothing baked. */
  const ORBIT_INPUT: SelectionInput = {
    planet: PLANET,
    views: [view(above(400_000), lookingDown(above(400_000)))],
    setting: "high",
    grounded: [],
    heightRanges: bakedToDepth(5),
  };

  it("samples at the first selection of each whole second, none in a second with no selection", () => {
    const script: Script = { context: at(0) };
    const counter = counterOf(script);
    for (const scriptTimeS of [0.2, 0.7, 1.1, 3.4, 3.9, 3.95]) {
      script.context = at(scriptTimeS);
      passSelects(counter, ORBIT_INPUT);
    }
    expect(counter.series().scriptTimeS).toEqual([0.2, 1.1, 3.4]);
    expect(counter.summary().samples).toBe(3);
  });

  it("logs each view's margin and the counting's time a sample", () => {
    const script: Script = { context: at(0) };
    let nowMs = 0;
    const counter = counterOf(script, {
      nowMs: () => {
        nowMs += 7;
        return nowMs;
      },
    });
    passSelects(counter, descentInput());
    const series = counter.series();
    expect({
      margins: series.views.map((v) => v.marginRad[0]),
      ms: series.countingMs,
    }).toEqual({ margins: [0.05, 0.05], ms: [7] });
  });

  it("counts a view the input does not hold as null, and records a failed sample without throwing", () => {
    const script: Script = { context: at(0) };
    const counter = counterOf(script, {
      sampledLowestM: () => {
        throw new Error("no header");
      },
    });
    passSelects(counter, ORBIT_INPUT);
    script.context = at(1);
    const fine = counterOf(script);
    passSelects(fine, ORBIT_INPUT);
    expect({
      failed: counter.summary().failedSamples,
      message: counter.summary().firstFailure,
      samples: counter.summary().samples,
      secondView: fine.series().views[1]?.frustum,
    }).toEqual({ failed: 1, message: "no header", samples: 0, secondView: [null] });
  });

  it("counts each view at the input index its placement gives, and a view placed nowhere or twice as null", () => {
    const script: Script = { context: at(0) };
    const input = descentInput();
    const inOrder = counterOf(script);
    passSelects(inOrder, input);
    // The input's views the other way round: the placement finds each where it now is.
    const reversed = { ...input, views: input.views.toReversed() };
    const placed = counterOf(script, { viewIndices: () => [1, 0] });
    passSelects(placed, reversed);
    const twice = counterOf(script, { viewIndices: () => [0, 0] });
    passSelects(twice, input);
    const nowhere = counterOf(script, { viewIndices: () => [null, 7] });
    passSelects(nowhere, input);
    expect({
      placed: frustumOf(placed),
      twice: frustumOf(twice),
      nowhere: frustumOf(nowhere),
      union: nowhere.series().union.frustum,
    }).toEqual({
      placed: frustumOf(inOrder),
      twice: [frustumOf(inOrder)[0], [null, null]],
      nowhere: [
        [null, null],
        [null, null],
      ],
      union: inOrder.series().union.frustum,
    });
  });
});

/** A cache of `slots` slots of 100 B each. */
function cacheOf(slots: number): PatchCache {
  return new PatchCache(
    slotLayout([{ name: "heights", storage: "storage-buffer", bytes: 100 }], slots * 100),
  );
}

/** `key` resident in `cache` over its level's whole subtree range, its bounds unchanged. */
function insert(cache: PatchCache, key: PatchKey): void {
  cache.insert({
    key,
    generation: 1,
    originM: vec3(0, 0, 0),
    heightRangeM: subtreeHeightRangeM(PLANET, key.level),
    boundingRadiusM: 1,
  });
}

describe("the all-round peak's residency (decision-r10-t14b-measure §3)", () => {
  it("reads the resident share from the cache, the swept share from every selection, and the cap", () => {
    // From ten radii out no root is refined, so the all-round union is the roots above the horizon.
    const far = above(9 * WGS84_FIGURE.equatorialRadiusM);
    const cache = cacheOf(12);
    const input: SelectionInput = {
      planet: PLANET,
      views: [view(far, lookingDown(far))],
      setting: "high",
      grounded: [],
      heightRanges: cache,
    };
    const allRoundKeys = (): PatchKey[] =>
      [
        ...selectPatches({
          ...input,
          views: [view(far, lookingDown(far), { allRound: true })],
        }).patches.values(),
      ]
        .filter((p) => !p.forced && p.seen)
        .map((p) => p.key);
    const keys = allRoundKeys();
    const [k0, k1, k2, k3] = keys;
    if (k0 === undefined || k1 === undefined || k2 === undefined || k3 === undefined) {
      throw new Error(`the all-round union holds at least four roots, got ${keys.length}`);
    }
    insert(cache, k0);
    insert(cache, k1);
    expect(allRoundKeys()).toEqual(keys);
    const script: Script = { context: at(0) };
    const counter = counterOf(script, { views: [{ id: "main", style: "photorealistic" }] });
    counter.onSelect(input, selectionOf([k2]));
    script.context = at(0.5);
    counter.onSelect(input, selectionOf([k3]));
    script.context = at(1);
    counter.onSelect(input, selectionOf([]));
    const series = counter.series();
    const n = keys.length;
    expect({
      resident: series.residentShare,
      swept: series.sweptShare,
      cap: series.cap,
      used: series.usedSlots,
    }).toEqual({
      resident: [2 / n, 2 / n],
      swept: [1 / n, 2 / n],
      cap: [12 / n, 12 / n],
      used: [2, 2],
    });
    expect(counter.summary()).toMatchObject({
      slots: 12,
      allRoundEverResident: false,
      residentShare: { share: 2 / n, at: at(0) },
      sweptShare: { share: 2 / n, at: at(1) },
      capAtAllRoundPeak: 12 / n,
    });
  });
});

/** A figure of `slots` slots at the counters' 310,000 B a slot. */
function figure(slots: number): { slots: number; bytes: number } {
  return { slots, bytes: slotBudgetBytes(slots, 310_000) };
}

/** The maximum and the nearest-rank median of some heights, metres. */
function maxAndMedian(heights: ReadonlyArray<number>): { maxM: number; medianM: number } {
  const sorted = heights.toSorted((a, b) => a - b);
  return { maxM: sorted.at(-1) ?? 0, medianM: sorted[Math.ceil(sorted.length / 2) - 1] ?? 0 };
}

describe("the rule's figures (decision-r10-t14b-measure §2, §5)", () => {
  it("sizes by cacheBudget's rules on the peaks, at the counter's bytes a slot", () => {
    const script: Script = { context: at(0) };
    const counter = counterOf(script, { sampledLowestM: () => -1_000 });
    for (const [n, altitudeM] of [4_000, 1_500, 600].entries()) {
      script.context = at(n);
      passSelects(counter, descentInput(altitudeM));
    }
    const summary = counter.summary();
    const peaksOf = (frustum: "frustum" | "frustumDiagnostic") => {
      const allRound = frustum === "frustum" ? "allRound" : "allRoundDiagnostic";
      return summary.views.map((_, k) => ({
        allRound: summary.counts[allRound].views[k]?.patches ?? 0,
        frustum: summary.counts[frustum].views[k]?.patches ?? 0,
      }));
    };
    const forced = summary.forced.patches;
    expect(summary.rule).toMatchObject({
      ruled: figure(sizingRuleSlots(peaksOf("frustum"), forced)),
      union: figure(
        sizingRuleSlots(
          [
            {
              allRound: summary.counts.allRound.union.patches,
              frustum: summary.counts.frustum.union.patches,
            },
          ],
          forced,
        ),
      ),
      frustum: figure(frustumRuleSlots(peaksOf("frustum"), forced)),
    });
    expect(summary.ruleDiagnostic).toMatchObject({
      ruled: figure(sizingRuleSlots(peaksOf("frustumDiagnostic"), forced)),
      frustum: figure(frustumRuleSlots(peaksOf("frustumDiagnostic"), forced)),
    });
    expect(summary.rule.sameSample.slots).toBeLessThanOrEqual(summary.rule.ruled.slots);
    expect(summary.rule.sameSample.slots).toBeGreaterThan(0);
    expect(summary.forced.patches).toBeGreaterThan(0);
    expect(summary.samples).toBe(3);
  });

  it("prices the diagnostic occluder in the rule's slots, 1.3 (AR − AR′) and 3 (F − F′)", () => {
    const script: Script = { context: at(0) };
    const counter = counterOf(script, { sampledLowestM: () => -1_000 });
    passSelects(counter, descentInput());
    const summary = counter.summary();
    const price = summary.occluderPrice;
    if (price === null) {
      throw new Error("a depth was known, so the price was made");
    }
    const { counts } = summary;
    const allRound = counts.allRound.union.patches - counts.allRoundDiagnostic.union.patches;
    const frustum = counts.frustum.union.patches - counts.frustumDiagnostic.union.patches;
    expect(allRound).toBeGreaterThan(0);
    expect(price.union).toMatchObject({ allRoundPatches: allRound, frustumPatches: frustum });
    expect([price.union.allRoundSlots, price.union.frustumSlots]).toEqual([
      1.3 * allRound,
      3 * frustum,
    ]);
    expect(price.ruledSlots).toBeCloseTo(
      summary.rule.ruled.slots - (summary.ruleDiagnostic?.ruled.slots ?? 0),
      9,
    );
  });

  it("gives each share of the sized all-round peak, the ruled sum's Σ 1.3 AR_peak", () => {
    const script: Script = { context: at(0) };
    const counter = counterOf(script, { sampledLowestM: () => -1_000 });
    passSelects(counter, descentInput());
    const summary = counter.summary();
    const price = summary.occluderPrice;
    if (price === null) {
      throw new Error("a depth was known, so the price was made");
    }
    const sized = summary.counts.allRound.views.reduce((sum, peak) => sum + 1.3 * peak.patches, 0);
    expect(price.sizedAllRoundSlots).toBeCloseTo(sized, 9);
    expect([
      price.union.allRoundShare,
      price.union.frustumShare,
      price.views[1]?.allRoundShare,
      price.ruledShare,
    ]).toEqual([
      price.union.allRoundSlots / price.sizedAllRoundSlots,
      price.union.frustumSlots / price.sizedAllRoundSlots,
      (price.views[1]?.allRoundSlots ?? 0) / price.sizedAllRoundSlots,
      price.ruledSlots / price.sizedAllRoundSlots,
    ]);
  });

  it("gives a pass's only view its all-round share as (AR − AR′) ÷ AR_peak", () => {
    const script: Script = { context: at(0) };
    const counter = counterOf(script, {
      views: [{ id: "main", style: "photorealistic" }],
      sampledLowestM: () => -1_000,
    });
    passSelects(counter, descentInput());
    const summary = counter.summary();
    const peak = summary.counts.allRound.views[0]?.patches ?? 0;
    const primed = summary.counts.allRoundDiagnostic.views[0]?.patches ?? 0;
    expect(summary.occluderPrice?.views[0]?.allRoundShare).toBeCloseTo((peak - primed) / peak, 12);
  });

  it("logs k_n = ε_n P₀ ÷ e_n per level table, none for a planet object anew over the same table", () => {
    const script: Script = { context: at(0) };
    const counter = counterOf(script);
    passSelects(counter, descentInput());
    script.context = at(1);
    const again = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
    passSelects(counter, descentInput(1_400, { planet: again }));
    script.context = at(2);
    const other = planetGeometry(WGS84_FIGURE, goldenLevelTable("on"));
    passSelects(counter, { ...descentInput(), planet: other });
    const tables = counter.summary().boundRatios;
    const level7 = tables[0]?.levels[7];
    const edgeM = PATCH_QUADS * vertexSpacing(WGS84_FIGURE.equatorialRadiusM, 7).meanM;
    expect({
      versions: tables.map((t) => [t.version, t.firstSeenS, t.levels.length]),
      ratio: level7?.ratio,
    }).toEqual({
      versions: [
        [0, 0, PLANET.finestLevel + 1],
        [1, 2, other.finestLevel + 1],
      ],
      ratio: (levelBoundM(PLANET, 7) * REFERENCE_PX_PER_RAD) / edgeM,
    });
  });

  it("logs count 1's box heights by level, along each box's normal", () => {
    const script: Script = { context: at(0) };
    const counter = counterOf(script);
    passSelects(counter, descentInput());
    const expected = new Map<number, number[]>();
    for (const p of selectPatches(unbudgeted(descentInput())).patches.values()) {
      if (p.seen) {
        expected.set(p.key.level, [
          ...(expected.get(p.key.level) ?? []),
          2 * p.bounds.box.halfExtentsM[0],
        ]);
      }
    }
    expect(
      counter
        .summary()
        .boxHeights.map((b) => [b.level, { maxM: b.maxM, medianM: b.medianAtFrustumPeakM }]),
    ).toEqual(
      [...expected]
        .toSorted(([a], [b]) => a - b)
        .map(([level, heights]) => [level, maxAndMedian(heights)]),
    );
    expect(expected.size).toBeGreaterThan(1);
  });
});

/** The keys of `b`'s patches whose objects are `a`'s own: what selection's memo kept between them. */
function keptObjects(a: Selection, b: Selection): string[] {
  return [...b.patches].filter(([k, p]) => a.patches.get(k) === p).map(([k]) => k);
}

describe("the counter's own planet (decision-r10-t14b-measure §2)", () => {
  it("leaves the pass's selections, and the objects its memo keeps, as they are without it", () => {
    // Two planet objects alike: the first the counted pass's, the second a pass no counter hears.
    const counted = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
    const alone = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
    const script: Script = { context: at(0) };
    const counter = counterOf(script, { sampledLowestM: () => -1_000 });
    const poses = [1_500, 1_500, 1_200, 1_200];
    const runs = poses.map((altitudeM, n) => {
      script.context = at(n);
      return {
        counted: passSelects(counter, descentInput(altitudeM, { planet: counted })),
        alone: selectPatches(descentInput(altitudeM, { planet: alone })),
      };
    });
    expect(runs.map((run) => digest(run.counted))).toEqual(runs.map((run) => digest(run.alone)));
    const keptBy = (side: "counted" | "alone"): string[][] =>
      runs.slice(1).map((run, n) => keptObjects(runs[n]?.[side] ?? run[side], run[side]));
    expect(keptBy("counted")).toEqual(keptBy("alone"));
    expect(keptBy("alone")[0]?.length).toBeGreaterThan(0);
    expect(counter.summary().samples).toBe(poses.length);
    expect(counter.summary().boundRatios).toHaveLength(1);
  });
});

describe("the forced region's lead (T14.c's finding 3)", () => {
  /** Baked to level 7 and at the finest level, where the forced leaves are. */
  const FORCED_BAKED: HeightRangeLookup = {
    heightRangeM: (key) =>
      key.level <= 7 || key.level === PLANET.finestLevel ? [-100, 100] : undefined,
  };

  /**
   * A counter's samples: none forced at 0 s, forced and none resident at 1 s, then every forced
   * patch resident at 2 s and 3 s, the moment then left at `restS`.
   */
  function flown(restS: number): PassSizeCounter {
    const script: Script = { context: at(0) };
    const counter = counterOf(script);
    passSelects(counter, descentInput(1_500, { grounded: [], heightRanges: FORCED_BAKED }));
    script.context = at(1);
    passSelects(counter, descentInput(1_500, { heightRanges: bakedToDepth(7) }));
    script.context = at(2);
    passSelects(counter, descentInput(1_500, { heightRanges: FORCED_BAKED }));
    script.context = at(3);
    passSelects(counter, descentInput(1_500, { heightRanges: FORCED_BAKED }));
    script.context = at(restS);
    return counter;
  }

  it("logs the selection's forced patches resident at each sample", () => {
    const series = flown(3).series();
    expect(series.forced[1]).toBeGreaterThan(0);
    expect(series.forcedResident).toEqual([0, 0, series.forced[2], series.forced[3]]);
  });

  it("runs from the first sample with every forced patch resident to touchdown", () => {
    const counter = flown(5.5);
    const lead = counter.markTouchdown();
    expect(lead).toMatchObject({
      firstForcedS: 1,
      allResidentS: 2,
      allResidentHeld: true,
      touchdown: { atS: 5.5 },
      leadS: 3.5,
      sessionContact: null,
    });
    expect(lead.touchdown?.forced).toBeGreaterThan(0);
    expect(lead.touchdown?.forcedResident).toBe(lead.touchdown?.forced);
  });

  it("keeps the contact by the session's rule as a second time, which bounds no lead", () => {
    const counter = flown(1.2);
    const contact = counter.markContact();
    expect(contact).toMatchObject({ sessionContact: { atS: 1.2 }, touchdown: null, leadS: null });
  });

  it("marks touchdown and the session's contact once each", () => {
    const script: Script = { context: at(0) };
    const counter = counterOf(script);
    passSelects(counter, descentInput(1_500, { heightRanges: FORCED_BAKED }));
    script.context = at(2);
    counter.markTouchdown();
    counter.markContact();
    script.context = at(3);
    const again = { touchdown: counter.markTouchdown(), contact: counter.markContact() };
    expect(again.touchdown).toEqual(again.contact);
    expect([again.touchdown.touchdown?.atS, again.contact.sessionContact?.atS]).toEqual([2, 2]);
  });

  it("states that the forced patches did not stay resident where a later sample lost one", () => {
    const script: Script = { context: at(0) };
    const counter = counterOf(script);
    passSelects(counter, descentInput(1_500, { heightRanges: FORCED_BAKED }));
    script.context = at(1);
    passSelects(counter, descentInput(1_500, { heightRanges: bakedToDepth(7) }));
    expect(counter.summary().contact).toMatchObject({ allResidentS: 0, allResidentHeld: false });
  });
});

describe("the diagnostic occluder's depth from the resident bakes (decision-r10-t14b-measure §5)", () => {
  it("reads every resident bake over the counts' leaves, their resident ancestors too", () => {
    const cache = cacheOf(64);
    const input = descentInput(1_500, { grounded: [], heightRanges: cache });
    const leaf = [...selectPatches(unbudgeted(input)).patches.values()].find((p) => p.seen)?.key;
    let ancestor = leaf ?? null;
    while (ancestor !== null && ancestor.level > 2) {
      ancestor = parentKey(ancestor);
    }
    if (ancestor === null) {
      throw new Error("the descent's counts see a leaf below level 2");
    }
    // Resident and deep, while no leaf of the counts is resident.
    cache.insert({
      key: ancestor,
      generation: 1,
      originM: vec3(0, 0, 0),
      heightRangeM: [-7_000, 50],
      boundingRadiusM: 1,
    });
    const script: Script = { context: at(0) };
    const counter = counterOf(script);
    passSelects(counter, input);
    expect(counter.series().occluderM).toEqual([-7_000]);
  });
});

/** `selection`'s patches resident in {@link descentInput}'s bakes, to level 7. */
function residentToLevel7(selection: Selection): number {
  return [...selection.patches.values()].filter((p) => p.key.level <= 7).length;
}

describe('the pass\'s own counts by segment (decision-r10-t14b-measure §2, "Logged beside")', () => {
  it("keeps each at its largest over the segment's samples, in the order the segments came", () => {
    const script: Script = { context: at(0) };
    const counter = counterOf(script);
    const selections = [4_000, 1_500, 600].map((altitudeM, n) => {
      script.context = { ...at(n), segment: n < 2 ? "approach" : "hover" };
      return passSelects(counter, descentInput(altitudeM));
    });
    const [high, middle, low] = selections;
    if (high === undefined || middle === undefined || low === undefined) {
      throw new Error("three selections were made");
    }
    expect(counter.summary().logged).toEqual([
      {
        segment: "approach",
        samples: 2,
        selectedMax: Math.max(high.patches.size, middle.patches.size),
        limitedSamples: [high, middle].filter((s) => s.limited).length,
        selectedResidentMax: Math.max(residentToLevel7(high), residentToLevel7(middle)),
        usedSlotsMax: null,
        pinnedForcedMax: null,
        pinnedDrawnMax: null,
      },
      {
        segment: "hover",
        samples: 1,
        selectedMax: low.patches.size,
        limitedSamples: low.limited ? 1 : 0,
        selectedResidentMax: residentToLevel7(low),
        usedSlotsMax: null,
        pinnedForcedMax: null,
        pinnedDrawnMax: null,
      },
    ]);
  });
});

/** A pool that records the demand and bakes it flat on the test's command (`terrainPassObserve.test.ts`'). */
class FlatPool implements TerrainPool {
  demand: ReadonlyArray<PatchRequest> = [];
  readonly #listeners = new Set<(bake: BakedPatch) => void>();

  constructor(readonly settings: BakeSettings) {}

  reprioritise(demand: ReadonlyArray<PatchRequest>): void {
    this.demand = demand;
  }

  onBaked(cb: (bake: BakedPatch) => void): () => void {
    this.#listeners.add(cb);
    return () => {
      this.#listeners.delete(cb);
    };
  }

  terminate(): void {}

  /** Bakes every patch demanded, flat, for `pass`. */
  bakeDemand(pass: TerrainPass, planetRadiusM: number): void {
    const n = normalsAtlasOf(pass.layout).samplesPerSide;
    for (const { key } of this.demand) {
      const bake: BakedPatch = {
        key,
        generation: 1,
        originM: { x: 1_000, y: 2_000, z: planetRadiusM },
        heights: new Float32Array(65 * 65 * 2),
        offsets:
          this.settings.vertexPath === "baked-offsets" ? new Float32Array(65 * 65 * 6) : null,
        normals: new Float16Array(n * n * 2),
        heightRangeM: [0, 0],
        boundingRadiusM: 1,
        originHeightM: 0,
        skirtDepthM: 1,
        layers: null,
      };
      for (const listener of this.#listeners) {
        listener(bake);
      }
    }
  }
}

/** A frame 1,000 km above the north pole at `nowMs`, tilted `tiltRad` from looking straight down. */
function aboveThePole(nowMs: number, tiltRad = 0): TerrainFrameInput {
  return {
    view: {
      rotation: IDENTITY_ROTATION,
      cameraM: { x: 0, y: 0, z: PLANET.figure.polarRadiusM + 1_000_000 },
      orientation: lookAlong(
        { x: Math.sin(tiltRad), y: 0, z: -Math.cos(tiltRad) },
        { x: Math.cos(tiltRad), y: 0, z: Math.sin(tiltRad) },
      ),
      fovXRad: Math.PI / 3,
      viewport: { widthPx: 640, heightPx: 360 },
    },
    grounded: [],
    sunDirectionBodyFixed: { x: 0, y: 0, z: 1 },
    sunIlluminanceLx: [1.3e5, 1.3e5, 1.3e5],
    exposureScale: 1e-5,
    nowMs,
  };
}

/** The test planet's pass on the low setting, and its pool. */
async function lowPass(): Promise<{ readonly pass: TerrainPass; readonly pool: () => FlatPool }> {
  const engine = await countingRenderEngine({
    maxStorageBufferBindingSize: MAX_REQUESTED_BUFFER_BYTES,
    maxBufferSize: MAX_REQUESTED_BUFFER_BYTES,
  });
  let made: FlatPool | null = null;
  const pass = new TerrainPass({
    engine,
    setting: "low",
    planet: planetGeometry(WGS84_FIGURE, goldenLevelTable("off")),
    source: { kind: "test-planet", ridges: "off" },
    frustumMargin: "motion",
    createPool: (bake) => {
      made = new FlatPool(bake);
      return made;
    },
  });
  await pass.ready();
  return {
    pass,
    pool: () => {
      if (made === null) {
        throw new Error("no pool was made");
      }
      return made;
    },
  };
}

describe("a counted terrain pass (decision-r10-t14b-measure, for the lane, item 2)", () => {
  it("selects as an uncounted pass does, frame for frame", async () => {
    const counted = await lowPass();
    const alone = await lowPass();
    const script: Script = { context: at(0) };
    const counter = counterOf(script, {
      views: [{ id: "main", style: "photorealistic" }],
      sampledLowestM: () => -1_000,
    });
    counter.attach(counted.pass);
    const frames = [
      aboveThePole(0),
      aboveThePole(1_100),
      aboveThePole(2_200, 0.3),
      aboveThePole(3_300, 0.35),
    ];
    for (const frame of frames) {
      script.context = at(frame.nowMs / 1_000);
      const withCounter = counted.pass.frame(frame);
      const without = alone.pass.frame(frame);
      expect(digest(withCounter.selection)).toEqual(digest(without.selection));
      counted.pool().bakeDemand(counted.pass, PLANET.figure.polarRadiusM);
      alone.pool().bakeDemand(alone.pass, PLANET.figure.polarRadiusM);
    }
    const summary = counter.summary();
    expect(summary.samples).toBeGreaterThan(1);
    expect(summary.slots).toBe(counted.pass.layout.slots.slotCount);
    expect(summary.failedSamples).toBe(0);
  });
});
