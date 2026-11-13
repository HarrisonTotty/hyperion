import { describe, expect, it } from "vitest";

import golden from "../../../../../../../crates/hyperion-surface/tests/golden/field/months.golden?raw";
import { FRAC_PI_6, monthBlend, monthEdges } from "./monthBlend";

/** A double's bits as 16 hexadecimal digits, as the golden prints them. */
function hexBits(x: number): string {
  const view = new DataView(new ArrayBuffer(8));
  view.setFloat64(0, x);
  return view.getBigUint64(0).toString(16).padStart(16, "0");
}

/** The golden's lines of one kind, by their label. */
interface GoldenMonths {
  /** `e <e> edge[<k>]` to its bits. */
  readonly edges: ReadonlyMap<string, string>;
  /** `e <e> m <m>` to the blend's months and its weight's bits. */
  readonly blends: ReadonlyMap<
    string,
    { readonly e: number; readonly m: number; from: number; to: number; weight: string }
  >;
}

/** The crate's `months.golden`, parsed: its edges and its blends. */
function parseGolden(): GoldenMonths {
  const edges = new Map<string, string>();
  const blends = new Map<
    string,
    { readonly e: number; readonly m: number; from: number; to: number; weight: string }
  >();
  for (const line of golden.split("\n")) {
    const edge = /^e (?<e>\S+) edge\[(?<k>\d+)\] = 0x(?<bits>[0-9a-f]{16})/u.exec(line);
    if (edge?.groups !== undefined) {
      edges.set(`${edge.groups["e"] ?? ""} ${edge.groups["k"] ?? ""}`, edge.groups["bits"] ?? "");
      continue;
    }
    const months = /^e (?<e>\S+) m (?<m>\S+): month \d+, blend (?<from>\d+) to (?<to>\d+)$/u.exec(
      line,
    );
    if (months?.groups !== undefined) {
      const e = months.groups["e"] ?? "";
      const m = months.groups["m"] ?? "";
      blends.set(`${e} ${m}`, {
        e: Number(e),
        m: Number(m),
        from: Number(months.groups["from"]),
        to: Number(months.groups["to"]),
        weight: "",
      });
      continue;
    }
    const weight = /^e (?<e>\S+) m (?<m>\S+) weight = 0x(?<bits>[0-9a-f]{16})/u.exec(line);
    if (weight?.groups !== undefined) {
      const held = blends.get(`${weight.groups["e"] ?? ""} ${weight.groups["m"] ?? ""}`);
      if (held === undefined) {
        throw new Error(`a weight with no blend before it: ${line}`);
      }
      held.weight = weight.groups["bits"] ?? "";
    }
  }
  return { edges, blends };
}

describe("the month blend's twin (R11.T8.b)", () => {
  const { edges, blends } = parseGolden();

  it("holds π ÷ 6 as Rust's FRAC_PI_6, a bit above Math.PI / 6", () => {
    expect(hexBits(FRAC_PI_6)).toBe("3fe0c152382d7366");
    expect(FRAC_PI_6).toBeGreaterThan(Math.PI / 6);
  });

  it("gives the golden's edges of every orbit to the bit", () => {
    expect(edges.size).toBe(5 * 13);
    for (const [label, bits] of edges) {
      const [e = "", k = ""] = label.split(" ");
      const at = monthEdges(Number(e))?.[Number(k)];
      expect({ label, bits: at === undefined ? "none" : hexBits(at) }).toEqual({ label, bits });
    }
  });

  it("gives the golden's blends of every orbit and time to the bit", () => {
    expect(blends.size).toBe(5 * 13);
    for (const [label, held] of blends) {
      const blend = monthBlend(12, held.e, held.m);
      expect({
        label,
        from: blend?.from,
        to: blend?.to,
        weight: blend === null ? "none" : hexBits(blend.weight),
      }).toEqual({ label, from: held.from, to: held.to, weight: held.weight });
    }
  });

  it("gives month 0 alone in a one-month year, and refuses what no header holds", () => {
    expect(monthBlend(1, 0, 2)).toEqual({ from: 0, to: 0, weight: 0 });
    expect(monthBlend(6, 0, 1)).toBeNull();
    expect(monthBlend(12, 1, 1)).toBeNull();
    expect(monthBlend(12, -0.1, 1)).toBeNull();
    expect(monthBlend(12, NaN, 1)).toBeNull();
    expect(monthBlend(1, 1, 1)).toBeNull();
    expect(monthBlend(12, 0.1, Infinity)).toBeNull();
    expect(monthBlend(12, 0.1, NaN)).toBeNull();
  });
});
