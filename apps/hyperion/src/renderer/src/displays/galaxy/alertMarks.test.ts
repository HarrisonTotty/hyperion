import type { BearingDto, ContactDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { localFrameAt } from "../../geometry/frame";
import { dot, norm, sub, vec3, type Vec3 } from "../../geometry/vec3";
import type { CentreLy, QueryMode } from "../../lib/galaxy/model";
import { toChartResult } from "../../lib/galaxy/wire";
import { OBSERVER_MARK_ID } from "../../spatial/drawList";
import type { SpatialScene } from "../../spatial/marks";
import { aBearingContact, aResolvedContact } from "../../test/alertFixtures";
import { aSystemsInRange } from "../../test/galaxyFixtures";
import {
  alertLegendOf,
  alertMarks,
  bearingRay,
  bearingRayId,
  RAY_REACH_LY,
  TRANSIENT_LABEL,
  TRANSIENT_SIZE_CLASS,
  transientHost,
  transientMarkId,
  withAlertMarks,
} from "./alertMarks";

/** The sim's test point for its bearings, where the named directions are turned 30° from the axes. */
const FROM: CentreLy = [13_000, 22_516.66, 150];

function local(azimuthDeg: number, elevationDeg: number): BearingDto {
  return { azimuth_deg: azimuthDeg, elevation_deg: elevationDeg, frame: "local" };
}

function expectVec(actual: Vec3, expected: Vec3): void {
  expect(actual.x).toBeCloseTo(expected.x, 12);
  expect(actual.y).toBeCloseTo(expected.y, 12);
  expect(actual.z).toBeCloseTo(expected.z, 12);
}

/** The vector the other way. */
function negate(v: Vec3): Vec3 {
  return vec3(-v.x, -v.y, -v.z);
}

describe("bearingRay (plan 12, P12.T12)", () => {
  const frame = localFrameAt(vec3(...FROM));

  it.each([
    ["coreward", 0, 0, frame.coreward],
    ["spinward", 90, 0, frame.spinward],
    ["rimward", 180, 0, negate(frame.coreward)],
    ["antispinward", 270, 0, negate(frame.spinward)],
    ["north", 0, 90, frame.north],
    ["south", 0, -90, negate(frame.north)],
  ] as const)("points %s at azimuth %f° and elevation %f°", (_name, azimuth, elevation, along) => {
    const ray = bearingRay(FROM, local(azimuth, elevation));

    expect(norm(ray)).toBeCloseTo(1, 12);
    expectVec(ray, along);
  });

  it("is a unit vector, half-way between the plane and north at 45° up", () => {
    const ray = bearingRay(FROM, local(0, 45));

    expect(norm(ray)).toBeCloseTo(1, 12);
    expect(dot(ray, frame.coreward)).toBeCloseTo(Math.SQRT1_2, 12);
    expect(dot(ray, frame.north)).toBeCloseTo(Math.SQRT1_2, 12);
  });

  it("runs from galactic +x through +y in the galactic_x frame, near the axis", () => {
    const near: CentreLy = [0.3, -0.4, 200];
    const east = bearingRay(near, { azimuth_deg: 0, elevation_deg: 0, frame: "galactic_x" });

    expect(norm(east)).toBeCloseTo(1, 12);
    expectVec(east, vec3(1, 0, 0));

    expectVec(
      bearingRay(near, { azimuth_deg: 0, elevation_deg: 0, frame: "galactic_x" }),
      vec3(1, 0, 0),
    );
    expectVec(
      bearingRay(near, { azimuth_deg: 90, elevation_deg: 0, frame: "galactic_x" }),
      vec3(0, 1, 0),
    );
  });
});

const CENTRE: CentreLy = [0, 26_000, 0];

/**
 * A chart of 50 ly about {@link CENTRE} holding one system 10 ly coreward, answered in `mode`: its
 * row carries its observation, so that an answer asked observed reads observed (`answeredMode`).
 */
function chartIn(mode: QueryMode) {
  return toChartResult(
    aSystemsInRange({
      centreLy: CENTRE,
      radiusLy: 50,
      systems: [
        {
          relLy: [0, -10, 0],
          layer: "a",
          ...(mode.kind === "observed"
            ? { observed: { lightAgeYr: 15, apparentRelLy: [0, -10, 0] as const } }
            : {}),
        },
      ],
    }),
    mode,
  );
}

const OBSERVED: QueryMode = { kind: "observed", observerLy: [0, 26_005, 0] };

describe("alertMarks (plan 12, P12.T12)", () => {
  it("draws a bearing's ray from the observer to past the display's edge, labelled there", () => {
    const result = chartIn(OBSERVED);
    const contact = aBearingContact({ bearing: local(90, 0) });

    const { points, segments } = alertMarks([contact], result);

    expect(points).toEqual([]);
    expect(segments).toHaveLength(1);
    const [ray] = segments;
    expectVec(ray?.from ?? vec3(0, 0, 0), vec3(0, 5, 0));
    const direction = sub(ray?.to ?? vec3(0, 0, 0), ray?.from ?? vec3(0, 0, 0));
    expect(norm(direction)).toBeCloseTo(RAY_REACH_LY, 3);
    expectVec(
      vec3(direction.x / RAY_REACH_LY, direction.y / RAY_REACH_LY, direction.z / RAY_REACH_LY),
      localFrameAt(vec3(0, 26_005, 0)).spinward,
    );
    expect(ray).toMatchObject({
      id: bearingRayId(1),
      line: "solid",
      stroke: "text",
      widthPx: 1,
      fromMark: OBSERVER_MARK_ID,
      endLabel: `${TRANSIENT_LABEL} 090° +00°`,
    });
  });

  it("puts the transient's star on a resolved host inside the sphere, at the host's own row", () => {
    const result = chartIn(OBSERVED);
    const host = result.systems[0];
    const contact = aResolvedContact([0, 25_990.5, 0], { host: host?.id ?? "" });

    const { points, segments } = alertMarks([contact], result);

    expect(segments).toEqual([]);
    expect(points).toEqual([
      expect.objectContaining({
        id: transientMarkId(2),
        position: host?.relLy,
        shape: "transient",
        sizeClass: TRANSIENT_SIZE_CLASS,
        status: "plain",
        label: TRANSIENT_LABEL,
      }),
    ]);
  });

  it("puts it at the host's present position where the answer has no row for the host", () => {
    const { points } = alertMarks([aResolvedContact([3, 26_020, -1])], chartIn({ kind: "now" }));

    expect(points).toHaveLength(1);
    expectVec(points[0]?.position ?? vec3(0, 0, 0), vec3(3, 20, -1));
  });

  it("draws a resolved host outside the sphere as a ray with its range, to where its light left", () => {
    const contact = aResolvedContact([0, 26_005 + 1240, 0]);

    const { points, segments } = alertMarks([contact], chartIn(OBSERVED));

    expect(points).toEqual([]);
    expectVec(segments[0]?.to ?? vec3(0, 0, 0), vec3(0, 1245, 0));
    expect(segments[0]?.endLabel).toBe(`${TRANSIENT_LABEL} 1240.00 ly`);
  });

  it("draws no ray in now, which draws no observer, but still marks a resolved host", () => {
    const now = chartIn({ kind: "now" });
    const contacts = [aBearingContact(), aResolvedContact([0, 25_995, 0])];

    const { points, segments } = alertMarks(contacts, now);

    expect(segments).toEqual([]);
    expect(points.map((point) => point.id)).toEqual([transientMarkId(2)]);
  });

  it("draws nothing for a contact seen from its host's own place, which has no bearing", () => {
    expect(alertMarks([aBearingContact({ bearing: null })], chartIn(OBSERVED))).toEqual({
      points: [],
      segments: [],
    });
  });
});

describe("withAlertMarks and the legend (plan 12, P12.T12)", () => {
  const result = chartIn(OBSERVED);
  const scene: SpatialScene = {
    frame: localFrameAt(vec3(...CENTRE)),
    points: [],
    spheres: [],
    plane: { spacing: 10, extent: 50, rings: [] },
    selectedId: null,
    destinationId: null,
  };

  it("keeps the scene itself where there is nothing to mark, so that an idle chart paints nothing", () => {
    expect(withAlertMarks(scene, alertMarks([], result))).toBe(scene);
  });

  it("adds the transients to the points and the rays to the segments, which the legend names", () => {
    const contacts: ReadonlyArray<ContactDto> = [
      aBearingContact(),
      aResolvedContact([0, 25_995, 0]),
    ];
    const marked = withAlertMarks(scene, alertMarks(contacts, result));

    expect(marked.points.map((point) => point.shape)).toEqual(["transient"]);
    expect(marked.segments?.map((segment) => segment.id)).toEqual([bearingRayId(1)]);
    expect(alertLegendOf(marked)).toEqual({ transients: true, bearings: true });
    expect(alertLegendOf(scene)).toEqual({ transients: false, bearings: false });
  });

  it("selects a transient's host when its star is picked, and nothing for any other mark", () => {
    const contacts = [aBearingContact(), aResolvedContact([0, 25_995, 0])];

    expect(transientHost(transientMarkId(2), contacts)).toBe("00000000000000c3");
    expect(transientHost(transientMarkId(1), contacts)).toBeNull();
    expect(transientHost("00000000000000c3", contacts)).toBeNull();
  });
});
