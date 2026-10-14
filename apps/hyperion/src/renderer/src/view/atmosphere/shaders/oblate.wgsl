// The atmosphere on an oblate body (plan R08, Design notes 17 and 18; R08.T6.f): the body's figure,
// gravity and slicing, a point's height and latitude on the datum from its ground quadric, the
// radius of curvature along a ray, and the sliced lookup of a source's transmittance, R08.T6.e's
// twin (`tablesCpu.ts`'s `OblateReader`) operation for operation where `f32` allows. Prepended,
// after `common.wgsl`, to the sky-view, aerial-perspective and ray-march kernels (after
// `medium.wgsl`) and to the composite. It declares nothing: each consumer binds the body's table,
// `oblateBody` (`tables.ts`'s `packOblateBody`), and passes it and the transmittance tables in, so
// that the composite, which declares no medium, reads them as the kernels do.
//
// The body's table, `rgba32float`, one row: texel 0 the figure (a, c, R_ref, e²), m; texel 1 the
// gravity ratio's coefficients (A, B, K, O), s(φ) = (A cos²φ + B sin²φ) ÷ √(cos²φ + K sin²φ) + O,
// Somigliana's γ(φ) + the body's added gravity over g_ref, divided through by a (A = γ_e ÷ g_ref,
// B = (c ÷ a) γ_p ÷ g_ref, K = (c ÷ a)², O = the offset ÷ g_ref), all formed in f64; a body with no
// spheroid has A = B = 0 and O = 1, so that s is 1 to the bit; texel 2 the slices, the bands, the
// multiple-scattering layers a band (its light slots) and the medium's top in h*; texel 3 k = a ÷ c,
// k², and 1 ÷ s at the equator and at the pole, by which the ray march's shells stand through h*
// (Design note 17); texels 4 to 4 + MAX_SLICES − 1 slice k's (κ_k R_ref, √κ_k, ln κ_k, 0); and the next
// MAX_BANDS band b's (s_b, ln s_b, 0, 0).
//
// Heights follow Design note 18: none is a difference of radii. A point's height comes from its
// ground quadric Q = |S p|² − a² (S = diag(1, 1, a ÷ c)), which every caller forms without
// cancellation (the ray march's near and closest-approach forms, or from the camera's `f64` q_g),
// as the root of the exact quadratic along the normal ν of the similar spheroid through p:
// s = 2(F − 1) ÷ (G + √(G² − 4κ′(F − 1))), F − 1 = Q ÷ a², G = |∇F(p)|, κ′ = (ν_x² + ν_y²) ÷ a² +
// ν_z² ÷ c², here in units of a. On a sphere it is |p| − a; on a Saturn-class figure it is within
// 2 × 10⁻⁶ of the geodetic height to 1,000 km (`precision.test.ts`). The latitude is the normal at
// the root's foot. No function here calls WGSL's trigonometry.

// The most slices and bands a body's tables hold (`tables.ts`'s MAX_SLICES and MAX_BANDS).
const MAX_SLICES : u32 = 16u;
const MAX_BANDS : u32 = 16u;

// The body table's texels before its slices.
const OBLATE_HEADER_TEXELS : u32 = 4u;

// The least local scale height the fitted lookup takes, m of h* (`hillaire.ts`'s FIT_SCALE_FLOOR_M):
// below it the fit's differences of h* are lost in f32's rounding of h*.
const OBLATE_SCALE_FLOOR_M : f32 = 1000.0;

// Below this squared horizontal part a ray is taken as vertical, its radius of curvature the
// Gaussian √(MN) (`tablesCpu.ts`'s VERTICAL_BELOW is 10⁻²⁴ in f64; in f32 a vertical direction's
// parts round to about 10⁻¹⁴, and a ray this near the vertical hardly depends on its sphere).
const OBLATE_VERTICAL_BELOW : f32 = 1e-10;

