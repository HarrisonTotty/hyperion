/**
 * An atmosphere run's record (plan R08, R08.T10.b) whose captures pass, on the high setting: the
 * main process's tests of its reader, its results and its session start from it and change one
 * thing each.
 */
import type {
  AtmosphereCheckFrames,
  AtmosphereCheckRecord,
  AtmosphereCheckStage,
  AtmosphereFixtureCapture,
  AtmosphereNightCapture,
} from "../../preload/api";

/** The primary's frames over a capture's window, ending at `toMs`. */
function frames(fromMs: number, toMs: number): AtmosphereCheckFrames {
  return {
    fromMs,
    toMs,
    draws: Math.round((toMs - fromMs) / 16.7),
    intervalMedianMs: 16.7,
    intervalP95Ms: 18.2,
    gpuMedianMs: 4.1,
    gpuP95Ms: 5.3,
  };
}

/** A fixture seen from the free camera at `rangeM` and `phaseDeg`, taken at `atMs`. */
export function fixtureCapture(
  designation: string,
  rangeM: number,
  phaseDeg: number,
  fromMs: number,
  atMs: number,
): AtmosphereFixtureCapture {
  return {
    kind: "fixture",
    label: `fixture-${designation.toLowerCase().replaceAll(" ", "-")}`,
    designation,
    camera: "FREE · RATE 316 km/s",
    rangeM,
    phaseDeg,
    exposure: "EV100 12.0 AUTO",
    exposureSettleMs: 2_000,
    atMs,
    frames: frames(fromMs, atMs),
    statements: ["PHOTOREALISTIC", `TARGET ${designation}`],
    faults: [],
  };
}

/** The moonlit night from the seat on TEST EARTH's night side, the run's first capture. */
export const MOONLIT_NIGHT: AtmosphereNightCapture = {
  kind: "night",
  label: "moonlit-night",
  camera: "SEAT",
  exposure: "EV100 -3.0 AUTO",
  exposureSettleMs: 22_000,
  atMs: 20_000,
  frames: frames(5_000, 20_000),
  statements: ["PHOTOREALISTIC", "NIGHT SIDE"],
  faults: [],
};

/** The climb's stage, and the first fixture's three: the full tour's first legs as flown. */
export const FIRST_STAGES: ReadonlyArray<AtmosphereCheckStage> = [
  {
    target: "TEST MOON",
    stage: 1,
    rateStep: 13,
    askedM: 5e7,
    holdMs: 15_811,
    heldMs: 15_817,
    longestStepMs: 17,
    flownM: 5.002e7,
    shortOfCentreM: 3.3e8,
  },
  {
    target: "TEST EARTH O3 SLAB",
    stage: 1,
    rateStep: 20,
    askedM: 1.995e11,
    holdMs: 19_950,
    heldMs: 19_942,
    longestStepMs: 17,
    flownM: 1.9942e11,
    shortOfCentreM: 1.058e10,
  },
  {
    target: "TEST EARTH O3 SLAB",
    stage: 2,
    rateStep: 18,
    askedM: 1.0055e10,
    holdMs: 10_055,
    heldMs: 10_050,
    longestStepMs: 17,
    flownM: 1.005e10,
    shortOfCentreM: 5.3e8,
  },
  {
    target: "TEST EARTH O3 SLAB",
    stage: 3,
    rateStep: 15,
    askedM: 4.79e8,
    holdMs: 15_148,
    heldMs: 15_150,
    longestStepMs: 17,
    flownM: 4.7905e8,
    shortOfCentreM: 5.095e7,
  },
];

/** The passing record: the night, then the four air fixtures in the tour's order. */
export const ATMOSPHERE_RECORD: AtmosphereCheckRecord = {
  timer: "full",
  devicePixelRatio: 1,
  canvas: { widthPx: 1126, heightPx: 906 },
  mediaReadyMs: 4_200,
  captures: [
    MOONLIT_NIGHT,
    fixtureCapture("TEST EARTH O3 SLAB", 5.1e7, 45, 20_000, 40_000),
    fixtureCapture("TEST MARS", 2.7e7, 60, 40_000, 60_000),
    fixtureCapture("TEST SATURN", 4.8e8, 60, 60_000, 80_000),
    fixtureCapture("TEST EARTH", 5.1e7, 45, 80_000, 98_000),
  ],
  stages: FIRST_STAGES,
  faults: [],
};
