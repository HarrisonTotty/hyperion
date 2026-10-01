/**
 * The simulation's in-system golden vectors (rendering plan R03, R03.T3) and the `system_bodies`
 * frames of the same three systems as the server sends them (R03.T13), read for the client's
 * apparent positions to be held to them.
 *
 * @remarks
 * Both files are read from the repository, so that a re-blessed golden reaches the client's test
 * at once: `crates/hyperion-sim/tests/golden/observe/in_system.golden` holds what two observers
 * see of every body and star at the epoch and a century on, every float as its bits;
 * `crates/hyperion-server/tests/golden/scene_systems.golden` holds the frames the client builds
 * its tracks from, each decoded as a frame is.
 */
import { decodeServerMessage, type SystemBodiesDto, type UniverseTime } from "@hyperion/protocol";

import inSystem from "../../../../../../crates/hyperion-sim/tests/golden/observe/in_system.golden?raw";
import sceneSystems from "../../../../../../crates/hyperion-server/tests/golden/scene_systems.golden?raw";
import type { Vec3 } from "../geometry/vec3";
import type { Span } from "../lib/scene/lightTime";

/** The three systems, in the goldens' order. */
export const GOLDEN_SYSTEMS = ["solar_like", "wide_binary", "close_binary"] as const;

/** The two times, in the goldens' order. */
export const GOLDEN_TIMES = ["epoch", "plus_100_years"] as const;

/** What the simulation saw of one source. */
export interface GoldenSeen {
  readonly emitted: UniverseTime;
  readonly lightTime: Span;
  readonly corrections: number;
  readonly residual: Span;
  readonly geometricM: Vec3;
  readonly apparentM: Vec3;
}

/** One source in one reading: a body or a star, by body index. */
export interface GoldenSource {
  readonly kind: "body" | "star";
  readonly bodyIndex: number;
  /** Its velocity at the observer's time, m/s, where the simulation gave one. */
  readonly velocityNowMPerS: Vec3 | null;
  /** What was seen, or `null` where the simulation saw nothing. */
  readonly seen: GoldenSeen | null;
  /** Its Hill radius at pericentre, m, where it has a mass and a bound orbit. */
  readonly hillRadiusM: number | null;
}

/** One observer's reading of one system at one time. */
export interface GoldenReading {
  readonly system: (typeof GOLDEN_SYSTEMS)[number];
  readonly when: (typeof GOLDEN_TIMES)[number];
  readonly who: string;
  readonly time: UniverseTime;
  readonly observerM: Vec3;
  readonly observerVelocityMPerS: Vec3;
  readonly sources: ReadonlyArray<GoldenSource>;
}

const SECONDS_PER_JULIAN_YEAR = 31_557_600;

function timeOf(when: string): UniverseTime {
  return when === "epoch"
    ? { seconds: 0, nanos: 0 }
    : { seconds: 100 * SECONDS_PER_JULIAN_YEAR, nanos: 0 };
}

/** The float a golden line's `0x…` bits encode. */
function bitsToFloat(hex: string): number {
  const view = new DataView(new ArrayBuffer(8));
  view.setBigUint64(0, BigInt(hex));
  return view.getFloat64(0);
}

/** A golden `<seconds>s <nanos>ns` pair. */
function pairOf(text: string): { seconds: number; nanos: number } {
  const match = /^(-?\d+)s (\d+)ns$/.exec(text);
  if (match === null) {
    throw new Error(`not a time: ${text}`);
  }
  return { seconds: Number(match[1]), nanos: Number(match[2]) };
}

/** The values of one entry, by key. */
type Fields = Map<string, string>;

function float(fields: Fields, key: string): number {
  const value = fields.get(key);
  if (value === undefined) {
    throw new Error(`the golden lacks ${key}`);
  }
  return bitsToFloat(value);
}

function vector(fields: Fields, prefix: string, unit: string): Vec3 {
  return {
    x: float(fields, `${prefix}_x_${unit}`),
    y: float(fields, `${prefix}_y_${unit}`),
    z: float(fields, `${prefix}_z_${unit}`),
  };
}

