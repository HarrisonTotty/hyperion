/**
 * A terrain run's record (plan R10, R10.T16.d) that passes every check, on the high setting: the
 * main process's tests of its reader, its results and its session start from it and change one
 * thing each.
 */
import type {
  TerrainCheckPhaseRecord,
  TerrainCheckRecord,
  TerrainCheckViewRecord,
} from "../../preload/api";

/** A photorealistic view that handed `TEST MOON` over at 1.2 s of its phase and drew the terrain 0.1 s later. */
function photoreal(
  name: TerrainCheckViewRecord["name"],
  phaseStartMs: number,
): TerrainCheckViewRecord {
  return {
    name,
    style: "photorealistic",
    canvas: { widthPx: 1126, heightPx: 906 },
    draws: 120,
    gpuMs: [2.1, 2.4],
    submitMs: [0.8, 0.9],
    missedFrames: 0,
    longestGapMs: 34,
    handedOverDraws: 60,
    offSurveyDraws: 12,
    terrainDraws: 60,
    terrainGaps: 0,
    terrainGapSpansS: [],
    firstHandoverMs: phaseStartMs + 1_200,
    firstHandoverScriptS: 38.5,
    firstTerrainMs: phaseStartMs + 1_300,
    readBacks: 4,
    nonFiniteTexels: 0,
  };
}

/**
 * A wireframe view, which hands `TEST MOON` over to its wireframe pass at 1.5 s of its phase and
 * draws the wireframe's terrain 0.2 s later (plan R10, R10.T11), and reads nothing back.
 */
function wireframe(
  name: TerrainCheckViewRecord["name"],
  phaseStartMs: number,
): TerrainCheckViewRecord {
  return {
    name,
    style: "wireframe",
    canvas: { widthPx: 555, heightPx: 254 },
    draws: 60,
    gpuMs: [0.4],
    submitMs: [0.3],
    missedFrames: 0,
    longestGapMs: 40,
    handedOverDraws: 30,
    offSurveyDraws: 6,
    terrainDraws: 30,
    terrainGaps: 0,
    terrainGapSpansS: [],
    firstHandoverMs: phaseStartMs + 1_500,
    firstHandoverScriptS: 270.2,
    firstTerrainMs: phaseStartMs + 1_700,
    readBacks: 0,
    nonFiniteTexels: 0,
  };
}

/** A phase from `startMs` to `endMs` of the views given. */
function phase(
  name: TerrainCheckPhaseRecord["name"],
  startMs: number,
  endMs: number,
  views: ReadonlyArray<TerrainCheckViewRecord>,
): TerrainCheckPhaseRecord {
  return {
    name,
    startMs,
    endMs,
    scriptStartS: 2,
    scriptEndS: 265,
    views,
    frameIntervalsMs: [16.7, 16.6],
    mainThreadMs: [4, 5],
    frameGpuMs: [2.5, 2.6],
    patchesDrawn: [40, 52, 61],
    bakes: { posted: 300, baked: 296, refused: 1 },
    fieldPosts: { inits: 3, chunks: 21, releases: 0 },
    fieldAsks: 2,
    blocksSent: 8,
    memory: {
      workerFieldBytes: 3_000_000,
      keptFieldBytes: 28_000,
      sessionGpuBytes: [{ category: "coarse-field-gpu", bytes: 24_576 }],
      liveBytes: [
        { category: "coarse-field-gpu", bytes: 30_000 },
        { category: "height-cache", bytes: 20_000_000 },
      ],
      peakBytes: [
        { category: "coarse-field-gpu", bytes: 30_000 },
        { category: "height-cache", bytes: 20_000_000 },
      ],
    },
  };
}

/** The loss phase's window, ms. */
export const LOSS_WINDOW = { startMs: 600_000, endMs: 760_000 } as const;

/** When the harness destroyed the device in {@link TERRAIN_RECORD}, ms. */
export const DESTROYED_AT_MS = 725_000;

/** A high-setting record that passes every check. */
export const TERRAIN_RECORD: TerrainCheckRecord = {
  timer: "full",
  devicePixelRatio: 1,
  phases: [
    phase("terrain-alone", 3_000, 270_000, [photoreal("view", 3_000)]),
    phase("terrain-photoreal-wireframe", 280_000, 580_000, [
      photoreal("view", 280_000),
      photoreal("instrument-1", 280_000),
      wireframe("instrument-2", 280_000),
    ]),
    phase("terrain-loss", LOSS_WINDOW.startMs, LOSS_WINDOW.endMs, [
      photoreal("view", LOSS_WINDOW.startMs),
    ]),
  ],
  faults: [
    {
      atMs: DESTROYED_AT_MS + 5,
      kind: "device-lost",
      reason: "destroyed",
      line: "device-lost (destroyed): Device was destroyed.",
    },
  ],
  restoresAtMs: [DESTROYED_AT_MS + 400],
  loss: {
    destroyedAtMs: DESTROYED_AT_MS,
    destroyedAtScriptS: 121.4,
    notDestroyed: null,
    blocksToCome: 2,
    bakesInFlight: 3,
    after: { fieldAsks: 0, fieldInits: 0, fieldChunks: 6, blocksSent: 2 },
    workers: 3,
    terrainAgainAtMs: DESTROYED_AT_MS + 900,
    masks: {
      before: 30,
      afterRestore: 30,
      end: 48,
      expected: 48,
      beforeKept: true,
      endAsExpected: true,
    },
  },
  teardown: {
    disposed: true,
    workersMade: 6,
    workersLive: 0,
    sessionGpuBytes: 0,
    liveBytes: [
      { category: "coarse-field-gpu", bytes: 0 },
      { category: "height-cache", bytes: 0 },
    ],
  },
  handover: { rangeM: 2.08e7, scriptS: 38.2 },
};
