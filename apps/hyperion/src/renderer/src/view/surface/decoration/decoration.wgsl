// The decoration library (plan R11, R11.T2.a; Design notes 3 and 20): the integer lattice and its
// hash, band-limited gradient noise for the shading normal and the colour, wind-oriented ripples
// and crater scars below 2 m, each faded across one octave of pixel footprint. Its reference is
// `decorationLattice.ts`, whose integers and hashes it gives to the bit and whose fields to its
// `f32` rounding; that module's remarks hold the design. It declares no binding: its caller, R10's
// `terrain_decoration` hook, gives it the patch's key, the texel's place and the body's inputs.
//
// A patch key is (face, level, i, j) as a `vec4u`; a place's `uv` is the lit terrain's `patchUv`,
// [0, 1] from the patch's low corner. A `ripple` is `RippleDirection`'s (p, q, n) as a `vec3i`,
// n = 0 for none.

// `DECORATION_SALT` (`decorationLattice.ts`).
const DECORATION_SALT_NOISE : u32 = 1u;
const DECORATION_SALT_WOBBLE : u32 = 2u;
const DECORATION_SALT_SCAR : u32 = 3u;

// `DECORATION_OCTAVES`, `SCAR_OCTAVES`, `SCAR_LEVEL_OFFSET` and `RIPPLE_LEVEL_OFFSET`.
const DECORATION_OCTAVES : u32 = 6u;
const DECORATION_SCAR_OCTAVES : u32 = 4u;
const DECORATION_SCAR_LEVEL_OFFSET : u32 = 2u;
const DECORATION_RIPPLE_LEVEL_OFFSET : u32 = 3u;

// `ALBEDO_OCTAVE_SHARE`, `RIPPLE_WAVELENGTH_M`, `RIPPLE_INDEX`, `SCARS_PER_CELL` and
// `SCAR_DEPTH_RATIO`, each sourced there.
const DECORATION_ALBEDO_OCTAVE_SHARE : f32 = 0.70710678118654752;
const DECORATION_RIPPLE_WAVELENGTH_M : f32 = 0.1;
const DECORATION_RIPPLE_INDEX : f32 = 18.0;
const DECORATION_SCARS_PER_CELL : f32 = 0.948;
const DECORATION_SCAR_MATURE : f32 = 0.05;
const DECORATION_SCAR_FRESH : f32 = 0.1;

const DECORATION_PI : f32 = 3.14159265358979;
const DECORATION_SQRT_HALF : f32 = 0.70710678118654752;
const DECORATION_SQRT_TWO : f32 = 1.41421356237309505;

// Wellons's `lowbias32` (`lowbias32`).
fn decoration_mix(x : u32) -> u32 {
  var h = x;
  h = h ^ (h >> 16u);
  h = h * 0x7feb352du;
  h = h ^ (h >> 15u);
  h = h * 0x846ca68bu;
  h = h ^ (h >> 16u);
  return h;
}

// `latticePrefix`: the hash every point of one lattice and salt shares.
fn decoration_prefix(seed : u32, salt : u32, face : u32, level : u32) -> u32 {
  return decoration_mix(seed ^ decoration_mix((salt << 8u) | (face << 5u) | level));
}

// `latticePoint`.
fn decoration_point(prefix : u32, x : u32, y : u32) -> u32 {
  return decoration_mix(decoration_mix(prefix ^ x) ^ y);
}

// `latticeHash`.
fn decoration_hash(seed : u32, salt : u32, face : u32, level : u32, x : u32, y : u32) -> u32 {
  return decoration_point(decoration_prefix(seed, salt, face, level), x, y);
}

// A texel's place on one lattice (`LatticePlace`): its cell and its fraction across it.
struct DecorationPlace {
  cell : vec2u,
  frac : vec2f,
}

// `latticePlace`: the place of `uv` in patch `key` = (face, level, i, j) on the lattice of `level`.
fn decoration_place(key : vec4u, uv : vec2f, level : u32) -> DecorationPlace {
  var out : DecorationPlace;
  let n = key.y;
  if (level >= n) {
    let shift = level - n;
    let along = uv * f32(1u << shift);
    let whole = floor(along);
    out.cell = vec2u((key.z << shift) + u32(whole.x), (key.w << shift) + u32(whole.y));
    out.frac = along - whole;
  } else {
    let shift = n - level;
    let mask = (1u << shift) - 1u;
    out.cell = vec2u(key.z >> shift, key.w >> shift);
    out.frac = (vec2f(f32(key.z & mask), f32(key.w & mask)) + uv) / f32(1u << shift);
  }
  return out;
}

