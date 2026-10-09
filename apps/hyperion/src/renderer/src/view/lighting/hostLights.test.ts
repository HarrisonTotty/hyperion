import type { HostDiscDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import { aHostDisc } from "../../test/litFixtures";
import { PHASE_STAR, phaseScene } from "../scenes/phaseScene";
import { precisionScene } from "../scenes/precision";
import { annulusEdges, DISC_ANNULI_HIGH, DISC_ANNULI_LOW } from "./annuli";
import {
  HOST_ANNULI_KEPT,
  hostAnnuli,
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
  it("says nothing when lit, PENDING while the sky is asked, NOT RECEIVED otherwise", () => {
    const lit = hostLights(phase, phase.hostDiscs ?? []);
    expect(lightingStatement(lightingState(lit, true))).toBeNull();
    expect(lightingStatement(lightingState([], true))).toBe("LIGHTING: PENDING");
    expect(lightingStatement(lightingState([], false))).toBe("LIGHTING: NOT RECEIVED");
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

/** A host whose three channels darken by one power-2 law. */
function hostOf(c: number, alpha: number): HostDiscDto {
  return aHostDisc({
    limb: [
      { c, alpha },
      { c, alpha },
      { c, alpha },
    ],
  });
}

describe("hostAnnuli", () => {
  // The annuli are kept across tests, so each test below uses laws of its own.

  it("makes a host's annuli once for a scene that makes an equal host each frame", () => {
    const frames = [0, 1, 2].map(() => hostAnnuli(aHostDisc({ star: 0 }), DISC_ANNULI_HIGH));
    expect(new Set(frames).size).toBe(1);
  });

  it("makes a host's annuli again when another host has other laws", () => {
    hostAnnuli(hostOf(0.61, 0.7), DISC_ANNULI_HIGH);
    expect(hostAnnuli(hostOf(0.62, 0.7), DISC_ANNULI_HIGH)[0]).toEqual(
      annulusEdges(0.62, 0.7, DISC_ANNULI_HIGH),
    );
  });

  it("makes a host's annuli again when its own laws change in place", () => {
    const host = hostOf(0.63, 0.7);
    hostAnnuli(host, DISC_ANNULI_HIGH);
    // R06's R law, the display's red channel.
    host.limb[2] = { c: 0.64, alpha: 0.7 };
    expect(hostAnnuli(host, DISC_ANNULI_HIGH)[0]).toEqual(
      annulusEdges(0.64, 0.7, DISC_ANNULI_HIGH),
    );
  });

  it("keeps a law's annuli apart for each K", () => {
    const host = hostOf(0.65, 0.7);
    hostAnnuli(host, DISC_ANNULI_HIGH);
    expect(hostAnnuli(host, DISC_ANNULI_LOW)[0].edges).toHaveLength(DISC_ANNULI_LOW + 1);
  });

  it("keeps the annuli of the last HOST_ANNULI_KEPT laws made", () => {
    const first = hostAnnuli(hostOf(0.4, 0.51), DISC_ANNULI_LOW);
    for (let i = 1; i < HOST_ANNULI_KEPT; i += 1) {
      hostAnnuli(hostOf(0.4 + i / 1000, 0.51), DISC_ANNULI_LOW);
    }
    expect(hostAnnuli(hostOf(0.4, 0.51), DISC_ANNULI_LOW)).toBe(first);
  });

  it("drops a law's annuli once HOST_ANNULI_KEPT others are used after it", () => {
    const first = hostAnnuli(hostOf(0.4, 0.52), DISC_ANNULI_LOW);
    for (let i = 1; i <= HOST_ANNULI_KEPT; i += 1) {
      hostAnnuli(hostOf(0.4 + i / 1000, 0.52), DISC_ANNULI_LOW);
    }
    expect(hostAnnuli(hostOf(0.4, 0.52), DISC_ANNULI_LOW)).not.toBe(first);
  });

  it("keeps a law used each frame through HOST_ANNULI_KEPT new ones", () => {
    const first = hostAnnuli(hostOf(0.4, 0.53), DISC_ANNULI_LOW);
    for (let i = 1; i <= HOST_ANNULI_KEPT; i += 1) {
      hostAnnuli(hostOf(0.4 + i / 1000, 0.53), DISC_ANNULI_LOW);
      hostAnnuli(hostOf(0.4, 0.53), DISC_ANNULI_LOW);
    }
    expect(hostAnnuli(hostOf(0.4, 0.53), DISC_ANNULI_LOW)).toBe(first);
  });
});
