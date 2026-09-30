import type { BodyIdHex } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

// The simulation's answers (plan R02, R02.T4.b), read from the repository, so that a re-blessed
// golden moves this test with it.
import fixture from "../../../../../../../crates/hyperion-sim/tests/golden/frame/body_frames.golden?raw";
import { norm, vec3 } from "../../geometry/vec3";
import {
  BODY_FRAME_ENTRY,
  type CameraFrameCandidate,
  cameraFrameCandidate,
  clampToTidalRadius,
  selectCameraFrame,
} from "./frames";

/** One case of the golden: its candidates, the current frame and the simulation's answer. */
interface GoldenCase {
  readonly line: string;
  readonly current: BodyIdHex | null;
  readonly answer: BodyIdHex | null;
  readonly candidates: readonly CameraFrameCandidate[];
}

function idOrNull(text: string): BodyIdHex | null {
  return text === "-" ? null : text;
}

function field(part: string | undefined, name: string): string {
  const prefix = `${name}=`;
  if (part?.startsWith(prefix) !== true) {
    throw new Error(`expected ${prefix}… in the golden, got ${String(part)}`);
  }
  return part.slice(prefix.length);
}

function parseGolden(text: string): GoldenCase[] {
  return text
    .split("\n")
    .filter((line) => line.startsWith("current="))
    .map((line) => {
      const [current, answer, candidates] = line.split(" ");
      const listed = field(candidates, "candidates")
        .split(";")
        .map((entry) => {
          const [id = "", parent = "", distance = "", hill = ""] = entry.split("/");
          return cameraFrameCandidate(id, idOrNull(parent), Number(distance), Number(hill));
        });
      return {
        line,
        current: idOrNull(field(current, "current")),
        answer: idOrNull(field(answer, "answer")),
        candidates: listed,
      };
    });
}

const SYSTEM = "0200080020000000";

function body(index: number): BodyIdHex {
  return `${SYSTEM}.${index.toString(16).padStart(4, "0")}`;
}

describe("selectCameraFrame", () => {
  const cases = parseGolden(fixture);

  it("reads every case of the simulation's golden", () => {
    // R02.T4.b writes 200 cases; a re-blessed golden with another count should be noticed here.
    expect(cases).toHaveLength(200);
  });

  it("gives the simulation's answer on every line of the golden", () => {
    const disagreements = cases.filter(
      (c) => selectCameraFrame(c.candidates, c.current) !== c.answer,
    );
    expect(disagreements.map((c) => c.line)).toEqual([]);
  });

  it("gives the same answer whatever the candidates' order", () => {
    const disagreements = cases.filter(
      (c) => selectCameraFrame(c.candidates.toReversed(), c.current) !== c.answer,
    );
    expect(disagreements.map((c) => c.line)).toEqual([]);
  });

  const earth = body(1);
  const moon = body(2);
  const moonHillM = 5.8e7;
  const at = (ratio: number): CameraFrameCandidate[] => [
    cameraFrameCandidate(earth, null, 3.844e8 + ratio * moonHillM, 1.47e9),
    cameraFrameCandidate(moon, earth, ratio * moonHillM, moonHillM),
  ];

  it("enters the Moon's frame at nine tenths of its Hill radius and not before", () => {
    expect(selectCameraFrame(at(0.95), earth)).toBe(earth);
    expect(selectCameraFrame(at(BODY_FRAME_ENTRY), earth)).toBe(moon);
  });

  it("stays in the Moon's frame up to its Hill radius and leaves above it", () => {
    expect(selectCameraFrame(at(0.95), moon)).toBe(moon);
    expect(selectCameraFrame(at(1), moon)).toBe(moon);
    expect(selectCameraFrame(at(1.01), moon)).toBe(earth);
  });

  it("is in the system frame outside every sphere", () => {
    expect(selectCameraFrame([cameraFrameCandidate(body(1), null, 2e9, 1e9)], body(1))).toBeNull();
    expect(selectCameraFrame([], null)).toBeNull();
  });

  it("refuses what the simulation refuses", () => {
    expect(() => cameraFrameCandidate(body(1), null, -1, 1)).toThrow(RangeError);
    expect(() => cameraFrameCandidate(body(1), null, 1, 0)).toThrow(RangeError);
    expect(() => cameraFrameCandidate(body(1), body(1), 1, 1)).toThrow(RangeError);
    expect(() => cameraFrameCandidate(body(1), null, Number.NaN, 1)).toThrow(RangeError);
    expect(() => cameraFrameCandidate(body(1), null, 1, Infinity)).toThrow(RangeError);
  });

  it("stores a distance of −0 as +0", () => {
    expect(Object.is(cameraFrameCandidate(body(1), null, -0, 1).distanceM, 0)).toBe(true);
  });
});

describe("clampToTidalRadius", () => {
  it("pulls a camera beyond the system's sphere back to it along its own direction", () => {
    const clamped = clampToTidalRadius(vec3(3e16, 4e16, 0), 1e16);
    expect(norm(clamped)).toBeCloseTo(1e16, -2);
    expect(clamped.x / clamped.y).toBeCloseTo(0.75, 12);
  });

  it("leaves a camera inside the sphere where it is", () => {
    const inside = vec3(1e12, -2e12, 3e12);
    expect(clampToTidalRadius(inside, 1e16)).toBe(inside);
  });

  it("refuses a tidal radius that is not positive", () => {
    expect(() => clampToTidalRadius(vec3(0, 0, 0), 0)).toThrow(RangeError);
    expect(() => clampToTidalRadius(vec3(0, 0, 0), Number.NaN)).toThrow(RangeError);
    expect(() => clampToTidalRadius(vec3(0, 0, 0), Infinity)).toThrow(RangeError);
  });
});