// S2's quadratic warp and its rate (`patchVertex.wgsl`'s `stToUv`, here for the library alone).
fn decoration_st_to_uv(s : f32) -> f32 {
  if (s >= 0.5) {
    return (4.0 * s * s - 1.0) / 3.0;
  }
  let r = 1.0 - s;
  return (1.0 - 4.0 * r * r) / 3.0;
}

fn decoration_warp_rate(s : f32) -> f32 {
  return select(8.0 * (1.0 - s) / 3.0, 8.0 * s / 3.0, s >= 0.5);
}

// `latticeCellM`: the widths, m, of a cell of the lattice at `level` at face coordinates `st` on a
// body of radius `radius_m`, along the face's x and y.
fn decoration_cell_m(st : vec2f, radius_m : f32, level : u32) -> vec2f {
  let u = decoration_st_to_uv(st.x);
  let v = decoration_st_to_uv(st.y);
  let q = 1.0 + u * u + v * v;
  let cells = radius_m / f32(1u << level);
  return vec2f(
    decoration_warp_rate(st.x) * sqrt(1.0 + v * v),
    decoration_warp_rate(st.y) * sqrt(1.0 + u * u),
  ) * cells / q;
}

// The eight unit gradients (`GRADIENTS`), by three bits of a hash from bit `shift`.
fn decoration_gradient(hash : u32, shift : u32) -> vec2f {
  switch ((hash >> shift) & 7u) {
    case 0u: { return vec2f(1.0, 0.0); }
    case 1u: { return vec2f(DECORATION_SQRT_HALF, DECORATION_SQRT_HALF); }
    case 2u: { return vec2f(0.0, 1.0); }
    case 3u: { return vec2f(-DECORATION_SQRT_HALF, DECORATION_SQRT_HALF); }
    case 4u: { return vec2f(-1.0, 0.0); }
    case 5u: { return vec2f(-DECORATION_SQRT_HALF, -DECORATION_SQRT_HALF); }
    case 6u: { return vec2f(0.0, -1.0); }
    default: { return vec2f(DECORATION_SQRT_HALF, -DECORATION_SQRT_HALF); }
  }
}

// Perlin's quintic fade and its rate.
fn decoration_quintic(t : vec2f) -> vec2f {
  return t * t * t * (t * (6.0 * t - 15.0) + 10.0);
}

fn decoration_quintic_rate(t : vec2f) -> vec2f {
  return 30.0 * t * t * (t - 1.0) * (t - 1.0);
}

// `gradientNoise`: gradient noise's value, then its rates along x and y per cell, ×√2; `which` 0
// for the normal's field and 3 for the albedo's, both of one hash a corner.
fn decoration_noise(prefix : u32, place : DecorationPlace, which : u32) -> vec3f {
  let c = place.cell;
  let f = place.frac;
  let g00 = decoration_gradient(decoration_point(prefix, c.x, c.y), which);
  let g10 = decoration_gradient(decoration_point(prefix, c.x + 1u, c.y), which);
  let g01 = decoration_gradient(decoration_point(prefix, c.x, c.y + 1u), which);
  let g11 = decoration_gradient(decoration_point(prefix, c.x + 1u, c.y + 1u), which);
  let a = dot(g00, f);
  let b = dot(g10, f - vec2f(1.0, 0.0));
  let cc = dot(g01, f - vec2f(0.0, 1.0));
  let d = dot(g11, f - vec2f(1.0, 1.0));
  let q = decoration_quintic(f);
  let dq = decoration_quintic_rate(f);
  let k = a - b - cc + d;
  let value = a + q.x * (b - a) + q.y * (cc - a) + q.x * q.y * k;
  let rates =
    g00 + q.x * (g10 - g00) + q.y * (g01 - g00) + q.x * q.y * (g00 - g10 - g01 + g11) +
    dq * vec2f(b - a + q.y * k, cc - a + q.x * k);
  return DECORATION_SQRT_TWO * vec3f(value, rates);
}

// `octaveFade`: 1 while a cell of `cell_m` spans 2 px or more at `distance_m`, 0 at 1 px or less.
fn decoration_fade(cell_m : f32, distance_m : f32, px_per_rad : f32) -> f32 {
  if (!(distance_m > 0.0)) {
    return 1.0;
  }
  return clamp(log2(cell_m * px_per_rad / distance_m), 0.0, 1.0);
}

