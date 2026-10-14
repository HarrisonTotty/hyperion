// Surface lighting through the air (plan R08, R08.T9.b; Design notes 7, 17, 18 and 19): each
// source's transmittance from a lit point to it, and the sky's diffuse irradiance there, which the
// lit passes of R07 (`litBody.wgsl`'s callers in `bodyDisc.wgsl`), R10 (`terrainLit.wgsl`) and R11
// read. Composed after `view/shaders/preciseTrig.wgsl`, `common.wgsl` and `oblate.wgsl`; it
// declares no binding of its own. Its includer declares, at its own group and bindings, read by
// `textureLoad` (so `unfilterable-float` in a material's layout):
//   - `atmosphere_body : texture_2d<f32>`, the body's table (`tables.ts`'s `packOblateBody`,
//     `rgba32float`);
//   - `atmosphere_transmittance`, `atmosphere_absorber_columns`,
//     `atmosphere_absorber_pressure_columns` and `atmosphere_last_absorber_pressure_column`, each
//     `texture_2d_array<f32>`, the transmittance tables, one layer a κ slice;
//   - `atmosphere_absorber_curves : texture_2d<f32>`, the lights' curves of growth;
//   - `atmosphere_irradiance : texture_2d_array<f32>`, the sky's irradiance, one layer a latitude
//     band and light slot (`irradiance.wgsl`).
// A body without an atmosphere binds one-texel tables of zeros, whose body table's a of 0 says so:
// the sun's transmittance is then 1 and the sky's irradiance 0, as R07's stubs were.
//
// Every lookup takes the caller's geodetic height above the datum (R05 Design note 5: a baked or
// interpolated height, never |P| − R), the source's direction cosine against the datum's normal,
// the geodetic latitude and the source's azimuth from local north (Design note 17), and the light's
// absorber-curve slot (Design note 5; a planetshine source passes slot 0). On a body of revolution
// the longitude is any, so the point is placed at longitude 0: its foot on the datum (N cos φ, 0,
// N (1 − e²) sin φ), its normal (cos φ, 0, sin φ), north (−sin φ, 0, cos φ) and east (0, 1, 0), its
// ground quadric 2hN + h² (cos²φ + k² sin²φ), formed from the height and never as a difference of
// squares (Design note 18); the source's transmittance is the sliced lookup of `oblate.wgsl` from
// there, as the per-frame kernels take it at a sample (R08.T6.f), its stored optical depth over s
// with the light slot's curves added, and the ground's shadow the lookup's. A body of several bands
// places the fit's second point by the medium's local scale height, which the body's table holds,
// since the lit passes declare no medium. No function here calls WGSL's trigonometry; the angles
// the signatures take are turned into direction cosines by `preciseTrig.wgsl`'s forms.

// The three-node rule's offset of its outer nodes, in units of the source's angular radius: √2 ÷ 2,
// the zeros of the Chebyshev polynomial of the second kind U₃ at ±√2 ÷ 2 (Design note 7).
const SOURCE_NODE_OFFSET : f32 = 0.70710678118654752;

// The rule's weights, the upper node first: Gauss–Chebyshev's of the second kind for three nodes,
// π ÷ 4 sin²(kπ ÷ 4) over π ÷ 2.
const SOURCE_NODE_WEIGHTS = array<f32, 3>(0.25, 0.5, 0.25);

// Below this sine of its zenith angle a source is taken as at the zenith, its disc's vertical
// diameter along north.
const SOURCE_AT_ZENITH : f32 = 1e-6;

// Whether the bound body has an atmosphere: a body table whose equatorial radius is above 0.
fn atmosphereHasAir(body : OblateBody) -> bool {
  return body.figure.x > 0.0;
}

// sin and cos of an angle in [−2π, 2π], by `preciseSinCos` of its magnitude reduced to [0, π].
fn atmosphereSinCos(angle : f32) -> vec2f {
  var a = angle;
  if (a > TRIG_PI) {
    a = a - 2.0 * TRIG_PI;
  } else if (a < -TRIG_PI) {
    a = a + 2.0 * TRIG_PI;
  }
  let magnitude = preciseSinCos(min(abs(a), TRIG_PI));
  return vec2f(select(magnitude.x, -magnitude.x, a < 0.0), magnitude.y);
}