// The body table's first four texels.
struct OblateBody {
  // a, c, R_ref, e² = (a − c)(a + c) ÷ a².
  figure : vec4f,
  // A, B, K, O.
  gravity : vec4f,
  // Slices, bands, multiple-scattering layers a band, the medium's top in h*, m.
  counts : vec4f,
  // k = a ÷ c, k², 1 ÷ s_e, 1 ÷ s_p.
  scales : vec4f,
}

fn oblateBodyOf(table : texture_2d<f32>) -> OblateBody {
  return OblateBody(
    textureLoad(table, vec2u(0u, 0u), 0),
    textureLoad(table, vec2u(1u, 0u), 0),
    textureLoad(table, vec2u(2u, 0u), 0),
    textureLoad(table, vec2u(3u, 0u), 0),
  );
}

fn oblateSliceCount(body : OblateBody) -> u32 {
  return clamp(u32(body.counts.x), 1u, MAX_SLICES);
}

fn oblateBandCount(body : OblateBody) -> u32 {
  return clamp(u32(body.counts.y), 1u, MAX_BANDS);
}

// Whether a lookup needs its start's latitude: a body of several slices or bands, whose fit takes
// R_α and ds ÷ dφ there, or whose s varies with latitude. A body of one slice and one band with no
// spheroid, s ≡ 1, needs none, and its kernels take no datum point for it.
fn oblateNeedsLatitude(body : OblateBody) -> bool {
  return oblateSliceCount(body) > 1u || oblateBandCount(body) > 1u || body.gravity.x != 0.0
    || body.gravity.y != 0.0;
}

// The multiple-scattering layers a band takes: one a light slot (R08.T7), band b's slot k at layer
// b × layers + k.
fn oblateLayersPerBand(body : OblateBody) -> u32 {
  return max(u32(body.counts.z), 1u);
}

// Slice k: x its ground's radius κ_k R_ref, m; y √κ_k; z ln κ_k.
fn oblateSlice(table : texture_2d<f32>, k : u32) -> vec4f {
  return textureLoad(table, vec2u(OBLATE_HEADER_TEXELS + k, 0u), 0);
}

// Band b: x s_b; y ln s_b.
fn oblateBand(table : texture_2d<f32>, b : u32) -> vec4f {
  return textureLoad(table, vec2u(OBLATE_HEADER_TEXELS + MAX_SLICES + b, 0u), 0);
}

// s = g(φ) ÷ g_ref at the geodetic latitude whose sine squared is `sin2` (`oblate.ts`'s
// gravityRatio: Somigliana's closed form, the added gravity included).
fn gravityRatioAt(body : OblateBody, sin2 : f32) -> f32 {
  let cos2 = 1.0 - sin2;
  let g = body.gravity;
  return (g.x * cos2 + g.y * sin2) / sqrt(cos2 + g.z * sin2) + g.w;
}

// ds ÷ dφ, rad⁻¹ (`oblate.ts`'s gravityRatioSlope): with N = A cos²φ + B sin²φ and
// D = cos²φ + K sin²φ, sin 2φ ((B − A) ÷ √D − N (K − 1) ÷ (2 D √D)); 0 with no spheroid.
fn gravityRatioSlopeAt(body : OblateBody, sinLatitude : f32, cosLatitude : f32) -> f32 {
  let g = body.gravity;
  let sin2 = sinLatitude * sinLatitude;
  let cos2 = cosLatitude * cosLatitude;
  let d = cos2 + g.z * sin2;
  let root = sqrt(d);
  let n = g.x * cos2 + g.y * sin2;
  let sinTwice = 2.0 * sinLatitude * cosLatitude;
  return sinTwice * ((g.y - g.x) / root - n * (g.z - 1.0) / (2.0 * d * root));
}

// S p, S = diag(1, 1, a ÷ c): the space in which the datum is the sphere of radius a.
fn oblateScaled(body : OblateBody, p : vec3f) -> vec3f {
  return vec3f(p.x, p.y, p.z * body.scales.x);
}

// A point against the datum: its height along the similar spheroid's normal, and the geodetic
// normal at the foot of that height, unit, whose z is the sine of the geodetic latitude.
struct DatumPoint {
  heightM : f32,
  normal : vec3f,
}

