import { galacticPositionFromLy, universeTimeFromYears } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import {
  anObservation,
  aRangeRequest,
  aSystemRecord,
  aSystemsInRange,
  UNIVERSE_ID,
} from "../../test/galaxyFixtures";
import { isFormedPlanet, NOW_MODE, observedAgeMyr, type QueryMode, sameQueryMode } from "./model";
import { toChartResult, toQueryModeDto, toRangeRequest } from "./wire";

/** An observer a little coreward of the fixtures' chart centre, off every axis. */
const OBSERVER_LY = [25_990.5, -12.25, 3.75] as const;

const OBSERVED: QueryMode = { kind: "observed", observerLy: OBSERVER_LY };

describe("toRangeRequest's mode (plan 12, P12.T11)", () => {
  it("leaves the mode out in now, so that the request is byte for byte today's", () => {
    const request = toRangeRequest(UNIVERSE_ID, [26_000, 0, 0], 50, 0, "a", NOW_MODE);

    expect(request).not.toHaveProperty("mode");
    expect(request).toEqual(toRangeRequest(UNIVERSE_ID, [26_000, 0, 0], 50, 0, "a"));
    expect(request).toEqual(aRangeRequest());
    expect(JSON.stringify(request)).toBe(JSON.stringify(aRangeRequest()));
  });

  it("carries the observer exactly, as a cell and an offset, in observed mode", () => {
    const request = toRangeRequest(UNIVERSE_ID, [26_000, 0, 0], 50, 0, "a", OBSERVED);

    expect(request.mode).toEqual({
      type: "observed",
      observer: galacticPositionFromLy(OBSERVER_LY),
    });
    expect(request).toEqual(aRangeRequest({ observerLy: OBSERVER_LY }));
  });

  it("writes no mode for now and the observer for observed mode", () => {
    expect(toQueryModeDto(NOW_MODE)).toBeUndefined();
    expect(toQueryModeDto(OBSERVED)).toEqual({
      type: "observed",
      observer: galacticPositionFromLy(OBSERVER_LY),
    });
  });

  it("refuses an observer that is not finite", () => {
    expect(() =>
      toRangeRequest(UNIVERSE_ID, [0, 0, 0], 50, 0, "a", {
        kind: "observed",
        observerLy: [Number.NaN, 0, 0],
      }),
    ).toThrow(RangeError);
  });
});

describe("toChartResult's observation (plan 12, P12.T11)", () => {
  it("reads an observed row into the client's units, apparent position from the centre", () => {
    const result = toChartResult(
      aSystemsInRange({
        timeYr: 12.5,
        systems: [
          {
            relLy: [3, 4, 0],
            layer: "c",
            observed: {
              lightAgeYr: 4_210,
              apparentRelLy: [3.25, 3.5, -0.125],
              boundLy: 2.5e-5,
              boundArcsec: 1.25e-3,
            },
          },
        ],
      }),
      OBSERVED,
    );

    const [system] = result.systems;
    expect(system?.observed?.lightAgeYr).toBe(4_210);
    expect(system?.observed?.emittedYr).toBeCloseTo(12.5 - 4_210, 9);
    expect(system?.observed?.apparentRelLy.x).toBeCloseTo(3.25, 9);
    expect(system?.observed?.apparentRelLy.y).toBeCloseTo(3.5, 9);
    expect(system?.observed?.apparentRelLy.z).toBeCloseTo(-0.125, 9);
    expect(system?.observed?.curvatureBoundLy).toBe(2.5e-5);
    expect(system?.observed?.curvatureBoundArcsec).toBe(1.25e-3);
    // The row's own position stays the present's.
    expect(system?.relLy.x).toBeCloseTo(3, 9);
  });

  it("keeps a hundredth of a light-year in the apparent position 60,000 ly out", () => {
    const record = aSystemRecord({
      positionLy: [60_000.01, 0, 0],
      observed: anObservation({ apparentLy: [60_000.02, 0, 0], lightAgeYr: 1 }),
    });
    const response = aSystemsInRange({ centreLy: [60_000, 0, 0] });
    const result = toChartResult({ ...response, systems: [record] }, OBSERVED);

    expect(result.systems[0]?.observed?.apparentRelLy.x).toBeCloseTo(0.02, 9);
  });

  it("gives a row without an observation none, whatever was asked", () => {
    const response = aSystemsInRange({ systems: [{ relLy: [1, 0, 0], layer: "a" }] });

    expect(toChartResult(response).systems[0]?.observed).toBeNull();
    expect(toChartResult(response, OBSERVED).systems[0]?.observed).toBeNull();
  });

  it("names the mode it was asked in, now when none is passed", () => {
    const response = aSystemsInRange();

    expect(toChartResult(response).mode).toEqual(NOW_MODE);
    expect(toChartResult(response, OBSERVED).mode).toEqual(OBSERVED);
  });

  it("reads the emitted time to the nanosecond the wire carries", () => {
    const record = aSystemRecord({
      positionLy: [26_001, 0, 0],
      observed: {
        ...anObservation({ apparentLy: [26_001, 0, 0], lightAgeYr: 1 }),
        emitted: universeTimeFromYears(-0.5),
      },
    });
    const result = toChartResult({ ...aSystemsInRange(), systems: [record] }, OBSERVED);

    expect(result.systems[0]?.observed?.emittedYr).toBeCloseTo(-0.5, 12);
  });
});

describe("the age as observed", () => {
  it("is the age now less the light's age, and none for a row without an observation", () => {
    const [observed, plain] = toChartResult(
      aSystemsInRange({
        systems: [
          {
            relLy: [1, 0, 0],
            layer: "e",
            ageMyr: 12,
            observed: { lightAgeYr: 20_000, apparentRelLy: [1, 0, 0] },
          },
          { relLy: [2, 0, 0], layer: "e", ageMyr: 12 },
        ],
      }),
      OBSERVED,
    ).systems;

    expect(observed === undefined ? null : observedAgeMyr(observed)).toBeCloseTo(11.98, 12);
    expect(plain === undefined ? undefined : observedAgeMyr(plain)).toBeNull();
  });

  it("counts a free-floating planet formed after its light left as not formed", () => {
    const [planet] = toChartResult(
      aSystemsInRange({
        minLayer: "rogue_planet",
        systems: [
          {
            relLy: [1, 0, 0],
            layer: "rogue_planet",
            ageMyr: 0.001,
            observed: { lightAgeYr: 5_000, apparentRelLy: [1, 0, 0] },
          },
        ],
      }),
      OBSERVED,
    ).systems;

    expect(planet === undefined ? true : isFormedPlanet(planet)).toBe(false);
  });
});

describe("sameQueryMode", () => {
  it("tells now from observed, and one observer from another", () => {
    expect(sameQueryMode(NOW_MODE, { kind: "now" })).toBe(true);
    expect(sameQueryMode(NOW_MODE, OBSERVED)).toBe(false);
    expect(sameQueryMode(OBSERVED, { kind: "observed", observerLy: [...OBSERVER_LY] })).toBe(true);
    expect(sameQueryMode(OBSERVED, { kind: "observed", observerLy: [25_990.5, -12.25, 3.5] })).toBe(
      false,
    );
  });
});
