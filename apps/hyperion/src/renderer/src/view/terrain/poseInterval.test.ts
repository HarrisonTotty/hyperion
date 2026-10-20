/**
 * One contour interval for a pose (plan R10, R10.T12.b; decision-r10-view-block-room §10): the
 * stage's views state the interval through `PoseIntervals`, so that an instrument at the primary's
 * pose states the primary's where its own hysteresis alone would pick the step beside it, while
 * views at other points, or over another body, keep their own.
 */
import { describe, expect, it } from "vitest";

import { type Vec3, vec3 } from "../../geometry/vec3";
import { viewId } from "../camera/state";
import { POSE_TOLERANCE_M, poseInterval, type PosePoint, PoseIntervals } from "./poseInterval";
import { contourInterval } from "./terrainWire";

const PRIMARY = viewId("view");
const INSTRUMENT = viewId("instrument-1");

const EARTH = "fixture-earth";
const MOON = "fixture-moon";

/** The body's radius under the camera, m. */
const GROUND_RADIUS_M = 6_371_000;

/**
 * A camera `aglM` above the ground over `body`, `offsetM` across from the vertical: the point the
 * holder compares, body-fixed.
 */
function pointAt(aglM: number, body = EARTH, offsetM = 0): PosePoint {
  const cameraBodyFixedM: Vec3 = vec3(GROUND_RADIUS_M + aglM, offsetM, 0);
  return { body, cameraBodyFixedM };
}

/** A view's hysteresis at `aglM`, as the frame's terrain gives it to `poseInterval`. */
function stepAt(aglM: number): (previousM: number | null) => number {
  return (previousM) => contourInterval(aglM, previousM);
}

/**
 * The primary descended from 4 km, 200 m, to 2.5 km, inside the band between 100 m and 200 m,
 * holding 200 m by its hysteresis: the holder as its frame at `nowMs` leaves it.
 */
function primaryHolding200(nowMs: number): PoseIntervals {
  const intervals = new PoseIntervals();
  const high = poseInterval(intervals, PRIMARY, pointAt(4000), nowMs - 16, null, stepAt(4000));
  poseInterval(intervals, PRIMARY, pointAt(2500), nowMs, high, stepAt(2500));
  return intervals;
}

