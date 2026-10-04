import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { aHostDisc } from "../../test/litFixtures";
import { PHASE_STAR, phaseScene } from "../scenes/phaseScene";
import { precisionScene } from "../scenes/precision";
import {
  hostLights,
  lightingState,
  lightingStatement,
  placeLights,
  sceneHostDiscs,
} from "./hostLights";

const phase = phaseScene().sceneAt(0);

describe("hostLights", () => {
  it("joins each disc to the scene's star of its body index", () => {
    expect(hostLights(phase, [aHostDisc({ star: 0 })])).toEqual([
      { disc: aHostDisc({ star: 0 }), body: PHASE_STAR },
    ]);
  });

  it("leaves out a disc whose star the scene does not hold, and orders by star", () => {
    const discs = [aHostDisc({ star: 7 }), aHostDisc({ star: 0 }), aHostDisc({ star: 2 })];
    expect(hostLights(phase, discs).map((light) => light.disc.star)).toEqual([0]);
  });

  it("does not take a planet for a star", () => {
    expect(hostLights(phase, [aHostDisc({ star: 1 })])).toEqual([]);
  });
});

describe("sceneHostDiscs", () => {
  it("lights a kept scene by its own discs and a server scene by its sky's", () => {
    const sky = [aHostDisc({ star: 3 })];
    expect(sceneHostDiscs(phase, sky)).toBe(phase.hostDiscs);
    expect(sceneHostDiscs(precisionScene().sceneAt(0), sky)).toEqual([]);
    const server = { ...phase, provenance: { kind: "server" as const } };
    expect(sceneHostDiscs(server, sky)).toBe(sky);
    expect(sceneHostDiscs(server, null)).toEqual([]);
  });
});

describe("the lighting statement", () => {
  it("says nothing when lit, PENDING while the sky is asked, STAR DISCS NOT RECEIVED otherwise", () => {
    const lit = hostLights(phase, phase.hostDiscs ?? []);
    expect(lightingStatement(lightingState(lit, true))).toBeNull();
    expect(lightingStatement(lightingState([], true))).toBe("LIGHTING: PENDING");
    expect(lightingStatement(lightingState([], false))).toBe("LIGHTING: STAR DISCS NOT RECEIVED");
  });
});

describe("placeLights", () => {
  it("places each light at its star's centre in the frame, dropping one the frame lacks", () => {
    const lights = hostLights(phase, phase.hostDiscs ?? []);
    const centre = vec3(1, 2, 3);
    expect(placeLights(lights, () => centre)).toEqual([{ disc: lights[0]?.disc, centreM: centre }]);
    expect(placeLights(lights, () => null)).toEqual([]);
  });
});
