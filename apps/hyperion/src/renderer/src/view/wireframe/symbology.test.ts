import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { FIXTURE_MOON, FIXTURE_PLANET } from "../../test/viewFixtures";
import { type ProjectionCamera, project, type Viewport } from "../camera/projection";
import { IDENTITY_QUATERNION, quaternionFromAxisAngle } from "../camera/quaternion";
import type { CameraTarget } from "../camera/state";
import { bodyKindSymbol } from "../scene/model";
import {
  bodySymbolMark,
  bracketReticle,
  destinationReticle,
  flightPathMarker,
  type ScreenMark,
  symbologyMarks,
  targetMark,
} from "./symbology";

const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };
const CAMERA: ProjectionCamera = {
  orientation: quaternionFromAxisAngle(vec3(0, 1, 0), 0.2),
  fovXRad: Math.PI / 3,
};
const REM_PX = 16;
const PLANET: CameraTarget = { kind: "body", body: FIXTURE_PLANET };
const MOON: CameraTarget = { kind: "body", body: FIXTURE_MOON };

/** The mark's strokes' bounding box. */
function bounds(mark: ScreenMark): { left: number; right: number; top: number; bottom: number } {
  const xs = mark.segments.flatMap(([a, b]) => [a.xPx, b.xPx]);
  const ys = mark.segments.flatMap(([a, b]) => [a.yPx, b.yPx]);
  return {
    left: Math.min(...xs),
    right: Math.max(...xs),
    top: Math.min(...ys),
    bottom: Math.max(...ys),
  };
}

describe("the bracket reticle", () => {
  it("encloses the selected mark's projected position", () => {
    const p = project(vec3(1e6, 2e5, -4e6), CAMERA, VIEWPORT);
    const box = bounds(bracketReticle(PLANET, { xPx: p.xPx, yPx: p.yPx }, 5, REM_PX));
    expect([box.left < p.xPx, box.right > p.xPx, box.top < p.yPx, box.bottom > p.yPx]).toEqual([
      true,
      true,
      true,
      true,
    ]);
  });

  it("is drawn in --accent as four corner brackets, a margin outside the mark", () => {
    const reticle = bracketReticle(PLANET, { xPx: 100, yPx: 100 }, 5, REM_PX);
    expect([reticle.token, reticle.segments.length, bounds(reticle).left]).toEqual([
      "accent",
      8,
      100 - 5 - 0.25 * REM_PX,
    ]);
  });
});

describe("the destination reticle", () => {
  it("is drawn in --target, outside the selection's brackets", () => {
    const at = { xPx: 100, yPx: 100 };
    const destination = destinationReticle(PLANET, at, 5, REM_PX);
    expect([
      destination.token,
      bounds(destination).left < bounds(bracketReticle(PLANET, at, 5, REM_PX)).left,
    ]).toEqual(["target", true]);
  });
});

describe("a target's mark", () => {
  it("is four open ticks on the cardinal directions, outside the mark, unlike the brackets", () => {
    const at = { xPx: 100, yPx: 100 };
    const mark = targetMark(MOON, at, 6, vec3(0, 0, -1e3), null);
    // Each tick is one axis-aligned stroke along a ray from the centre, starting at the radius.
    const ticks = mark.segments.map(([a, b]) => {
      const inner = Math.hypot(a.xPx - at.xPx, a.yPx - at.yPx);
      const outer = Math.hypot(b.xPx - at.xPx, b.yPx - at.yPx);
      const radial = (a.xPx === b.xPx && a.xPx === at.xPx) || (a.yPx === b.yPx && a.yPx === at.yPx);
      return [radial, inner, outer > inner];
    });
    expect([mark.token, ticks]).toEqual(["text", Array.from({ length: 4 }, () => [true, 6, true])]);
  });

  it("carry its range and closure rate, closing positive", () => {
    const bracket = targetMark(MOON, { xPx: 10, yPx: 10 }, 5, vec3(3e3, 4e3, 0), vec3(-3, -4, 0));
    expect([bracket.rangeM, bracket.closureMPerS]).toEqual([5e3, 5]);
  });

  it("carry no closure rate with no own ship", () => {
    expect(
      targetMark(MOON, { xPx: 10, yPx: 10 }, 5, vec3(0, 0, -1e3), null).closureMPerS,
    ).toBeNull();
  });
});

describe("the flight path marker", () => {
  it("projects the own ship's velocity direction", () => {
    const velocity = vec3(100, 50, -2000);
    const marker = flightPathMarker(velocity, CAMERA, VIEWPORT, REM_PX);
    const p = project(velocity, CAMERA, VIEWPORT);
    expect([marker?.anchor.xPx, marker?.anchor.yPx].map((v) => Math.round((v ?? 0) * 1e6))).toEqual(
      [p.xPx, p.yPx].map((v) => Math.round(v * 1e6)),
    );
  });

  it("shows at a docking speed of a centimetre a second", () => {
    const straight: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
    expect(flightPathMarker(vec3(0, 0, -0.01), straight, VIEWPORT, REM_PX)?.anchor).toEqual({
      xPx: 960,
      yPx: 540,
    });
  });

  it("is absent with no own ship", () => {
    expect(flightPathMarker(null, CAMERA, VIEWPORT, REM_PX)).toBeNull();
  });

  it("is absent where the velocity points behind the camera", () => {
    const straight: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };
    expect(flightPathMarker(vec3(0, 0, 10), straight, VIEWPORT, REM_PX)).toBeNull();
  });
});

describe("a body's symbol", () => {
  it("marks a body 2 px across with its symbol", () => {
    const mark = bodySymbolMark(MOON, bodyKindSymbol("moon"), { xPx: 50, yPx: 50 }, 2, REM_PX);
    // The moon's pentagon: five strokes.
    expect([mark?.kind, mark?.segments.length]).toEqual(["body_symbol", 5]);
  });

  it("leaves a body 4 px across to its limb", () => {
    expect(
      bodySymbolMark(PLANET, bodyKindSymbol("planet"), { xPx: 50, yPx: 50 }, 4, REM_PX),
    ).toBeNull();
  });

  it("draws a ringed circle's ring and its disc", () => {
    const mark = bodySymbolMark(
      PLANET,
      { shape: "ringed-circle", sizeClass: 3 },
      { xPx: 50, yPx: 50 },
      1,
      REM_PX,
    );
    expect(mark?.segments.length).toBe(24 + 16);
  });
});

describe("symbologyMarks", () => {
  it("marks a small body, brackets the selection, the destination and a craft, and adds the flight path marker", () => {
    const marks = symbologyMarks(
      {
        anchors: [
          {
            target: MOON,
            at: { xPx: 300, yPx: 300 },
            body: { symbol: bodyKindSymbol("moon"), diameterPx: 1 },
            craft: null,
          },
          {
            target: { kind: "craft", craft: "other" },
            at: { xPx: 500, yPx: 300 },
            body: null,
            craft: { relativeM: vec3(0, 0, -1e3), relativeVelocityMPerS: null },
          },
        ],
        selection: MOON,
        destination: MOON,
        ownVelocityMPerS: vec3(0, 0, -1),
        remPx: REM_PX,
      },
      { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 },
      VIEWPORT,
    );
    expect(marks.map((m) => [m.kind, m.token])).toEqual([
      ["body_symbol", "text"],
      ["selection", "accent"],
      ["destination", "target"],
      ["target", "text"],
      ["flight_path", "text"],
    ]);
  });
});
