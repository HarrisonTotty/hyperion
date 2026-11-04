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
//     band and light slot (`irradiance.wgsl`);
//   - `atmosphere_below_deck : texture_2d_array<f32>`, the table below a cloud deck over a surface
//     (plan R08, R08.T15.b.2; `belowDeck.wgsl`), or a one-texel stub where the body has none;
//   - `atmosphere_source_table : texture_2d_array<f32>`, the sources' transmittance by height and
//     cosine, one layer a latitude band and light slot (`sourceTransmittance.wgsl`; R08.T9.b's
//     lever, `AtmosphereSettings.sourceTransmittance`'s `table`), or a one-texel stub, through
//     which each source's transmittance is the sliced lookup per fragment, as before the lever.
// A body without an atmosphere binds one-texel tables of zeros, whose body table's a of 0 says so:
// the sun's transmittance is then 1 and the sky's irradiance 0, as R07's stubs were.
//
// Beneath a cloud deck (R08.T15.b.2), where the bound table below a deck holds one, the lookups take
// the caller's height above the body's own datum, which the tables' datum, the deck's level surface,
// lies the split's h* over: a point whose h* = s h is under the split's reads the table below, the
// sky's irradiance its downward flux the kernels' beam leaves out and a source's transmittance the
// kernels' beam through the column (`belowDeck.wgsl`'s belowDeckLightAt), and a point above it the
// tables at h less the split's h* over s. With no deck the split is 0 and every lookup is as before.
// Composed after `belowDeck.wgsl` too.
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

// The inspection profile's probe of the lit passes' cost (`surfaceLighting.ts`' `litProbedWgsl`,
// R08.T9.b's measurement): 0, the lookups as drawn, on every launch but a `--lit-probe` profile's,
// whose materials and kernels set 1, every source's transmittance 1 with no lookup; 2, a
// planetshine source's at its centre node alone; 3, the eclipse term with no annuli
// (`bodyDisc.wgsl`'s and `terrainLit.wgsl`'s), every star unhidden; or 4, each source's
// transmittance per fragment though the sources' table is bound (`atmosphereTabled`), the lever's
// way back in the same build. Never a player's picture but under 4, the way back's.
const LIT_PROBE : u32 = 0u;

// Whether this module is a lit module's table-only twin (R08.T9.b's lever; `sourceTableEngine.ts`):
// false as written; true in the twin a lit pass draws by where its body's one-band sources' table
// is bound, whose every lookup that table holds, so that the sliced lookup a source per fragment is
// compiled out of it, its arithmetic else the module's own.
const SOURCE_TABLE_ONLY : bool = false;

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

// The lit point beneath a cloud deck (R08.T15.b.2): its h* and s where the bound table below a deck
// holds one and the point's h* = s h lies under the split's, else (-1, s).
fn atmosphereBelowDeckPlace(body : OblateBody, altitude_m : f32, latitude_rad : f32) -> vec2f {
  let latitude = atmosphereSinCos(clamp(latitude_rad, -TRIG_HALF_PI, TRIG_HALF_PI));
  let s = gravityRatioAt(body, latitude.x * latitude.x);
  let split = belowDeckSplitOf(atmosphere_below_deck);
  let z = s * altitude_m;
  if (split > 0.0 && z < split) {
    return vec2f(max(z, 0.0), s);
  }
  return vec2f(-1.0, s);
}

// A lit point's height over the tables' datum: its height above the body's own less the split's h*
// over s, the deck's level surface's height there (R08.T15.b.2); itself with no deck.
fn atmosphereAboveDeck(altitude_m : f32, s : f32) -> f32 {
  return altitude_m - belowDeckSplitOf(atmosphere_below_deck) / s;
}

// The light slot a lookup reads: `curve`, held to the body's light slots (its multiple-scattering
// and irradiance layers a band; one for a medium with no absorber, which reads no curve).
fn atmosphereSlot(body : OblateBody, curve : u32) -> u32 {
  return min(curve, oblateLayersPerBand(body) - 1u);
}

// The optical depth along a sliced ray to space under light slot `curve`'s curves of growth: its
// stored depth over s and the absorbers' by the curves, whether or not the ground hides space.
fn atmosphereDepthOf(body : OblateBody, ray : SlicedRay, curve : u32) -> vec3f {
  let absorbers = oblateAbsorbersOf(atmosphere_body);
  let stored = slicedStoredOf(
    ray, body, atmosphere_body, atmosphere_transmittance, atmosphere_absorber_columns,
    atmosphere_absorber_pressure_columns, atmosphere_last_absorber_pressure_column, absorbers.x
  );
  let absorbed = absorberDepth(
    atmosphere_absorber_curves, absorbers.x, atmosphereSlot(body, curve), absorbers.y,
    stored.columns, stored.pressureColumns
  );
  return stored.depth + absorbed;
}

