import type {
  SpikeSizesCache,
  SpikeSizesColumn,
  SpikeSizesCountColumns,
  SpikeSizesCountPeaks,
  SpikeSizesPass,
  SpikeSizesReport,
  SpikeSizesRule,
  SpikeStyleName,
} from "../../preload/api";

/** The descent's segments as the spike's script names them, in order. */
const SEGMENTS = [
  "orbit coast",
  "deorbit burn",
  "descent arc",
  "atmosphere entry",
  "high glide",
  "braking",
  "low fast pass",
  "slowdown 1",
  "slowdown 2",
  "slowdown 3",
  "approach",
  "vertical descent",
  "hover",
] as const;

/**
 * One pass of the fixture: its cache, and its described views' count, 0 for a pass that only stood
 * in (not sized, its samples left out).
 */
export interface SizesFixturePass {
  readonly cache: SpikeSizesCache;
  readonly views: 0 | 1 | 2;
}

/** What {@link sizesReportOf} makes. */
export interface SizesFixtureOptions {
  /** Samples a pass, one a second of script time. */
  readonly samples: number;
  readonly passes: ReadonlyArray<SizesFixturePass>;
}

/** A Lehmer generator from 1: the same numbers every run. */
function lehmer(): () => number {
  let state = 1;
  return () => {
    state = (state * 48_271) % 2_147_483_647;
    return state / 2_147_483_647;
  };
}

/** A column of `values`, none missing. */
function full(values: ReadonlyArray<number>): SpikeSizesColumn {
  return { samples: [...values], gaps: [] };
}

/** A column of `values` whose first sample is missing for `reason`. */
function firstMissing(values: ReadonlyArray<number>, reason: string): SpikeSizesColumn {
  return {
    samples: values.map((value, i) => (i === 0 ? -1 : value)),
    gaps: values.length === 0 ? [] : [{ from: 0, to: 0, reason }],
  };
}

const MOMENT = { scriptTimeS: 1_104.02, segment: "slowdown 3", heightAboveGroundM: 1_203.4 };

function peaks(views: number, patches: number): SpikeSizesCountPeaks {
  const peak = { patches, at: MOMENT };
  return { views: Array.from({ length: views }, () => peak), union: peak };
}

function rule(slots: number): SpikeSizesRule {
  const figure = { slots, bytes: Math.ceil(slots) * 310_000 };
  return { ruled: figure, sameSample: figure, union: figure, frustum: figure };
}