// The datum point of the body-fixed `p`, whose ground quadric Q = |S p|² − a² is `quadricM2`:
// with g = (p_x, p_y, k² p_z) ÷ a, u = |g|, ν = g ÷ u and κ″ = ν_x² + ν_y² + k² ν_z², the height is
// a q ÷ (u + √(u² − κ″ q)), q = Q ÷ a², the module's root in units of a, and the normal is that of
// the datum at p − height ν.
fn datumPointOf(body : OblateBody, p : vec3f, quadricM2 : f32) -> DatumPoint {
  let a = body.figure.x;
  let k2 = body.scales.y;
  let g = vec3f(p.x, p.y, p.z * k2) / a;
  let u = length(g);
  let nu = g / u;
  let kappa = nu.x * nu.x + nu.y * nu.y + k2 * (nu.z * nu.z);
  let q = quadricM2 / (a * a);
  let heightM = a * q / (u + sqrt(max(u * u - kappa * q, 0.0)));
  let foot = p - heightM * nu;
  return DatumPoint(heightM, normalize(vec3f(foot.x, foot.y, foot.z * k2)));
}

// The ground quadric Q at distance x along the unit `d` from a point whose quadric is `quadricM2`
// and whose S p is `scaled`: Q + x (2 (S p) · (S d) + x |S d|²), `scaledDirection` S d. The point's
// rounding moves it by about 2ε|S p| x, a height of about ε x.
fn quadricAlong(quadricM2 : f32, scaled : vec3f, scaledDirection : vec3f, x : f32) -> f32 {
  return quadricM2 + x * (2.0 * dot(scaled, scaledDirection) + x * dot(scaledDirection, scaledDirection));
}

// The principal radii of curvature at geodetic latitude φ, m: (M, N), N = a ÷ √(1 − e² sin²φ) and
// M = a (1 − e²) ÷ (1 − e² sin²φ)^(3/2) (`oblate.ts`'s principalRadiiM).
fn principalRadiiAt(body : OblateBody, sin2 : f32) -> vec2f {
  let a = body.figure.x;
  let e2 = body.figure.w;
  let w2 = 1.0 - e2 * sin2;
  let w = sqrt(w2);
  return vec2f(a * (1.0 - e2) / (w2 * w), a / w);
}

// A direction's vertical plane at a point (`tablesCpu.ts`'s verticalPlaneOf): its radius of
// curvature R_α, m, its part along the point's north, √(1 − μ²) cos α, and M, m.
struct VerticalPlane {
  radiusM : f32,
  towardsNorth : f32,
  meridionalM : f32,
}

// The vertical plane of the unit `d` at a point of geodetic latitude whose sine squared is `sin2`,
// its north and east taken from the unit normal `n` (the march's own): R_α by Euler's theorem,
// 1 ÷ R_α = cos²α ÷ M + sin²α ÷ N, as M ÷ (1 − (1 − M ÷ N) sin²α) with 1 − M ÷ N formed as
// e² cos²φ ÷ (1 − e² sin²φ), which is 0 on a sphere, so that a sphere's R_α is its radius to the bit;
// √(MN) for a vertical direction. At a pole, where every azimuth is the same, north is ∓x and east
// y, as the twin's.
fn verticalPlaneOf(body : OblateBody, sin2 : f32, n : vec3f, d : vec3f) -> VerticalPlane {
  let radii = principalRadiiAt(body, sin2);
  let e2 = body.figure.w;
  let cos2Normal = n.x * n.x + n.y * n.y;
  var along : f32;
  var across : f32;
  if (cos2Normal < 1e-20) {
    along = select(d.x, -d.x, n.z >= 0.0);
    across = d.y;
  } else {
    let cosNormal = sqrt(cos2Normal);
    across = (d.y * n.x - d.x * n.y) / cosNormal;
    along = (d.z * cos2Normal - n.z * (d.x * n.x + d.y * n.y)) / cosNormal;
  }
  let horizontal = along * along + across * across;
  var radiusM = sqrt(radii.x * radii.y);
  if (horizontal > OBLATE_VERTICAL_BELOW) {
    let gap = e2 * (1.0 - sin2) / (1.0 - e2 * sin2);
    radiusM = radii.x / (1.0 - gap * (across * across / horizontal));
  }
  return VerticalPlane(radiusM, along, radii.x);
}

