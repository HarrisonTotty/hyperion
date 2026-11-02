import type {
  CrustLidDto,
  DestructionCauseDto,
  HabitabilityClassDto,
  RedoxClassDto,
} from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import {
  crustLidLabel,
  destructionCauseLabel,
  envelopeLabel,
  habitabilityClassLabel,
  habitabilityLimitLabel,
  redoxClassLabel,
  surfaceStateLabel,
} from "./bodyWords";
import type { HabitabilityLimit, SurfaceState } from "./model";

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
  // Every surface state the wire carries, and one this build does not know, which reads Missing.
  it.each<readonly [SurfaceState, string | null]>([
    ["magma_ocean", "MAGMA OCEAN"],
    ["airless", "AIRLESS"],
    ["runaway_greenhouse", "RUNAWAY GREENHOUSE"],
    ["temperate", "TEMPERATE"],
    ["snowball", "SNOWBALL"],
    ["steam_envelope", "SUPERCRITICAL H2O"],
    ["tenuous", "TENUOUS ATMOSPHERE"],
    ["desert", "DESERT"],
    ["unknown", null],
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

describe("habitabilityClassLabel", () => {
  it.each<readonly [HabitabilityClassDto, string]>([
    ["hostile", "HOSTILE"],
    ["subsurface_ocean", "SUBSURFACE OCEAN"],
    ["marginal", "MARGINAL"],
    ["temperate", "TEMPERATE"],
  ])("names %s as %s", (habitabilityClass, word) => {
    expect(habitabilityClassLabel(habitabilityClass)).toBe(word);
  });
});

describe("habitabilityLimitLabel", () => {
  // Every limit the wire carries, the plan's thirteen and P14.T25's four, and one this build does
  // not know, which reads Missing.
  it.each<readonly [HabitabilityLimit, string | null]>([
    ["outside_optimistic_zone", "OUTSIDE OPTIMISTIC ZONE"],
    ["outside_conservative_zone", "OUTSIDE CONSERVATIVE ZONE"],
    ["no_atmosphere", "NO ATMOSPHERE"],
    ["below_triple_point", "BELOW H2O TRIPLE POINT"],
    ["runaway_greenhouse", "RUNAWAY GREENHOUSE"],
    ["snowball", "SNOWBALL"],
    ["gas_envelope", "GAS ENVELOPE"],
    ["tidally_locked", "TIDALLY LOCKED"],
    ["high_eccentricity", "HIGH ECCENTRICITY"],
    ["active_host", "ACTIVE HOST"],
    ["young_host", "YOUNG HOST"],
    ["leaving_main_sequence", "HOST LEAVING MAIN SEQUENCE"],
    ["extrapolated_zone", "EXTRAPOLATED ZONE"],
    ["molten_surface", "MOLTEN SURFACE"],
    ["dry_surface", "DRY SURFACE"],
    ["mass_outside_temperate_range", "MASS OUTSIDE TEMPERATE RANGE"],
    ["frozen_surface", "FROZEN SURFACE"],
    ["unknown", null],
  ])("names %s as %s", (limit, word) => {
    expect(habitabilityLimitLabel(limit)).toBe(word);
  });
});
