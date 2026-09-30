import {
  type BodyIdHex,
  galacticPositionFromLy,
  METRES_PER_LIGHT_YEAR,
  type SystemIdHex,
} from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { add, norm, sub, vec3, type Vec3 } from "../../geometry/vec3";
import type { CameraPose } from "../camera/pose";
import { narrow } from "./narrow";
import type { ViewPosition } from "./position";
import {
  type CameraOrigins,
  type OriginRelative,
  originMinusCamera,
  relativeToCamera,
} from "./relative";

const SYSTEM: SystemIdHex = "0200080020000000";
const PLANET: BodyIdHex = `${SYSTEM}.0103`;
const EARTH_RADIUS_M = 6.371e6;
const IDENTITY = { w: 1, x: 0, y: 0, z: 0 };
/** One pixel of a 1920 px, 60° view at its centre, rad: 2 tan 30° ÷ 1920. */
const PIXEL_RAD = (2 * Math.tan(Math.PI / 6)) / 1920;

/** A ship 1 ly from the barycentre along +x, in the system frame. */
const SHIP: ViewPosition = { kind: "system", system: SYSTEM, m: vec3(METRES_PER_LIGHT_YEAR, 0, 0) };

function origins(): CameraOrigins {
  const barycentre = galacticPositionFromLy([8_000, 26_000, 20]);
  return {
    systemBarycentre: () => barycentre,
    bodyCentreM: () => vec3(1.5e11, 0, 0),
    bodyFixedRotation: () => null,
    craftPosition: () => SHIP,
  };
}

function toVec3(f: Float32Array): Vec3 {
  return vec3(f[0] ?? Number.NaN, f[1] ?? Number.NaN, f[2] ?? Number.NaN);
}

/** The gap from the `f32` nearest `d` to the next `f32` above it (IEEE single precision). */
function f32SpacingAt(d: number): number {
  const f = Math.fround(d);
  const next = new Float32Array([f]);
  const bits = new Uint32Array(next.buffer);
  bits[0] = (bits[0] ?? 0) + 1;
  return (next[0] ?? Number.NaN) - f;
}

describe("relativeToCamera and narrow", () => {
  it("keeps two features 1 cm apart distinct at a planet's surface, where the naive path merges them", () => {
    const camera: CameraPose = {
      frame: { kind: "body", body: PLANET },
      positionM: vec3(EARTH_RADIUS_M + 2, 0, 0),
      orientation: IDENTITY,
    };
    // Two features 1 cm apart radially, just above the surface.
    const a: ViewPosition = { kind: "body", body: PLANET, m: vec3(EARTH_RADIUS_M + 0.3, 0, 0) };
    const b: ViewPosition = { kind: "body", body: PLANET, m: vec3(EARTH_RADIUS_M + 0.31, 0, 0) };
    const o = origins();
    const narrowedA = toVec3(narrow(relativeToCamera(a, camera, o)));
    const narrowedB = toVec3(narrow(relativeToCamera(b, camera, o)));
    expect(narrowedB.x - narrowedA.x).toBeCloseTo(0.01, 6);
    // Narrowed first and subtracted after, both land on the same f32 at 6,371 km, where the f32
    // spacing is 0.5 m.
    const naive = [a, b].map((p) => Math.fround(p.m.x) - Math.fround(camera.positionM.x));
    expect(naive[0]).toBe(naive[1]);
  });

  it("narrows to 0.06 mm spacing at 1 km from the camera and 6 cm at 1,000 km", () => {
    expect(f32SpacingAt(1_000)).toBeCloseTo(6.1e-5, 6);
    expect(f32SpacingAt(1_000_000)).toBeCloseTo(0.0625, 6);
  });

  it("reproduces a 1 km patch's vertices at a planetary radius to 1 mm", () => {
    const o = origins();
    const originM = vec3(EARTH_RADIUS_M, 0, 0);
    const patch: OriginRelative = {
      origin: { kind: "body_fixed", body: PLANET, m: originM },
      offsetsF32: new Float32Array([0, 0, 0, 0, 1_000, 0, 0, 0, 1_000, -2, 707.1, -707.1]),
    };
    const camera: CameraPose = {
      frame: { kind: "body", body: PLANET },
      positionM: vec3(EARTH_RADIUS_M + 800, 450, -120),
      orientation: IDENTITY,
    };
    const toOrigin = toVec3(originMinusCamera(patch, camera, o));
    for (let i = 0; i < patch.offsetsF32.length; i += 3) {
      const offset = vec3(
        patch.offsetsF32[i] ?? Number.NaN,
        patch.offsetsF32[i + 1] ?? Number.NaN,
        patch.offsetsF32[i + 2] ?? Number.NaN,
      );
      const drawn = add(toOrigin, offset);
      const exact = sub(add(originM, offset), camera.positionM);
      expect(norm(sub(drawn, exact))).toBeLessThanOrEqual(1e-3);
    }
  });

  it("keeps the own hull within 0.1 px of a seat camera in the craft frame 1 ly out", () => {
    const o = origins();
    // The hull is drawn about the craft's reference point; a plate 1 m ahead of the eye point.
    const eyeM = vec3(0.37, 1.25, -0.5);
    const hull: OriginRelative = { origin: SHIP, offsetsF32: new Float32Array([1.37, 1.25, -0.5]) };
    const drawn = (camera: CameraPose): Vec3 =>
      add(toVec3(originMinusCamera(hull, camera, o)), toVec3(hull.offsetsF32));
    const exact = vec3(1, 0, 0);

    const seat: CameraPose = {
      frame: { kind: "craft", craft: "own" },
      positionM: eyeM,
      orientation: IDENTITY,
    };
    // 1 m away, so the error in metres is the angular error in radians.
    expect(norm(sub(drawn(seat), exact))).toBeLessThanOrEqual(0.1 * PIXEL_RAD);

    // The same pose held in the system frame errs by up to a metre: f64 at 1 ly is spaced at 2 m.
    const inSystem: CameraPose = {
      frame: { kind: "system", system: SYSTEM },
      positionM: add(SHIP.m, eyeM),
      orientation: IDENTITY,
    };
    expect(norm(sub(drawn(inSystem), exact))).toBeGreaterThan(0.1);
  });
});
