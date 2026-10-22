/**
 * The maturity nodes' formula (plan R10, R10.T1.c's client half; decision-r10-t1c-client-scope
 * §1.4, §2.5 test 5): the TypeScript twin's grading, segment and interpolation; the twin against
 * `hyperion_surface`'s interpolation and the nodes' own law, as the layer golden's `twin` cases pin
 * them; and the shader's library line for line against the twin's constants.
 */

import { describe, expect, it } from "vitest";

import layerGolden from "../../../../../../../crates/hyperion-surface/tests/golden/worker/maturity_layer.golden?raw";
import maturityNodesWgsl from "../shaders/maturityNodes.wgsl?raw";
import {
  MATURITY_CODE_MAX,
  MATURITY_NODES,
  maturityCodeRatio,
  maturityNodeAlbedo,
  maturityNodeShare,
  maturityRatioOfShare,
  maturitySegment,
} from "./maturityNodes";

/** The `f64` of a golden's `0x` and 16 hexadecimal digits of bits. */
function fromBits(token: string | undefined): number {
  if (token === undefined || !/^0x[0-9a-f]{16}$/u.test(token)) {
    throw new Error(`${String(token)} is not a float's bits`);
  }
  const view = new DataView(new ArrayBuffer(8));
  view.setBigUint64(0, BigInt(token));
  return view.getFloat64(0);
}

/** One of the layer golden's twin cases: a material's nodes at a level, and its samples. */
interface TwinCase {
  readonly name: string;
  readonly bodyMaturity: number;
  /** Each node's share of m_body, then its B, V and R. */
  readonly nodes: ReadonlyArray<{ readonly share: number; readonly bvr: ReadonlyArray<number> }>;
  /** Each sample's share x of m_body, the nodes' own law T there, and Rust's interpolation. */
  readonly samples: ReadonlyArray<{
    readonly share: number;
    readonly law: ReadonlyArray<number>;
    readonly interpolated: ReadonlyArray<number>;
  }>;
}

/** The layer golden's `twin` cases, each its head, its `node` lines and its `sample` lines. */
function twinCases(text: string): TwinCase[] {
  const cases: Array<{
    name: string;
    bodyMaturity: number;
    nodes: Array<{ share: number; bvr: number[] }>;
    samples: Array<{ share: number; law: number[]; interpolated: number[] }>;
  }> = [];
  for (const raw of text.split(/\r?\n/u)) {
    const words = raw.split(" ");
    const [kind, ...rest] = words;
    if (kind === "twin") {
      const head = /^twin (.+): m_body (0x[0-9a-f]{16})$/u.exec(raw);
      if (head === null) {
        throw new Error(`a twin case's head is <name>: m_body <bits>, not ${raw}`);
      }
      cases.push({
        name: head[1] ?? "",
        bodyMaturity: fromBits(head[2]),
        nodes: [],
        samples: [],
      });
      continue;
    }
    const current = cases.at(-1);
    if (kind === "node" && current !== undefined) {
      const [j, share, ...bvr] = rest;
      if (Number(j) !== current.nodes.length || bvr.length !== 3) {
        throw new Error(`a node line is <j> <x> <B> <V> <R>, in order, not ${raw}`);
      }
      current.nodes.push({ share: fromBits(share), bvr: bvr.map(fromBits) });
    } else if (kind === "sample" && current !== undefined) {
      const [share, ...values] = rest;
      if (values.length !== 6) {
        throw new Error(`a sample line is <x> and six albedos, not ${raw}`);
      }
      const bands = values.map(fromBits);
      current.samples.push({
        share: fromBits(share),
        law: bands.slice(0, 3),
        interpolated: bands.slice(3),
      });
    } else if (current !== undefined && raw.length > 0) {
      // The twin cases end the file.
      throw new Error(`a twin case holds node and sample lines alone, not ${raw}`);
    }
  }
  return cases;
}

const TWINS = twinCases(layerGolden);

