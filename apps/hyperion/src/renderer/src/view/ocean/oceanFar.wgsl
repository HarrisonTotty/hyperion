// The ocean from space (plan R11, R11.T8.b; Design notes 11 and 12): the sea-level spheroid met
// exactly by a pixel's ray where the body's ocean map holds open water, a closed basin's raised
// spheroid where its texel's level says so, shaded by Cox and Munk's slope distribution filtered per
// pixel and shadowed by Smith's term, Fresnel at the liquid's index, the sky's light, the water's
// own upwelling light and the whitecaps. A library with no binding of its own, composed after shaders/cubeTexel.wgsl, whose
// `class_map_texel` finds a point's texel. Its CPU twin, in f64, is ocean/oceanFar.ts, whose
// remarks give each figure's source; every constant here is its twin's.

const OCEAN_PI : f32 = 3.14159265358979;
// SLOPE_VARIANCE_FLOOR, and Cox and Munk's fits (seaState.ts).
const OCEAN_SLOPE_FLOOR : f32 = 1e-4;
const OCEAN_UPWIND_PER_MPS : f32 = 3.16e-3;
const OCEAN_CROSSWIND_CALM : f32 = 0.003;
const OCEAN_CROSSWIND_PER_MPS : f32 = 1.92e-3;
const OCEAN_COX_MUNK_MAX_MPS : f32 = 20.0;
// Monahan and O'Muircheartaigh's whitecaps, and their effective reflectance (oceanFar.ts).
const OCEAN_WHITECAP_COEFFICIENT : f32 = 3.84e-6;
const OCEAN_WHITECAP_EXPONENT : f32 = 3.41;
const OCEAN_WHITECAP_ALBEDO : f32 = 0.22;
// The upwelling's Q, sr (liquids.ts' UPWELLING_Q_SR), and the footprint's least view cosine.
const OCEAN_UPWELLING_Q : f32 = 3.14159265358979;
const OCEAN_FOOTPRINT_MIN_COSINE : f32 = 0.01;

// What the far ocean reads that is not a texel's: `oceanFar.ts`' `OceanFarOptics`, packed by
// `packOceanFar`, body-fixed.
struct OceanFar {
  // x, y the sea-level spheroid's equatorial and polar radii, m; z a pixel's angle, rad; w 1 with
  // the whitecaps drawn, else 0.
  figure : vec4f,
  // xyz the unit direction to the sun; w the glint's cap, cd/m².
  sun : vec4f,
  // xyz the sun's illuminance normal to its rays, lux, the body's own (the probe's light); w the
  // generation threshold, m/s.
  sunLux : vec4f,
  // xyz the sky's irradiance on the horizontal, lux, the body's own; w U_eq per 10 m wind.
  skyLux : vec4f,
  // xyz the liquid's index; w Cox and Munk's 12.5 m equivalent wind per 10 m wind.
  index : vec4f,
  // xyz a + b_b, 1/m; w 1 where the absorption is modelled, else 0 (an opaque column).
  attenuation : vec4f,
  // xyz R∞, 0 under Beer–Lambert alone; w R̄, the flat surface's reflectance of a uniform sky.
  deep : vec4f,
  // xyz the bed's albedo; w unused.
  bed : vec4f,
}

// Where a ray first meets a spheroid from outside.
struct OceanSpheroidHit {
  found : bool,
  // The distance along the ray, m.
  t : f32,
  // The point from the body's centre, m, and its outward geodetic normal.
  point : vec3f,
  normal : vec3f,
}

// A ray from `origin` (from the body's centre, m) along the unit `dir` against the spheroid of
// equatorial radius `a` and polar radius `c` about z: in the stretched space z a / c, a sphere of
// radius a, the near root t = (|o'|^2 - a^2) / (b + sqrt(disc)), free of b - sqrt(disc)'s
// cancellation (`raySpheroidHit`).
fn ocean_spheroid_hit(origin : vec3f, dir : vec3f, a : f32, c : f32) -> OceanSpheroidHit {
  let k = a / c;
  let o = vec3f(origin.xy, origin.z * k);
  let d = vec3f(dir.xy, dir.z * k);
  let dd = dot(d, d);
  let along = -dot(o, d);
  let distance = length(o);
  let excess = (distance - a) * (distance + a);
  let disc = along * along - dd * excess;
  var out : OceanSpheroidHit;
  out.found = excess > 0.0 && along > 0.0 && disc >= 0.0;
  if (!out.found) {
    return out;
  }
  out.t = excess / (along + sqrt(disc));
  out.point = origin + out.t * dir;
  out.normal = normalize(vec3f(out.point.xy / (a * a), out.point.z / (c * c)));
  return out;
}