// The transmittance from the lit point to space along the unit `d` under light slot `curve`'s
// curves of growth, exponentiated in f32; 0 where the ground hides space.
fn atmosphereTransmittanceAlong(body : OblateBody, place : SurfacePlace, d : vec3f, curve : u32) -> vec3f {
  let ray = slicedRayOf(body, atmosphere_body, place.start, d);
  if (ray.shadowed) {
    return vec3f(0.0);
  }
  return exp(-atmosphereDepthOf(body, ray, curve));
}

// Whether the bound sources' table holds the transmittance (R08.T9.b's lever): any table but the
// one-texel stub, through which each lookup is the sliced lookup per fragment, as under the lit
// probe's `per-fragment` form.
fn atmosphereTabled() -> bool {
  return atmosphereTableOnly() || (LIT_PROBE != 4u && textureDimensions(atmosphere_source_table).x > 1u);
}

// Whether this module is the table-only twin (`SOURCE_TABLE_ONLY`), but under the lit probe's
// `per-fragment` form, which takes the sliced lookup in either.
fn atmosphereTableOnly() -> bool {
  return SOURCE_TABLE_ONLY && LIT_PROBE != 4u;
}

// Within this of the horizon's zenith cosine a body of several bands takes a source's transmittance
// per fragment, though its table is bound (lean, 2026-11-03): the table's rays stand at one azimuth
// (`sourceTransmittance.wgsl`), and on the Saturn-class figure (5 slices, 4 bands) its reads missed
// 1/3 stop at 581 of 6,890 lit points, every one within 0.05 of the horizon, the azimuth's split
// of a grazing path across the bands the most of it; on Earth's one band it held 99.96% of them.
const SOURCE_TABLE_HORIZON_COSINE : f32 = 0.1;

// Whether a source at zenith cosine `mu` from h* `scaled_height_m` takes the bound sources' table:
// on a body of one band always (the table-only twin draws no other), on one of several unless it
// lies within SOURCE_TABLE_HORIZON_COSINE of the horizon of the reference sphere R_ref.
fn atmosphereTableHolds(body : OblateBody, scaled_height_m : f32, mu : f32) -> bool {
  if (atmosphereTableOnly() || oblateBandCount(body) == 1u) {
    return true;
  }
  let bottom = body.figure.z;
  let z = max(scaled_height_m, 0.0);
  let horizon = -sqrt(shellAboveGround(bottom, z)) / (bottom + z);
  return mu - horizon >= SOURCE_TABLE_HORIZON_COSINE;
}

// A source's transmittance from the bound sources' table (`sourceTransmittance.wgsl`): its optical
// depth at the point's h* = s h above the tables' datum (`above_deck_m`, held to [0, top]) and the
// source's cosine `mu` on the reference sphere's axes, linear in ln s between the bands about the
// point's s as the sky's irradiance is (`bandTableAt`), at light slot `curve`, exponentiated in f32;
// 0 where the reference sphere's ground hides the source.
fn atmosphereTabledTransmittance(
  body : OblateBody,
  s : f32,
  above_deck_m : f32,
  mu : f32,
  curve : u32,
) -> vec3f {
  let bottom = body.figure.z;
  let top = body.counts.w;
  let z = clamp(s * above_deck_m, 0.0, top);
  if (groundHitFromHeight(bottom, z, mu)) {
    return vec3f(0.0);
  }
  let unit = transmittanceUnitAt(bottom, top, z, clamp(mu, -1.0, 1.0));
  return exp(-bandTableAt(
    body, atmosphere_body, atmosphere_source_table, unit, s, atmosphereSlot(body, curve)
  ));
}