// The lowest height of a ray above a sphere of radius `radiusM`, from `heightM` at zenith cosine
// `mu` < 0, m: h − (r + h) μ² ÷ (1 + √(1 − μ²)), which does not cancel (`tablesCpu.ts`'s
// lowestOnSphereM).
fn lowestOnSphereM(radiusM : f32, heightM : f32, mu : f32) -> f32 {
  let sine = sqrt(max((1.0 - mu) * (1.0 + mu), 0.0));
  return heightM - (radiusM + heightM) * mu * mu / (1.0 + sine);
}

// The zenith cosine from `heightM` above a sphere of radius `radiusM` of the ray whose lowest point
// is `lowestM` above it: −√((1 − ρ)(1 + ρ)), 1 − ρ = (h − lowest) ÷ (r + h) (`cosineToLowest`).
fn cosineToLowestOf(radiusM : f32, heightM : f32, lowestM : f32) -> f32 {
  let short = (heightM - lowestM) / (radiusM + heightM);
  return -sqrt(max(short * (2.0 - short), 0.0));
}

// Where a lookup starts (Design note 17, _The ray's own gradient_): the body-fixed place and its
// S p, its ground quadric there, formed without cancellation by the caller; the march's own height
// and unit normal there (the sky view's and the aerial perspective's sphere R_α, or the datum's for
// the ray march and the composite); the sine and cosine of the place's geodetic latitude; s there,
// h* = s h, and the local scale height in h* there (`localScaleHeightOf`).
struct LookupStart {
  place : vec3f,
  scaledPlace : vec3f,
  quadricM2 : f32,
  heightM : f32,
  normal : vec3f,
  sinLatitude : f32,
  cosLatitude : f32,
  gravityRatio : f32,
  scaledHeightM : f32,
  scaleHeightM : f32,
}

// The local scale height in h* at h* = `scaledHeightM` (`tablesCpu.ts`'s localScaleHeightM): the
// stored vertical optical depth above it in green, slice 0's, over the extinction there in green,
// `extinctionGreen`, held to the room to the top, all of it where nothing extinguishes, and to at
// least OBLATE_SCALE_FLOOR_M.
fn localScaleHeightOf(
  body : OblateBody,
  bodyTable : texture_2d<f32>,
  depthTable : texture_2d_array<f32>,
  scaledHeightM : f32,
  extinctionGreen : f32,
) -> f32 {
  let top = body.counts.w;
  let room = max(top - scaledHeightM, 1.0);
  if (!(extinctionGreen > 0.0)) {
    return max(room, OBLATE_SCALE_FLOOR_M);
  }
  let height = clamp(scaledHeightM, 0.0, top);
  let unit = transmittanceUnitAt(oblateSlice(bodyTable, 0u).x, top, height, 1.0);
  let vertical = tableAtUnitLayer(depthTable, 0u, unit).g;
  return max(min(vertical / extinctionGreen, room), OBLATE_SCALE_FLOOR_M);
}

// The h* of the point x along the unit `d` from the lookup's start, on the datum: the datum point
// of its quadric from the start's, s at its latitude times its height.
fn scaledHeightAlong(
  body : OblateBody,
  start : LookupStart,
  d : vec3f,
  scaledDirection : vec3f,
  x : f32,
) -> f32 {
  let quadric = quadricAlong(start.quadricM2, start.scaledPlace, scaledDirection, x);
  let point = datumPointOf(body, start.place + x * d, quadric);
  return gravityRatioAt(body, point.normal.z * point.normal.z) * point.heightM;
}