/** One pass's figures over `samples` samples of the descent, its counts wandering as a run's would. */
function passOf(pass: SizesFixturePass, samples: number, next: () => number): SpikeSizesPass {
  const n = samples;
  const whole = (base: number, spread: number): number => Math.round(base + spread * next());
  const style: SpikeStyleName = pass.cache === "lit" ? "photorealistic" : "wireframe";
  const views = [
    { id: "spike", style, weight: 1 },
    { id: "instrument-1", style, weight: 0.25 },
  ].slice(0, pass.views);
  const sized = pass.views > 0;
  const countColumns = (): SpikeSizesCountColumns => {
    const frustum = Array.from({ length: n }, () => whole(300, 500));
    const allRound = frustum.map((f) => f + whole(800, 2_000));
    return {
      frustum: full(frustum),
      frustumNoMargin: full(frustum.map((f) => f - whole(0, 40))),
      allRound: full(allRound),
      frustumDiagnostic: firstMissing(
        frustum.map((f) => f - whole(0, 30)),
        "no diagnostic depth yet",
      ),
      allRoundDiagnostic: firstMissing(
        allRound.map((ar) => ar - whole(0, 200)),
        "no diagnostic depth yet",
      ),
    };
  };
  const index = Array.from({ length: n }, (_, i) => i);
  // The segments in turn, an equal share of the samples each.
  const segmentAt = (i: number): number =>
    Math.min(SEGMENTS.length - 1, Math.floor((SEGMENTS.length * i) / n));
  const forced = index.map((i) => (i > n - 40 ? whole(40, 20) : 0));
  const priceTerm = {
    allRoundPatches: 140,
    frustumPatches: 11,
    allRoundSlots: 182,
    frustumSlots: 33,
    allRoundShare: 0.025,
    frustumShare: 0.005,
  };
  return {
    cache: pass.cache,
    views,
    notSized: sized
      ? null
      : `stand-in only, not sized: no view of the ${pass.cache} cache's style was asked`,
    bytesPerSlot: 310_000,
    slots: 1_286,
    attachments: 1,
    samples: n,
    failedSamples: 0,
    firstFailure: null,
    counts: {
      frustum: peaks(pass.views, 812),
      frustumNoMargin: peaks(pass.views, 790),
      allRound: peaks(pass.views, 2_790),
      frustumDiagnostic: peaks(pass.views, 801),
      allRoundDiagnostic: peaks(pass.views, 2_650),
    },
    forced: { patches: 61, at: MOMENT, seen: 40 },
    residentShare: { share: 0.873, at: MOMENT },
    sweptShare: { share: 0.991, at: MOMENT },
    allRoundEverResident: false,
    capAtAllRoundPeak: 0.461,
    deepestOccluderM: { value: -4_512.3, reason: null },
    asBuiltOccluderM: -66_604,
    boundRatios: [
      {
        version: 0,
        firstSeenS: 0,
        levels: Array.from({ length: 25 }, (_, level) => ({
          level,
          boundM: 9_000 / 2 ** level,
          chargedM: 9_100 / 2 ** level,
          edgeM: 1e7 / 2 ** level,
          ratio: 1.49,
          chargedRatio: 1.51,
        })),
      },
    ],
    boxHeights: Array.from({ length: 20 }, (_, level) => ({
      level,
      maxM: 4_000 / (level + 1),
      medianAtFrustumPeakM: level < 18 ? 1_000 / (level + 1) : null,
    })),
    countingMs: { p50: 8.2, p95: 14.1, max: 31.7 },
    rule: sized ? rule(3_000) : null,
    ruleDiagnostic: sized ? rule(2_850) : null,
    occluderPrice: sized
      ? {
          views: Array.from({ length: pass.views }, () => priceTerm),
          union: priceTerm,
          sizedAllRoundSlots: pass.views * 1.3 * 2_790,
          ruledSlots: 150,
          ruledShare: 150 / (pass.views * 1.3 * 2_790),
        }
      : null,
    contact: {
      firstForcedS: 1_190,
      allResidentS: 1_192,
      allResidentHeld: true,
      touchdown: { atS: 1_199.9, forced: 61, forcedResident: 61 },
      leadS: 7.9,
      sessionContact: { atS: 1_190, forced: 61, forcedResident: 12 },
    },
    streaming: SEGMENTS.map((segment) => ({
      segment,
      frames: 5_000,
      drawn: { mean: 212.4, max: 250 },
      standingIn: { mean: 160.2, max: 175 },
      missing: { mean: 0.4, max: 3 },
      standInShare: { mean: 0.41, max: 0.7 },
      limitedFrames: 12,
      residents: 900,
      residentsPerS: 9.8,
    })),
    logged: SEGMENTS.map((segment) => ({
      segment,
      samples: Math.round(n / SEGMENTS.length),
      selectedMax: 600,
      limitedSamples: 2,
      selectedResidentMax: 550,
      usedSlotsMax: 1_286,
      pinnedForcedMax: 61,
      pinnedDrawnMax: 480,
    })),
    series: sized
      ? {
          seconds: { first: 0, skipped: [] },
          segmentRuns: index.flatMap((i) => {
            const segment = segmentAt(i);
            return i > 0 && segmentAt(i - 1) === segment ? [] : [{ segment, from: i }];
          }),
          heightAboveGroundM: index.map((i) => Math.round(400_000 * (1 - i / n) ** 2)),
          views: Array.from({ length: pass.views }, countColumns),
          union: pass.views === 1 ? null : countColumns(),
          unionDiffers: 0,
          forced,
          forcedResident: full(forced.map((f) => (f === 0 ? 0 : f - whole(0, 5)))),
          allRoundResident: full(index.map(() => whole(700, 1_500))),
          allRoundSwept: index.map(() => whole(900, 1_800)),
          usedSlots: full(index.map(() => whole(1_000, 286))),
        }
      : null,
  };
}