// Unpolarised Fresnel reflectance at index n and incidence cosine c (`fresnelReflectance`).
fn ocean_fresnel(n : f32, cosIncidence : f32) -> f32 {
  let c = clamp(cosIncidence, 0.0, 1.0);
  let g = sqrt(n * n - 1.0 + c * c);
  let a = (g - c) / (g + c);
  let b = (c * (g + c) - 1.0) / (c * (g - c) + 1.0);
  return 0.5 * a * a * (1.0 + b * b);
}

fn ocean_fresnel3(n : vec3f, cosIncidence : f32) -> vec3f {
  return vec3f(
    ocean_fresnel(n.x, cosIncidence),
    ocean_fresnel(n.y, cosIncidence),
    ocean_fresnel(n.z, cosIncidence),
  );
}

// Snell's law: the refracted cosine into index n (`refractedCosine`).
fn ocean_refracted_cosine(n : vec3f, cosIncidence : f32) -> vec3f {
  let c = clamp(cosIncidence, 0.0, 1.0);
  return sqrt(vec3f(1.0) - vec3f(1.0 - c * c) / (n * n));
}

// Cox and Munk's variances along and across a 10 m wind, at the floor below the generation
// threshold (`oceanSlopeVariance`).
fn ocean_slope_variance(wind10 : f32, far : OceanFar) -> vec2f {
  if (!(wind10 >= far.sunLux.w)) {
    return vec2f(OCEAN_SLOPE_FLOOR);
  }
  let u = clamp(far.index.w * wind10, 0.0, OCEAN_COX_MUNK_MAX_MPS);
  return max(
    vec2f(OCEAN_UPWIND_PER_MPS * u, OCEAN_CROSSWIND_CALM + OCEAN_CROSSWIND_PER_MPS * u),
    vec2f(OCEAN_SLOPE_FLOOR),
  );
}

// The whitecap fraction under a 10 m wind, on the high setting (`oceanWhitecapFraction`).
fn ocean_whitecaps(wind10 : f32, far : OceanFar) -> f32 {
  let eq = far.skyLux.w * wind10;
  if (far.figure.w < 0.5 || !(wind10 >= far.sunLux.w) || !(eq > 0.0)) {
    return 0.0;
  }
  return min(OCEAN_WHITECAP_COEFFICIENT * pow(eq, OCEAN_WHITECAP_EXPONENT), 1.0);
}

// The local east and north at a unit normal: east = z x up, +y at a pole (`eastNorthOf`).
fn ocean_east(normal : vec3f) -> vec3f {
  let raw = cross(vec3f(0.0, 0.0, 1.0), normal);
  return select(vec3f(0.0, 1.0, 0.0), normalize(raw), length(raw) > 1e-6);
}

// erfc x for x >= 0, Bruneton et al. 2010's approximation (`erfcApprox`).
fn ocean_erfc(x : f32) -> f32 {
  return 2.0 * exp(-x * x) / (2.319 * x + sqrt(4.0 + 1.52 * x * x));
}

// Smith's Lambda for a Gaussian of variance `variance` along a direction of cosine `mu`
// (`smithLambda`).
fn ocean_smith_lambda(mu : f32, variance : f32) -> f32 {
  let sin2 = 1.0 - mu * mu;
  if (!(sin2 > 0.0)) {
    return 0.0;
  }
  let nu = mu / sqrt(2.0 * variance * sin2);
  let root = sqrt(OCEAN_PI);
  return max((exp(-nu * nu) - nu * root * ocean_erfc(nu)) / (2.0 * nu * root), 0.0);
}