/** A node table of K nodes, node j's B, V and R at 3j. */
function tableOf(twin: TwinCase): Float64Array {
  return Float64Array.from(twin.nodes.flatMap(({ bvr }) => bvr));
}

/** The twin's albedo at share `share` of m_body over the node table `table`. */
function twinAt(table: Float64Array, share: number): [number, number, number] {
  return maturityNodeAlbedo(table, 0, maturitySegment(maturityRatioOfShare(share)));
}

/** A rising node table of K nodes in three channels, distinct in each. */
const RISING = Float64Array.from({ length: 3 * MATURITY_NODES }, (_, n) => {
  const j = Math.floor(n / 3);
  return 0.05 + 0.01 * j + 0.001 * (n % 3) + 0.0003 * j * j;
});

describe("the maturity nodes' grading and segments (R10.T1.c)", () => {
  it("holds the surface crate's 16 nodes, from fresh ground at node 0 to the background at node 15", () => {
    expect(MATURITY_NODES).toBe(16);
    const shares = Array.from({ length: MATURITY_NODES }, (_, j) => maturityNodeShare(j));
    expect([shares[0], shares.at(-1)]).toEqual([0, 1]);
    expect(shares.every((share, j) => j === 0 || share > (shares[j - 1] ?? 1))).toBe(true);
    for (const node of [-1, MATURITY_NODES, 1.5]) {
      expect(() => maturityNodeShare(node)).toThrow(RangeError);
    }
  });

  it("reads a code's deficit ratio as unorm16 does: 0 at the background, 1 on fresh ground", () => {
    expect(MATURITY_CODE_MAX).toBe(2 ** 16 - 1);
    expect([maturityCodeRatio(0), maturityCodeRatio(MATURITY_CODE_MAX)]).toEqual([0, 1]);
    expect([maturityRatioOfShare(1), maturityRatioOfShare(0)]).toEqual([0, 1]);
  });

  it("draws node K − 1 from code 0 and node 0 from code 65,535, each to the bit", () => {
    const background = maturitySegment(maturityCodeRatio(0));
    const fresh = maturitySegment(maturityCodeRatio(MATURITY_CODE_MAX));
    expect([background, fresh]).toEqual([
      { node: MATURITY_NODES - 2, fraction: 1 },
      { node: 0, fraction: 0 },
    ]);
    expect(maturityNodeAlbedo(RISING, 0, background)).toEqual([
      ...RISING.subarray(3 * (MATURITY_NODES - 1)),
    ]);
    expect(maturityNodeAlbedo(RISING, 0, fresh)).toEqual([...RISING.subarray(0, 3)]);
  });

  it("draws each node at its own maturity, to rounding", () => {
    const misses = Array.from({ length: MATURITY_NODES }, (_, j) => j).flatMap((j) => {
      const drawn = twinAt(RISING, maturityNodeShare(j));
      const off = Math.max(...drawn.map((value, c) => Math.abs(value - (RISING[3 * j + c] ?? 0))));
      return off <= 1e-15 ? [] : [[j, off]];
    });
    expect(misses).toEqual([]);
  });

  it("draws monotone nodes monotonically: darker as the code rises towards fresh ground", () => {
    let previous = Number.POSITIVE_INFINITY;
    for (let code = 0; code <= MATURITY_CODE_MAX; code += 97) {
      const [, v] = maturityNodeAlbedo(RISING, 0, maturitySegment(maturityCodeRatio(code)));
      expect(v).toBeLessThanOrEqual(previous);
      previous = v;
    }
  });

  it("refuses a node table too short for the segment it reads", () => {
    expect(() => maturityNodeAlbedo(RISING.subarray(0, 6), 0, { node: 1, fraction: 0.5 })).toThrow(
      RangeError,
    );
  });
});

