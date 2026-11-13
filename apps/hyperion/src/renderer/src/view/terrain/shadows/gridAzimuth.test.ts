import { describe, expect, it } from "vitest";

import fixture from "../../../../../../../../crates/hyperion-surface/tests/golden/horizon/grid_azimuth.golden?raw";
import type { Xyz } from "../cube";
import { FACES, type Face } from "../patchKey";
import { gridAzimuthRad, warpRate } from "./gridAzimuth";

/** The `f64` of 16 hexadecimal digits of bits. */
function fromBits(hex: string | undefined): number {
  const view = new DataView(new ArrayBuffer(8));
  view.setBigUint64(0, BigInt(`0x${hex ?? "nan"}`));
  return view.getFloat64(0);
}

/** One `azimuth` line of the fixture. */
interface Case {
  readonly face: Face;
  readonly at: Xyz;
  readonly towards: Xyz;
  /** The crate's grid azimuth, radians; `null` for none. */
  readonly phi: number | null;
}

function cases(): Case[] {
  const out: Case[] = [];
  for (const line of fixture.split("\n")) {
    const words = line.split(" ");
    if (words[0] !== "azimuth") {
      continue;
    }
    const face = FACES[Number(words[1])];
    if (face === undefined) {
      throw new Error(`the fixture names no face: ${line}`);
    }
    const v = (n: number): number => fromBits(words[n]);
    out.push({
      face,
      at: [v(2), v(3), v(4)],
      towards: [v(5), v(6), v(7)],
      phi: words[8] === "none" ? null : v(8),
    });
  }
  return out;
}

/** The angle from `a` to `b` round the turn, radians, in (−π, π]. */
function turnDifference(a: number, b: number): number {
  const d = (a - b) % (2 * Math.PI);
  return d > Math.PI ? d - 2 * Math.PI : d <= -Math.PI ? d + 2 * Math.PI : d;
}

describe("the sun's grid azimuth (decision-r10-t8a §2)", () => {
  it("is the crate's grid_azimuth at every case of its fixture, to 10⁻⁵ rad", () => {
    const all = cases();
    // Six faces, nine places on each, seven directions at each.
    expect(all).toHaveLength(6 * 9 * 7);
    expect(new Set(all.map((c) => c.face)).size).toBe(6);
    const answers = all.map((c) => ({
      crate: c.phi,
      twin: gridAzimuthRad(c.face, c.at, c.towards),
    }));
    // None exactly where the crate has none: the zenith and the nadir at every place.
    expect(answers.filter((a) => (a.crate === null) !== (a.twin === null))).toEqual([]);
    expect(answers.filter((a) => a.crate === null)).toHaveLength(6 * 9 * 2);
    let worst = 0;
    for (const { crate, twin } of answers) {
      if (crate !== null && twin !== null && (twin < 0 || twin >= 2 * Math.PI)) {
        throw new Error(`the twin's azimuth ${String(twin)} is outside [0, 2π)`);
      }
      worst =
        crate === null || twin === null
          ? worst
          : Math.max(worst, Math.abs(turnDifference(twin, crate)));
    }
    expect(worst).toBeLessThan(1e-5);
  });

  it("has a warp rate continuous at s = ½", () => {
    expect(warpRate(0.5)).toBe(4 / 3);
    expect(Math.abs(warpRate(0.5 - 1e-12) - warpRate(0.5))).toBeLessThan(1e-11);
  });

  it("refuses a point behind its face", () => {
    expect(() => gridAzimuthRad(0, [-1, 0, 0], [0, 1, 0])).toThrow(/not in front of face 0/u);
  });
});
