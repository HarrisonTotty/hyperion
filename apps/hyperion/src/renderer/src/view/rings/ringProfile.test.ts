import type { RingDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import {
  PROVISIONAL_GAP_TAU_FLOOR,
  PROVISIONAL_GAP_WIDTH_FRACTION,
  profileBandAt,
  profileTauAt,
  provisionalGapAlbedo,
  provisionalGapTau,
  RING_PARTICLE_ALBEDO,
  ringProfile,
  type RingShape,
} from "./ringProfile";

/** A Saturn-like massive ring as plan 14 sends it: τ 1.5, Mimas's 2:1 and 3:2, Janus's 2:1. */
const FIXTURE: RingDto = {
  ring_kind: "massive",
  material: "porous_ice",
  inner_edge_m: 74.66e6,
  outer_edge_m: 136.78e6,
  optical_depth: 1.5,
  gaps: [
    { moon: "0200080020000000.0110", resonance: [2, 1], radius_m: 117.58e6 },
    { moon: "0200080020000000.0110", resonance: [3, 2], radius_m: 142.0e6 },
    { moon: "0200080020000000.0111", resonance: [2, 1], radius_m: 96.0e6 },
  ],
};

/** The fixture as the view's ring carries it (`fromServer.ts`' mapping). */
function shapeOf(dto: RingDto): RingShape {
  return {
    ringKind: dto.ring_kind,
    material: dto.material,
    innerRadiusM: dto.inner_edge_m,
    outerRadiusM: dto.outer_edge_m,
    opticalDepth: dto.optical_depth,
    gapRadiiM: dto.gaps.map((gap) => gap.radius_m),
  };
}

describe("a ring's provisional profile", () => {
  it("names its gap width provisional: 0.039 of the radius, the Cassini Division's", () => {
    expect(PROVISIONAL_GAP_WIDTH_FRACTION).toBe(0.039);
    expect(4_590e3 / 117_580e3).toBeCloseTo(PROVISIONAL_GAP_WIDTH_FRACTION, 3);
  });

  it("makes each gap inside the ring a band from its radius, 0.039 of it wide, at 0.1 of τ", () => {
    const profile = ringProfile(shapeOf(FIXTURE));
    const gaps = profile.bands.filter((band) => band.region === "gap");
    // The 3:2 resonance at 142,000 km lies beyond the outer edge, and draws nothing.
    expect(gaps.map((band) => band.innerM)).toEqual([96.0e6, 117.58e6]);
    for (const band of gaps) {
      expect(band.outerM - band.innerM).toBeCloseTo(0.039 * band.innerM, 3);
      expect(band.tau).toBeCloseTo(0.1 * FIXTURE.optical_depth, 15);
    }
  });

  it("covers the annulus with contiguous bands, the ring's τ between the gaps", () => {
    const profile = ringProfile(shapeOf(FIXTURE));
    expect(profile.bands[0]?.innerM).toBe(FIXTURE.inner_edge_m);
    expect(profile.bands.at(-1)?.outerM).toBe(FIXTURE.outer_edge_m);
    profile.bands.slice(1).forEach((band, i) => {
      expect(band.innerM).toBe(profile.bands[i]?.outerM);
    });
    expect(profile.bands.map((band) => band.region)).toEqual([
      "dense",
      "gap",
      "dense",
      "gap",
      "dense",
    ]);
    expect(profileTauAt(profile, 80e6)).toBe(1.5);
    expect(profileTauAt(profile, 118e6)).toBeCloseTo(0.15, 15);
    expect(profileTauAt(profile, 140e6)).toBe(0);
    expect(profileTauAt(profile, 70e6)).toBe(0);
  });

  it("merges gaps that overlap and clips one at the outer edge", () => {
    const profile = ringProfile({
      ...shapeOf(FIXTURE),
      gapRadiiM: [100e6, 101e6, 135e6],
    });
    const gaps = profile.bands.filter((band) => band.region === "gap");
    expect(gaps.map((band) => [band.innerM, band.outerM])).toEqual([
      [100e6, 101e6 * 1.039],
      [135e6, FIXTURE.outer_edge_m],
    ]);
  });

  it("floors a gap's τ at 0.05 but never above its ring's", () => {
    expect(provisionalGapTau(0.2)).toBe(PROVISIONAL_GAP_TAU_FLOOR);
    expect(provisionalGapTau(0.03)).toBe(0.03);
    expect(provisionalGapTau(3)).toBeCloseTo(0.3, 15);
  });

  it("gives a gap darker, less red material than its ring", () => {
    const ring = RING_PARTICLE_ALBEDO.porous_ice;
    const gap = provisionalGapAlbedo(ring);
    expect(gap[0]).toBeLessThan(ring[0]);
    expect(gap[0] / gap[2]).toBeLessThan(ring[0] / ring[2]);
  });

  it("draws a sparse massive ring as C-ring-like and a dusty ring as dust", () => {
    const sparse = ringProfile({ ...shapeOf(FIXTURE), opticalDepth: 0.1, gapRadiiM: [] });
    expect(sparse.bands.map((band) => band.region)).toEqual(["sparse"]);
    const dusty = ringProfile({
      ...shapeOf(FIXTURE),
      ringKind: "dusty",
      material: "rock",
      opticalDepth: 3e-6,
      gapRadiiM: [],
    });
    expect(dusty.bands.map((band) => band.region)).toEqual(["dusty"]);
  });

  it("puts a boundary radius in the outer band and the outer edge in none", () => {
    const profile = ringProfile(shapeOf(FIXTURE));
    expect(profileBandAt(profile, 96.0e6)?.region).toBe("gap");
    expect(profileBandAt(profile, FIXTURE.outer_edge_m)).toBeNull();
  });

  it("gives a ring of no width, whose edges the wire lets meet, no band", () => {
    const edge = FIXTURE.inner_edge_m;
    expect(ringProfile({ ...shapeOf(FIXTURE), outerRadiusM: edge }).bands).toEqual([]);
  });

  it("refuses a ring the wire would have refused", () => {
    expect(() => ringProfile({ ...shapeOf(FIXTURE), opticalDepth: 0 })).toThrow(RangeError);
    expect(() => ringProfile({ ...shapeOf(FIXTURE), outerRadiusM: 1e6 })).toThrow(RangeError);
  });
});