// The slope variance along the azimuth of `d` about `normal` (`varianceToward`).
fn ocean_variance_toward(d : vec3f, normal : vec3f, upwind : vec3f, variance : vec2f) -> f32 {
  let tangent = d - normal * dot(d, normal);
  let length2 = dot(tangent, tangent);
  if (!(length2 > 0.0)) {
    return variance.x;
  }
  let c2 = min(dot(tangent, upwind) * dot(tangent, upwind) / length2, 1.0);
  return variance.x * c2 + variance.y * (1.0 - c2);
}

// Smith's shadowing and masking, 1 / (1 + Lambda(mu_v) + Lambda(mu_s)) (`smithShadowing`).
fn ocean_shadowing(view : vec3f, sun : vec3f, normal : vec3f, upwind : vec3f, variance : vec2f) -> f32 {
  let viewLambda = ocean_smith_lambda(
    dot(normal, view),
    ocean_variance_toward(view, normal, upwind, variance),
  );
  let sunLambda = ocean_smith_lambda(
    dot(normal, sun),
    ocean_variance_toward(sun, normal, upwind, variance),
  );
  return 1.0 / (1.0 + viewLambda + sunLambda);
}

// The glint's factor, sr^-1, per channel: F(v.h) p(zeta) / (4 mu_v cos^4 beta), shadowed
// (`oceanGlintFactor`).
fn ocean_glint_factor(
  view : vec3f,
  sun : vec3f,
  normal : vec3f,
  upwind : vec3f,
  variance : vec2f,
  index : vec3f,
) -> vec3f {
  let muV = dot(normal, view);
  let muS = dot(normal, sun);
  if (!(muV > 0.0 && muS > 0.0)) {
    return vec3f(0.0);
  }
  let halfVector = normalize(view + sun);
  let cosBeta = dot(normal, halfVector);
  let across = cross(normal, upwind);
  let zu = dot(halfVector, upwind) / cosBeta;
  let zc = dot(halfVector, across) / cosBeta;
  let density = exp(-0.5 * (zu * zu / variance.x + zc * zc / variance.y))
    / (2.0 * OCEAN_PI * sqrt(variance.x * variance.y));
  let cos2 = cosBeta * cosBeta;
  let geometry = density * ocean_shadowing(view, sun, normal, upwind, variance)
    / (4.0 * muV * cos2 * cos2);
  return ocean_fresnel3(index, dot(view, halfVector)) * geometry;
}

// An ocean map's texel (`ocean/oceanMap.ts`' layout): open water where its depth is above 0.
struct OceanTexel {
  open : bool,
  depth : f32,
  level : f32,
  // East and north, m/s, at 10 m.
  wind : vec2f,
  // (i, j, face), `class_map_texel`'s.
  cell : vec3u,
}

fn ocean_texel(oceanMap : texture_2d_array<f32>, p : vec3f) -> OceanTexel {
  let n = textureDimensions(oceanMap).x;
  let cell = class_map_texel(p, n);
  let v = textureLoad(oceanMap, vec2i(cell.xy), i32(cell.z), 0);
  var out : OceanTexel;
  out.open = v.x > 0.0;
  out.depth = v.x;
  out.level = v.y;
  out.wind = v.zw;
  out.cell = cell;
  return out;
}

// The light on the sea at one point, lux (`OceanPointLight`): the `ocean` pass takes it from R08's
// surfaceLighting.wgsl at the hit; the probe from `OceanFar`'s own.
struct OceanLight {
  // The sun's illuminance on a plane normal to its rays.
  sunLux : vec3f,
  // The sky's irradiance on the horizontal.
  skyLux : vec3f,
}

