/**
 * Both analytic shadows of a ringed planet (plan R11, R11.T9.a; Design note 14): the planet's on its
 * ring, a ray against the sphere with the penumbra of R07's eclipse term across R06's disc, and the
 * ring's on the planet, a ray against the ring plane reading the profile's transmittance.
 *
 * @remarks
 * Neither reads depth. The ring's shadow on the planet is `ringShadow.wgsl`'s `ring_shadow_on_body`,
 * which R07's disc and mesh regimes call for a ringed body (`bodyDisc.wgsl`, `discAir.wgsl`) where
 * R11's `ringShadowOnBody` is on, from the ring's rows in the body's disc record
 * ({@link packRingShadowRows}); {@link ringShadowOnBody} is its twin. The planet's shadow on the
 * ring is the slab's (R11.T9.c), whose twin {@link planetShadowOnRing} is. Pure, `f64`, no state.
 */
import { add, dot, norm, scale, sub, type Vec3 } from "../../geometry/vec3";
import { eclipseVisible, type LimbDarkenedDisc } from "../lighting/annuli";
import type { RingProfile } from "./ringProfile";

/**
 * The bands a ring's shadow on its planet reads at most: 16, a ring and seven gaps. A profile of
 * more keeps its first 15 and gives the rest of the annulus its densest band's τ, the ring's own
 * rather than a narrow gap's ({@link ringShadowRecordOf}).
 */
export const MAX_RING_SHADOW_BANDS = 16;

/**
 * `vec4f` rows of a ring's shadow in a disc record (`ringShadow.wgsl`'s `RING_SHADOW_ROWS`): the
 * plane's normal and the inner edge, the band count, then two bands a row.
 */
export const RING_SHADOW_ROWS = 2 + MAX_RING_SHADOW_BANDS / 2;

/** One band of a ring as its shadow reads it, out from the previous band's outer edge. */
export interface RingShadowBand {
  /** Its outer edge's radius over the planet's equatorial radius a. */
  readonly outerOverA: number;
  /** Its normal optical depth. */
  readonly tau: number;
}

/**
 * A ring as its shadow on its planet reads it, every length over the planet's equatorial radius a,
 * so that no large number meets another in `f32` (R07 Design note 19).
 */
export interface RingShadowRecord {
  /** The ring plane's unit normal, along the axes the record is packed in. */
  readonly normal: Vec3;
  /** The inner edge's radius over a. */
  readonly innerOverA: number;
  /** The bands, inside out, contiguous from the inner edge; at most {@link MAX_RING_SHADOW_BANDS}. */
  readonly bands: ReadonlyArray<RingShadowBand>;
}

/**
 * A ring's shadow record from its profile, its plane's unit normal and its planet's equatorial
 * radius, m: its bands in units of a, at most {@link MAX_RING_SHADOW_BANDS}. A profile of more
 * keeps its first 15, and a 16th runs from there to the outer edge at its densest band's τ.
 */
export function ringShadowRecordOf(
  profile: RingProfile,
  normal: Vec3,
  equatorialRadiusM: number,
): RingShadowRecord {
  const a = equatorialRadiusM;
  const all = profile.bands.map((band) => ({ outerOverA: band.outerM / a, tau: band.tau }));
  if (all.length <= MAX_RING_SHADOW_BANDS) {
    return { normal, innerOverA: profile.innerM / a, bands: all };
  }
  const densest = Math.max(...profile.bands.map((band) => band.tau));
  const bands = [
    ...all.slice(0, MAX_RING_SHADOW_BANDS - 1),
    { outerOverA: profile.outerM / a, tau: densest },
  ];
  return { normal, innerOverA: profile.innerM / a, bands };
}

/** The ring's normal optical depth at radius `r` over a: 0 off the annulus. */
function recordTauAt(ring: RingShadowRecord, r: number): number {
  if (!(r >= ring.innerOverA)) {
    return 0;
  }
  for (const band of ring.bands) {
    if (r < band.outerOverA) {
      return band.tau;
    }
  }
  return 0;
}

/**
 * The ring's shadow on its planet at a point, the share of a star's direct light the ring lets
 * through: exp(−τ(r) ÷ |μ₀|) where the ray from the point towards the star crosses the ring plane
 * ahead of it at radius r inside the annulus, μ₀ the cosine of the star against the plane's normal;
 * 1 where it crosses nowhere ahead or off the annulus, and where the star lies in the plane.
 *
 * @param p - The point, from the planet's centre over a, along the record's axes.
 * @param sun - The unit direction from the point towards the star, along the same axes.
 */
