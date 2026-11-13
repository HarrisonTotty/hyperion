// The `disc cells` pass, packed (plan R07, the owner's Mac; `GeometrySettings.discCells` `packed`):
// the jobs, workgroups and sums of `bodyDiscCells.wgsl`, `per-cell`, the way back, but each
// workgroup's points shaded one an invocation over the list of them, not each cell's by its own.
// Composed after frame.wgsl, litBody.wgsl and bodyDisc.wgsl, as that kernel is.
//
// A cell is shaded at no point (outside the limb), at its centre (deep inside) or at up to nine
// (`cell_samples`' Gauss points across the limb), so an invocation a cell leaves most of its
// workgroup idle beside the few that shade nine. Here each invocation first places its cell's
// points (`cell_samples`), as `cell_sum` places them, in the workgroup's list from the exclusive sum
// of the counts before it; then every invocation shades the list's points k, k + 64, …, each point
// against the spheroid and by `shade` as `add_sample` takes it, leaving its light and whether it
// was lit, or that the ray missed; then each cell sums its own points' in their order, weighted as
// `add_sample` weights them, and each pixel its cells' in theirs, as the per-cell kernel does. The
// same functions on the same values in the same order: the sums are that kernel's, bit for bit.
// A workgroup's jobs are one record's (`discShading.ts`' `runsOf`), which every invocation shades.

struct Draw {
  // The workgroup's record in `discs`, which `disc_row` reads.
  disc : u32,
}

// Set from the workgroup's first job, so that the library's `disc_row` is unchanged.
var<private> draw : Draw;

struct CellPass {
  // The dispatch's workgroups; its last row of workgroups may hold more, which do nothing.
  groups : u32,
  // Its first workgroup in `cell_groups`.
  first : u32,
}

@group(1) @binding(0) var<uniform> cell_pass : CellPass;

// Each job: its record's index, the pixel's x and y, px, and 0.
@group(1) @binding(1) var<storage, read> cell_jobs : array<vec4i>;

// Each workgroup (T8.g): its first job, its jobs, their cells per axis n (1 to 8), and 0.
@group(1) @binding(2) var<storage, read> cell_groups : array<vec4u>;

// Two `vec4f` a job, in the jobs' order.
@group(2) @binding(5) var<storage, read_write> cell_sums : array<vec4f>;

// Invocations a workgroup (`discShading.ts`' CELL_INVOCATIONS), as the per-cell kernel's.
const CELL_INVOCATIONS : u32 = 64u;

// The points a cell is shaded at at most: three Gauss points in each of its profile's three pieces
// (`CellSamples`).
const CELL_POINTS : u32 = 9u;

// The workgroups a dispatch lays along x at most (WebGPU's maxComputeWorkgroupsPerDimension).
const GROUPS_PER_ROW : u32 = 65535u;

var<workgroup> job_cells : array<CellSum, CELL_INVOCATIONS>;

// Each invocation's first point in `points`, then the workgroup's points.
var<workgroup> first_point : array<u32, 65>;

// The workgroup's points: each one's place on the view, px, then once shaded its light and its
// state (0 the ray missed, 1 shaded, 2 shaded and lit). 9 KiB of the 16 KiB WebGPU guarantees.
var<workgroup> points : array<vec4f, 576>;

@compute @workgroup_size(64)
fn main(@builtin(workgroup_id) group : vec3u, @builtin(local_invocation_index) k : u32) {
  let local_group = group.x + group.y * GROUPS_PER_ROW;
  var run = vec4u(0u);
  if (local_group < cell_pass.groups) {
    run = cell_groups[cell_pass.first + local_group];
  }
  // At most 8 × 8 cells: the jobs hold no record of more (`discCellJobs`); none in an idle group.
  let n = min(run.z, 8u);
  let cells = max(n * n, 1u);
  let slot = k / cells;
  let c = k % cells;
  let taken = n > 0u && slot < run.y;
  let job = run.x + slot;
  if (n > 0u && run.y > 0u) {
    draw.disc = u32(cell_jobs[run.x].x);
  }
  var samples : CellSamples;
  samples.count = 0u;
  samples.coverage = 0.0;
  var px = vec2f(0.0);
  if (taken) {
    let pixel = cell_jobs[job];
    let i = c / n;
    let j = c % n;
    let centre_px = vec2f(f32(pixel.y), f32(pixel.z)) + 0.5;
    let offset = (vec2f(f32(i), f32(j)) + 0.5) / f32(n) - 0.5;
    px = centre_px + offset;
    samples = cell_samples(body_of(), px, n);
  }
  first_point[k] = samples.count;
  workgroupBarrier();
  // The exclusive sum of the counts, in the invocations' order; its total last.
  if (k == 0u) {
    var total = 0u;
    for (var m = 0u; m < CELL_INVOCATIONS; m = m + 1u) {
      let count = first_point[m];
      first_point[m] = total;
      total = total + count;
    }
    first_point[CELL_INVOCATIONS] = total;
  }
  workgroupBarrier();
  let base = first_point[k];
  for (var s = 0u; s < samples.count; s = s + 1u) {
    points[base + s] = vec4f(px + samples.offsets[s] * samples.normal, 0.0, 0.0);
  }
  workgroupBarrier();
  // One call of the shading, as `cell_sum`'s note asks of a kernel a compiler inlines whole.
  let total = first_point[CELL_INVOCATIONS];
  let body = body_of();
  for (var p = k; p < total; p = p + CELL_INVOCATIONS) {
    let ray = view_ray(points[p].xy);
    let hit = hit_spheroid(body, ray);
    var light = vec4f(0.0);
    if (hit.w != 0.0) {
      let shaded = shade(body, hit.xyz, ray);
      light = vec4f(shaded.radiance, select(1.0, 2.0, shaded.lit));
    }
    points[p] = light;
  }
  workgroupBarrier();
  if (taken) {
    var sum : CellSum;
    sum.radiance = vec3f(0.0);
    sum.coverage = 0.0;
    sum.lit = 0u;
    sum.shaded = 0u;
    for (var s = 0u; s < samples.count; s = s + 1u) {
      let light = points[base + s];
      if (light.w != 0.0) {
        sum.radiance = sum.radiance + samples.weights[s] * light.xyz;
        sum.lit = sum.lit + select(0u, 1u, light.w > 1.5);
        sum.shaded = sum.shaded + 1u;
      }
    }
    sum.coverage = samples.coverage;
    job_cells[k] = sum;
  }
  workgroupBarrier();
  if (!taken || c != 0u) {
    return;
  }
  var radiance = vec3f(0.0);
  var coverage = 0.0;
  var lit = 0u;
  var shaded = 0u;
  for (var m = 0u; m < cells; m = m + 1u) {
    let one = job_cells[k + m];
    radiance = radiance + one.radiance;
    coverage = coverage + one.coverage;
    lit = lit + one.lit;
    shaded = shaded + one.shaded;
  }
  let count = f32(cells);
  cell_sums[2u * job] = vec4f(radiance / count, coverage / count);
  cell_sums[2u * job + 1u] = vec4f(f32(lit), f32(shaded), 0.0, 0.0);
}
