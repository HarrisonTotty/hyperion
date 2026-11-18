/**
 * A ring's radial profile (plan R11, R11.T9.a; Design note 13): its normal optical depth and its
 * particles' albedo across the annulus, provisional until plan 14 sends a profile of its own.
 *
 * @remarks
 * Plan 14 sends one optical depth for a ring and the radii of the gaps its moons clear (`RingDto`).
 * Until it sends a radial profile, the ring is that τ across the annulus, and each gap is a band
 * that starts at the gap's radius, the resonance being the gap's inner edge, as Mimas's 2:1 is the
 * Cassini Division's, and runs outward by {@link PROVISIONAL_GAP_WIDTH_FRACTION} of that radius, at
 * a tenth of the ring's τ and of darker, less red material. Every band of the profile is a constant
 * τ, so the profile is exact at any radius ({@link profileBandAt}); the slab samples it as a
 * texture at the mip of the pixel's footprint (R11.T9.c). Pure, `f64`, no state.
 *
 * _Slice:_ plan 14's radial profile (the asks of this plan's Consumes) removes the provisional
 * constants and {@link ringProfile}'s bands with them.
 */
import type { RingKindDto, RingMaterialDto } from "@hyperion/protocol";

import type { Rgb } from "../photometry/toneCurve";

/**
 * A provisional gap's width as a share of its radius: 0.039, the Cassini Division's 4,590 km at the
 * B ring's outer edge of 117,580 km (Design note 13). Provisional: plan 14's profile removes it.
 */
export const PROVISIONAL_GAP_WIDTH_FRACTION = 0.039;

/**
 * A provisional gap's optical depth as a share of its ring's: 0.1 (Design note 13). The Cassini
 * Division holds τ ≈ 0.05–0.12 against the B ring's 1–5, so a gap is not empty. Provisional.
 */
export const PROVISIONAL_GAP_TAU_SHARE = 0.1;

/**
 * The floor of a provisional gap's optical depth: 0.05, the Division's least (Design note 13),
 * never above its ring's own τ. Provisional.
 */
export const PROVISIONAL_GAP_TAU_FLOOR = 0.05;

/**
 * The optical depth below which a band is drawn as a C-ring-like sparse region rather than an A- or
 * B-like dense one, for its opposition surge (Design note 14): 0.3, between the C ring's 0.05–0.2
 * and the A ring's 0.4–1 (Cuzzi et al. 2009, "Saturn from Cassini–Huygens", ch. 15; from memory).
 * This module's lean (2026-11-02).
 */
export const SPARSE_BELOW_TAU = 0.3;

/**
 * What a band of a ring is like for its photometry: `dense`, A- and B-ring-like; `sparse`, C-ring
 * like; `gap`, Cassini-Division-like; `dusty`, a tenuous sheet of dust, which scatters forward.
 */
export type RingRegion = "dense" | "sparse" | "gap" | "dusty";

/**
 * The particles' single-scattering albedo ϖ₀ per display channel (r, g, b) by material, a lean of
 * this module (2026-11-02): porous ice reddish and bright, after the B ring's particle Bond albedos
 * of 0.57 at 814 nm and 0.21 at 336 nm that Salo and French 2010 (Icarus 210, 785, §5.2) fit, with
 * the rings' spectrum steep below 550 nm and flat above 650 nm (Cuzzi et al. 2009, ch. 15; from
 * memory); rock dark and nearly grey, of the order of the dark rings of Uranus and Jupiter.
 */
export const RING_PARTICLE_ALBEDO: Readonly<Record<RingMaterialDto, Rgb>> = {
  porous_ice: [0.56, 0.5, 0.4],
  rock: [0.2, 0.18, 0.16],
};

/**
 * A provisional gap's particles, darker and less red than its ring's (Design note 13): the ring's
 * albedo halved towards its mean, times {@link GAP_ALBEDO_SHARE}, as Saturn's C ring and Cassini
 * Division are darker and more neutral than the A and B rings (Cuzzi et al. 2009; from memory).
 */
export const GAP_ALBEDO_SHARE = 0.6;

/** A ring as its profile reads it: the view's ring (`ViewRing`), or plan 14's `RingDto` mapped. */
export interface RingShape {
  readonly ringKind: RingKindDto;
  readonly material: RingMaterialDto;
  /** The inner edge's radius from the planet's centre, m. */
  readonly innerRadiusM: number;
  /** The outer edge's radius, m, above the inner. */
  readonly outerRadiusM: number;
  /** The ring's normal optical depth, positive. */
  readonly opticalDepth: number;
  /** Each gap's radius from the planet's centre, m: where its band starts. */
  readonly gapRadiiM: ReadonlyArray<number>;
}

/** One band of a profile: a constant optical depth and albedo between two radii. */
export interface RingBand {
  /** Its inner radius, m. */
  readonly innerM: number;
  /** Its outer radius, m, above the inner. */
  readonly outerM: number;
  /** Its normal optical depth. */
  readonly tau: number;
  /** Its particles' ϖ₀ per channel. */
  readonly albedo: Rgb;
  readonly region: RingRegion;
}

