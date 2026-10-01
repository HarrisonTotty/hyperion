import { describe, expect, it, vi } from "vitest";

import { FakeAdapter, INTEL_UHD_620_INFO } from "../../../test/fakeGpu";
import { summariseAdapter } from "../platform";
import {
  classifyRounding,
  firstTexel,
  halfToNumber,
  PROBE_VALUES,
  probeTargetRounding,
  probedFormats,
  unpackRg11b10,
} from "./roundingProbe";

/** The bits of an `rg11b10ufloat` texel from its three channels' bits. */
function packRg11b10(red: number, green: number, blue: number): number {
  return (red | (green << 11) | (blue << 22)) >>> 0;
}

/** A small float of 1 + `mantissa` steps, exponent 15 (value 1), in `mantissaBits`. */
function oneAndSteps(mantissa: number, mantissaBits: number): number {
  return (15 << mantissaBits) | mantissa;
}

/** The half float `steps` steps above 1. */
function half(steps: number): number {
  return halfToNumber((15 << 10) | steps);
}

describe("the rounding classifier on rgba16float", () => {
  it("reads the upper neighbour as nearest", () => {
    expect(classifyRounding("rgba16float", [half(1), half(1), half(1)])).toBe("nearest");
  });

  it("reads the lower neighbour as toward-zero", () => {
    expect(classifyRounding("rgba16float", [half(0), half(0), half(0)])).toBe("toward-zero");
  });

  it("reads anything else as unknown", () => {
    expect(classifyRounding("rgba16float", [half(1), half(0), half(1)])).toBe("unknown");
    expect(classifyRounding("rgba16float", [0, 0, 0])).toBe("unknown");
  });

  it("decodes a read-back's first texel", () => {
    const bytes = new Uint16Array([(15 << 10) | 1, (15 << 10) | 1, (15 << 10) | 1, 15 << 10])
      .buffer;
    expect(classifyRounding("rgba16float", firstTexel("rgba16float", bytes))).toBe("nearest");
  });
});

describe("the rounding classifier on rg11b10ufloat", () => {
  it("reads the upper neighbours as nearest", () => {
    const bits = packRg11b10(oneAndSteps(1, 6), oneAndSteps(1, 6), oneAndSteps(1, 5));
    expect(classifyRounding("rg11b10ufloat", unpackRg11b10(bits))).toBe("nearest");
  });

  it("reads the lower neighbours as toward-zero", () => {
    const bits = packRg11b10(oneAndSteps(0, 6), oneAndSteps(0, 6), oneAndSteps(0, 5));
    expect(classifyRounding("rg11b10ufloat", unpackRg11b10(bits))).toBe("toward-zero");
  });

  it("is unknown without rg11b10Renderable, which leaves the format unprobed", () => {
    const capabilities = summariseAdapter(
      new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] }),
    ).capabilities;
    expect(probedFormats(capabilities)).toEqual(["rgba16float"]);
    expect(probedFormats({ ...capabilities, rg11b10Renderable: true })).toEqual([
      "rgba16float",
      "rg11b10ufloat",
    ]);
  });
});

describe("the probe's values", () => {
  it("sit three quarters of a step above 1", () => {
    expect(PROBE_VALUES.rgba16float[0]).toBe(1 + 0.75 / 1024);
    expect(PROBE_VALUES.rg11b10ufloat).toEqual([1 + 0.75 / 64, 1 + 0.75 / 64, 1 + 0.75 / 32]);
  });
});

describe("the probe's run", () => {
  it("gives unknown, and no fault, for a read that fails", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const rounding = await probeTargetRounding(["rgba16float"], () =>
      Promise.reject(new Error("device lost")),
    );
    expect(rounding).toEqual({ rgba16float: "unknown", rg11b10ufloat: "unknown" });
  });

  it("classes each format it reads", async () => {
    const nearest = new Uint16Array([(15 << 10) | 1, (15 << 10) | 1, (15 << 10) | 1, 0]).buffer;
    const rounding = await probeTargetRounding(["rgba16float"], () => Promise.resolve(nearest));
    expect(rounding.rgba16float).toBe("nearest");
    // Not asked for, as without rg11b10Renderable: unknown.
    expect(rounding.rg11b10ufloat).toBe("unknown");
  });
});
