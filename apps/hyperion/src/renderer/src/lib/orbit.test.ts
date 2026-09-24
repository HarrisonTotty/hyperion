import type { UniverseTime } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

// The server's fixture (plan 14, P14.T39), read from the repository, so that a re-blessed golden
// moves this test with it.
import fixture from "../../../../../../crates/hyperion-sim/tests/golden/orbit/states.golden?raw";
import { add, cross, dot, norm, scale, sub, type Vec3 } from "../spatial/vec3";
import {
  type BodyPlacement,
  composePosition,
  type KeplerOrbit,
  NEAR_PARABOLIC_ECCENTRICITY,
  orbitPolyline,
  positionAt,
  solveKepler,
  stateAt,
} from "./orbit";

/** One line of the fixture: the orbit, the time, and the server's state then. */
interface FixtureCase {
  readonly label: string;
  readonly orbit: KeplerOrbit;
  readonly time: UniverseTime;
  readonly positionM: Vec3;
  readonly velocityMPerS: Vec3;
}

/**
 * The fields of a line, in the order its header gives:
 * `a e i node peri m0 period mu t_s t_ns x y z vx vy vz`.
 */
const FIELDS_PER_LINE = 16;

function parseFixture(text: string): FixtureCase[] {
  return text
    .split("\n")
    .filter((line) => line.startsWith("state["))
    .map((line) => {
      const [label = "", values = ""] = line.split(" = ");
      const fields = values.split(" ").map(Number);
      const [a, e, i, node, peri, m0, period, , seconds, nanos, x, y, z, vx, vy, vz] = fields;
      if (
        fields.length !== FIELDS_PER_LINE ||
        !fields.every(Number.isFinite) ||
        a === undefined ||
        e === undefined ||
        i === undefined ||
        node === undefined ||
        peri === undefined ||
        m0 === undefined ||
        period === undefined ||
        seconds === undefined ||
        nanos === undefined ||
        x === undefined ||
        y === undefined ||
        z === undefined ||
        vx === undefined ||
        vy === undefined ||
        vz === undefined
      ) {
        throw new Error(`the fixture's line ${label} does not hold ${FIELDS_PER_LINE} numbers`);
      }
      return {
        label,
        orbit: {
          semiMajorAxisM: a,
          eccentricity: e,
          inclinationRad: i,
          ascendingNodeRad: node,
          argumentOfPeriapsisRad: peri,
          meanAnomalyAtEpochRad: m0,
          periodS: period,
        },
        time: { seconds, nanos },
        positionM: { x, y, z },
        velocityMPerS: { x: vx, y: vy, z: vz },
      };
    });
}

const CASES = parseFixture(fixture);

/** The relative distance between two vectors, against the length of the expected one. */
function relativeError(actual: Vec3, expected: Vec3): number {
  return norm(sub(actual, expected)) / norm(expected);
}

const EPOCH: UniverseTime = { seconds: 0, nanos: 0 };

/** An orbit about the Sun at 1 au, its elements as given. */
function orbitOf(overrides: Partial<KeplerOrbit> = {}): KeplerOrbit {
  return {
    semiMajorAxisM: 1.495978707e11,
    eccentricity: 0.3,
    inclinationRad: 0.4,
    ascendingNodeRad: 1.1,
    argumentOfPeriapsisRad: 2.3,
    meanAnomalyAtEpochRad: 0.7,
    periodS: 31_558_196.020_381_22,
    ...overrides,
  };
}

describe("the server's fixture", () => {
  it("holds every line the header describes, in order", () => {
    expect(CASES.length).toBeGreaterThan(0);
    expect(CASES.map((entry) => entry.label)).toEqual(
      CASES.map((_, index) => `state[${String(index).padStart(2, "0")}]`),
    );
  });

  it.each(CASES.map((entry) => [entry.label, entry] as const))(
    "gives the server's position and velocity for %s to 1E-9 relative",
    (_, { orbit, time, positionM, velocityMPerS }) => {
      const state = stateAt(orbit, time);

      expect(relativeError(state.positionM, positionM)).toBeLessThan(1e-9);
      expect(relativeError(state.velocityMPerS, velocityMPerS)).toBeLessThan(1e-9);
    },
  );
});

