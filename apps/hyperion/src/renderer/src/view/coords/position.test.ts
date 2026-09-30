import {
  type BodyIdHex,
  type GalacticPosition,
  galacticPositionFromLy,
  METRES_PER_LIGHT_YEAR,
  type SystemIdHex,
} from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

// The simulation's body-fixed conversions (plan R02, R02.T3), read from the repository, so that a
// re-blessed golden moves this test with it.
import fixture from "../../../../../../../crates/hyperion-sim/tests/golden/coords/body_fixed.golden?raw";
import { norm, sub, vec3, type Vec3 } from "../../geometry/vec3";
import {
  differenceM,
  expressIn,
  type FrameOrigins,
  galacticDeltaM,
  galacticTranslated,
  type ViewPosition,
} from "./position";
import {
  IDENTITY_ROTATION,
  type Rotation3,
  rotateToBody,
  rotateToBodyFixed,
  rotation3FromRows,
} from "./rotation";

const SYSTEM: SystemIdHex = "0200080020000000";
const PLANET: BodyIdHex = `${SYSTEM}.0103`;
const AU_M = 1.495_978_707e11;

/** Origins for one system at the Sun-like point, with a planet at 30 au. */
function origins(rotation: Rotation3 | null = null): FrameOrigins {
  const barycentre = galacticPositionFromLy([8_000.25, 26_000.5, 20.75]);
  return {
    systemBarycentre: () => barycentre,
    bodyCentreM: () => vec3(30 * AU_M, -4.5 * AU_M, 0.25 * AU_M),
    bodyFixedRotation: () => rotation,
  };
}

/** One case of `coords/body_fixed.golden`: a rotation and a conversion each way. */
interface GoldenCase {
  readonly name: string;
  readonly rotation: Rotation3;
  readonly fixed: Vec3;
  readonly toBody: Vec3;
  readonly body: Vec3;
  readonly toBodyFixed: Vec3;
}

function parseGolden(text: string): GoldenCase[] {
  return text
    .split("\n\n")
    .filter((block) => block.startsWith("case "))
    .map((block) => {
      const values = new Map<string, number>();
      for (const line of block.split("\n").slice(1)) {
        const [label = "", rest = ""] = line.split(" = ");
        const decimal = rest.split("# ")[1] ?? "";
        values.set(label, Number(decimal));
      }
      const get = (label: string): number => {
        const value = values.get(label);
        if (value === undefined) {
          throw new Error(`the golden has no ${label}`);
        }
        return value;
      };
      const v = (prefix: string): Vec3 =>
        vec3(get(`${prefix}.x`), get(`${prefix}.y`), get(`${prefix}.z`));
      const row = (i: number): Vec3 =>
        vec3(get(`r[${i}][0]`), get(`r[${i}][1]`), get(`r[${i}][2]`));
      return {
        name: block.split("\n")[0] ?? "",
        rotation: rotation3FromRows([row(0), row(1), row(2)]),
        fixed: v("fixed"),
        toBody: v("to_body"),
        body: v("body"),
        toBodyFixed: v("to_body_fixed"),
      };
    });
}

/** The largest component of `a − b` relative to the larger of the two lengths (or 1 m). */
function relativeError(a: Vec3, b: Vec3): number {
  return norm(sub(a, b)) / Math.max(norm(a), norm(b), 1);
}

describe("galacticDeltaM", () => {
  it("differences a point 1 m from the camera 60,000 ly from the centre to within 2 m", () => {
    const camera = galacticPositionFromLy([60_000, 0.999_999_999_999_9, -3]);
    const point: GalacticPosition = {
      cell_ly: camera.cell_ly,
      offset_m: [camera.offset_m[0], camera.offset_m[1] + 1, camera.offset_m[2]],
    };
    const delta = galacticDeltaM(camera, galacticTranslated(point, vec3(0, 0, 0)));
    expect(Math.abs(delta.y - 1)).toBeLessThanOrEqual(2);
    expect(Math.abs(delta.x)).toBeLessThanOrEqual(2);
  });

  it("carries an offset across a cell boundary", () => {
    const camera = galacticPositionFromLy([60_000, 0.999_999_999_999_9, -3]);
    const across = galacticTranslated(camera, vec3(0, METRES_PER_LIGHT_YEAR, 1));
    const back = galacticDeltaM(camera, across);
    expect(Math.abs(back.y - METRES_PER_LIGHT_YEAR)).toBeLessThanOrEqual(2);
    expect(Math.abs(back.z - 1)).toBeLessThanOrEqual(2);
  });

  it("subtracts cells before offsets", () => {
    // An offset near a whole light-year is spaced at 2 m, the frame's resolution.
    const from: GalacticPosition = {
      cell_ly: [-1, 0, 0],
      offset_m: [METRES_PER_LIGHT_YEAR - 2, 0, 0],
    };
    const to: GalacticPosition = { cell_ly: [0, 0, 0], offset_m: [0.5, 0, 0] };
    expect(Math.abs(galacticDeltaM(from, to).x - 2.5)).toBeLessThanOrEqual(2);
  });
});

