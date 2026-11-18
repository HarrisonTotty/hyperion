/**
 * Design note 6's lower bound on the GPU process's resident memory (research-leans-54, ruling 3),
 * on synthetic clients: a buffer held by several clients, one shared only outside the set, and the
 * two-client case the earlier form already counted right.
 */
import { describe, expect, it } from "vitest";

import type { DrmClient } from "../fdinfo";
import { drmDriversOf, drmSampleOf, residentLowerBoundOf } from "./samplers";

const MIB = 1024 ** 2;
const I915 = drmDriversOf(0x80_86);

/** An i915 client holding `privateBytes` of its own and `sharedBytes` it shares, in `system0`. */
function client(id: string, privateBytes: number, sharedBytes: number): DrmClient {
  return {
    driver: "i915",
    pdev: "0000:00:02.0",
    clientId: id,
    totalBytes: { system0: privateBytes + sharedBytes },
    residentBytes: { system0: privateBytes + sharedBytes },
    sharedBytes: { system0: sharedBytes },
    engineNs: {},
  };
}

describe("the resident lower bound", () => {
  const b = 64 * MIB;
  const p = 10 * MIB;

  it("counts a buffer three clients hold once: P + B, where the half-shared form gave P + 1.5 B", () => {
    const clients = [client("40", p, b), client("41", 0, b), client("42", 0, b)];
    expect(residentLowerBoundOf(clients)).toBe(p + b);
    expect(drmSampleOf(clients, I915, 0).value?.residentLowerBoundBytes).toBe(p + b);
  });

  it("counts a buffer shared with Xorg alone in full, its truth", () => {
    expect(residentLowerBoundOf([client("41", p, b)])).toBe(p + b);
  });

  it("keeps the two-client case: one buffer both hold, counted once, as the earlier form did", () => {
    const clients = [client("40", p, b), client("41", 2 * p, b)];
    const earlier = 3 * p + 2 * b - (2 * b) / 2;
    expect(residentLowerBoundOf(clients)).toBe(earlier);
  });

  it("sums the bound over regions, each its own", () => {
    const stolen: DrmClient = {
      ...client("40", p, b),
      residentBytes: { system0: p + b, "stolen-system0": 4 * MIB },
      sharedBytes: { system0: b, "stolen-system0": 0 },
    };
    expect(residentLowerBoundOf([stolen, client("41", 0, b)])).toBe(p + b + 4 * MIB);
  });
});