describe("solveKepler", () => {
  const ECCENTRICITIES = [0, 0.1, 0.5, 0.9, 0.99, 0.999];
  const MEAN_ANOMALIES = [0, 1e-12, 1e-6, 0.01, 0.5, 1.7, 3, Math.PI - 1e-9, Math.PI];

  it.each(ECCENTRICITIES)("solves Kepler's equation to the rounding floor at e = %f", (e) => {
    const residuals = MEAN_ANOMALIES.map((m) => {
      const anomaly = solveKepler(m, e);
      return Math.abs(anomaly - e * Math.sin(anomaly) - m);
    });

    expect(Math.max(...residuals)).toBeLessThan(1e-14);
  });

  it("is odd in the mean anomaly, exactly", () => {
    for (const m of MEAN_ANOMALIES) {
      expect(solveKepler(-m, 0.7)).toBe(-solveKepler(m, 0.7));
    }
  });

  it("reduces the mean anomaly into [-π, π] first", () => {
    expect(solveKepler(1.25 + 4 * Math.PI, 0.4)).toBeCloseTo(solveKepler(1.25, 0.4), 14);
    expect(solveKepler(-1.25 - 2 * Math.PI, 0.4)).toBeCloseTo(solveKepler(-1.25, 0.4), 14);
  });

  it("gives the mean anomaly itself on a circle", () => {
    expect(solveKepler(2.5, 0)).toBe(2.5);
  });

  it("refuses an orbit the client does not propagate", () => {
    expect(() => solveKepler(1, NEAR_PARABOLIC_ECCENTRICITY)).toThrow(RangeError);
    expect(() => solveKepler(1, -0.1)).toThrow(RangeError);
    expect(() => solveKepler(Number.NaN, 0.1)).toThrow(RangeError);
  });
});

describe("stateAt and positionAt", () => {
  it("gives stateAt's position", () => {
    const time = { seconds: 123_456_789, nanos: 5 };

    expect(positionAt(orbitOf(), time)).toEqual(stateAt(orbitOf(), time).positionM);
  });

  it("is at periapsis, along P at a (1 - e), at the epoch when M₀ is 0", () => {
    const orbit = orbitOf({
      inclinationRad: 0,
      ascendingNodeRad: 0,
      argumentOfPeriapsisRad: 0,
      meanAnomalyAtEpochRad: 0,
    });

    expect(positionAt(orbit, EPOCH)).toEqual({ x: 1.495978707e11 * 0.7, y: 0, z: 0 });
  });

  it("repeats bit for bit a whole number of periods later, a thousand years out", () => {
    // A one-day orbit: 365,250 periods in a thousand Julian years. A float of seconds times the mean
    // motion would move its phase in the tenth figure; the exact reduction does not move it at all.
    const orbit = orbitOf({ semiMajorAxisM: 3.2e9, eccentricity: 0.2, periodS: 86_400 });
    const time = { seconds: 12_345, nanos: 678_000_000 };
    const later = { seconds: 12_345 + 365_250 * 86_400, nanos: 678_000_000 };
    const earlier = { seconds: 12_345 - 365_250 * 86_400, nanos: 678_000_000 };

    expect(stateAt(orbit, later)).toEqual(stateAt(orbit, time));
    expect(stateAt(orbit, earlier)).toEqual(stateAt(orbit, time));
  });

  it("keeps a nanosecond before the epoch apart from the epoch on a sub-second orbit", () => {
    // A white-dwarf pair a quarter of a second round: the nanoseconds are reduced too.
    const orbit = orbitOf({ semiMajorAxisM: 1e7, eccentricity: 0, periodS: 0.25 });
    const before = positionAt(orbit, { seconds: -1, nanos: 999_999_999 });
    const at = positionAt(orbit, EPOCH);

    // 1 ns of a 0.25 s circle at 1E7 m is 2π × 1E7 × 4E-9 m, about 0.25 m.
    expect(norm(sub(before, at))).toBeCloseTo((2 * Math.PI * 1e7 * 1e-9) / 0.25, 6);
  });

  it("moves at the speed the vis-viva equation gives", () => {
    const orbit = orbitOf();
    const mu = (4 * Math.PI ** 2 * orbit.semiMajorAxisM ** 3) / orbit.periodS ** 2;
    const { positionM, velocityMPerS } = stateAt(orbit, { seconds: 9_876_543, nanos: 0 });

    const visViva = Math.sqrt(mu * (2 / norm(positionM) - 1 / orbit.semiMajorAxisM));
    expect(norm(velocityMPerS) / visViva).toBeCloseTo(1, 13);
  });

  it.each([
    ["a near-parabolic eccentricity", { eccentricity: NEAR_PARABOLIC_ECCENTRICITY }],
    ["an eccentricity of 1", { eccentricity: 1 }],
    ["a negative semi-major axis", { semiMajorAxisM: -1 }],
    ["a period of zero", { periodS: 0 }],
    ["an inclination beyond π", { inclinationRad: 3.2 }],
    ["an angle that is not finite", { ascendingNodeRad: Number.POSITIVE_INFINITY }],
  ])("refuses %s", (_, overrides) => {
    expect(() => stateAt(orbitOf(overrides), EPOCH)).toThrow(RangeError);
  });

  it.each([
    ["fractional seconds", { seconds: 1.5, nanos: 0 }],
    ["seconds beyond a safe integer", { seconds: 2 ** 53, nanos: 0 }],
    ["a whole second of nanoseconds", { seconds: 0, nanos: 1_000_000_000 }],
    ["negative nanoseconds", { seconds: 0, nanos: -1 }],
  ])("refuses a time with %s", (_, time) => {
    expect(() => stateAt(orbitOf(), time)).toThrow(RangeError);
  });
});

