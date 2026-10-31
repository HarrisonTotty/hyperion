import type {
  CrustLidDto,
  DestructionCauseDto,
  RedoxClassDto,
  SurfaceStateDto,
} from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import {
  crustLidLabel,
  destructionCauseLabel,
  envelopeLabel,
  redoxClassLabel,
  surfaceStateLabel,
} from "./bodyWords";

describe("envelopeLabel", () => {
  // An H₂/He envelope's null, the steam envelope's water, other condensables' formulas and a name
  // the list does not hold, whose key never reaches the screen.
  it.each<readonly [string | null, string]>([
    [null, "GAS ENVELOPE"],
    ["H2O", "SUPERCRITICAL H2O"],
    ["CH4", "SUPERCRITICAL CH4"],
    ["O2", "SUPERCRITICAL O2"],
    ["future_fluid", "SUPERCRITICAL UNLISTED MATERIAL"],
  ])("names a supercritical key of %s as %s", (supercritical, word) => {
    expect(envelopeLabel(supercritical)).toBe(word);
  });
});

describe("surfaceStateLabel", () => {
  // Every surface state the wire carries, the two no rule produces yet included.
  it.each<readonly [SurfaceStateDto, string]>([
    ["magma_ocean", "MAGMA OCEAN"],
    ["airless", "AIRLESS"],
    ["runaway_greenhouse", "RUNAWAY GREENHOUSE"],
    ["temperate", "TEMPERATE"],
    ["snowball", "SNOWBALL"],
    ["steam_envelope", "SUPERCRITICAL H2O"],
    ["tenuous", "TENUOUS ATMOSPHERE"],
  ])("names %s as %s", (state, word) => {
    expect(surfaceStateLabel(state)).toBe(word);
  });
});

describe("destructionCauseLabel", () => {
  it("names every cause the wire carries", () => {
    const causes: [DestructionCauseDto, string][] = [
      ["dispersed", "DISPERSED"],
      ["engulfed", "ENGULFED"],
      ["tidally_disrupted", "TIDALLY DISRUPTED"],
      ["collided", "COLLIDED"],
    ];
    for (const [cause, word] of causes) {
      expect(destructionCauseLabel(cause)).toBe(word);
    }
  });
});

describe("crustLidLabel", () => {
  // Every lid the wire carries; `bulk` never as `BULK`, the record's own section.
  it.each<readonly [CrustLidDto, string]>([
    ["primary", "PRIMARY"],
    ["secondary", "SECONDARY"],
    ["bulk", "UNDIFFERENTIATED"],
  ])("names %s as %s", (lid, word) => {
    expect(crustLidLabel(lid)).toBe(word);
  });
});

describe("redoxClassLabel", () => {
  // Every redox class the wire carries.
  it.each<readonly [RedoxClassDto, string]>([
    ["reduced", "REDUCED"],
    ["intermediate", "INTERMEDIATE"],
    ["oxidised", "OXIDISED"],
  ])("names %s as %s", (redox, word) => {
    expect(redoxClassLabel(redox)).toBe(word);
  });
});
