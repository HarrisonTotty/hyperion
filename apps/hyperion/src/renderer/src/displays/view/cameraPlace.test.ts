import { galacticPositionFromLy } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { localFrameAt } from "../../geometry/frame";
import { normalise, vec3 } from "../../geometry/vec3";
import type { CameraPose } from "../../view/camera/pose";
import { IDENTITY_QUATERNION, lookAlong } from "../../view/camera/quaternion";
import { AU_M } from "../../view/scenes/kept";
import { precisionScene } from "../../view/scenes/precision";
import { cameraPlace, directionReading, placeLines, placeStale } from "./cameraPlace";
import { startRun, type ViewRun } from "./viewRun";

/** At the kept system's place, 26,000 ly out along +y: coreward −y, spinward −x. */
const KEPT_LOCAL = localFrameAt(vec3(0, 26_000, 0));

/** The precision scene with its camera free at `pose`. */
function freeAt(pose: Partial<CameraPose>): ViewRun {
  const run = startRun(precisionScene());
  return {
    ...run,
    camera: {
      ...run.camera,
      preset: "free",
      pose: {
        frame: { kind: "system", system: run.scene.system },
        positionM: vec3(0, -AU_M, 0),
        orientation: IDENTITY_QUATERNION,
        ...pose,
      },
    },
  };
}

describe("a direction as the view reads it (R07.T19.f)", () => {
  it("runs its azimuth from COREWARD through SPINWARD, its elevation positive NORTH", () => {
    expect([
      directionReading(vec3(0, -1, 0), KEPT_LOCAL),
      directionReading(vec3(-1, 0, 0), KEPT_LOCAL),
      directionReading(vec3(0, 1, 0), KEPT_LOCAL),
      directionReading(vec3(0, -1, 1), KEPT_LOCAL),
      directionReading(vec3(1, 0, -0.2), KEPT_LOCAL),
    ]).toEqual(["000° +00°", "090° +00°", "180° +00°", "000° +45°", "270° -11°"]);
  });

  it("runs from +x and says so where the place is not known, or on the galactic axis", () => {
    const onAxis = localFrameAt(vec3(0, 0, 5));
    expect([
      directionReading(vec3(1, 0, 0), null),
      directionReading(vec3(0, -1, 0), null),
      directionReading(vec3(0, 1, 0), onAxis),
    ]).toEqual(["000° +00° FROM +X", "090° +00° FROM +X", "270° +00° FROM +X"]);
  });
});

describe("where a view's camera is (R07.T19.f)", () => {
  it("reads the range from the frame's centre and its direction, and the line of sight", () => {
    const place = cameraPlace(freeAt({}), null);
    // The camera is 1 AU coreward of the barycentre, looking down its frame's −z, the vertical.
    expect([place.position, place.pointing, place.unit, place.heldToCraft]).toEqual([
      "1.00 AU 000° +00°",
      "— -90°",
      "AU",
      false,
    ]);
  });

  it("reads the pointing along the line of sight", () => {
    const along = normalise(vec3(-1, 0, 1));
    const place = cameraPlace(freeAt({ orientation: lookAlong(along, vec3(0, 0, 1)) }), null);
    expect(place.pointing).toBe("090° +45°");
  });

  it("keeps the range's unit within its hysteresis", () => {
    const run = freeAt({ positionM: vec3(0, -1.02e9, 0) });
    expect([cameraPlace(run, null).position, cameraPlace(run, "Mm").position]).toEqual([
      "1.02 Gm 000° +00°",
      "1020 Mm 000° +00°",
    ]);
  });

  it("reads a camera held to the own ship in the frame its FRAME line names", () => {
    const run = startRun(precisionScene());
    expect(run.camera.pose.frame.kind).toBe("craft");
    const place = cameraPlace(run, null);
    expect(place.position).toMatch(/^[\d,.]+ (km|Mm|Gm|AU) \d{3}° [+-]\d{2}°$/);
    expect(place.pointing).toMatch(/^(\d{3}°|—) [+-]\d{2}°$/);
    expect(place.heldToCraft).toBe(true);
  });

  it("reads FROM +X where the system's place is not known", () => {
    const run = freeAt({});
    const unplaced: ViewRun = { ...run, scene: { ...run.scene, barycentre: null } };
    expect(cameraPlace(unplaced, null)).toEqual({
      position: "1.00 AU 090° +00° FROM +X",
      pointing: "— -90°",
      unit: "AU",
      heldToCraft: false,
      followsHull: false,
    });
  });

  it.each([
    [0.5, "1.00 AU 090° +00° FROM +X"],
    [1.5, "1.00 AU 270° +00°"],
  ])(
    "reads FROM +X with the barycentre %f ly from the galactic axis, within 1 ly",
    (ly, position) => {
      const run = freeAt({});
      const near: ViewRun = {
        ...run,
        scene: { ...run.scene, barycentre: galacticPositionFromLy([ly, 0, 0]) },
      };
      expect(cameraPlace(near, null).position).toBe(position);
    },
  );

  it("reads a camera at its frame's centre with no direction there", () => {
    expect(cameraPlace(freeAt({ positionM: vec3(0, 0, 0) }), null).position).toBe("0.00 km —");
  });

  it("reads the GALACTIC frame's RADIUS, ANGLE and HEIGHT", () => {
    const origin = galacticPositionFromLy([0, 26_000, 20]);
    const place = cameraPlace(
      freeAt({ frame: { kind: "galactic", origin }, positionM: vec3(0, 0, 0) }),
      null,
    );
    expect([place.position, place.unit]).toEqual([
      "RADIUS 26,000.0 ly · ANGLE 090.0° · HEIGHT +20.0 ly",
      null,
    ]);
  });

  it("reads the GALACTIC frame's ANGLE as missing on the galactic axis", () => {
    const origin = galacticPositionFromLy([0, 0, 20]);
    const place = cameraPlace(
      freeAt({ frame: { kind: "galactic", origin }, positionM: vec3(0, 0, 0) }),
      null,
    );
    expect(place.position).toBe("RADIUS 0.0 ly · ANGLE — · HEIGHT +20.0 ly");
  });
});

describe("a camera's place going stale (R07.T19.f)", () => {
  it("goes stale with the server's scene in SEAT and CHASE, POSITION and POINTING both", () => {
    const seat = startRun(precisionScene());
    const chase = { ...seat, camera: { ...seat.camera, preset: "chase" as const } };
    expect([
      placeStale(cameraPlace(seat, null), true),
      placeStale(cameraPlace(chase, null), true),
      placeStale(cameraPlace(seat, null), false),
    ]).toEqual([
      { position: true, pointing: true },
      { position: true, pointing: true },
      { position: false, pointing: false },
    ]);
  });

  it("takes POSITION stale and POINTING live for a free camera held to the own ship", () => {
    const seat = startRun(precisionScene());
    const heldFree = { ...seat, camera: { ...seat.camera, preset: "free" as const } };
    expect(placeStale(cameraPlace(heldFree, null), true)).toEqual({
      position: true,
      pointing: false,
    });
  });

  it("never goes stale for a free camera held to no craft", () => {
    expect(placeStale(cameraPlace(freeAt({}), null), true)).toEqual({
      position: false,
      pointing: false,
    });
  });

  it("marks the label block's POSITION and POINTING stale while it is", () => {
    const seat = cameraPlace(startRun(precisionScene()), null);
    expect(placeLines(seat, true).map((line) => [line.label, line.stale === true])).toEqual([
      ["POSITION", true],
      ["POINTING", true],
    ]);
    expect(placeLines(seat, false).map((line) => line.stale === true)).toEqual([false, false]);
  });
});