describe("one contour interval for a pose (R10.T12.b)", () => {
  it("rests on a band where a descending view holds 200 m and a fresh one picks 100 m", () => {
    expect([contourInterval(2500, 200), contourInterval(2500, null)]).toEqual([200, 100]);
  });

  it("states the primary's 200 m on an instrument opened at its pose in the same frame", () => {
    const intervals = primaryHolding200(16);
    expect(poseInterval(intervals, INSTRUMENT, pointAt(2500), 16, null, stepAt(2500))).toBe(200);
  });

  it("steps an instrument's interval from the primary's where the primary stated it in an earlier frame", () => {
    const intervals = primaryHolding200(16);
    expect(poseInterval(intervals, INSTRUMENT, pointAt(2500), 32, null, stepAt(2500))).toBe(200);
  });

  it("states one interval in every frame two SEAT views draw through the band's crossing, the instrument every second frame", () => {
    const intervals = new PoseIntervals();
    // 60 Hz primary, 30 Hz instrument opened at 2.5 km with no interval of its own.
    const heightsM = [4000, 3000, 2500, 2300, 2200, 2100, 2000, 1900];
    let primaryM: number | null = null;
    let instrumentM: number | null = null;
    const stated: number[][] = [];
    for (const [frame, aglM] of heightsM.entries()) {
      const nowMs = frame * 16;
      primaryM = poseInterval(intervals, PRIMARY, pointAt(aglM), nowMs, primaryM, stepAt(aglM));
      const inFrame = [primaryM];
      if (frame >= 2 && frame % 2 === 0) {
        instrumentM = poseInterval(
          intervals,
          INSTRUMENT,
          pointAt(aglM),
          nowMs,
          instrumentM,
          stepAt(aglM),
        );
        inFrame.push(instrumentM);
      }
      stated.push(inFrame);
    }
    expect(stated).toEqual([[200], [200], [200, 200], [200], [100, 100], [100], [100, 100], [100]]);
  });

  it("keeps each view's own interval where their cameras stand at different heights", () => {
    const intervals = primaryHolding200(16);
    // Ten metres up: another point, whose fresh interval is the step below.
    expect(poseInterval(intervals, INSTRUMENT, pointAt(2510), 16, null, stepAt(2510))).toBe(100);
  });

  it("keeps each view's own interval over another body", () => {
    const intervals = primaryHolding200(16);
    expect(poseInterval(intervals, INSTRUMENT, pointAt(2500, MOON), 16, null, stepAt(2500))).toBe(
      100,
    );
  });

  it("forgets a view's interval, so that a view at its point keeps its own", () => {
    const intervals = primaryHolding200(16);
    intervals.forget(PRIMARY);
    expect(poseInterval(intervals, INSTRUMENT, pointAt(2500), 16, null, stepAt(2500))).toBe(100);
  });

  it("counts a camera 1 m across as at the same point", () => {
    const intervals = primaryHolding200(16);
    expect(
      poseInterval(
        intervals,
        INSTRUMENT,
        pointAt(2500, EARTH, POSE_TOLERANCE_M),
        16,
        null,
        stepAt(2500),
      ),
    ).toBe(200);
  });

  it("counts a camera 1.5 m across as at another point", () => {
    const intervals = primaryHolding200(16);
    expect(
      poseInterval(intervals, INSTRUMENT, pointAt(2500, EARTH, 1.5), 16, null, stepAt(2500)),
    ).toBe(100);
  });

  it("says whether the interval another view holds was stated in the frame asked about", () => {
    const intervals = primaryHolding200(16);
    expect([
      intervals.at(INSTRUMENT, pointAt(2500), 16),
      intervals.at(INSTRUMENT, pointAt(2500), 32),
    ]).toEqual([
      { intervalM: 200, thisFrame: true },
      { intervalM: 200, thisFrame: false },
    ]);
  });

  it("offers a view its own interval to step from, never as one to take as it is", () => {
    const intervals = primaryHolding200(16);
    expect(intervals.at(PRIMARY, pointAt(2500), 16)).toEqual({ intervalM: 200, thisFrame: false });
  });

  it("offers the latest interval held at the point, a view's own newer one before another's older one", () => {
    const intervals = new PoseIntervals();
    intervals.hold(INSTRUMENT, pointAt(2300), 200, 0);
    intervals.hold(PRIMARY, pointAt(2300), 100, 16);
    expect(intervals.at(PRIMARY, pointAt(2300), 32)).toEqual({ intervalM: 100, thisFrame: false });
  });

  it("holds the primary's 100 m by its own hysteresis as the ground under a still camera wavers, the instrument every second frame", () => {
    // The camera stands still; the ground under it rises and falls as its heights refine, so its
    // height reads 2300 m (both views draw), 2250 m (the primary alone), then 2270 m (both). One
    // view's hysteresis steps 200 m down to 100 m at 2250 m and holds 100 m at 2270 m.
    const intervals = new PoseIntervals();
    const still = pointAt(2300);
    const frames = [
      { aglM: 2300, instrument: true },
      { aglM: 2250, instrument: false },
      { aglM: 2270, instrument: true },
    ];
    let primaryM: number | null = 200;
    let instrumentM: number | null = null;
    const primaryStated: number[] = [];
    for (const [frame, { aglM, instrument }] of frames.entries()) {
      const nowMs = frame * 16;
      primaryM = poseInterval(intervals, PRIMARY, still, nowMs, primaryM, stepAt(aglM));
      primaryStated.push(primaryM);
      if (instrument) {
        instrumentM = poseInterval(intervals, INSTRUMENT, still, nowMs, instrumentM, stepAt(aglM));
      }
    }
    expect(primaryStated).toEqual([200, 100, 100]);
  });

  it("states the primary's interval on the instrument in that frame too", () => {
    const intervals = new PoseIntervals();
    const still = pointAt(2300);
    poseInterval(intervals, PRIMARY, still, 0, 200, stepAt(2300));
    poseInterval(intervals, INSTRUMENT, still, 0, null, stepAt(2300));
    poseInterval(intervals, PRIMARY, still, 16, 200, stepAt(2250));
    const primaryM = poseInterval(intervals, PRIMARY, still, 32, 100, stepAt(2270));
    expect(poseInterval(intervals, INSTRUMENT, still, 32, 200, stepAt(2270))).toBe(primaryM);
  });
});
