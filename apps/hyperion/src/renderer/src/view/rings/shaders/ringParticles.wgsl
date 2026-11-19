// A ring's local particle field (plan R11, R11.T9.d; Design note 16; `ringField.ts`): a thread a
// cell of the field's octaves' windows about the camera, each placing its cell's particles, those
// whose diameter fills their own distance's pixel and whose bounds meet the view, appended for R14's
// instance cull (`clusters/particlePath.ts`) in `particles.ts`' record. Each draws its numbers from
// its cell and index alone, by PCG hashes, so that the field is the same whatever the dispatch's
// order, and the cells' rows turn at their own Keplerian rates, offset by S a row from the
// camera's; across the ring it follows the profile's radial steps, and stops at the annulus's edges. Composed after `folded_group` (`clusters/fold.ts`). The TypeScript twin is
// `ringField.ts`' `cellParticles`; the kernel is `presentation-only`, its f32 the device's.

struct FieldLevel {
  // The cells' side, m; the octave's radii, low and high, m; the particles a cell holds on average.
  size : vec4f,
  // The camera's cell row (an i32's bits), its sheared column at its row (mod 2³²), the window's
  // half-width in cells, and the octave's first cell among the field's.
  cell : vec4u,
  // The camera's x and y within its cell, in [0, 1).
  frac : vec4f,
}

struct FieldParams {
  // The sheet's radial, prograde and normal axes in the passes' frame; w, the camera's height over
  // the plane (m), the pixel's angle (rad) and the layer's σ (m).
  ex : vec4f,
  ey : vec4f,
  ez : vec4f,
  // The ring's radial steps' edges, m from the camera's radius, ascending, 3.4 × 10³⁸ for none.
  edges : vec4f,
  // The octaves, the path's capacity, the field's cells and its seed.
  counts : vec4u,
  // The shear's whole part per row (mod 2³²), the view's planes.
  extra : vec4u,
  // The shear's fraction per row, S's; the field's reference τ; the star's sine over the plane,
  // signed along its normal.
  shear : vec4f,
  // The view's planes: inward unit normals, w the offset, m, the camera at the origin.
  planes : array<vec4f, 6>,
  // Each radial step's particles' albedo per channel, w its τ over the field's reference τ.
  steps : array<vec4f, 5>,
  levels : array<FieldLevel, 9>,
}

@group(0) @binding(0) var<uniform> params : FieldParams;
@group(0) @binding(1) var<storage, read_write> instances : array<u32>;
@group(0) @binding(2) var<storage, read_write> instance_count : array<atomic<u32>>;

const FIELD_GROUP : u32 = 64u;
const FIELD_MAX_PER_CELL : u32 = 255u;
const FIELD_TAU : f32 = 6.283185307179586;
// FIELD_BOUNDS_SCALE, the templates' bounds scale (`sphereParticleTemplates`, the instance cull's
// own), is composed ahead of this source by `ringParticles.ts`.

// The PCG hash's output of `v` (Jarzynski and Olano 2020, "Hash Functions for GPU Rendering", JCGT
// 9(3), §18 `pcg`), as `instanceTestPlace.wgsl` takes it.
fn field_pcg(v : u32) -> u32 {
  let state = v * 747796405u + 2891336453u;
  let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
  return (word >> 22u) ^ word;
}

// A number in [0, 1) from a hash's top 24 bits.
fn field_unit(h : u32) -> f32 {
  return f32(h >> 8u) / 16777216.0;
}