// A lit point as the lookups take it: its lookup start (`oblate.wgsl`'s `LookupStart`) at longitude
// 0, and its north and east, unit and horizontal.
struct SurfacePlace {
  start : LookupStart,
  north : vec3f,
  east : vec3f,
}

// The lit point `altitudeM` above the datum along its normal at geodetic latitude `latitudeRad`.
fn atmosphereSurfacePlace(body : OblateBody, altitudeM : f32, latitudeRad : f32) -> SurfacePlace {
  let latitude = atmosphereSinCos(clamp(latitudeRad, -TRIG_HALF_PI, TRIG_HALF_PI));
  let sinL = latitude.x;
  let cosL = latitude.y;
  let a = body.figure.x;
  let e2 = body.figure.w;
  let k2 = body.scales.y;
  let primeVertical = a / sqrt(1.0 - e2 * sinL * sinL);
  let normal = vec3f(cosL, 0.0, sinL);
  let foot = vec3f(primeVertical * cosL, 0.0, primeVertical * (1.0 - e2) * sinL);
  let place = foot + altitudeM * normal;
  // |S p|² − a² from the foot's 0: (S foot) · (S n) = N, since k² (1 − e²) = 1, and |S n|².
  let quadric = altitudeM * (2.0 * primeVertical + altitudeM * (cosL * cosL + k2 * sinL * sinL));
  let s = gravityRatioAt(body, sinL * sinL);
  let z = s * altitudeM;
  var scaleM = 0.0;
  if (oblateBandCount(body) > 1u) {
    scaleM = oblateScaleHeightAt(body, atmosphere_body, z);
  }
  let start = LookupStart(
    place, oblateScaled(body, place), quadric, altitudeM, normal, sinL, cosL, s, z, scaleM
  );
  return SurfacePlace(start, vec3f(-sinL, 0.0, cosL), vec3f(0.0, 1.0, 0.0));
}

// The unit direction of zenith cosine `mu` and azimuth `azimuthRad` from north towards east.
fn atmosphereDirection(place : SurfacePlace, mu : f32, azimuthRad : f32) -> vec3f {
  let m = clamp(mu, -1.0, 1.0);
  let horizontal = sqrt(max((1.0 - m) * (1.0 + m), 0.0));
  let azimuth = atmosphereSinCos(azimuthRad);
  return m * place.start.normal + horizontal * (azimuth.y * place.north + azimuth.x * place.east);
}

// The light slot a lookup reads: `curve`, held to the body's light slots (its multiple-scattering
// and irradiance layers a band; one for a medium with no absorber, which reads no curve).
fn atmosphereSlot(body : OblateBody, curve : u32) -> u32 {
  return min(curve, oblateLayersPerBand(body) - 1u);
}

// The transmittance from the lit point to space along the unit `d` under light slot `curve`'s
// curves of growth, exponentiated in f32; 0 where the ground hides space.
fn atmosphereTransmittanceAlong(body : OblateBody, place : SurfacePlace, d : vec3f, curve : u32) -> vec3f {
  let ray = slicedRayOf(body, atmosphere_body, place.start, d);
  if (ray.shadowed) {
    return vec3f(0.0);
  }
  let absorbers = oblateAbsorbersOf(atmosphere_body);
  let stored = slicedStoredOf(
    ray, body, atmosphere_body, atmosphere_transmittance, atmosphere_absorber_columns,
    atmosphere_absorber_pressure_columns, atmosphere_last_absorber_pressure_column, absorbers.x
  );
  let absorbed = absorberDepth(
    atmosphere_absorber_curves, absorbers.x, atmosphereSlot(body, curve), absorbers.y,
    stored.columns, stored.pressureColumns
  );
  return exp(-(stored.depth + absorbed));
}

// A source's transmittance from a lit point `altitude_m` above the datum at geodetic latitude
// `latitude_rad`, the source at zenith cosine `mu_sun` against the datum's normal and azimuth
// `sun_azimuth_rad` from local north towards east, under light slot `curve`'s curves of growth: the
// sliced lookup along the source's centre (a star's disc is under about 1°; Design note 7), 0 where
// the ground hides it; 1 with no atmosphere.
fn atmosphere_sun_transmittance(
  altitude_m : f32,
  mu_sun : f32,
  latitude_rad : f32,
  sun_azimuth_rad : f32,
  curve : u32,
) -> vec3f {
  let body = oblateBodyOf(atmosphere_body);
  if (!atmosphereHasAir(body)) {
    return vec3f(1.0);
  }
  let place = atmosphereSurfacePlace(body, altitude_m, latitude_rad);
  return atmosphereTransmittanceAlong(
    body, place, atmosphereDirection(place, mu_sun, sun_azimuth_rad), curve
  );
}

