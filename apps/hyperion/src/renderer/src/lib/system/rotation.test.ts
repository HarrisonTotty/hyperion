import type { UniverseTime } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import bodyRotations from "../../../../../../../crates/hyperion-sim/tests/golden/frame/body_rotations.golden?raw";
import { cross, dot, vec3, type Vec3 } from "../../geometry/vec3";
import type { RotationLock, SystemBodyRotation } from "./model";
import { bodyFixedAxesAt, rotationAngleAt, spinRateAt } from "./rotation";

/** P14.T46.f's tolerance for the client's twin of `angle_at` (R07.T1, item 5). */
const TWIN_TOLERANCE_RAD = 1e-9;

/** One law of the golden, with W at each of its pinned times. */
interface GoldenLaw {
  readonly name: string;
  readonly law: SystemBodyRotation;
  readonly angles: ReadonlyArray<{ readonly time: UniverseTime; readonly wRad: number }>;
}

function numbersOf(text: string): number[] {
  return text.split(",").map(Number);
}

function vectorOf(text: string): Vec3 {
  const [x = Number.NaN, y = Number.NaN, z = Number.NaN] = numbersOf(text);
  return vec3(x, y, z);
}

function timeOf(text: string): UniverseTime {
  const [seconds = Number.NaN, nanos = Number.NaN] = numbersOf(text);
  return { seconds, nanos };
}

/** The golden's `key=value` fields of one line, after its first two words. */
function fieldsOf(line: string): ReadonlyMap<string, string> {
  return new Map(
    line
      .split(" ")
      .slice(2)
      .map((word): [string, string] => {
        const at = word.indexOf("=");
        return [word.slice(0, at), word.slice(at + 1)];
      }),
  );
}

function field(fields: ReadonlyMap<string, string>, key: string): string {
  const value = fields.get(key);
  if (value === undefined) {
    throw new Error(`the golden's law has no ${key}`);
  }
  return value;
}

/** The golden's `locking_age` and `locks_at`, each `-` where it has none, as the client's lock. */
function lockOf(lockingAge: string, locksAt: string): RotationLock {
  if (lockingAge === "-") {
    return { kind: "never" };
  }
  const lockingAgeS = Number(lockingAge);
  return locksAt === "-"
    ? { kind: "outside_clock", lockingAgeS }
    : { kind: "in_clock", lockingAgeS, locksAt: timeOf(locksAt) };
}

function lawOf(fields: ReadonlyMap<string, string>): SystemBodyRotation {
  const scalar = (key: string): number => Number(field(fields, key));
  const resonance = field(fields, "resonance");
  if (resonance !== "synchronous" && resonance !== "three_to_two") {
    throw new Error(`not a resonance: ${resonance}`);
  }
  return {
    pole: vectorOf(field(fields, "pole")),
    equatorNode: vectorOf(field(fields, "node")),
    equatorQuarter: vectorOf(field(fields, "quarter")),
    obliquityRad: scalar("obliquity"),
    initialRateRadPerS: scalar("initial_rate"),
    lockedRateRadPerS: scalar("locked_rate"),
    ageAtEpochS: scalar("age_at_epoch"),
    lock: lockOf(field(fields, "locking_age"), field(fields, "locks_at")),
    resonance,
    clockPeriodS: scalar("clock_period"),
    clockMeanAnomalyAtEpochRad: scalar("clock_m0"),
    subPrimaryAngleRad: scalar("sub_primary"),
    phaseAtEpochRad: scalar("phase_at_epoch"),
    capturePhaseRad: scalar("capture_phase"),
  };
}

