import type { CatalogueClassWire, MemberClassWire, SystemOriginWire } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import {
  aBlackHoleState,
  aCentreDetail,
  aCloudDetail,
  aClusterDetail,
  OPEN_CLUSTER,
} from "../../test/featureFixtures";
import { anUnknownLayer, UNIVERSE_ID } from "../../test/galaxyFixtures";
import {
  toBlackHole,
  toBlackHoleRequest,
  toCatalogueClass,
  toFeatureDetail,
  toFeatureDetailRequest,
  toMemberClass,
  toSupernovaState,
  toSystemOrigin,
} from "./featureWire";

/** An origin the decoder hands on as one, though this build does not know it. */
function decodedOrigin(value: unknown): SystemOriginWire {
  // The decoder's own trust at the wire (`decodeServerMessage`), made where the test plays the
  // server.
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion
  return value as SystemOriginWire;
}

/** A catalogue class the decoder hands on as one, though this build does not know it. */
function decodedClass(value: unknown): CatalogueClassWire {
  // The decoder's own trust at the wire, made where the test plays the server.
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion
  return value as CatalogueClassWire;
}

/** A member class the decoder hands on as one, though this build does not know it. */
function decodedMemberClass(value: unknown): MemberClassWire {
  // The decoder's own trust at the wire, made where the test plays the server.
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion
  return value as MemberClassWire;
}

describe("toSystemOrigin (plan 09, P09.T41.c)", () => {
  it("reads a row without an origin as one the server does not send", () => {
    expect(toSystemOrigin(undefined)).toEqual({ kind: "not_sent" });
  });

  it.each<[SystemOriginWire, unknown]>([
    [{ type: "grid" }, { kind: "grid" }],
    [
      { type: "feature_member", feature: OPEN_CLUSTER.id },
      { kind: "feature_member", feature: OPEN_CLUSTER.id },
    ],
    [{ type: "centre_member" }, { kind: "centre_member" }],
    [
      { type: "catalogue_system", class: "xray_binary" },
      { kind: "catalogue_system", catalogueClass: "xray_binary" },
    ],
  ])("reads %o", (origin, model) => {
    expect(toSystemOrigin(origin)).toEqual(model);
  });

  it.each<[SystemOriginWire, unknown]>([
    [
      { type: "stream", stream: 3 },
      { kind: "stream", stream: 3 },
    ],
    [
      { type: "dwarf_core", core: 0 },
      { kind: "dwarf_core", core: 0 },
    ],
  ])("reads plan 10's member %o by its entry's number (P10.T11)", (origin, model) => {
    expect(toSystemOrigin(origin)).toEqual(model);
  });

  it("reads an origin it does not know, a pinned system's, as unknown", () => {
    expect(toSystemOrigin(decodedOrigin({ type: "pinned" }))).toEqual({
      kind: "unknown",
    });
  });

  it("reads a catalogue class it does not know as unknown, keeping the origin", () => {
    expect(
      toSystemOrigin({
        type: "catalogue_system",
        class: decodedClass("magnetar"),
      }),
    ).toEqual({ kind: "catalogue_system", catalogueClass: "unknown" });
    expect(toCatalogueClass(decodedClass("toString"))).toBe("unknown");
  });
});

describe("toMemberClass (plan 09, P09.T41.c)", () => {
  it("reads a cluster's, the centre's and a listed entry's class", () => {
    expect(
      toMemberClass({
        type: "cluster",
        kind: "tail",
        band: "b",
        generation: "second",
        multiplicity: "single",
      }),
    ).toEqual({
      kind: "cluster",
      classKind: "tail",
      band: "b",
      generation: "second",
      multiplicity: "single",
    });
    expect(
      toMemberClass({
        type: "centre",
        kind: "living",
        band: "a",
        episode: 4,
        tracer: "young_disc",
      }),
    ).toEqual({ kind: "centre", classKind: "living", band: "a", episode: 4, tracer: "young_disc" });
    expect(toMemberClass({ type: "listed", class: "tidal_disruption_victim" })).toEqual({
      kind: "listed",
      catalogueClass: "tidal_disruption_victim",
    });
  });

  it("reads a class it does not know as unknown, and a listed magnetar's class as unknown", () => {
    expect(toMemberClass(decodedMemberClass({ type: "stream", arm: "leading" }))).toEqual({
      kind: "unknown",
    });
    expect(toMemberClass({ type: "listed", class: decodedClass("magnetar") })).toEqual({
      kind: "listed",
      catalogueClass: "unknown",
    });
  });

  it("reads a band's layer it does not know as unknown, keeping the class", () => {
    expect(
      toMemberClass({
        type: "cluster",
        kind: "living",
        band: anUnknownLayer(),
        generation: "first",
        multiplicity: "binary",
      }),
    ).toEqual({
      kind: "cluster",
      classKind: "living",
      band: "unknown",
      generation: "first",
      multiplicity: "binary",
    });
    expect(
      toMemberClass({
        type: "centre",
        kind: "living",
        band: anUnknownLayer(),
        episode: 0,
        tracer: "stars",
      }),
    ).toEqual({
      kind: "centre",
      classKind: "living",
      band: "unknown",
      episode: 0,
      tracer: "stars",
    });
  });
});

