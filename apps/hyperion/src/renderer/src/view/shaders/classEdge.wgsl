// The edges between a surface's classes as R07's disc and R10's terrain draw them (lane
// fix-jagged-biome-edges): a boundary between lattice points of different classes given a natural,
// deterministic sub-texel shape by a fractal gradient noise of the world's seed, about
// EDGE_WIDTH_PX wide and never a blur wider than one lattice spacing. A library with no binding,
// composed by concatenation before bodyDisc.wgsl (R07's lit bodies) and before terrainLit.wgsl
// (R10's lit terrain), each of which applies it to its own laws' shares (`edge_shape_disc`,
// `edge_shape_terrain`). The TypeScript twin, in f64, is `bodies/classEdge.ts`, whose remarks give
// the rule; the constants are its.

const EDGE_BASE_WAVELENGTH_RAD : f32 = 0.024543692606170259;
const EDGE_OCTAVES : u32 = 11u;
const EDGE_OCTAVE_GAIN : f32 = 0.5946035575013605;
const EDGE_FADE_FROM_PX : f32 = 2.0;
const EDGE_FADE_TO_PX : f32 = 4.0;
const EDGE_NOISE_WEIGHT : f32 = 1.1;
const EDGE_WIDTH_PX : f32 = 1.25;
const EDGE_SHARPNESS_MAX : f32 = 64.0;
const EDGE_NOISE_SLOPE_RMS : f32 = 0.694;
// EDGE_NOISE_RMS (0.2702) × √(Σ gain^2o over the octaves), the octaves' whole sum's deviation.
const EDGE_NOISE_SIGMA : f32 = 0.3360598821671867;
// 2 ln 9, the logit's 10%–90% rise, and 4, a bilinear share's logit slope a lattice spacing at its
// midpoint.
const EDGE_RISE_LOGIT : f32 = 4.394449154672439;
const EDGE_MIDPOINT_SLOPE : f32 = 4.0;
const EDGE_OCTAVE_OFFSET : vec3f = vec3f(0.618034, 0.414214, 0.732051);
// The datum's μ below which a pixel's footprint stops growing: so that the limb's foreshortened
// pixels take a finite one.
const EDGE_GRAZING_MU : f32 = 0.2;
// The lattice spacings, px: at and below the first a boundary is drawn from the box-smoothed
// fractions whole, and from the second from the lattice's own blend alone (`edgeSmoothing`).
const EDGE_SMOOTH_FULL_PX : f32 = 3.0;
const EDGE_SMOOTH_NONE_PX : f32 = 6.0;

// Wellons' `lowbias32`: a 32-bit word's avalanche (`classEdge.ts`' `mix32`).
fn edge_mix32(x : u32) -> u32 {
  var h = x;
  h = h ^ (h >> 16u);
  h = h * 0x7feb352du;
  h = h ^ (h >> 15u);
  h = h * 0x846ca68bu;
  h = h ^ (h >> 16u);
  return h;
}

// The hash of lattice corner `c` under `key` (`edgeHash`).
fn edge_hash(c : vec3i, key : u32) -> u32 {
  var h = edge_mix32(key ^ bitcast<u32>(c.x));
  h = edge_mix32(h ^ bitcast<u32>(c.y));
  return edge_mix32(h ^ bitcast<u32>(c.z));
}

// Perlin's (2002) gradient of `h`'s low four bits, dotted with `p` (`gradientDot`).
fn edge_gradient_dot(h : u32, p : vec3f) -> f32 {
  let g = h & 15u;
  let u = select(p.y, p.x, g < 8u);
  let v = select(select(p.z, p.x, g == 12u || g == 14u), p.y, g < 4u);
  return select(-u, u, (g & 1u) == 0u) + select(-v, v, (g & 2u) == 0u);
}

// Improved gradient noise (Perlin 2002) at `x`, lattice units, of `key` (`gradientNoise`).
fn edge_gradient_noise(x : vec3f, key : u32) -> f32 {
  let i = floor(x);
  let f = x - i;
  let c = vec3i(i);
  let n000 = edge_gradient_dot(edge_hash(c, key) >> 28u, f);
  let n100 = edge_gradient_dot(edge_hash(c + vec3i(1, 0, 0), key) >> 28u, f - vec3f(1.0, 0.0, 0.0));
  let n010 = edge_gradient_dot(edge_hash(c + vec3i(0, 1, 0), key) >> 28u, f - vec3f(0.0, 1.0, 0.0));
  let n110 = edge_gradient_dot(edge_hash(c + vec3i(1, 1, 0), key) >> 28u, f - vec3f(1.0, 1.0, 0.0));
  let n001 = edge_gradient_dot(edge_hash(c + vec3i(0, 0, 1), key) >> 28u, f - vec3f(0.0, 0.0, 1.0));
  let n101 = edge_gradient_dot(edge_hash(c + vec3i(1, 0, 1), key) >> 28u, f - vec3f(1.0, 0.0, 1.0));
  let n011 = edge_gradient_dot(edge_hash(c + vec3i(0, 1, 1), key) >> 28u, f - vec3f(0.0, 1.0, 1.0));
  let n111 = edge_gradient_dot(edge_hash(c + vec3i(1, 1, 1), key) >> 28u, f - vec3f(1.0, 1.0, 1.0));
  let w = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
  return mix(
    mix(mix(n000, n100, w.x), mix(n010, n110, w.x), w.y),
    mix(mix(n001, n101, w.x), mix(n011, n111, w.x), w.y),
    w.z,
  );
}