/** Every law of `frame/body_rotations.golden` and its pinned angles, in the file's order. */
function goldenLaws(): ReadonlyArray<GoldenLaw> {
  const laws: { name: string; law: SystemBodyRotation; angles: GoldenLaw["angles"][number][] }[] =
    [];
  for (const line of bodyRotations.split("\n")) {
    if (line.startsWith("body ")) {
      laws.push({ name: line.split(" ")[1] ?? "", law: lawOf(fieldsOf(line)), angles: [] });
    } else if (line.startsWith("w ")) {
      const [, t = "", w = ""] = line.split(" ");
      const current = laws.at(-1);
      if (current === undefined) {
        throw new Error("the golden pins an angle before any law");
      }
      current.angles.push({ time: timeOf(t.slice("t=".length)), wRad: Number(w.slice(2)) });
    }
  }
  return laws;
}

/** `a − b` reduced into `[−π, π)`. */
function angleBetween(a: number, b: number): number {
  const d = (((a - b + Math.PI) % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI);
  return d - Math.PI;
}

function isBefore(a: UniverseTime, b: UniverseTime): boolean {
  return a.seconds === b.seconds ? a.nanos < b.nanos : a.seconds < b.seconds;
}

const LAWS = goldenLaws();

describe("rotationAngleAt against the simulation's frame/body_rotations.golden", () => {
  it("reads every law and angle the golden pins", () => {
    expect(LAWS).toHaveLength(134);
    expect(LAWS.reduce((n, { angles }) => n + angles.length, 0)).toBe(968);
  });

  it("covers a law that never locks in the window, one locked, and each side of a lock", () => {
    const branches = new Set<string>();
    for (const { law, angles } of LAWS) {
      for (const { time } of angles) {
        const { lock } = law;
        if (lock.kind !== "in_clock") {
          branches.add("no lock in the clock");
        } else if (!isBefore(time, lock.locksAt)) {
          branches.add(`locked ${law.resonance}`);
        } else {
          const ahead = lock.lockingAgeS - law.ageAtEpochS > 0;
          branches.add(ahead ? "before a lock ahead" : "before a lock behind");
        }
      }
    }
    expect([...branches].toSorted()).toEqual([
      "before a lock ahead",
      "before a lock behind",
      "locked synchronous",
      "locked three_to_two",
      "no lock in the clock",
    ]);
  });

  it("agrees with every pinned W to 10⁻⁹ rad", () => {
    let worst = 0;
    for (const { law, angles } of LAWS) {
      for (const { time, wRad } of angles) {
        worst = Math.max(worst, Math.abs(angleBetween(rotationAngleAt(law, time), wRad)));
      }
    }
    expect(worst).toBeLessThan(TWIN_TOLERANCE_RAD);
  });

  it("reduces W into [0, 2π)", () => {
    for (const { law, angles } of LAWS) {
      for (const { time } of angles) {
        const w = rotationAngleAt(law, time);
        expect(w).toBeGreaterThanOrEqual(0);
        expect(w).toBeLessThan(2 * Math.PI);
      }
    }
  });

  it("turns a body that never locks at its initial rate", () => {
    const law = LAWS[0]?.law;
    if (law === undefined) {
      throw new Error("the golden has a law");
    }
    const never: SystemBodyRotation = { ...law, lock: { kind: "never" } };
    const s = 86_400;
    const expected = (law.phaseAtEpochRad + law.initialRateRadPerS * s) % (2 * Math.PI);
    expect(rotationAngleAt(never, { seconds: s, nanos: 0 })).toBeCloseTo(expected, 12);
  });
});

describe("bodyFixedAxesAt", () => {
  const law = LAWS[0]?.law;
  if (law === undefined) {
    throw new Error("the golden has a law");
  }
  const times: UniverseTime[] = [
    { seconds: 0, nanos: 0 },
    { seconds: 3_600, nanos: 0 },
    { seconds: -31_557_600, nanos: 500_000_000 },
  ];

  it("turns the node into the prime meridian by W about the pole", () => {
    for (const time of times) {
      const w = rotationAngleAt(law, time);
      const { meridian } = bodyFixedAxesAt(law, time);
      expect(dot(meridian, law.equatorNode)).toBeCloseTo(Math.cos(w), 12);
      expect(dot(meridian, law.equatorQuarter)).toBeCloseTo(Math.sin(w), 12);
      expect(dot(meridian, law.pole)).toBeCloseTo(0, 12);
    }
  });

  it("gives a right-handed orthonormal triad with the law's pole", () => {
    for (const time of times) {
      const { meridian, east, pole } = bodyFixedAxesAt(law, time);
      expect(pole).toBe(law.pole);
      const product = cross(meridian, east);
      expect([product.x, product.y, product.z]).toEqual([
        expect.closeTo(pole.x, 12),
        expect.closeTo(pole.y, 12),
        expect.closeTo(pole.z, 12),
      ]);
      expect([dot(meridian, meridian), dot(east, east), dot(meridian, east)]).toEqual([
        expect.closeTo(1, 14),
        expect.closeTo(1, 14),
        expect.closeTo(0, 14),
      ]);
    }
  });
});

describe("spinRateAt", () => {
  const law = LAWS[0]?.law;
  if (law === undefined) {
    throw new Error("the golden has a law");
  }

  /** The rate of the capture's phase δ (Δ ÷ d)², which `rate_at` leaves out, at `time`. */
  function captureRate(each: SystemBodyRotation, time: UniverseTime): number {
    const { lock } = each;
    if (lock.kind !== "in_clock" || !isBefore(time, lock.locksAt)) {
      return 0;
    }
    const d = lock.lockingAgeS - each.ageAtEpochS;
    return d > 0 ? (2 * each.capturePhaseRad * Math.max(time.seconds, 0)) / (d * d) : 0;
  }

  it("is the rate every golden law's W turns at, less the capture's phase", () => {
    let worst = 0;
    for (const { law: each, angles } of LAWS) {
      for (const { time } of angles) {
        const later = { seconds: time.seconds + 1, nanos: time.nanos };
        const earlier = { seconds: time.seconds - 1, nanos: time.nanos };
        const slope =
          angleBetween(rotationAngleAt(each, later), rotationAngleAt(each, earlier)) / 2;
        const rate = spinRateAt(each, time) + captureRate(each, time);
        worst = Math.max(worst, Math.abs(slope - rate) / rate);
      }
    }
    expect(worst).toBeLessThan(1e-6);
  });

  it("is the locked rate at and after a lock in the clock", () => {
    const locked = LAWS.find(({ law: each }) => each.lock.kind === "in_clock")?.law;
    if (locked === undefined || locked.lock.kind !== "in_clock") {
      throw new Error("the golden has a law that locks in the clock");
    }
    const at = locked.lock.locksAt;
    expect(spinRateAt(locked, at)).toBe(locked.lockedRateRadPerS);
    expect(spinRateAt(locked, { seconds: at.seconds + 86_400, nanos: 0 })).toBe(
      locked.lockedRateRadPerS,
    );
  });

  it("is the initial rate for a body that never locks", () => {
    const never: SystemBodyRotation = { ...law, lock: { kind: "never" } };
    expect(spinRateAt(never, { seconds: 31_557_600, nanos: 0 })).toBe(law.initialRateRadPerS);
  });

  it("is halfway between the initial and locked rates halfway through the locking age", () => {
    const despinning: SystemBodyRotation = {
      ...law,
      lock: { kind: "outside_clock", lockingAgeS: 2 * law.ageAtEpochS },
    };
    expect(spinRateAt(despinning, { seconds: 0, nanos: 0 })).toBeCloseTo(
      (law.initialRateRadPerS + law.lockedRateRadPerS) / 2,
      15,
    );
  });

  it("holds the locked rate once the locking age has passed", () => {
    const past: SystemBodyRotation = {
      ...law,
      lock: { kind: "outside_clock", lockingAgeS: law.ageAtEpochS / 2 },
    };
    // ω₀ + (ω_L − ω₀) × 1, the simulation's arithmetic, which rounds.
    expect(spinRateAt(past, { seconds: 0, nanos: 0 }) / law.lockedRateRadPerS).toBeCloseTo(1, 12);
  });
});