describe("toSupernovaState (plan 09, P09.T41.c)", () => {
  it("reads a bare remnant and what it left", () => {
    expect(
      toSupernovaState({
        type: "bare_remnant",
        age_yr: 120_000,
        remnant: { offset_ly: null, bow_shock: false, nebula: null, survivor_offset_ly: 41 },
      }),
    ).toEqual({
      kind: "bare_remnant",
      ageYr: 120_000,
      remnant: { offsetLy: null, bowShock: false, nebulaPowerW: null, survivorOffsetLy: 41 },
    });
  });
});

describe("toFeatureDetail (plan 09, P09.T41.c)", () => {
  it("reads the golden's cluster at +100 yr, its classes among its structure", () => {
    const detail = toFeatureDetail(aClusterDetail());

    expect(detail.id).toBe(OPEN_CLUSTER.id);
    expect(detail.timeYr).toBe(100);
    expect(detail.structure).toMatchObject({
      kind: "cluster",
      tidalRadiusLy: 26.909_037_649_658_29,
      coreCollapsed: false,
      blackHoles: 0,
    });
    expect(detail.structure.kind === "cluster" ? detail.structure.classes : []).toHaveLength(2);
  });

  it("reads the golden's cloud and centre", () => {
    expect(toFeatureDetail(aCloudDetail()).structure).toEqual({
      kind: "cloud",
      coreRadiusLy: 15.779_983_166_698_207,
      centralDensityPerCm3: 1_073.989_208_075_869,
    });
    expect(toFeatureDetail(aCentreDetail()).structure).toMatchObject({
      kind: "centre",
      lossConeRadiusLy: 0.000_085_231_461_947_658_41,
    });
  });

  it("asks for a feature's detail at the chart's time", () => {
    expect(toFeatureDetailRequest(UNIVERSE_ID, OPEN_CLUSTER.id, 100)).toEqual({
      kind: "feature_detail",
      universe: UNIVERSE_ID,
      feature: OPEN_CLUSTER.id,
      time: { seconds: 3_155_760_000, nanos: 0 },
    });
  });
});

describe("toBlackHole (plan 09, P09.T41.c)", () => {
  it("reads the golden's black hole, its total luminosity and its disruptions, newest first", () => {
    const blackHole = toBlackHole(aBlackHoleState());

    expect(blackHole.massMsun).toBe(84_173_629.874_983_71);
    expect(blackHole.luminosityW).toBe(1.689_609_449_323_589_3e28);
    expect(blackHole.disruptions.map((each) => each.victim)).toEqual([
      "f000000700000009",
      "f000000700000008",
    ]);
    expect(blackHole.disruptions[0]?.timeYr).toBeCloseTo(-1_372_618_232_176.856 / 31_557_600, 6);
  });

  it("reads a luminosity the server leaves out as unknown", () => {
    const { luminosity: _, ...withoutLuminosity } = aBlackHoleState();

    expect(toBlackHole(withoutLuminosity).luminosityW).toBeNull();
  });

  it("asks for the black hole at the chart's time", () => {
    expect(toBlackHoleRequest(UNIVERSE_ID, 0)).toEqual({
      kind: "black_hole_state",
      universe: UNIVERSE_ID,
      time: { seconds: 0, nanos: 0 },
    });
  });
});