// The far ocean's radiance at a point of open water under `light`, cd/m^2, the glint capped
// (`oceanFarLight`).
fn ocean_far_light(
  far : OceanFar,
  light : OceanLight,
  view : vec3f,
  normal : vec3f,
  texel : OceanTexel,
  t : f32,
) -> vec3f {
  let sun = far.sun.xyz;
  let east = ocean_east(normal);
  let north = cross(normal, east);
  let windVector = east * texel.wind.x + north * texel.wind.y;
  let wind10 = length(windVector);
  let upwind = select(east, windVector / wind10, wind10 > 0.0);
  let muV = dot(normal, view);
  let muS = max(dot(normal, sun), 0.0);
  let footprint = t * far.figure.z / sqrt(max(muV, OCEAN_FOOTPRINT_MIN_COSINE));
  let spread = footprint / far.figure.x;
  let variance = ocean_slope_variance(wind10, far) + vec2f(spread * spread / 12.0);
  let index = far.index.xyz;
  let glint = light.sunLux * ocean_glint_factor(view, sun, normal, upwind, variance, index);
  let fresnelView = ocean_fresnel3(index, muV);
  let sky = fresnelView * light.skyLux / OCEAN_PI;
  let down = light.sunLux * muS + light.skyLux;
  var water = vec3f(0.0);
  if (far.attenuation.w > 0.5) {
    let kd = far.attenuation.xyz / ocean_refracted_cosine(index, muS);
    let deep = far.deep.xyz;
    let reflectance = deep + (far.bed.xyz - deep) * exp(-2.0 * kd * max(texel.depth, 0.0));
    // The sun's light through the surface at its own incidence, the sky's at a uniform sky's.
    let entering = light.sunLux * muS * (vec3f(1.0) - ocean_fresnel3(index, muS))
      + light.skyLux * (1.0 - far.deep.w);
    water = (vec3f(1.0) - fresnelView) * reflectance * entering / (index * index * OCEAN_UPWELLING_Q);
  }
  let whitecaps = ocean_whitecaps(wind10, far);
  let foam = OCEAN_WHITECAP_ALBEDO * down / OCEAN_PI;
  return (1.0 - whitecaps) * (min(glint, vec3f(far.sun.w)) + sky + water) + whitecaps * foam;
}

// What a ray meets of the ocean, before it is lit.
struct OceanFarHit {
  found : bool,
  t : f32,
  // The point from the body's centre, m, and its outward geodetic normal.
  point : vec3f,
  normal : vec3f,
  texel : OceanTexel,
}

// A ray from `origin` (from the body's centre, body-fixed, m) along the unit `dir` against the
// ocean (`oceanFarSample`'s hit): the sea-level spheroid where its texel is open water, a closed
// basin's raised one where the texel's level is not 0 and that hit stays in the texel's cell; none
// elsewhere, over dry ground, sea ice and every unsurveyed cell. The `ocean` pass lights the hit
// with its own point's light (`ocean_far_light`).
fn ocean_far_hit(
  oceanMap : texture_2d_array<f32>,
  far : OceanFar,
  origin : vec3f,
  dir : vec3f,
) -> OceanFarHit {
  var out : OceanFarHit;
  let a = far.figure.x;
  let c = far.figure.y;
  let sea = ocean_spheroid_hit(origin, dir, a, c);
  if (!sea.found) {
    return out;
  }
  let texel = ocean_texel(oceanMap, sea.point);
  if (!texel.open) {
    return out;
  }
  var hit = sea;
  if (texel.level != 0.0) {
    let lake = ocean_spheroid_hit(origin, dir, a + texel.level, c + texel.level);
    if (!lake.found) {
      return out;
    }
    let there = class_map_texel(lake.point, textureDimensions(oceanMap).x);
    if (any(there != texel.cell)) {
      return out;
    }
    hit = lake;
  }
  out.found = true;
  out.t = hit.t;
  out.point = hit.point;
  out.normal = hit.normal;
  out.texel = texel;
  return out;
}

// What a ray meets of the ocean, lit by the body's own light (`oceanFarSample`).
struct OceanFarSample {
  found : bool,
  t : f32,
  point : vec3f,
  radiance : vec3f,
}

fn ocean_far_sample(
  oceanMap : texture_2d_array<f32>,
  far : OceanFar,
  origin : vec3f,
  dir : vec3f,
) -> OceanFarSample {
  var out : OceanFarSample;
  let hit = ocean_far_hit(oceanMap, far, origin, dir);
  if (!hit.found) {
    return out;
  }
  out.found = true;
  out.t = hit.t;
  out.point = hit.point;
  let light = OceanLight(far.sunLux.xyz, far.skyLux.xyz);
  out.radiance = ocean_far_light(far, light, -dir, hit.normal, hit.texel, hit.t);
  return out;
}