// The octaves a footprint draws (`EdgePlan`): each one's weight, and the sum's slope a pixel.
struct EdgePlan {
  weights : array<f32, EDGE_OCTAVES>,
  slope_per_px : f32,
}

// The octaves drawn at a pixel of footprint `footprint_rad` on the unit sphere (`edgePlan`).
fn edge_plan(footprint_rad : f32) -> EdgePlan {
  var plan : EdgePlan;
  var slope = 0.0;
  var amplitude = 1.0;
  var wavelength = EDGE_BASE_WAVELENGTH_RAD;
  for (var o = 0u; o < EDGE_OCTAVES; o = o + 1u) {
    let wavelength_px = wavelength / footprint_rad;
    let weight = smoothstep(EDGE_FADE_FROM_PX, EDGE_FADE_TO_PX, wavelength_px);
    plan.weights[o] = weight;
    slope = slope + weight * amplitude * EDGE_NOISE_SLOPE_RMS / wavelength_px;
    amplitude = amplitude * EDGE_OCTAVE_GAIN;
    wavelength = wavelength * 0.5;
  }
  plan.slope_per_px = slope / EDGE_NOISE_SIGMA;
  return plan;
}

// The edge noise of `key` at the body-fixed unit direction `d` under `plan` (`edgeNoise`).
fn edge_noise(d : vec3f, key : u32, plan : EdgePlan) -> f32 {
  var sum = 0.0;
  var amplitude = 1.0;
  var frequency = 1.0 / EDGE_BASE_WAVELENGTH_RAD;
  for (var o = 0u; o < EDGE_OCTAVES; o = o + 1u) {
    let weight = plan.weights[o];
    if (weight > 0.0) {
      let octave_key = edge_mix32(key ^ ((o + 1u) * 0x9e3779b9u));
      sum = sum + weight * amplitude * edge_gradient_noise(d * frequency + f32(o) * EDGE_OCTAVE_OFFSET, octave_key);
    }
    amplitude = amplitude * EDGE_OCTAVE_GAIN;
    frequency = frequency * 2.0;
  }
  return sum / EDGE_NOISE_SIGMA;
}

// The sharpness q of an edge whose lattice spacing spans `lattice_px` pixels, its fractions
// box-smoothed by `smoothing` (`edgeSharpness`).
fn edge_sharpness(lattice_px : f32, plan : EdgePlan, smoothing : f32) -> f32 {
  let flattened = 1.0 - 0.5 * smoothing;
  let slope = EDGE_MIDPOINT_SLOPE * flattened / lattice_px + 1.4142135623730951 * EDGE_NOISE_WEIGHT * plan.slope_per_px;
  return clamp(EDGE_RISE_LOGIT / (EDGE_WIDTH_PX * slope), 1.0 / flattened, EDGE_SHARPNESS_MAX);
}

// The weight of the box-smoothed reconstruction where a lattice spacing spans `lattice_px` pixels
// (`edgeSmoothing`).
fn edge_smoothing(lattice_px : f32) -> f32 {
  return 1.0 - smoothstep(EDGE_SMOOTH_FULL_PX, EDGE_SMOOTH_NONE_PX, lattice_px);
}

// A keyed class's box-smoothed fraction `mean`, held to the band about its own fraction `own`
// where the lattice's reconstruction allows it (`edgeHeld`).
fn edge_held(own : f32, mean : f32) -> f32 {
  return clamp(mean, 2.0 * own - 1.0, 2.0 * own);
}

// The boundary's gate from the lattice points' mean purity and the purity of their mean (`edgeGate`).
fn edge_gate(mean_purity : f32, purity : f32) -> f32 {
  let impurity = 1.0 - purity;
  if (!(impurity > 1e-6)) {
    return 0.0;
  }
  return clamp((mean_purity - purity) / impurity, 0.0, 1.0);
}

// A keyed class's logit of share `share` of the keyed classes' sum, under gate `gate`, sharpness
// `q` and its noise `noise` (`edgeShapedShares`).
fn edge_logit(share : f32, gate : f32, q : f32, noise : f32) -> f32 {
  return (1.0 + gate * (q - 1.0)) * log(share) + gate * q * EDGE_NOISE_WEIGHT * noise;
}