export function ringShadowOnBody(ring: RingShadowRecord, p: Vec3, sun: Vec3): number {
  const mu0 = dot(sun, ring.normal);
  const height = dot(p, ring.normal);
  // The ray p + t sun meets the plane at t = −height ÷ μ₀, ahead of the point where t > 0.
  if (!(Math.abs(mu0) > 0) || !(height * mu0 < 0)) {
    return 1;
  }
  const crossing = add(p, scale(sun, -height / mu0));
  const tau = recordTauAt(ring, norm(crossing));
  return tau > 0 ? Math.exp(-tau / Math.abs(mu0)) : 1;
}

/**
 * A disc record's ring rows ({@link RING_SHADOW_ROWS} `vec4f`s, `ringShadow.wgsl`'s
 * `ring_shadow_row`): row 0 the normal and the inner edge over a; row 1 x the band count, 0 where
 * the body casts no ring shadow; then each pair of bands' (outer edge over a, τ).
 */
export function packRingShadowRows(ring: RingShadowRecord | undefined): Float32Array {
  const out = new Float32Array(RING_SHADOW_ROWS * 4);
  if (ring === undefined || ring.bands.length === 0) {
    return out;
  }
  const bands = ring.bands.slice(0, MAX_RING_SHADOW_BANDS);
  out.set([ring.normal.x, ring.normal.y, ring.normal.z, ring.innerOverA], 0);
  out[4] = bands.length;
  bands.forEach((band, j) => {
    out.set([band.outerOverA, band.tau], 8 + 2 * j);
  });
  return out;
}

/** A planet as it shadows its ring: a spheroid about its pole. */
export interface ShadowingPlanet {
  /** Its centre, m, in the frame of the ring point and the star. */
  readonly centreM: Vec3;
  /** Its equatorial radius a, m. */
  readonly radiusM: number;
  /** Its pole's unit direction. */
  readonly pole: Vec3;
  /** c ÷ a, 1 for a sphere. */
  readonly polarOverEquatorial: number;
}

/** A point stretched along the pole about the centre by a ÷ c, which makes the spheroid a sphere. */
function stretched(planet: ShadowingPlanet, x: Vec3): Vec3 {
  const offset = sub(x, planet.centreM);
  const along = (1 / planet.polarOverEquatorial - 1) * dot(offset, planet.pole);
  return add(planet.centreM, add(offset, scale(planet.pole, along)));
}

/**
 * The planet's shadow on its ring at a point, the share of one channel's flux from the star's disc
 * the planet leaves visible there (Design note 14): R07's eclipse term (`eclipseVisible`) with the
 * planet the one occluder, whose penumbra is the star's angular diameter times the distance from the
 * planet's limb. An oblate planet is taken in the frame stretched by a ÷ c along its pole, in which
 * it is the sphere of its equatorial radius: exact for the umbra's edge of a point star, the
 * penumbra's width off by under the flattening.
 *
 * @param pointM - The ring point, m.
 * @param star - The star's disc in the channel, in the same frame.
 * @param annuli - The eclipse term's annuli, `DISC_ANNULI_HIGH` or `DISC_ANNULI_LOW`.
 */
export function planetShadowOnRing(
  pointM: Vec3,
  star: LimbDarkenedDisc,
  planet: ShadowingPlanet,
  annuli: number,
): number {
  const disc = { ...star, centreM: stretched(planet, star.centreM) };
  return eclipseVisible(
    disc,
    stretched(planet, pointM),
    [{ centreM: planet.centreM, radiusM: planet.radiusM }],
    annuli,
  );
}

/** A ring of a planet as the shadow on the planet picks it: by its normal optical depth. */
export interface ShadowCaster {
  readonly opticalDepth: number;
}

/**
 * The ring whose shadow a planet carries: its densest, since a dusty ring's τ of 10⁻⁷–10⁻⁵ casts
 * none worth a lookup; the first of equal ones; `null` for none.
 */
export function shadowingRing<R extends ShadowCaster>(rings: ReadonlyArray<R>): R | null {
  let best: R | null = null;
  for (const ring of rings) {
    if (best === null || ring.opticalDepth > best.opticalDepth) {
      best = ring;
    }
  }
  return best;
}