describe("orbitPolyline", () => {
  const ORBIT = orbitOf({ eccentricity: 0.9 });

  it("closes, its last point its first", () => {
    const points = orbitPolyline(ORBIT, 64);

    expect(points).toHaveLength(65);
    expect(points.at(-1)).toBe(points[0]);
  });

  it("starts at periapsis and passes apoapsis half-way round", () => {
    const points = orbitPolyline(ORBIT, 64);

    expect(norm(points[0] ?? { x: 0, y: 0, z: 0 })).toBeCloseTo(1.495978707e11 * 0.1, 0);
    expect(norm(points[32] ?? { x: 0, y: 0, z: 0 }) / (1.495978707e11 * 1.9)).toBeCloseTo(1, 14);
  });

  it("spaces its points evenly in eccentric anomaly, at r = a (1 - e cos E)", () => {
    const points = orbitPolyline(ORBIT, 16);

    const ratios = points.slice(0, -1).map((point, index) => {
      const anomaly = (2 * Math.PI * index) / 16;
      return norm(point) / (ORBIT.semiMajorAxisM * (1 - ORBIT.eccentricity * Math.cos(anomaly)));
    });
    for (const ratio of ratios) {
      expect(ratio).toBeCloseTo(1, 12);
    }
  });

  it("lies in the orbit's plane", () => {
    const points = orbitPolyline(ORBIT, 16);
    const { positionM, velocityMPerS } = stateAt(ORBIT, { seconds: 4_000_000, nanos: 0 });
    const normal = cross(positionM, velocityMPerS);

    for (const point of points) {
      expect(Math.abs(dot(point, normal)) / (norm(point) * norm(normal))).toBeLessThan(1e-15);
    }
  });

  it("is the ellipse on which stateAt puts the body", () => {
    // P and Q from the polyline itself: periapsis at E = 0, and a (-e) P + b Q at E = π/2.
    const points = orbitPolyline(ORBIT, 16);
    const a = ORBIT.semiMajorAxisM;
    const b = a * Math.sqrt(1 - ORBIT.eccentricity ** 2);
    const p = scale(points[0] ?? { x: 0, y: 0, z: 0 }, 1 / (a * (1 - ORBIT.eccentricity)));
    const q = scale(
      add(points[4] ?? { x: 0, y: 0, z: 0 }, scale(p, a * ORBIT.eccentricity)),
      1 / b,
    );

    for (const seconds of [0, 1_000_000, 7_777_777, -20_000_000]) {
      const positionM = positionAt(ORBIT, { seconds, nanos: 0 });
      const along = dot(positionM, p) / a + ORBIT.eccentricity;
      const across = dot(positionM, q) / b;
      expect(along ** 2 + across ** 2).toBeCloseTo(1, 12);
    }
  });

  it.each([2, 16.5, Number.NaN])("refuses %f segments", (segments) => {
    expect(() => orbitPolyline(ORBIT, segments)).toThrow(RangeError);
  });
});

