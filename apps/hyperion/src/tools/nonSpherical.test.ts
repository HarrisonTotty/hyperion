import { readdirSync, readFileSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import {
  decodeInt16,
  encodeNode,
  ENSTATITE_NODE,
  ENSTATITE_SAMPLE,
  excerptSizeValues,
  gammaNumberDensity,
  iceAbsorptionText,
  type KernelExcerpt,
  type KernelSource,
  kernelReachText,
  kernelFor,
  LONGWAVE,
  MATERIAL_GRID_NM,
  NODE_K_FACTOR,
  NODE_N_STEP,
  nodeRuleText,
  parseAngles,
  parseYangIsca,
  phaseAngleIndices,
  reduceTamudust,
  reductionError,
  SHORTWAVE,
  SPHERICITY,
  TAMUDUST_MATERIALS,
  TAMUDUST_PHASE_ANGLES,
  thinningText,
  thinSizes,
  wavelengthNodes,
} from "./nonSpherical";

const ATMOSPHERE = new URL("../renderer/src/view/atmosphere/", import.meta.url);

function readJson(relative: string): unknown {
  return JSON.parse(readFileSync(fileURLToPath(new URL(relative, ATMOSPHERE)), "utf8"));
}

/** A string field of a parsed JSON object. */
function textField(value: unknown, field: string): string {
  const v: unknown =
    typeof value === "object" && value !== null && field in value
      ? Reflect.get(value, field)
      : undefined;
  if (typeof v !== "string") {
    throw new TypeError(`no string field ${field}`);
  }
  return v;
}

/** An array field of a parsed JSON object. */
function arrayField(value: unknown, field: string): unknown[] {
  const v: unknown =
    typeof value === "object" && value !== null && field in value
      ? Reflect.get(value, field)
      : undefined;
  if (!Array.isArray(v)) {
    throw new TypeError(`no array field ${field}`);
  }
  return v;
}

function isExcerpt(value: unknown): value is KernelExcerpt {
  return (
    typeof value === "object" &&
    value !== null &&
    "records" in value &&
    Array.isArray(value.records) &&
    "phaseAnglesDeg" in value &&
    Array.isArray(value.phaseAnglesDeg)
  );
}

const rawExcerpt = readJson("fixtures/tamudust-excerpt.json");
if (!isExcerpt(rawExcerpt)) {
  throw new Error("fixtures/tamudust-excerpt.json is not an excerpt");
}
const EXCERPT = rawExcerpt;

describe("the reduction, on the committed raw excerpt", () => {
  // The excerpt's records at k 10⁻³ (n 1.60, sphericity 0.712): every one of the kernel's 169
  // size nodes, P₁₁ at the files' 48 angles.
  const every = excerptSizeValues(EXCERPT, 1);
  const kept = thinSizes(every);

  it("drops size nodes", () => {
    expect(every.length).toBe(SHORTWAVE.sizes);
    expect(kept.length).toBeLessThan(every.length);
    expect(kept[0]).toBe(0);
    expect(kept.at(-1)).toBe(every.length - 1);
  });

  for (const rEff of [0.3, 1, 3]) {
    for (const vEff of [0.1, 0.3]) {
      it(`keeps a gamma r_eff ${rEff} µm, v_eff ${vEff} to 1% in a₁ and 0.002 in g and ω`, () => {
        const error = reductionError(every, kept, 550, gammaNumberDensity(rEff, vEff));
        expect(error.a1).toBeLessThan(0.01);
        expect(error.g).toBeLessThan(0.002);
        expect(error.omega).toBeLessThan(0.002);
      });
    }
  }
});

/**
 * A fake kernel's isca record: 3 sizes, n 1.37 and 1.70, k 10⁻⁴ and 0.1, sphericity 0.695 and
 * 0.785, the record depending on the sphericity through V alone.
 */
function fakeIsca(s: number, r: number, i: number, p: number): Float64Array {
  const x = 10 ** s;
  const sphericity = p === 0 ? 0.695 : 0.785;
  const area = 0.3 * x * x;
  return Float64Array.from([
    x,
    area,
    0.1 * sphericity * x ** 3,
    2 * area,
    1.9 * area,
    1.9 * area * 0.7,
    r === 0 ? 1.37 : 1.7,
    i === 0 ? 1e-4 : 0.1,
    sphericity,
  ]);
}

describe("the reduction's sphericity, on a fake kernel", () => {
  // P₁₁ isotropic, the other elements 0.
  const layout = { ...SHORTWAVE, sizes: 3, reals: 2, imaginaries: 2, sphericities: 2 };
  const fake: KernelSource = {
    layout,
    isca: fakeIsca,
    pmat: (s, r, i, p, angleIndices) => {
      const out = new Float64Array(6 * angleIndices.length);
      out.fill(fakeIsca(s, r, i, p)[4] ?? Number.NaN, 0, angleIndices.length);
      return out;
    },
  };
  const angles = Array.from({ length: 181 }, (_, i) => i);
  const reduce = (sphericity?: number): ReturnType<typeof reduceTamudust> =>
    reduceTamudust(
      ENSTATITE_SAMPLE,
      "(test)",
      { shortwave: fake, longwave: fake },
      angles,
      [ENSTATITE_NODE],
      sphericity,
    );

  it("states a sphericity other than the class's in the file and its header", () => {
    const file = reduce(0.74);
    expect(file.sphericity).toBe(0.74);
    expect(file.particles).toContain("sphericity 0.74");
    expect(file.reduction).toContain("at sphericity 0.74");
  });

  it("leaves the clause out at the class's sphericity, the default", () => {
    const file = reduce();
    expect(file.sphericity).toBe(SPHERICITY);
    expect(file.reduction).not.toContain("at sphericity");
  });

  it("refuses a sphericity outside the kernel", () => {
    expect(() => reduce(0.9)).toThrow(RangeError);
  });
});

describe("the reduction's headers, against fixed values", () => {
  it("give the kernels' reach in maximum dimension", () => {
    expect(kernelReachText(11_810)).toContain(
      "x = 11,810 (D = 714 µm at 380 nm, 1,466 µm at 780 nm)",
    );
    expect(kernelReachText(1476)).toContain("D = 89 µm at 380 nm, 183 µm at 780 nm");
  });

  it("give ice's largest k and k·x at 1 cm", () => {
    expect(iceAbsorptionText({ wavelengthsNm: [380, 780], k: [2e-11, 3.5e-5] })).toContain(
      "k, at most 3.50 × 10⁻⁵, keeps k·x under 2.8 at D ≤ 1 cm",
    );
  });
});

describe("the choices", () => {
  it("snaps a₁'s angles to 48 distinct source angles from 0° to 180°", () => {
    const angles = EXCERPT.phaseAnglesDeg;
    expect(angles.length).toBe(TAMUDUST_PHASE_ANGLES);
    expect(angles[0]).toBe(0);
    expect(angles.at(-1)).toBe(180);
    for (let i = 1; i < angles.length; i += 1) {
      expect(angles[i] ?? 0).toBeGreaterThan(angles[i - 1] ?? 0);
    }
  });

  it("finds the nearest source angle to each even step in √θ", () => {
    const source = Array.from({ length: 181 }, (_, i) => i);
    const indices = phaseAngleIndices(source, 5);
    expect(indices).toEqual([0, 11, 45, 101, 180]);
  });

  it("chooses wavelength nodes inside the n and k rule", () => {
    const n = MATERIAL_GRID_NM.map((nm) => 1.5 + 0.0001 * (nm - 380));
    const k = MATERIAL_GRID_NM.map((nm) => 0.01 * Math.exp(-(nm - 380) / 150));
    const nodes = wavelengthNodes({ n, k }, 0.01, 1e-4);
    expect(nodes[0]).toBe(0);
    expect(nodes.at(-1)).toBe(80);
    for (let i = 1; i < nodes.length; i += 1) {
      const a = nodes[i - 1] ?? 0;
      const b = nodes[i] ?? 0;
      expect(Math.abs((n[b] ?? 0) - (n[a] ?? 0))).toBeLessThan(0.01);
      expect(Math.abs(Math.log((k[b] ?? 1) / (k[a] ?? 1)))).toBeLessThan(Math.log(NODE_K_FACTOR));
    }
  });

  it("picks the kernel, and its floor beneath it", () => {
    expect(kernelFor(1.5, 3e-3)).toEqual({ layout: SHORTWAVE, kTable: 3e-3 });
    expect(kernelFor(1.58, 2e-5)).toEqual({ layout: SHORTWAVE, kTable: 1e-4 });
    expect(kernelFor(2.9, 3)).toEqual({ layout: LONGWAVE, kTable: 3 });
    expect(kernelFor(2.65, 0)).toEqual({ layout: LONGWAVE, kTable: 1e-3 });
    expect(() => kernelFor(3.5, 0)).toThrow(RangeError);
  });
});

describe("the encoding", () => {
  it("round-trips a₁ to 0.05% and the ratios to 2 × 10⁻⁵", () => {
    const node = encodeNode(
      { wavelengthNm: 550, n: 1.5, k: 1e-3, kernel: "shortwave", kTable: 1e-3 },
      [
        {
          x: 1,
          volume: 1,
          area: 1,
          extinction: 1,
          scattering: 0.9,
          scatteringAsymmetry: 0.5,
          p11: [123.456, 0.012_34],
          ratios: [[0.5], [-0.25], [0.99], [-0.123_45], [0.067_89]],
        },
      ],
    );
    const a1 = Array.from(decodeInt16(node.a1), (v) => Math.exp(v / 1000));
    expect(Math.abs((a1[0] ?? 0) / 123.456 - 1)).toBeLessThan(5e-4);
    expect(Math.abs((a1[1] ?? 0) / 0.012_34 - 1)).toBeLessThan(5e-4);
    expect(Math.abs((decodeInt16(node.b1)[0] ?? 0) / 30_000 + 0.123_45)).toBeLessThan(2e-5);
  });
});

describe("the parsers", () => {
  it("reads the 498 angles", () => {
    const text = Array.from({ length: 498 }, (_, i) => (i * 180) / 497).join("\n");
    expect(parseAngles(text).length).toBe(498);
    expect(() => parseAngles("0 1 2")).toThrow(/498/u);
  });

  it("reads Yang et al.'s isca rows", () => {
    const rows = parseYangIsca(" 0.38 2.0 1.5 2.5 2.1 0.99999 0.79\nnot a row\n");
    expect(rows).toEqual([
      {
        wavelengthUm: 0.38,
        maximumDimensionUm: 2,
        volumeUm3: 1.5,
        areaUm2: 2.5,
        extinctionEfficiency: 2.1,
        singleScatteringAlbedo: 0.99999,
        asymmetry: 0.79,
      },
    ]);
  });
});

describe("the committed phase files", () => {
  it("state the node rule, size reach and thinning tolerances the tool holds", () => {
    for (const file of TAMUDUST_MATERIALS.map((m) => m.file)) {
      const phase = readJson(`materials/phase/${file}`);
      const reduction = textField(phase, "reduction");
      const nodes = arrayField(phase, "nodes");
      const kernel = textField(nodes[0], "kernel");
      const reach = Math.max(
        ...nodes.map((n) => arrayField(n, "sizeParameters").map(Number).at(-1) ?? 0),
      );
      expect(reduction).toContain(thinningText());
      expect(reduction).toContain(kernelReachText(reach));
      expect(reduction).toContain(
        nodeRuleText(kernel === "longwave" ? NODE_N_STEP.longwave : NODE_N_STEP.shortwave),
      );
    }
  });

  it("state water ice's absorption between nodes from its index file", () => {
    const ice = readJson("materials/water-ice.json");
    const reduction = textField(readJson("materials/phase/water-ice.json"), "reduction");
    expect(reduction).toContain(
      iceAbsorptionText({
        wavelengthsNm: arrayField(ice, "wavelengthsNm").map(Number),
        k: arrayField(ice, "k").map(Number),
      }),
    );
  });

  it("each stay under 400 kB", () => {
    const dir = fileURLToPath(new URL("materials/phase/", ATMOSPHERE));
    const files = readdirSync(dir).filter((f) => f.endsWith(".json"));
    expect(files.length).toBeGreaterThan(0);
    for (const file of files) {
      expect(statSync(`${dir}/${file}`).size).toBeLessThan(400_000);
    }
  });
});