describe("the twin against the surface crate's interpolation (decision-r10-t1c-client-scope §2.5, test 5)", () => {
  it("reads the layer golden's two twin cases, the 3.5 Gyr world's basalt regolith at levels 0 and 10", () => {
    expect(TWINS.map(({ name, nodes, samples }) => [name, nodes.length, samples.length])).toEqual([
      ["moon_like_3.5_gyr 1:1 level 0", MATURITY_NODES, 33],
      ["moon_like_3.5_gyr 1:1 level 10", MATURITY_NODES, 33],
    ]);
    // m_body is 1 there, so a share is the maturity itself.
    expect(TWINS.map(({ bodyMaturity }) => bodyMaturity)).toEqual([1, 1]);
  });

  it.each(TWINS)("places $name's nodes at the twin's own shares, to the bit", (twin) => {
    expect(twin.nodes.map(({ share }) => share)).toEqual(
      twin.nodes.map((_, j) => maturityNodeShare(j)),
    );
  });

  it.each(TWINS)(
    "interpolates $name's nodes as the surface crate does, to 10⁻¹² of each albedo",
    (twin) => {
      const table = tableOf(twin);
      const misses = twin.samples.flatMap(({ share, interpolated }) => {
        const drawn = twinAt(table, share / twin.bodyMaturity);
        return drawn.flatMap((value, band) => {
          const rust = interpolated[band] ?? Number.NaN;
          return Math.abs(value - rust) <= 1e-12 * Math.abs(rust)
            ? []
            : [[share, band, value, rust]];
        });
      });
      expect(misses).toEqual([]);
    },
  );

  it.each(TWINS)(
    "holds $name's interpolation within 0.5% of the nodes' own law in each band",
    (twin) => {
      const table = tableOf(twin);
      const misses = twin.samples.flatMap(({ share, law }) => {
        const drawn = twinAt(table, share / twin.bodyMaturity);
        return drawn.flatMap((value, band) => {
          const exact = law[band] ?? Number.NaN;
          const error = Math.abs(value / exact - 1);
          return error <= 0.005 ? [] : [[share, band, error]];
        });
      });
      expect(misses).toEqual([]);
    },
  );
});

describe("the shader's library, maturityNodes.wgsl, against the twin (R10.T1.c)", () => {
  it("carries the twin's K and K − 1", () => {
    expect(maturityNodesWgsl).toContain(`const MATURITY_NODES : u32 = ${String(MATURITY_NODES)}u;`);
    expect(maturityNodesWgsl).toContain(
      `const MATURITY_LAST_NODE : f32 = ${String(MATURITY_NODES - 1)}.0;`,
    );
  });

  it("takes a segment as the twin does: u = ∛(1 − r), guarded at 0, t = (K − 1) u, capped, i and f", () => {
    for (const line of [
      "fn maturity_segment(ratio : f32) -> MaturitySegment {",
      "let x = 1.0 - ratio;",
      "let u = select(pow(x, 1.0 / 3.0), 0.0, x <= 0.0);",
      "let t = min(MATURITY_LAST_NODE * u, MATURITY_LAST_NODE);",
      "let node = min(u32(floor(t)), MATURITY_NODES - 2u);",
      "return MaturitySegment(node, t - f32(node));",
    ]) {
      expect(maturityNodesWgsl).toContain(line);
    }
  });

  it("mixes a segment's two nodes linearly in the albedo, (1 − f) Tᵢ + f Tᵢ₊₁", () => {
    expect(maturityNodesWgsl).toContain(
      "fn maturity_mix(lo : vec3f, hi : vec3f, fraction : f32) -> vec3f {\n  return (1.0 - fraction) * lo + fraction * hi;\n}",
    );
  });

  it("binds and reads nothing, and names each of its items for the maturity alone", () => {
    expect(maturityNodesWgsl).not.toMatch(/@group|@binding|var</u);
    const names = [...maturityNodesWgsl.matchAll(/^(?:fn|const|struct) (\w+)/gmu)].map(
      (match) => match[1] ?? "",
    );
    expect(names).toEqual([
      "MATURITY_NODES",
      "MATURITY_LAST_NODE",
      "MaturitySegment",
      "maturity_segment",
      "maturity_mix",
    ]);
  });
});
