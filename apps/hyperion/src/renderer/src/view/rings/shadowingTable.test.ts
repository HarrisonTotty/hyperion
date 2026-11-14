import { describe, expect, it } from "vitest";

import { SHADOWING_CODES, SHADOWING_HEADER } from "./shadowingTable";

/** The module's size limit, bytes (plan R11, R11.T9.b: "under 50 kB"). */
const MODULE_LIMIT_BYTES = 50_000;

/** The generated module, as its text. */
const SOURCES: Readonly<Record<string, string>> = import.meta.glob<string>("./shadowingTable.ts", {
  query: "?raw",
  import: "default",
  eager: true,
});

/** The module's text, which the glob must have found. */
function moduleText(): string {
  const text = SOURCES["./shadowingTable.ts"];
  if (text === undefined) {
    throw new Error("the glob found no ./shadowingTable.ts beside its test");
  }
  return text;
}

const { tau, mu, phase, quantities, ratios, codesPerUnit } = SHADOWING_HEADER;
const cells = tau.nodes * mu.nodes * phase.nodes;

/** Quantity `name`'s values over (τ, μ, v), v fastest. */
function quantity(name: (typeof quantities)[number]): number[] {
  const q = quantities.indexOf(name);
  const perUnit = name === "shadowing" ? codesPerUnit.shadowing : codesPerUnit.orders;
  return Array.from(SHADOWING_CODES.subarray(q * cells, (q + 1) * cells), (c) => c / perUnit);
}

describe("the rings' shadowing table (R11.T9.b)", () => {
  it("names its tool, version and inputs in its header", () => {
    expect(SHADOWING_HEADER.tool).toBe("hyperion-fit ring-shadowing");
    expect(SHADOWING_HEADER.version).toBe(1);
    expect(SHADOWING_HEADER.radiiM).toEqual([0.5, 5]);
    expect(SHADOWING_HEADER.phaseExponent).toBe(3.09);
    expect(SHADOWING_HEADER.referenceFillingFactor).toBe(0.35);
    expect(SHADOWING_HEADER.tauNodes).toHaveLength(tau.nodes);
  });

  it("states its header in its doc comment", () => {
    const text = moduleText();
    expect(text.startsWith("/**\n * The rings' shadowing table")).toBe(true);
    for (const heading of ["**What it holds.**", "**Axes.**", "**Header.**", "**Acceptance**"]) {
      expect(text).toContain(heading);
    }
  });

  it("records acceptance figures within Salo and French's bands", () => {
    const a = SHADOWING_HEADER.acceptance;
    expect(Math.abs(a.fillingFactor - 0.35)).toBeLessThan(0.005);
    expect(Math.abs(a.tiltRatioAt6Deg - 1.3)).toBeLessThanOrEqual(0.05);
    expect(a.uniformTiltRatioAt6Deg).toBeLessThan(1.25);
    expect(Math.abs(a.reflectanceAt814Nm - 0.44)).toBeLessThanOrEqual(0.05);
    expect(Math.abs(a.reflectanceAt336Nm - 0.16)).toBeLessThanOrEqual(0.03);
  });

  it("stays under 50 kB", () => {
    expect(new TextEncoder().encode(moduleText()).length).toBeLessThan(MODULE_LIMIT_BYTES);
  });

  it("holds one code per node of each quantity and ratio", () => {
    expect(SHADOWING_CODES).toHaveLength(
      quantities.length * cells + ratios.length * tau.nodes * mu.nodes,
    );
  });

  it("solves each τ node on its axis, within 0.5% as measured", () => {
    for (const [k, node] of SHADOWING_HEADER.tauNodes.entries()) {
      const target = tau.scale * ((1 + tau.max / tau.scale) ** (k / (tau.nodes - 1)) - 1);
      expect(Math.abs(node.tauPhot - target)).toBeLessThan(1e-5);
      expect(Math.abs(node.measured - node.tauPhot)).toBeLessThanOrEqual(0.005 * node.tauPhot);
    }
  });

  it("leaves the classical layer unchanged at τ = 0", () => {
    expect(quantity("shadowing").slice(0, mu.nodes * phase.nodes)).toEqual(
      Array.from({ length: mu.nodes * phase.nodes }, () => 1),
    );
  });

  it("keeps the shadowing factor between 0.9 and the full opposition's doubling", () => {
    expect(quantity("shadowing").every((f) => f >= 0.9 && f <= 2.001)).toBe(true);
  });

  it("fully doubles the thickest layer at opposition below the zenith", () => {
    const shadowing = quantity("shadowing");
    const thickest = (tau.nodes - 1) * mu.nodes * phase.nodes;
    for (let m = 0; m < mu.nodes - 1; m += 1) {
      expect(shadowing[thickest + m * phase.nodes]).toBeGreaterThan(1.95);
    }
  });

  it("keeps the orders' I/F and the ratios in [0, 1)", () => {
    for (const name of ["litOrder2", "litOrder3", "unlitOrder2", "unlitOrder3"] as const) {
      expect(quantity(name).every((value) => value >= 0 && value < 1)).toBe(true);
    }
    const ratioCodes = SHADOWING_CODES.subarray(quantities.length * cells);
    expect(ratioCodes.every((c) => c / codesPerUnit.ratios < 1)).toBe(true);
  });
});