/** A ring's radial profile: its bands, inside out, contiguous from its inner edge to its outer. */
export interface RingProfile {
  readonly innerM: number;
  readonly outerM: number;
  readonly bands: ReadonlyArray<RingBand>;
}

/** The region of a band of optical depth `tau` of a ring of kind `kind` outside a gap. */
export function regionOf(kind: RingKindDto, tau: number): RingRegion {
  let region: RingRegion;
  switch (kind) {
    case "dusty":
      region = "dusty";
      break;
    case "massive":
      region = tau < SPARSE_BELOW_TAU ? "sparse" : "dense";
      break;
  }
  return region;
}

/** A provisional gap's optical depth in a ring of `tau`: a tenth of it, floored at 0.05, at most it. */
export function provisionalGapTau(tau: number): number {
  return Math.min(tau, Math.max(PROVISIONAL_GAP_TAU_SHARE * tau, PROVISIONAL_GAP_TAU_FLOOR));
}

/** A provisional gap's albedo: its ring's halved towards the ring's mean, then darkened. */
export function provisionalGapAlbedo(albedo: Rgb): Rgb {
  const mean = (albedo[0] + albedo[1] + albedo[2]) / 3;
  const toward = (x: number): number => GAP_ALBEDO_SHARE * (0.5 * x + 0.5 * mean);
  return [toward(albedo[0]), toward(albedo[1]), toward(albedo[2])];
}

/**
 * A ring's provisional profile (Design note 13): its τ across the annulus, each gap a band from its
 * radius outward by 0.039 of it at {@link provisionalGapTau}, held inside the ring; gaps that
 * overlap merge into one band. A ring of no width, whose edges the wire lets meet, has no band.
 *
 * @throws RangeError for a ring whose edges are not positive and in order or whose τ is not
 *   positive and finite: the wire's checks (`bodiesWire.ts`) refuse such a ring, so it is a bug.
 */
export function ringProfile(ring: RingShape): RingProfile {
  const { innerRadiusM: innerM, outerRadiusM: outerM, opticalDepth: tau } = ring;
  if (!(innerM > 0 && outerM >= innerM && Number.isFinite(outerM))) {
    throw new RangeError(`a ring's edges must be positive and in order: ${innerM}, ${outerM}`);
  }
  if (!(tau > 0 && Number.isFinite(tau))) {
    throw new RangeError(`a ring's optical depth must be positive and finite: ${tau}`);
  }
  const albedo = RING_PARTICLE_ALBEDO[ring.material];
  const ringBand = (from: number, to: number): RingBand => ({
    innerM: from,
    outerM: to,
    tau,
    albedo,
    region: regionOf(ring.ringKind, tau),
  });
  // Each gap's span inside the ring, merged where they overlap, inside out.
  const spans: Array<[number, number]> = [];
  for (const radiusM of ring.gapRadiiM.toSorted((a, b) => a - b)) {
    const from = Math.max(radiusM, innerM);
    const to = Math.min(radiusM * (1 + PROVISIONAL_GAP_WIDTH_FRACTION), outerM);
    if (!(to > from)) {
      continue;
    }
    const last = spans.at(-1);
    if (last !== undefined && from <= last[1]) {
      last[1] = Math.max(last[1], to);
    } else {
      spans.push([from, to]);
    }
  }
  const gapTau = provisionalGapTau(tau);
  const gapAlbedo = provisionalGapAlbedo(albedo);
  const bands: RingBand[] = [];
  let at = innerM;
  for (const [from, to] of spans) {
    if (from > at) {
      bands.push(ringBand(at, from));
    }
    bands.push({ innerM: from, outerM: to, tau: gapTau, albedo: gapAlbedo, region: "gap" });
    at = to;
  }
  if (outerM > at) {
    bands.push(ringBand(at, outerM));
  }
  return { innerM, outerM, bands };
}

/**
 * Whether a ring's profile is the provisional one of {@link ringProfile} (Design note 13), which
 * the view labels as not modelled (`RING PROFILE: NOT YET MODELLED`; plan R11, R11.T9.c): every
 * ring's, since plan 14's `RingDto` sends no radial profile; plan 14's profile, when it is sent,
 * makes it `false` for the rings that carry one.
 */
export function ringProfileProvisional(ring: RingShape): boolean {
  return ring.innerRadiusM > 0;
}

/**
 * The band of `profile` at radius `rM`, or `null` off the annulus. A radius on a boundary takes the
 * outer band, and the outer edge itself none.
 */
export function profileBandAt(profile: RingProfile, rM: number): RingBand | null {
  if (!(rM >= profile.innerM && rM < profile.outerM)) {
    return null;
  }
  // Bands are few (a ring's gaps), so a scan is enough.
  for (const band of profile.bands) {
    if (rM < band.outerM) {
      return band;
    }
  }
  return null;
}

/** The normal optical depth of `profile` at radius `rM`: 0 off the annulus. */
export function profileTauAt(profile: RingProfile, rM: number): number {
  return profileBandAt(profile, rM)?.tau ?? 0;
}
