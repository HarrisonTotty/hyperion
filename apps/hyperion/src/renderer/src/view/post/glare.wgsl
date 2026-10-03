// Veiling glare in the image's last pass (plan R07, T14.b, Design note 12): the bloom chain's
// light above the display's range, gathered back at full resolution, and each glare source's veil
// in closed form, in f32. A library: the tone-mapping pass (`tonemap.wgsl`) and the smoke check's
// composite concatenate it ahead of their own source. Its TypeScript twins are `bloomChain`'s last
// step in `bloom.ts` and `glareSourceVeilOf` in `glare.ts`.

// One glare source, as `packGlareSources` packs it: the unit direction to its centre in the
// scene's camera-relative frame and its angular radius, rad; its excess luminance per channel in
// the target's units and its solid angle, sr; and each narrow term's level inside the disc for a
// term of amplitude 1 (`rectangleInsideLevel`).
struct GlareSourceGpu {
  direction : vec3f,
  angularRadius : f32,
  excess : vec3f,
  solidAngle : f32,
  inside : vec4f,
}

// A view's spread function as `glareSpreadTerms` gives it: three narrow terms (amplitude, scale c
// in rad), the Lorentz and root terms, then the quadratic, constant and Gaussian ones.
struct GlareTerms {
  poisson0 : vec4f,
  poisson1 : vec4f,
  poisson2 : vec4f,
  // Lorentz amplitude and scale, root amplitude and scale.
  broad : vec4f,
  // Quadratic (per rad^2), constant, Gaussian amplitude and sigma (rad).
  misc : vec4f,
}

const GLARE_PI = 3.14159265358979;
// `POINT_SOURCE_FRACTION` in `glare.ts`.
const POINT_SOURCE_FRACTION = 0.01;

// The light above the threshold, per channel: L - min(L, T).
fn bloom_excess(value : vec3f, threshold : f32) -> vec3f {
  return value - min(value, vec3f(threshold));
}

// A bilinear sample of `level` at continuous texel coordinates (texel n's centre at n + 0.5),
// clamped, from four loads.
fn bloom_bilinear(level : texture_2d<f32>, x : f32, y : f32) -> vec3f {
  let size = vec2i(textureDimensions(level));
  let u = x - 0.5;
  let v = y - 0.5;
  let i0 = i32(floor(u));
  let j0 = i32(floor(v));
  let fu = u - floor(u);
  let fv = v - floor(v);
  let hi = size - 1;
  let a = textureLoad(level, clamp(vec2i(i0, j0), vec2i(0), hi), 0).rgb;
  let b = textureLoad(level, clamp(vec2i(i0 + 1, j0), vec2i(0), hi), 0).rgb;
  let c = textureLoad(level, clamp(vec2i(i0, j0 + 1), vec2i(0), hi), 0).rgb;
  let d = textureLoad(level, clamp(vec2i(i0 + 1, j0 + 1), vec2i(0), hi), 0).rgb;
  return (1.0 - fv) * ((1.0 - fu) * a + fu * b) + fv * ((1.0 - fu) * c + fu * d);
}

// The 3 x 3 tent upsample of `coarse` at the output pixel whose centre is `fragment`, in an output
// of `outputSize` pixels.
fn bloom_tent(coarse : texture_2d<f32>, fragment : vec2f, outputSize : vec2f) -> vec3f {
  let centre = fragment * vec2f(textureDimensions(coarse)) / outputSize;
  let offsets = array<f32, 3>(-1.0, 0.0, 1.0);
  let tent = array<f32, 3>(0.25, 0.5, 0.25);
  var up = vec3f(0.0);
  for (var j = 0; j < 3; j++) {
    for (var i = 0; i < 3; i++) {
      up += tent[i] * tent[j] * bloom_bilinear(coarse, centre.x + offsets[i], centre.y + offsets[j]);
    }
  }
  return up;
}

// The unit view-space direction through a pixel, from the pass's projection: R02's
// reversed-infinite perspective puts 1 / tan(half field) on its diagonal.
fn glare_pixel_direction(fragment : vec2f, viewport : vec4f, projection : mat4x4f) -> vec3f {
  let ndc = vec2f(fragment.x * viewport.z * 2.0 - 1.0, 1.0 - fragment.y * viewport.w * 2.0);
  return normalize(vec3f(ndc.x / projection[0][0], ndc.y / projection[1][1], -1.0));
}

// The angle between two unit vectors, rad, accurate at small angles where acos is not.
fn glare_angle(a : vec3f, b : vec3f) -> f32 {
  return atan2(length(cross(a, b)), dot(a, b));
}

fn glare_broad(terms : GlareTerms, theta : f32) -> f32 {
  let lorentz = terms.broad.x / (1.0 + (theta / terms.broad.y) * (theta / terms.broad.y));
  let root = terms.broad.z / sqrt(1.0 + (theta / terms.broad.w) * (theta / terms.broad.w));
  let gaussian = terms.misc.z * exp(-0.5 * (theta / terms.misc.w) * (theta / terms.misc.w));
  return lorentz + root + terms.misc.x * theta * theta + terms.misc.y + gaussian;
}

// One narrow term over the source: the rectangle's closed form beyond the limb and the level that
// keeps the energy inside it, or the point form for a source below `POINT_SOURCE_FRACTION` of the
// term's scale.
fn glare_poisson(term : vec4f, theta : f32, rho : f32, solidAngle : f32, inside : f32) -> f32 {
  let a = term.x;
  let c = term.y;
  if (a == 0.0) {
    return 0.0;
  }
  if (rho < POINT_SOURCE_FRACTION * c) {
    let q = 1.0 + (theta / c) * (theta / c);
    return solidAngle * a / (q * sqrt(q));
  }
  if (theta < rho) {
    return a * inside;
  }
  // F(x2) - F(x1) without cancellation (`poissonOverRectangle`): beyond the limb x1 >= 0, and
  // x2^2 - x1^2 = 4 rho theta exactly.
  let y = GLARE_PI * rho / 4.0;
  let x1 = theta - rho;
  let x2 = theta + rho;
  let k = c * c + y * y;
  let s1 = sqrt(k + x1 * x1);
  let s2 = sqrt(k + x2 * x2);
  let u1 = (x1 * y) / (c * s1);
  let u2 = (x2 * y) / (c * s2);
  let numerator = (y / c) * (4.0 * rho * theta) * k / ((x2 * s1 + x1 * s2) * s1 * s2);
  return a * c * c * 2.0 * atan(numerator / (1.0 + u1 * u2));
}

// The veil a source adds at the pixel in direction `pixel`, per channel.
fn glare_veil(source : GlareSourceGpu, pixel : vec3f, terms : GlareTerms) -> vec3f {
  let theta = glare_angle(source.direction, pixel);
  let rho = source.angularRadius;
  var perUnit = source.solidAngle * glare_broad(terms, theta);
  perUnit += glare_poisson(terms.poisson0, theta, rho, source.solidAngle, source.inside.x);
  perUnit += glare_poisson(terms.poisson1, theta, rho, source.solidAngle, source.inside.y);
  perUnit += glare_poisson(terms.poisson2, theta, rho, source.solidAngle, source.inside.z);
  return source.excess * perUnit;
}
