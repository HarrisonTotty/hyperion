import { add, dot, norm, scale, type Vec3 } from "../../geometry/vec3";

/**
 * A body's datum spheroid as the wireframe draws on it (plan R02, R02.T19): its figure's stretch
 * a ÷ c along its pole, by which the spheroid of equatorial radius a becomes the sphere of radius
 * a, as T16.f's surveyed datum already stretches its mask's direction (`SurveyedDatum.stretch`).
 *
 * @remarks
 * The stretch L v = v + (s − 1)(v · p) p, s = a ÷ c, is linear, so it maps the camera to itself,
 * lines of sight to lines of sight, and the spheroid about a centre c to the sphere of radius a
 * about L c. What the wireframe asks of a body that is not about the screen (which side of a
 * tangent plane the camera stands on, where a line of sight meets the body or grazes it) is kept by
 * any such map, so it is answered on that sphere with R02's closed forms and mapped back by L⁻¹:
 * the limb is the sphere's circle of tangency, the graticule's facing test the sphere's on its
 * normal, and the horizon test of a mark the sphere's. Only an oblate figure, s > 1, is a spheroid
 * here; a sphere, or a figure with no usable radii, has none, and every map below is then the
 * identity, returning the very vector it was given, so that a sphere's lines are built as before.
 */
export interface DatumSpheroid {
  /**
   * The spheroid's pole, its symmetry axis, a unit vector: along the camera frame's axes, or along
   * whichever axes the vectors it stretches are given in (the view's, for the image plane's sums).
   */
  readonly pole: Vec3;
  /** Its equatorial radius over its polar, a ÷ c, above 1. */
  readonly stretch: number;
}

/**
 * The spheroid a body's figure gives its datum, or `null` for a sphere: an equatorial radius
 * above its polar, both finite and positive, about `pole`.
 *
 * @param pole - The spheroid's pole along the camera frame's axes, of any non-zero length.
 */
export function datumSpheroidOf(
  figure: { readonly equatorialRadiusM: number; readonly polarRadiusM: number },
  pole: Vec3,
): DatumSpheroid | null {
  const stretch = figure.equatorialRadiusM / figure.polarRadiusM;
  const length = norm(pole);
  if (!(stretch > 1 && Number.isFinite(stretch) && length > 0 && Number.isFinite(length))) {
    return null;
  }
  return { pole: scale(pole, 1 / length), stretch };
}

/** v + k (v · p) p: a vector moved along the pole by k times its own component there. */
function alongPole(v: Vec3, pole: Vec3, k: number): Vec3 {
  return add(v, scale(pole, k * dot(v, pole)));
}

/**
 * A vector in the spheroid's stretched space, L v, where the spheroid is the sphere of its
 * equatorial radius; for a sphere (`null`) `v` itself.
 */
export function stretched(v: Vec3, spheroid: DatumSpheroid | null): Vec3 {
  return spheroid === null ? v : alongPole(v, spheroid.pole, spheroid.stretch - 1);
}

/** A vector of the stretched space back in the body's, L⁻¹ v; for a sphere (`null`) `v` itself. */
export function unstretched(v: Vec3, spheroid: DatumSpheroid | null): Vec3 {
  return spheroid === null ? v : alongPole(v, spheroid.pole, 1 / spheroid.stretch - 1);
}

/**
 * The most the stretch or its inverse turns two lines of sight apart or together, as a factor on
 * the angle between them: the ratio of L's singular values, s; 1 for a sphere.
 *
 * @remarks
 * The map of directions u ↦ L u ÷ |L u| moves a direction's tangent t by (L t)⊥ ÷ |L u|, at most
 * σ_max ÷ σ_min = s times as far, and its inverse likewise, so an angle γ in either space is at
 * least γ ÷ s in the other.
 */
export function angularDistortion(spheroid: DatumSpheroid | null): number {
  return spheroid === null ? 1 : spheroid.stretch;
}