// A source's ray as the sliced lookup reads it (`tablesCpu.ts`'s SlicedRay): the slices about its
// sphere, the upper's weight in the interpolation linear in √κ (ruled 2026-10-13), the zenith
// cosine each is read at, s and h* at the start, and whether the ground hides the source.
struct SlicedRay {
  lowerSlice : u32,
  weight : f32,
  lowerCosine : f32,
  upperCosine : f32,
  gravityRatio : f32,
  scaledHeightM : f32,
  shadowed : bool,
}

// A ray on the sphere of radius `sphereM` (in the frame of lengths s x, h*), or on no sphere
// (`flat`, a fitted sphere with no curvature): its slices, the nearer one beyond their range; each
// read at `fittedCosine`, or, for a ray that dips (`dips`), at the cosine that keeps its lowest h*,
// `lowestM`, the ground hiding the source where that is below the datum (`tablesCpu.ts`'s #read).
fn slicedRayOn(
  body : OblateBody,
  bodyTable : texture_2d<f32>,
  start : LookupStart,
  sphereM : f32,
  flat : bool,
  fittedCosine : f32,
  dips : bool,
  lowestM : f32,
) -> SlicedRay {
  let count = oblateSliceCount(body);
  let last = count - 1u;
  var lower = 0u;
  var weight = 0.0;
  if (!flat && !(sphereM > oblateSlice(bodyTable, 0u).x)) {
    lower = 0u;
  } else if (flat || !(sphereM < oblateSlice(bodyTable, last).x)) {
    lower = max(last, 1u) - 1u;
    weight = select(0.0, 1.0, last > 0u);
  } else {
    while (lower + 1u < last && oblateSlice(bodyTable, lower + 1u).x <= sphereM) {
      lower++;
    }
    let r0 = oblateSlice(bodyTable, lower).y;
    let r1 = oblateSlice(bodyTable, lower + 1u).y;
    weight = (sqrt(sphereM / body.figure.z) - r0) / (r1 - r0);
  }
  let z0 = start.scaledHeightM;
  var lowerCosine = fittedCosine;
  var upperCosine = fittedCosine;
  if (dips) {
    lowerCosine = cosineToLowestOf(oblateSlice(bodyTable, lower).x, z0, lowestM);
    let upper = min(lower + 1u, last);
    upperCosine = cosineToLowestOf(oblateSlice(bodyTable, upper).x, z0, lowestM);
  }
  return SlicedRay(
    lower, weight, lowerCosine, upperCosine, start.gravityRatio, z0, dips && lowestM < 0.0
  );
}

