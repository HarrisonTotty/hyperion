/**
 * The sim's `planetary/vertical_structure` golden (P14.T24.e): the Solar System table's Earth's,
 * Venus's and Titan's published vertical structures and the sim's `temperature_at` at stated
 * pressures, which R08.T3.e's `nodes` profile is held to bit for bit.
 *
 * @remarks
 * Read from the repository, as `lib/orbit.test.ts` reads its golden, so that a re-blessed golden
 * leaves no copy behind here.
 */
import golden from "../../../../../../crates/hyperion-sim/tests/golden/planetary/vertical_structure.golden?raw";

import type { ProfileNode } from "../lib/system/model";

/** One body's structure and the sim's temperatures at its levels. */
export interface GoldenStructure {
  readonly name: string;
  readonly surfacePa: number;
  readonly surfaceK: number;
  readonly alpha: number;
  readonly skinK: number;
  /** From the ground up, the pressures descending. */
  readonly nodes: ReadonlyArray<ProfileNode>;
  /** `temperature_at` at each level's pressure. */
  readonly levels: ReadonlyArray<ProfileNode>;
}

/** The double whose bits a golden's `0x` and sixteen hexadecimal digits spell. */
function fromBits(hex: string): number {
  const view = new DataView(new ArrayBuffer(8));
  view.setBigUint64(0, BigInt(hex));
  return view.getFloat64(0);
}

/** A body's nodes or levels, by index, as the golden lists them. */
function indexed(entries: ReadonlyMap<string, number>, list: string): ReadonlyArray<ProfileNode> {
  const nodes: ProfileNode[] = [];
  for (let i = 0; ; i += 1) {
    const at = `${list}[${String(i).padStart(2, "0")}]`;
    const pressurePa = entries.get(`${at}.pressure_pa`);
    const temperatureK = entries.get(`${at}.temperature_k`);
    if (pressurePa === undefined || temperatureK === undefined) {
      return nodes;
    }
    nodes.push({ pressurePa, temperatureK });
  }
}

/** The golden's bodies, in its order, each with its declared segment count checked. */
function parsed(text: string): ReadonlyArray<GoldenStructure> {
  const bodies = new Map<string, Map<string, number>>();
  const segments = new Map<string, number>();
  for (const line of text.split("\n")) {
    const match = /^(\w+)\.(\S+) = (\S+)/u.exec(line);
    if (match === null) {
      continue;
    }
    const [, name = "", key = "", value = ""] = match;
    const entries = bodies.get(name) ?? new Map<string, number>();
    bodies.set(name, entries);
    if (value.startsWith("0x")) {
      entries.set(key, fromBits(value));
    } else if (key === "segments") {
      segments.set(name, Number(value));
    }
  }
  return [...bodies].map(([name, entries]) => {
    const scalar = (key: string): number => {
      const value = entries.get(key);
      if (value === undefined) {
        throw new Error(`the golden's ${name} has no ${key}`);
      }
      return value;
    };
    const nodes = indexed(entries, "node");
    if (nodes.length !== (segments.get(name) ?? Number.NaN) + 1) {
      throw new Error(`the golden's ${name} lists ${nodes.length} nodes against its segments`);
    }
    return {
      name,
      surfacePa: scalar("surface_pressure_pa"),
      surfaceK: scalar("surface_temperature_k"),
      alpha: scalar("lapse_factor"),
      skinK: scalar("skin_temperature_k"),
      nodes,
      levels: indexed(entries, "level"),
    };
  });
}

/** Earth's, Venus's and Titan's structures, as the sim's golden pins them. */
export const GOLDEN_STRUCTURES: ReadonlyArray<GoldenStructure> = parsed(golden);

/** The golden's structure of `name`. */
export function goldenStructure(name: "earth" | "venus" | "titan"): GoldenStructure {
  const structure = GOLDEN_STRUCTURES.find((s) => s.name === name);
  if (structure === undefined) {
    throw new Error(`the golden has no ${name}`);
  }
  return structure;
}