// The sky's diffuse irradiance on a horizontal surface `altitude_m` above the datum at geodetic
// latitude `latitude_rad`, per unit of a source's illuminance face-on, the source at zenith cosine
// `mu_sun`, under light slot `curve`'s layer: the irradiance table at h* = s h, held at the datum
// below it, and μ_s (Bruneton's mapping, the height's root, `irradiance.wgsl`'s), linear in ln s
// between the latitude bands about the point's s (Design note 17); added below the horizon as above
// it, so that a set source still lights the twilight ground. 0 with no atmosphere.
fn atmosphere_sky_irradiance(
  altitude_m : f32,
  mu_sun : f32,
  latitude_rad : f32,
  curve : u32,
) -> vec3f {
  let body = oblateBodyOf(atmosphere_body);
  if (!atmosphereHasAir(body)) {
    return vec3f(0.0);
  }
  let latitude = atmosphereSinCos(clamp(latitude_rad, -TRIG_HALF_PI, TRIG_HALF_PI));
  let s = gravityRatioAt(body, latitude.x * latitude.x);
  let unit = vec2f(mu_sun * 0.5 + 0.5, sqrt(max(s * altitude_m, 0.0) / body.counts.w));
  return bandTableAt(
    body, atmosphere_body, atmosphere_irradiance, clamp(unit, vec2f(0.0), vec2f(1.0)), s,
    atmosphereSlot(body, curve)
  );
}

// A wide source's direct transmittance (Design note 7, _Planetshine_): Gauss–Chebyshev of the second
// kind over its disc of angular radius `angular_radius_rad`, three nodes along the disc's vertical
// diameter, at the centre and ±(√2 ÷ 2) ρ from it in zenith angle, weighted ¼, ½ and ¼ times each
// node's zenith cosine, exact for a uniform disc to degree 5; each node the sliced lookup along it,
// a node below the local horizon dropped. It is the irradiance-weighted mean transmittance of the
// disc's part above the horizon, Σ w μ T ÷ Σ w μ, which multiplies the caller's own irradiance
// factor of the source, and the centre's lookup as ρ → 0. 0 where no node is up; 1 with no
// atmosphere.
fn atmosphere_source_transmittance(
  altitude_m : f32,
  mu_centre : f32,
  latitude_rad : f32,
  azimuth_rad : f32,
  angular_radius_rad : f32,
  curve : u32,
) -> vec3f {
  let body = oblateBodyOf(atmosphere_body);
  if (!atmosphereHasAir(body)) {
    return vec3f(1.0);
  }
  let place = atmosphereSurfacePlace(body, altitude_m, latitude_rad);
  let centre = atmosphereDirection(place, mu_centre, azimuth_rad);
  let n = place.start.normal;
  // The disc's vertical diameter: the unit direction across the centre towards the zenith.
  let across = n - dot(n, centre) * centre;
  let sinZenith = length(across);
  var up = place.north;
  if (sinZenith > SOURCE_AT_ZENITH) {
    up = across / sinZenith;
  }
  let offset = preciseSinCos(min(abs(angular_radius_rad) * SOURCE_NODE_OFFSET, TRIG_PI));
  var nodes = array<vec3f, 3>(
    offset.y * centre + offset.x * up,
    centre,
    offset.y * centre - offset.x * up,
  );
  var sum = vec3f(0.0);
  var total = 0.0;
  for (var k = 0u; k < 3u; k++) {
    let mu = dot(nodes[k], n);
    if (!(mu > 0.0)) {
      continue;
    }
    let w = SOURCE_NODE_WEIGHTS[k] * mu;
    sum += w * atmosphereTransmittanceAlong(body, place, nodes[k], curve);
    total += w;
  }
  if (!(total > 0.0)) {
    return vec3f(0.0);
  }
  return sum / total;
}