// `DecorationMix`.
struct DecorationMix {
  roughness : f32,
  albedo : f32,
  ripples : f32,
  scars : f32,
}

// `DECORATION_CLASSES`: a material class's mix by its code; none for `Seabed` or a code past them.
fn decoration_class(code : u32) -> DecorationMix {
  switch (code) {
    case 0u: { return DecorationMix(0.06, 0.05, 0.0, 1.0); }
    case 1u: { return DecorationMix(0.03, 0.04, 0.0, 1.0); }
    case 2u: { return DecorationMix(0.03, 0.05, 0.0, 0.0); }
    case 3u: { return DecorationMix(0.015, 0.03, 1.0, 0.0); }
    case 4u: { return DecorationMix(0.02, 0.04, 0.0, 0.0); }
    case 5u: { return DecorationMix(0.06, 0.05, 0.0, 1.0); }
    case 6u: { return DecorationMix(0.015, 0.02, 0.0, 0.0); }
    case 7u: { return DecorationMix(0.02, 0.02, 0.0, 1.0); }
    case 8u: { return DecorationMix(0.015, 0.02, 0.0, 0.0); }
    case 9u: { return DecorationMix(0.025, 0.03, 0.0, 0.0); }
    case 10u: { return DecorationMix(0.015, 0.03, 0.0, 1.0); }
    case 11u: { return DecorationMix(0.02, 0.04, 0.0, 0.0); }
    case 13u: { return DecorationMix(0.02, 0.04, 0.0, 0.0); }
    case 14u: { return DecorationMix(0.02, 0.03, 0.0, 1.0); }
    default: { return DecorationMix(0.0, 0.0, 0.0, 0.0); }
  }
}

// A body's inputs: its decoration seed, its `decorationLevel`, and each scar octave's share of
// saturation, coarsest first (`scarShares`).
struct DecorationBody {
  seed : u32,
  level : u32,
  scar_shares : vec4f,
}

// `TexelDecoration`: a factor on the class albedo and a slope on the ground along the face's x and
// y.
struct DecorationTexel {
  albedo : f32,
  slope : vec2f,
}

// Bits 16–31 and 0–15 of a hash as fractions in [0, 1).
fn decoration_upper(hash : u32) -> f32 {
  return f32(hash >> 16u) / 65536.0;
}

fn decoration_lower(hash : u32) -> f32 {
  return f32(hash & 0xffffu) / 65536.0;
}

// `scarAt`: one scar octave's slope, 0 outside a scar: a scar is normal detail only (open
// question 18).
fn decoration_scar(
  body : DecorationBody,
  key : vec4u,
  uv : vec2f,
  cell_m : vec2f,
  octave : u32,
  share : f32,
) -> vec2f {
  let level = body.level - DECORATION_SCAR_LEVEL_OFFSET + octave;
  let place = decoration_place(key, uv, level);
  let first = decoration_hash(body.seed, DECORATION_SALT_SCAR, key.x, level, place.cell.x, place.cell.y);
  if (!(decoration_lower(first) < DECORATION_SCARS_PER_CELL * share)) {
    return vec2f(0.0);
  }
  let second = decoration_mix(first);
  let third = decoration_mix(second);
  let diameter = 1.0 / sqrt(16.0 - 12.0 * decoration_upper(first));
  let radius = diameter / 2.0;
  let centre = vec2f(radius) + (1.0 - diameter) * vec2f(decoration_lower(second), decoration_upper(second));
  let fresh = decoration_upper(third);
  let freshness = fresh * fresh * fresh;
  let depth_ratio = DECORATION_SCAR_MATURE + (DECORATION_SCAR_FRESH - DECORATION_SCAR_MATURE) * freshness;
  let s = cell_m * f32(1u << DECORATION_SCAR_LEVEL_OFFSET) / f32(1u << octave);
  let d = (place.frac - centre) * s;
  let radius_m = radius * min(s.x, s.y);
  let r = length(d) / radius_m;
  if (!(r < 1.0)) {
    return vec2f(0.0);
  }
  let slope = 8.0 * depth_ratio * r * (1.0 - r * r);
  var towards = vec2f(0.0);
  if (r > 0.0) {
    towards = d / (r * radius_m);
  }
  return slope * towards;
}

// The residue of `k` modulo `n` > 0, in [0, n).
fn decoration_residue(k : i32, n : i32) -> u32 {
  return u32(((k % n) + n) % n);
}