const BAKE_WINDOW = {
  posted: 12_000,
  baked: 11_800,
  failed: 0,
  unsurveyed: 12,
  postToReplyMs: { p50: 88.1, p95: 240.7, max: 1_201.3 },
  computeMs: { p50: 61.2, p95: 99.4, max: 310.6 },
  residents: 11_500,
  bakedNeverResident: 300,
  spanMs: 1_230_000,
  bakesPerSecond: 9.59,
};

/**
 * A well-formed `sizes` block: `options.passes`' passes of `options.samples` samples each, their
 * figures a run's of the descent, for the check's tests and the file's size.
 */
export function sizesReportOf(
  options: SizesFixtureOptions = { samples: 3, passes: [{ cache: "lit", views: 2 }] },
): SpikeSizesReport {
  const next = lehmer();
  return {
    segments: [...SEGMENTS],
    secondView: {
      id: "instrument-1",
      engineName: "spike-craft",
      style: "wireframe",
      weight: 0.25,
      viewports: [{ widthPx: 416, heightPx: 234, fromS: 0 }],
    },
    sampledLowestM: { value: -4_512.3, reason: null },
    passes: options.passes.map((pass) => passOf(pass, options.samples, next)),
    forcedSets:
      options.passes.length < 2
        ? { value: null, reason: "the lit and wireframe passes never stood in the same frame" }
        : {
            value: {
              comparedFrames: 70_000,
              differingFrames: 3,
              firstDifferingS: 1_220.1,
              largestDifference: 4,
            },
            reason: null,
          },
    segmentFigures: SEGMENTS.map((segment) => ({
      segment,
      bakes: BAKE_WINDOW,
      groundSpeedPerHeight: { mean: 0.41, max: 1.2 },
    })),
    bakes: {
      workersMade: 3,
      total: BAKE_WINDOW,
      byWorker: [0, 1, 2].map((worker) => ({
        worker,
        posted: 4_000,
        baked: 3_933,
        failed: 0,
        unsurveyed: 4,
        postToReplyMs: { p50: 88.1, p95: 240.7, max: 1_201.3 },
        computeMs: { p50: 61.2, p95: 99.4, max: 310.6 },
      })),
      requests: [
        { kind: "bake", count: 12_000 },
        { kind: "ground-readout", count: 4_900 },
      ],
      replies: [
        { kind: "baked", count: 11_800 },
        { kind: "ground-readout", count: 4_890 },
      ],
      inFlight: 2,
      abandoned: 0,
    },
    memory: {
      samples: options.samples,
      workerFieldBytes: { bytes: 3 * 25_165_824, atS: 0 },
      keptFieldBytes: { bytes: 25_165_824, atS: 0 },
      sessionGpu: [{ category: "coarse-field-gpu", bytes: 12_582_912, atS: 0 }],
      caches: options.passes.map((pass) => ({
        cache: pass.cache,
        slotCount: 1_286,
        bytesPerSlot: 310_000,
        allocationBytes: 398_660_000,
      })),
      cascades: { bytes: 67_108_864, source: "computed" },
      computedGpuBytes: options.passes.length * 398_660_000 + 12_582_912 + 67_108_864,
      computedTotalBytes:
        options.passes.length * 398_660_000 + 12_582_912 + 67_108_864 + 3 * 25_165_824,
      workersStarted: 3,
    },
    binding: {
      adapterBytes: { value: 4_294_967_292, reason: null },
      requestedBytes: { value: 1_073_741_824, reason: null },
      deviceBytes: { value: 1_073_741_824, reason: null },
      caches: options.passes.map((pass) => ({
        cache: pass.cache,
        slotCount: 1_286,
        ceilingAtRequestedSlots: 3_464,
        ceilingAtDefaultSlots: 433,
      })),
    },
    readouts: {
      intervalMs: 250,
      publishes: 4_920,
      answeredPublishes: 4_800,
      asked: 4_900,
      answered: 4_890,
      failed: 0,
      failComputationCalls: 0,
    },
  };
}