function seenOf(fields: Fields): GoldenSeen | null {
  const emitted = fields.get("emitted");
  const lightTime = fields.get("light_time");
  const corrections = fields.get("corrections");
  const residual = fields.get("residual");
  if (
    emitted === undefined ||
    lightTime === undefined ||
    corrections === undefined ||
    residual === undefined
  ) {
    return null;
  }
  return {
    emitted: pairOf(emitted),
    lightTime: pairOf(lightTime),
    corrections: Number(corrections),
    residual: pairOf(residual),
    geometricM: vector(fields, "geometric", "m"),
    apparentM: vector(fields, "apparent", "m"),
  };
}

function sourceOf(kind: "body" | "star", bodyIndex: number, fields: Fields): GoldenSource {
  return {
    kind,
    bodyIndex,
    velocityNowMPerS: fields.has("velocity_now_x_m_s")
      ? vector(fields, "velocity_now", "m_s")
      : null,
    seen: seenOf(fields),
    hillRadiusM: fields.has("hill_radius_m") ? float(fields, "hill_radius_m") : null,
  };
}

function isSystem(name: string): name is GoldenReading["system"] {
  return GOLDEN_SYSTEMS.some((system) => system === name);
}

function isTime(name: string): name is GoldenReading["when"] {
  return GOLDEN_TIMES.some((when) => when === name);
}

/** Every reading of the in-system golden, in its order; a reading with no observer is skipped. */
export function goldenReadings(): ReadonlyArray<GoldenReading> {
  const readings: GoldenReading[] = [];
  let header: { system: GoldenReading["system"]; when: GoldenReading["when"]; who: string } | null =
    null;
  let observer: Fields = new Map();
  let sources: GoldenSource[] = [];
  let source: { kind: "body" | "star"; bodyIndex: number; fields: Fields } | null = null;
  const closeSource = (): void => {
    if (source !== null) {
      sources.push(sourceOf(source.kind, source.bodyIndex, source.fields));
      source = null;
    }
  };
  const closeReading = (): void => {
    closeSource();
    if (header !== null) {
      readings.push({
        ...header,
        time: timeOf(header.when),
        observerM: vector(observer, "observer", "m"),
        observerVelocityMPerS: vector(observer, "observer_velocity", "m_s"),
        sources,
      });
    }
    header = null;
    observer = new Map();
    sources = [];
  };
  for (const line of inSystem.split("\n")) {
    if (line.startsWith("#") || line.trim() === "") {
      continue;
    }
    const entry = /^(body|star) (\d+)$/.exec(line);
    if (entry !== null) {
      closeSource();
      source = {
        kind: entry[1] === "star" ? "star" : "body",
        bodyIndex: Number(entry[2]),
        fields: new Map(),
      };
      continue;
    }
    const assignment = /^(\w+) = (\S+(?: \S+ns)?)/.exec(line);
    if (assignment !== null) {
      const [, key = "", value = ""] = assignment;
      (source === null ? observer : source.fields).set(key, value);
      continue;
    }
    if (line.startsWith("not seen")) {
      continue;
    }
    const [system = "", when = "", who = ""] = line.split(" ");
    closeReading();
    if (isSystem(system) && isTime(when) && !who.endsWith(":")) {
      header = { system, when, who };
    }
  }
  closeReading();
  return readings;
}

/** The `system_bodies` answer for a golden system at a golden time, as the server sent it. */
export function goldenSystemBodies(
  system: GoldenReading["system"],
  when: GoldenReading["when"],
): SystemBodiesDto {
  const frames = sceneSystems.split("\n").filter((line) => line.startsWith("{"));
  const index = GOLDEN_SYSTEMS.indexOf(system) * GOLDEN_TIMES.length + GOLDEN_TIMES.indexOf(when);
  const frame = frames[index];
  if (frame === undefined) {
    throw new Error(`no frame for ${system} at ${when}`);
  }
  const message = decodeServerMessage(frame);
  if (message.type !== "response" || message.body.kind !== "system_bodies") {
    throw new Error(`the frame for ${system} at ${when} is not a system's bodies`);
  }
  const { kind: _kind, ...bodies } = message.body;
  return bodies;
}
