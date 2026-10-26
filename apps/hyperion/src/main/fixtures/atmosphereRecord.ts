/**
 * An atmosphere run's record (plan R08, R08.T10.b) whose captures pass, on the high setting: the
 * main process's tests of its reader, its results and its session start from it and change one
 * thing each.
 */
import type {
  AtmosphereCheckCapture,
  AtmosphereCheckFrames,
  AtmosphereCheckRecord,
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
): AtmosphereCheckCapture {
  return {
    label: `fixture-${designation.toLowerCase().replaceAll(" ", "-")}`,
    designation,
    camera: "FREE · RATE 1.00 Mm/s",
    rangeM,
    phaseDeg,
    exposure: "EV100 12.0 AUTO",
    atMs,
    frames: frames(fromMs, atMs),
    statements: ["PHOTOREALISTIC", `TARGET ${designation}`],
    faults: [],
  };
}

/** The moonlit night from the seat on TEST EARTH's night side. */
export const MOONLIT_NIGHT: AtmosphereCheckCapture = {
  label: "moonlit-night",
  designation: null,
  camera: "SEAT",
  rangeM: null,
  phaseDeg: null,
  exposure: "EV100 -1.0 AUTO",
  atMs: 98_000,
  frames: frames(80_000, 98_000),
  statements: ["PHOTOREALISTIC", "NIGHT SIDE"],
  faults: [],
};

/** The passing record: four air fixtures, then the night. */
export const ATMOSPHERE_RECORD: AtmosphereCheckRecord = {
  timer: "full",
  devicePixelRatio: 1,
  canvas: { widthPx: 1126, heightPx: 906 },
  mediaReadyMs: 4_200,
  captures: [
    fixtureCapture("TEST EARTH", 2.5e7, 60, 5_000, 20_000),
    fixtureCapture("TEST EARTH SLAB", 2.5e7, 60, 20_000, 40_000),
    fixtureCapture("TEST MARS", 1.4e7, 45, 40_000, 60_000),
    fixtureCapture("TEST SATURN", 3.0e8, 30, 60_000, 80_000),
    MOONLIT_NIGHT,
  ],
  faults: [],
};