// A source's transmittance from a lit point `altitude_m` above the datum at geodetic latitude
// `latitude_rad`, the source at zenith cosine `mu_sun` against the datum's normal and azimuth
// `sun_azimuth_rad` from local north towards east, under light slot `curve`'s curves of growth: the
// sliced lookup along the source's centre (a star's disc is under about 1°; Design note 7), or the
// sources' table where one is bound (`atmosphereTabledTransmittance`), 0 where the ground hides it;
// 1 with no atmosphere. Each way is a loop of at most one pass, not an `if`, as the composite's
// reads are (`composite.wgsl`): SwiftShader runs both sides of a branch under its lanes' mask.
fn atmosphere_sun_transmittance(
  altitude_m : f32,
  mu_sun : f32,
  latitude_rad : f32,
  sun_azimuth_rad : f32,
  curve : u32,
) -> vec3f {
  let body = oblateBodyOf(atmosphere_body);
  if (!atmosphereHasAir(body) || LIT_PROBE == 1u) {
    return vec3f(1.0);
  }
  let below = atmosphereBelowDeckPlace(body, altitude_m, latitude_rad);
  if (below.x >= 0.0) {
    return belowDeckLightAt(
      atmosphere_below_deck, body, atmosphere_body, below.y, below.x, mu_sun, curve
    ).sun;
  }
  let aboveDeck = atmosphereAboveDeck(altitude_m, below.y);
  let tabled = atmosphereTabled() && atmosphereTableHolds(body, below.y * aboveDeck, mu_sun);
  var transmitted = vec3f(0.0);
  for (var read = tabled; read; read = false) {
    transmitted = atmosphereTabledTransmittance(body, below.y, aboveDeck, mu_sun, curve);
  }
  for (var direct = !tabled; direct; direct = false) {
    let place = atmosphereSurfacePlace(body, aboveDeck, latitude_rad);
    transmitted = atmosphereTransmittanceAlong(
      body, place, atmosphereDirection(place, mu_sun, sun_azimuth_rad), curve
    );
  }
  return transmitted;
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
  let below = atmosphereBelowDeckPlace(body, altitude_m, latitude_rad);
  if (below.x >= 0.0) {
    return belowDeckLightAt(
      atmosphere_below_deck, body, atmosphere_body, below.y, below.x, mu_sun, curve
    ).sky;
  }
  let s = below.y;
  let unit = vec2f(
    mu_sun * 0.5 + 0.5, sqrt(max(s * atmosphereAboveDeck(altitude_m, s), 0.0) / body.counts.w)
  );
  return bandTableAt(
    body, atmosphere_body, atmosphere_irradiance, clamp(unit, vec2f(0.0), vec2f(1.0)), s,
    atmosphereSlot(body, curve)
  );
}

// A wide source's direct transmittance (Design note 7, _Planetshine_): Gauss–Chebyshev of the second
// kind over its disc of angular radius `angular_radius_rad`, three nodes along the disc's vertical
// diameter, at the centre and ±(√2 ÷ 2) ρ from it in zenith angle, weighted ¼, ½ and ¼ times each
// node's zenith cosine, exact for a uniform disc to degree 5; each node the sliced lookup along it,
// a node below the local horizon dropped, or each from the sources' table where one is bound. It is
// the irradiance-weighted mean transmittance of the disc's part above the horizon, Σ w μ T ÷ Σ w μ,
// which multiplies the caller's own irradiance factor of the source, and the centre's lookup as
// ρ → 0. 0 where no node is up; 1 with no atmosphere.
fn atmosphere_source_transmittance(
  altitude_m : f32,
  mu_centre : f32,
  latitude_rad : f32,
  azimuth_rad : f32,
  angular_radius_rad : f32,
  curve : u32,
) -> vec3f {
  let body = oblateBodyOf(atmosphere_body);
  if (!atmosphereHasAir(body) || LIT_PROBE == 1u) {
    return vec3f(1.0);
  }
  let below = atmosphereBelowDeckPlace(body, altitude_m, latitude_rad);
  let aboveDeck = atmosphereAboveDeck(altitude_m, below.y);
  let place = atmosphereSurfacePlace(body, aboveDeck, latitude_rad);
  let tabled = atmosphereTabled();
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
    // The centre node alone under the probe's planetshine form.
    if (!(mu > 0.0) || (LIT_PROBE == 2u && k != 1u)) {
      continue;
    }
    let w = SOURCE_NODE_WEIGHTS[k] * mu;
    // Beneath a deck each node's the table below's beam at its cosine (R08.T15.b.2).
    if (below.x >= 0.0) {
      sum += w * belowDeckLightAt(
        atmosphere_below_deck, body, atmosphere_body, below.y, below.x, mu, curve
      ).sun;
    } else {
      var transmitted = vec3f(0.0);
      let held = tabled && atmosphereTableHolds(body, below.y * aboveDeck, mu);
      for (var read = held; read; read = false) {
        transmitted = atmosphereTabledTransmittance(body, below.y, aboveDeck, mu, curve);
      }
      for (var direct = !held; direct; direct = false) {
        transmitted = atmosphereTransmittanceAlong(body, place, nodes[k], curve);
      }
      sum += w * transmitted;
    }
    total += w;
  }
  if (!(total > 0.0)) {
    return vec3f(0.0);
  }
  return sum / total;
}