describe("composePosition", () => {
  const PLANET = orbitOf({ semiMajorAxisM: 7.78e11, eccentricity: 0.05, periodS: 3.74e8 });
  const MOON = orbitOf({ semiMajorAxisM: 4.2e8, eccentricity: 0.004, periodS: 152_870 });
  const BODIES: ReadonlyMap<string, BodyPlacement> = new Map<string, BodyPlacement>([
    ["host", { kind: "origin" }],
    ["planet", { kind: "orbit", parentId: "host", orbit: PLANET }],
    ["moon", { kind: "orbit", parentId: "planet", orbit: MOON }],
  ]);
  const TIME = { seconds: 987_654_321, nanos: 123 };

  it("places a host at the origin", () => {
    expect(composePosition(BODIES, "host", TIME)).toEqual({ x: 0, y: 0, z: 0 });
  });

  it("places a planet on its orbit about the host", () => {
    expect(composePosition(BODIES, "planet", TIME)).toEqual(positionAt(PLANET, TIME));
  });

  it("places a moon at its planet's position plus its offset from the planet", () => {
    expect(composePosition(BODIES, "moon", TIME)).toEqual(
      add(composePosition(BODIES, "planet", TIME), positionAt(MOON, TIME)),
    );
  });

  it("names a body that is not placed", () => {
    expect(() => composePosition(BODIES, "comet", TIME)).toThrow("comet");
  });

  it("names a body whose chain of parents never reaches the origin", () => {
    const looped = new Map<string, BodyPlacement>([
      ["a", { kind: "orbit", parentId: "b", orbit: PLANET }],
      ["b", { kind: "orbit", parentId: "a", orbit: PLANET }],
    ]);

    expect(() => composePosition(looped, "a", TIME)).toThrow("does not reach the origin");
  });

  describe("with the members of a pair", () => {
    // Sirius-like: a 2.5 M☉ inner member and a 1 M☉ outer one, 23.5 au apart on average, as the
    // protocol's wire-form pin of a system summary has them.
    const PAIR = orbitOf({ semiMajorAxisM: 3.515_625e12, eccentricity: 0.5, periodS: 1.921_7e9 });
    const INNER_MSUN = 2.5;
    const OUTER_MSUN = 1;
    const TOTAL_MSUN = INNER_MSUN + OUTER_MSUN;
    const STARS: ReadonlyMap<string, BodyPlacement> = new Map<string, BodyPlacement>([
      ["pair", { kind: "origin" }],
      ["a", { kind: "member", parentId: "pair", orbit: PAIR, share: -OUTER_MSUN / TOTAL_MSUN }],
      ["b", { kind: "member", parentId: "pair", orbit: PAIR, share: INNER_MSUN / TOTAL_MSUN }],
      ["planet", { kind: "orbit", parentId: "a", orbit: PLANET }],
    ]);

    it("puts each member at its share of the relative orbit about the barycentre", () => {
      const relativeM = positionAt(PAIR, TIME);

      expect(composePosition(STARS, "a", TIME)).toEqual(scale(relativeM, -OUTER_MSUN / TOTAL_MSUN));
      expect(composePosition(STARS, "b", TIME)).toEqual(scale(relativeM, INNER_MSUN / TOTAL_MSUN));
    });

    it("keeps the barycentre where the pair is placed", () => {
      const a = composePosition(STARS, "a", TIME);
      const b = composePosition(STARS, "b", TIME);
      const barycentre = scale(add(scale(a, INNER_MSUN), scale(b, OUTER_MSUN)), 1 / TOTAL_MSUN);

      expect(norm(barycentre)).toBeLessThan(1e-9 * norm(sub(b, a)));
    });

    it("keeps the members apart by the relative orbit", () => {
      const apart = sub(composePosition(STARS, "b", TIME), composePosition(STARS, "a", TIME));

      expect(norm(sub(apart, positionAt(PAIR, TIME)))).toBeLessThan(
        1e-12 * norm(positionAt(PAIR, TIME)),
      );
    });

    it("places a body on a member's orbit about the member where it is now", () => {
      expect(composePosition(STARS, "planet", TIME)).toEqual(
        add(composePosition(STARS, "a", TIME), positionAt(PLANET, TIME)),
      );
    });
  });
});