// The sliced lookup of the ray from `start` along the unit `d`, towards a source (plan R08,
// Design note 17, _The ray's own gradient_; `tablesCpu.ts`'s OblateReader.rayOf):
// - one slice and one band: slice 0 at h* and μ;
// - one band (s varies by under 2%): the start's own sphere, s R_α;
// - otherwise the sphere fitted to the ray's own h*(x): through the start with the ray's slope of
//   h* there, s μ + h (∂s ÷ ∂φ) √(1 − μ²) cos α ÷ (M + h), and through h* where the ray on the start's
//   R_α has risen one local scale height H ÷ s (one datum point); or, for a ray that dips by more
//   than half a local scale height, at its lowest point, by the parabola through h* at its tangent
//   point on the start's R_α and √(2 R_α H ÷ s) either side (three).
// The steps' distances are taken in the forms that do not cancel in f32: x₁'s root as
// rise (2 reach + rise) ÷ (reach μ + √(…)) upward, and the fitted radius's numerator with
// (h*₀ − h*₁)(h*₀ + h*₁).
fn slicedRayOf(
  body : OblateBody,
  bodyTable : texture_2d<f32>,
  start : LookupStart,
  d : vec3f,
) -> SlicedRay {
  let s0 = start.gravityRatio;
  let z0 = start.scaledHeightM;
  let h0 = start.heightM;
  let mu = clamp(dot(d, start.normal), -1.0, 1.0);
  if (oblateSliceCount(body) == 1u && oblateBandCount(body) == 1u) {
    let bottom = oblateSlice(bodyTable, 0u).x;
    return SlicedRay(0u, 0.0, mu, mu, s0, z0, groundHitFromHeight(bottom, max(z0, 0.0), mu));
  }
  let sin2 = start.sinLatitude * start.sinLatitude;
  let plane = verticalPlaneOf(body, sin2, start.normal, d);
  let rAlpha = plane.radiusM;
  if (oblateBandCount(body) == 1u) {
    let sphereM = s0 * rAlpha;
    let lowest = lowestOnSphereM(sphereM, z0, mu);
    return slicedRayOn(body, bodyTable, start, sphereM, false, mu, mu < 0.0, lowest);
  }
  let scaleM = start.scaleHeightM;
  let riseM = scaleM / s0;
  let reach = rAlpha + h0;
  let reachMu = reach * mu;
  let scaledDirection = oblateScaled(body, d);
  if (mu < 0.0) {
    // The dip on the start's sphere, h₀ − lowest, as (r + h) μ² ÷ (1 + √(1 − μ²)).
    let dipM = reach * mu * mu / (1.0 + sqrt(max((1.0 - mu) * (1.0 + mu), 0.0)));
    if (s0 * dipM > 0.5 * scaleM) {
      let xt = -reachMu;
      let delta = sqrt(2.0 * riseM * rAlpha);
      let zt = scaledHeightAlong(body, start, d, scaledDirection, xt);
      let zp = scaledHeightAlong(body, start, d, scaledDirection, xt + delta);
      let zm = scaledHeightAlong(body, start, d, scaledDirection, xt - delta);
      let curvature = (zp + zm - 2.0 * zt) / (2.0 * delta * delta);
      if (curvature > 0.0) {
        let lowest = zt - (zp - zm) * (zp - zm) / (16.0 * curvature * delta * delta);
        let sphereM = s0 * s0 / (2.0 * curvature) - lowest;
        let fitted = cosineToLowestOf(sphereM, z0, lowest);
        return slicedRayOn(body, bodyTable, start, sphereM, false, fitted, true, lowest);
      }
    }
  }
  let room = riseM * (2.0 * reach + riseM);
  let root = sqrt(reachMu * reachMu + room);
  var x1 = select(root - reachMu, room / (reachMu + root), mu >= 0.0);
  if (mu < 0.0) {
    x1 = max(x1, -2.0 * reachMu);
  }
  let z1 = scaledHeightAlong(body, start, d, scaledDirection, x1);
  let slope = s0 * mu
    + h0 * gravityRatioSlopeAt(body, start.sinLatitude, start.cosLatitude) * plane.towardsNorth
      / (plane.meridionalM + h0);
  let fitted = clamp(slope / s0, -1.0, 1.0);
  let xs = s0 * x1;
  let denominator = 2.0 * (z1 - z0 - fitted * xs);
  let flat = !(denominator > 0.0);
  var sphereM = oblateSlice(bodyTable, oblateSliceCount(body) - 1u).x;
  if (!flat) {
    sphereM = (xs * xs + 2.0 * z0 * fitted * xs + (z0 - z1) * (z0 + z1)) / denominator;
  }
  // A flat fit takes the last slice's sphere for its lowest point, as the twin's.
  let lowest = lowestOnSphereM(sphereM, z0, fitted);
  return slicedRayOn(body, bodyTable, start, sphereM, flat, fitted, fitted < 0.0, lowest);
}

// What a sliced lookup reads from the slices' tables: the stored optical depth of every term but
// the absorbers, the absorber columns and their pressure-weighted columns, each slice's read at its
// own cosine from h* held in the shell, weighted, and divided by s (Design note 17).
struct SlicedStored {
  depth : vec3f,
  columns : AbsorberColumns,
  pressureColumns : AbsorberColumns,
}