describe("galacticTranslated", () => {
  it("refuses a translation that is not finite or leaves the frame", () => {
    const origin = galacticPositionFromLy([0, 0, 0]);
    expect(() => galacticTranslated(origin, vec3(Number.NaN, 0, 0))).toThrow(RangeError);
    expect(() => galacticTranslated(origin, vec3(0, 3e9 * METRES_PER_LIGHT_YEAR, 0))).toThrow(
      RangeError,
    );
  });
});

describe("expressIn", () => {
  it("round-trips a point between the system and a body's frame at 30 au to a micrometre", () => {
    const o = origins();
    const inSystem: ViewPosition = {
      kind: "system",
      system: SYSTEM,
      m: vec3(30 * AU_M + 6.4e6, -4.5 * AU_M + 12.25, 0.25 * AU_M - 3.5e5),
    };
    const inBody = expressIn(inSystem, { kind: "body", body: PLANET }, o);
    const back = expressIn(inBody, { kind: "system", system: SYSTEM }, o);
    if (back.kind !== "system" || inBody.kind !== "body") {
      throw new Error("expressIn returned the wrong frame");
    }
    expect(norm(sub(back.m, inSystem.m))).toBeLessThanOrEqual(1e-6);
    expect(inBody.m.x).toBeCloseTo(6.4e6, 3);
  });

  it("round-trips a point through the galactic frame to the frame's resolution", () => {
    const o = origins();
    const inSystem: ViewPosition = { kind: "system", system: SYSTEM, m: vec3(1.5e11, 2.5e10, -7) };
    const galactic = expressIn(inSystem, { kind: "galactic" }, o);
    const back = expressIn(galactic, { kind: "system", system: SYSTEM }, o);
    if (back.kind !== "system") {
      throw new Error("expressIn returned the wrong frame");
    }
    expect(norm(sub(back.m, inSystem.m))).toBeLessThanOrEqual(4);
  });

  it("turns a body-fixed point into the body frame by the body's rotation", () => {
    const quarter = rotation3FromRows([vec3(0, -1, 0), vec3(1, 0, 0), vec3(0, 0, 1)]);
    const o = origins(quarter);
    const site: ViewPosition = { kind: "body_fixed", body: PLANET, m: vec3(6.371e6, 0, 0) };
    const inBody = expressIn(site, { kind: "body", body: PLANET }, o);
    if (inBody.kind !== "body") {
      throw new Error("expressIn returned the wrong frame");
    }
    expect(inBody.m).toEqual(vec3(0, 6.371e6, 0));
    expect(expressIn(inBody, { kind: "body_fixed", body: PLANET }, o)).toEqual(site);
  });

  it("takes the body-fixed axes as the body frame's where rotation is not modelled", () => {
    const site: ViewPosition = { kind: "body_fixed", body: PLANET, m: vec3(1, 2, 3) };
    const inBody = expressIn(site, { kind: "body", body: PLANET }, origins(null));
    expect(inBody).toEqual({ kind: "body", body: PLANET, m: vec3(1, 2, 3) });
  });
});

describe("differenceM", () => {
  it("differences two points of one frame exactly, however far from its origin", () => {
    const a: ViewPosition = { kind: "system", system: SYSTEM, m: vec3(40 * AU_M, 0, 0) };
    const b: ViewPosition = { kind: "system", system: SYSTEM, m: vec3(40 * AU_M + 2, 0, 0) };
    expect(differenceM(b, a, origins()).x).toBe(2);
  });

  it("differences a body point and a system point through the system frame", () => {
    const o = origins();
    const a: ViewPosition = { kind: "body", body: PLANET, m: vec3(1e7, 0, 0) };
    const b: ViewPosition = { kind: "system", system: SYSTEM, m: o.bodyCentreM(PLANET) };
    expect(differenceM(a, b, o)).toEqual(vec3(1e7, 0, 0));
  });
});

describe("rotation3 against the simulation's golden", () => {
  const cases = parseGolden(fixture);

  it("reads the golden's ten cases", () => {
    expect(cases).toHaveLength(10);
  });

  it("converts body-fixed to body and back as the simulation does, to 1e-9 relative", () => {
    const errors = cases.flatMap((c) => [
      relativeError(rotateToBody(c.rotation, c.fixed), c.toBody),
      relativeError(rotateToBodyFixed(c.rotation, c.body), c.toBodyFixed),
    ]);
    expect(Math.max(...errors)).toBeLessThanOrEqual(1e-9);
  });

  it("refuses a matrix that is not a rotation", () => {
    expect(() => rotation3FromRows([vec3(1, 1e-9, 0), vec3(0, 1, 0), vec3(0, 0, 1)])).toThrow(
      RangeError,
    );
    expect(() => rotation3FromRows([vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, -1)])).toThrow(
      /reflection/,
    );
  });

  it("leaves a vector unchanged under the identity rotation", () => {
    expect(rotateToBody(IDENTITY_ROTATION, vec3(1, 2, 3))).toEqual(vec3(1, 2, 3));
  });
});