// `rippleAt`: the ripples' slope along the face's x and y.
fn decoration_ripple(body : DecorationBody, key : vec4u, uv : vec2f, cell_m : vec2f, ripple : vec3i) -> vec2f {
  let level = body.level + DECORATION_RIPPLE_LEVEL_OFFSET;
  let place = decoration_place(key, uv, level);
  let n = ripple.z;
  let un = u32(n);
  let whole = (decoration_residue(ripple.x, n) * (place.cell.x % un) +
    decoration_residue(ripple.y, n) * (place.cell.y % un)) % un;
  let pq = vec2f(f32(ripple.x), f32(ripple.y));
  let phase = 2.0 * DECORATION_PI * (f32(whole) + dot(pq, place.frac)) / f32(n);
  let coarse = level - 2u;
  let wobble = decoration_noise(
    decoration_prefix(body.seed, DECORATION_SALT_WOBBLE, key.x, coarse),
    decoration_place(key, uv, coarse),
    0u,
  );
  let turn = DECORATION_PI / 2.0;
  let s = cell_m / f32(1u << DECORATION_RIPPLE_LEVEL_OFFSET);
  let rate = cos(phase + turn * wobble.x);
  let amplitude_m = f32(n) / length(pq) * sqrt(s.x * s.y) / (2.0 * DECORATION_RIPPLE_INDEX);
  let phi = 2.0 * DECORATION_PI * pq / f32(n) + turn * wobble.yz / 4.0;
  return amplitude_m * rate * phi / s;
}

// `decorateTexel`: the decoration of one texel of patch `key` at `uv`, the cells of its body's
// lattice level `cell_m` wide along the face's x and y (`decoration_cell_m`), at `distance_m` in a
// view of `px_per_rad` (`decorationPxPerRad`): the noise octaves, the ripples and the scars,
// each faded by its own octave of footprint, in that order.
fn decorate(
  body : DecorationBody,
  key : vec4u,
  uv : vec2f,
  cell_m : vec2f,
  distance_m : f32,
  px_per_rad : f32,
  mix : DecorationMix,
  ripple : vec3i,
) -> DecorationTexel {
  var albedo = 1.0;
  var slope = vec2f(0.0);
  let mean_cell = sqrt(cell_m.x * cell_m.y);
  var albedo_share = 1.0;
  for (var octave = 0u; octave < DECORATION_OCTAVES; octave = octave + 1u) {
    let fade = decoration_fade(mean_cell / f32(1u << octave), distance_m, px_per_rad);
    if (!(fade > 0.0)) {
      break;
    }
    let level = body.level + octave;
    let prefix = decoration_prefix(body.seed, DECORATION_SALT_NOISE, key.x, level);
    let place = decoration_place(key, uv, level);
    let normal = decoration_noise(prefix, place, 0u);
    let colour = decoration_noise(prefix, place, 3u);
    slope = slope + fade * mix.roughness * normal.yz * mean_cell / cell_m;
    albedo = albedo + fade * mix.albedo * albedo_share * colour.x;
    albedo_share = albedo_share * DECORATION_ALBEDO_OCTAVE_SHARE;
  }
  if (ripple.z > 0 && mix.ripples > 0.0) {
    let fade = decoration_fade(DECORATION_RIPPLE_WAVELENGTH_M / 2.0, distance_m, px_per_rad);
    if (fade > 0.0) {
      slope = slope + fade * mix.ripples * decoration_ripple(body, key, uv, cell_m, ripple);
    }
  }
  if (mix.scars > 0.0) {
    for (var octave = 0u; octave < DECORATION_SCAR_OCTAVES; octave = octave + 1u) {
      let octave_cell = mean_cell * f32(1u << DECORATION_SCAR_LEVEL_OFFSET) / f32(1u << octave);
      let fade = decoration_fade(octave_cell / 4.0, distance_m, px_per_rad);
      if (!(fade > 0.0)) {
        break;
      }
      let share = body.scar_shares[octave];
      if (share > 0.0) {
        slope = slope + fade * mix.scars * decoration_scar(body, key, uv, cell_m, octave, share);
      }
    }
  }
  return DecorationTexel(max(albedo, 0.0), slope);
}

// `perturbNormal`: `normal` perturbed by the ground's slope along the face grid's unit tangents.
fn decoration_normal(normal : vec3f, tangent_x : vec3f, tangent_y : vec3f, slope : vec2f) -> vec3f {
  return normalize(normal - slope.x * tangent_x - slope.y * tangent_y);
}