// One slice's read at its layer: as `transmittanceThroughLayer` reads one, the absorber
// tables only where the medium's `count` absorbers have them.
fn sliceStoredAt(
  depthTable : texture_2d_array<f32>,
  columnTable : texture_2d_array<f32>,
  pressureColumnTable : texture_2d_array<f32>,
  lastPressureColumnTable : texture_2d_array<f32>,
  count : u32,
  layer : u32,
  unit : vec2f,
) -> SlicedStored {
  let stored = tableAtUnitLayer(depthTable, layer, unit);
  var columns = AbsorberColumns(stored.a, vec4f(0.0));
  var pressureColumns = AbsorberColumns(0.0, vec4f(0.0));
  if (count > 0u) {
    let firstFour = tableAtUnitLayer(pressureColumnTable, layer, unit);
    pressureColumns = AbsorberColumns(
      firstFour.x,
      vec4f(firstFour.yzw, tableAtUnitLayer(lastPressureColumnTable, layer, unit).x),
    );
  }
  if (count > 1u) {
    columns.more = tableAtUnitLayer(columnTable, layer, unit);
  }
  return SlicedStored(stored.rgb, columns, pressureColumns);
}

// The stored values a sliced ray reads (`tablesCpu.ts`'s #storedAlong).
fn slicedStoredOf(
  ray : SlicedRay,
  body : OblateBody,
  bodyTable : texture_2d<f32>,
  depthTable : texture_2d_array<f32>,
  columnTable : texture_2d_array<f32>,
  pressureColumnTable : texture_2d_array<f32>,
  lastPressureColumnTable : texture_2d_array<f32>,
  count : u32,
) -> SlicedStored {
  let top = body.counts.w;
  let height = clamp(ray.scaledHeightM, 0.0, top);
  var sum = SlicedStored(vec3f(0.0), AbsorberColumns(0.0, vec4f(0.0)), AbsorberColumns(0.0, vec4f(0.0)));
  if (ray.weight < 1.0) {
    let k = ray.lowerSlice;
    let unit = transmittanceUnitAt(oblateSlice(bodyTable, k).x, top, height, ray.lowerCosine);
    let one = sliceStoredAt(
      depthTable, columnTable, pressureColumnTable, lastPressureColumnTable, count, k, unit
    );
    let w = 1.0 - ray.weight;
    sum = SlicedStored(
      one.depth * w,
      addColumns(sum.columns, one.columns, w),
      addColumns(sum.pressureColumns, one.pressureColumns, w),
    );
  }
  if (ray.weight > 0.0) {
    let k = ray.lowerSlice + 1u;
    let unit = transmittanceUnitAt(oblateSlice(bodyTable, k).x, top, height, ray.upperCosine);
    let one = sliceStoredAt(
      depthTable, columnTable, pressureColumnTable, lastPressureColumnTable, count, k, unit
    );
    let w = ray.weight;
    sum = SlicedStored(
      sum.depth + one.depth * w,
      addColumns(sum.columns, one.columns, w),
      addColumns(sum.pressureColumns, one.pressureColumns, w),
    );
  }
  let inverse = 1.0 / ray.gravityRatio;
  let none = AbsorberColumns(0.0, vec4f(0.0));
  return SlicedStored(
    sum.depth * inverse,
    addColumns(none, sum.columns, inverse),
    addColumns(none, sum.pressureColumns, inverse),
  );
}