// erfc(x) by Abramowitz and Stegun 1964's 7.1.26 (`ringField.ts`' `erfcApprox`).
fn field_erfc(x : f32) -> f32 {
  let a = abs(x);
  let t = 1.0 / (1.0 + 0.3275911 * a);
  let poly = t * (0.254829592 + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
  let tail = poly * exp(-a * a);
  return select(2.0 - tail, tail, x >= 0.0);
}

// The share of the star's beam reaching a particle at height z: exp(−τ Q(z_s ÷ σ) ÷ μ₀), z_s its
// height towards the star (`ringField.ts`' `beamThrough`).
fn field_beam(tau_ratio : f32, z : f32, sigma : f32) -> f32 {
  let star_sine = params.shear.z;
  if (star_sine == 0.0) {
    return 0.0;
  }
  let towards = select(-z, z, star_sine > 0.0);
  let beyond = 0.5 * field_erfc(towards / (1.4142135623730951 * sigma));
  return exp(-params.shear.y * tau_ratio * beyond / max(abs(star_sine), 1e-3));
}

// The step at radial offset `x` from the camera: the edges at or below it.
fn field_step(x : f32) -> u32 {
  return u32(params.edges.x <= x) + u32(params.edges.y <= x) + u32(params.edges.z <= x) + u32(params.edges.w <= x);
}

// Whether a sphere of `radius` about `centre` meets every plane's inside.
fn field_in_view(centre : vec3f, radius : f32) -> bool {
  for (var k = 0u; k < min(params.extra.y, 6u); k = k + 1u) {
    let plane = params.planes[k];
    if (dot(plane.xyz, centre) + plane.w < -radius) {
      return false;
    }
  }
  return true;
}

@compute @workgroup_size(64)
fn main(@builtin(workgroup_id) group : vec3u, @builtin(num_workgroups) groups : vec3u, @builtin(local_invocation_index) local : u32) {
  let cell = folded_group(group, groups) * FIELD_GROUP + local;
  if (cell >= params.counts.z) {
    return;
  }
  var level = 0u;
  for (var k = 1u; k < min(params.counts.x, 9u); k = k + 1u) {
    if (cell >= params.levels[k].cell.w) {
      level = k;
    }
  }
  let octave = params.levels[level];
  let half = octave.cell.z;
  let side = 2u * half + 1u;
  let index = cell - octave.cell.w;
  let di = i32(index / side) - i32(half);
  let dj = i32(index % side) - i32(half);
  let l = octave.size.x;
  // The camera's sheared column at this row: its own row's and the shear's S a row since.
  let along = octave.frac.y + params.shear.x * f32(di);
  let whole = floor(along);
  let column_here = octave.cell.y + bitcast<u32>(i32(whole)) + params.extra.x * bitcast<u32>(di);
  let fraction_here = along - whole;
  let row_here = bitcast<u32>(bitcast<i32>(octave.cell.x) + di);
  let column_cell = column_here + bitcast<u32>(dj);
  let cell_seed = field_pcg(level ^ field_pcg(row_here ^ field_pcg(column_cell ^ field_pcg(params.counts.w))));
  let ex = params.ex.xyz;
  let ey = params.ey.xyz;
  let ez = params.ez.xyz;
  let height = params.ex.w;
  let pixel = params.ey.w;
  let sigma = params.ez.w;
  let x_cell = (f32(di) - octave.frac.x) * l;
  let y_cell = (f32(dj) - fraction_here) * l;
  // The cell's own bounds against the view and the octave's reach.
  let centre_cell = ex * (x_cell + 0.5 * l) + ey * (y_cell + 0.5 * l) - ez * height;
  let cell_radius = l * 0.7071067811865476 + 3.0 * sigma + octave.size.z;
  let reach = 2.0 * octave.size.z / pixel;
  if (length(centre_cell) - cell_radius > reach || !field_in_view(centre_cell, cell_radius)) {
    return;
  }
  // Drawn at the greatest ratio the cell's width meets, each kept at its own step's over that.
  var most = 0.0;
  for (var k = field_step(x_cell); k <= field_step(x_cell + l); k = k + 1u) {
    most = max(most, params.steps[min(k, 4u)].w);
  }
  let count = min(u32(floor(octave.size.w * most + field_unit(cell_seed))), FIELD_MAX_PER_CELL);
  let low = 1.0 / (octave.size.y * octave.size.y);
  let high = 1.0 / (octave.size.z * octave.size.z);
  for (var m = 0u; m < count; m = m + 1u) {
    var h = field_pcg(cell_seed ^ field_pcg(m + 1u));
    var u : array<f32, 10>;
    for (var k = 0u; k < 10u; k = k + 1u) {
      h = field_pcg(h);
      u[k] = field_unit(h);
    }
    let step = params.steps[min(field_step(x_cell + u[0] * l), 4u)];
    if (!(u[9] * most < step.w)) {
      continue;
    }
    let z = sigma * sqrt(-2.0 * log(max(u[2], 1e-7))) * cos(FIELD_TAU * u[3]);
    let radius = inverseSqrt(low - u[4] * (low - high));
    let centre = ex * (x_cell + u[0] * l) + ey * (y_cell + u[1] * l) + ez * (z - height);
    // Instanced only where its diameter fills its own distance's pixel (`instancedShare`).
    if (2.0 * radius < length(centre) * pixel) {
      continue;
    }
    if (!field_in_view(centre, FIELD_BOUNDS_SCALE * radius)) {
      continue;
    }
    let slot = atomicAdd(&instance_count[0], 1u);
    if (slot >= params.counts.y) {
      continue;
    }
    let s = sqrt(1.0 - u[5]);
    let t = sqrt(u[5]);
    let q = vec4f(s * sin(FIELD_TAU * u[6]), s * cos(FIELD_TAU * u[6]), t * sin(FIELD_TAU * u[7]), t * cos(FIELD_TAU * u[7]));
    let grey = 0.85 + 0.3 * u[8];
    let base = slot * 8u;
    instances[base] = bitcast<u32>(centre.x);
    instances[base + 1u] = bitcast<u32>(centre.y);
    instances[base + 2u] = bitcast<u32>(centre.z);
    instances[base + 3u] = bitcast<u32>(radius);
    instances[base + 4u] = pack2x16snorm(q.xy);
    instances[base + 5u] = pack2x16snorm(q.zw);
    instances[base + 6u] = pack4x8unorm(vec4f(step.rgb * grey, field_beam(step.w, z, sigma)));
    instances[base + 7u] = cell_seed ^ (m + 1u);
  }
}