// The multiple scattering at h* `scaledHeightM`, s and the source's zenith cosine `muSun`, light slot
// `slot`'s layer of each band (sebh's unit range with its 10 m offset, Bruneton's texel mapping):
// linear in ln s between the two bands about s, the nearer band beyond them, one band's alone
// (`tablesCpu.ts`'s multiScatteringInto).
fn bandMultiScatteringAt(
  body : OblateBody,
  bodyTable : texture_2d<f32>,
  table : texture_2d_array<f32>,
  scaledHeightM : f32,
  s : f32,
  muSun : f32,
  slot : u32,
) -> vec3f {
  let unit = multiScatteringUnitAt(body.counts.w, max(scaledHeightM, 0.0), muSun);
  let layers = oblateLayersPerBand(body);
  let k = min(slot, layers - 1u);
  let bands = oblateBandCount(body);
  let last = bands - 1u;
  if (last == 0u) {
    return tableAtUnitLayer(table, k, unit).rgb;
  }
  let ls = log(s);
  if (!(ls > oblateBand(bodyTable, 0u).y)) {
    return tableAtUnitLayer(table, k, unit).rgb;
  }
  if (!(ls < oblateBand(bodyTable, last).y)) {
    return tableAtUnitLayer(table, last * layers + k, unit).rgb;
  }
  var lower = 0u;
  while (lower + 1u < last && oblateBand(bodyTable, lower + 1u).y <= ls) {
    lower++;
  }
  let l0 = oblateBand(bodyTable, lower).y;
  let w = (ls - l0) / (oblateBand(bodyTable, lower + 1u).y - l0);
  let below = tableAtUnitLayer(table, lower * layers + k, unit).rgb;
  let above = tableAtUnitLayer(table, (lower + 1u) * layers + k, unit).rgb;
  return below * (1.0 - w) + above * w;
}

// The radius of curvature at the camera along the unit `d`, m (`tablesCpu.ts`'s curvatureAlongM):
// R_α of the camera's principal radii M and N (`meridionalM`, `primeVerticalM`, formed in f64) and
// its unit normal `n`, as `verticalPlaneOf` takes it at a sample; √(MN) for a vertical direction.
fn cameraCurvatureAlong(meridionalM : f32, primeVerticalM : f32, n : vec3f, d : vec3f) -> f32 {
  let cos2Normal = n.x * n.x + n.y * n.y;
  var along : f32;
  var across : f32;
  if (cos2Normal < 1e-20) {
    along = select(d.x, -d.x, n.z >= 0.0);
    across = d.y;
  } else {
    let cosNormal = sqrt(cos2Normal);
    across = (d.y * n.x - d.x * n.y) / cosNormal;
    along = (d.z * cos2Normal - n.z * (d.x * n.x + d.y * n.y)) / cosNormal;
  }
  let horizontal = along * along + across * across;
  if (!(horizontal > OBLATE_VERTICAL_BELOW)) {
    return sqrt(meridionalM * primeVerticalM);
  }
  // 1 − M ÷ N, formed from the radii: 0 on a sphere, so that a sphere's R_α is its radius.
  let gap = (primeVerticalM - meridionalM) / primeVerticalM;
  return meridionalM / (1.0 - gap * (across * across / horizontal));
}

// The column sphere of the sky view at a direction (plan R08, R08.T6.f; `tablesCpu.ts`'s
// oblateSkyViewRayTwin and oblateSkyViewReadTwin): R_α at the camera along the direction's azimuth
// (`cameraCurvatureAlong`), the sky axis `axis` standing for a vertical direction's; and β, the
// angle from the nadir to the horizon on that sphere from the camera's height `heightM` on it,
// atan2(R_α, √(h (2 R_α + h))), taken as π ÷ 2 less the horizon's dip, preciseAtan(√(h (2 R_α + h))
// ÷ R_α), which keeps its relative precision near the ground. The sky view's write and every read
// take this one form, so that they agree; it replaces R08.T6.g's one β a frame formed in f64.
fn skyViewColumnOf(
  meridionalM : f32,
  primeVerticalM : f32,
  n : vec3f,
  axis : vec3f,
  d : vec3f,
  heightM : f32,
) -> vec2f {
  var flat = d - dot(d, n) * n;
  if (!(dot(flat, flat) > OBLATE_VERTICAL_BELOW)) {
    flat = axis;
  }
  let radiusM = cameraCurvatureAlong(meridionalM, primeVerticalM, n, flat);
  let horizon = sqrt(max(heightM * (2.0 * radiusM + heightM), 0.0));
  let beta = TRIG_HALF_PI - preciseAtan(horizon / radiusM);
  return vec2f(radiusM, beta);
}
