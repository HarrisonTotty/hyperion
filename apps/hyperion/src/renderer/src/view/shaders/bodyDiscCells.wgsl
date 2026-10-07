// The `disc cells` pass (plan R07, T8.d; decision-r07-small-disc-cost §1.2): every cell of every
// pixel of a disc under 32 px shaded at once, one invocation a cell, so that a small disc's cost is
// throughput, not the serial chain of `pixel_sum`'s n² cells of up to nine shades each. Composed
// after frame.wgsl, litBody.wgsl and bodyDisc.wgsl; `frame` is the `discs` pass's own (the scene
// target's size and projection), so that each cell is the one the pixel's draw would shade.
//
// A job is one pixel of one record's rectangle, [⌊left⌋, ⌈right⌉) × [⌊top⌋, ⌈bottom⌉), the records
// in their order and each rectangle's pixels in row order (`discCellJobs`), so that job j's sums are
// the j-th in `cell_sums`. One workgroup a job: invocation k < n² evaluates `cell_sum` at the cell
// `pixel_sum` takes k-th (i = ⌊k ÷ n⌋ outer, j = k mod n inner, `sampleOffset`); after the barrier,
// invocation 0 adds the n² cells in that order, as `pixel_sum` does, and writes the pixel's two
// `vec4f`: the light and the coverage as means over its cells, then its lit and shaded points.
// The draws read them (`pixel_cells`); nothing in `shade` or `cell_sum` changes.

struct Draw {
  // The job's record in `discs`, which `disc_row` reads.
  disc : u32,
}

// Set from each job, so that the library's `disc_row` is unchanged.
var<private> draw : Draw;

struct CellPass {
  // The jobs J; the dispatch's last row of workgroups may hold more, which do nothing.
  jobs : u32,
}

@group(1) @binding(0) var<uniform> cell_pass : CellPass;

// Each job: its record's index, the pixel's x and y, px, and 0.
@group(1) @binding(1) var<storage, read> cell_jobs : array<vec4i>;

// Two `vec4f` a job, in the jobs' order.
@group(2) @binding(5) var<storage, read_write> cell_sums : array<vec4f>;

// Invocations a workgroup: the 8 × 8 cells of a pixel under 4 px; 4 × 4 leave 48 idle.
const CELL_INVOCATIONS : u32 = 64u;

// The workgroups a dispatch lays along x at most (WebGPU's maxComputeWorkgroupsPerDimension).
const JOBS_PER_ROW : u32 = 65535u;

var<workgroup> job_cells : array<CellSum, CELL_INVOCATIONS>;

@compute @workgroup_size(64)
fn main(@builtin(workgroup_id) group : vec3u, @builtin(local_invocation_index) k : u32) {
  let job = group.x + group.y * JOBS_PER_ROW;
  let in_range = job < cell_pass.jobs;
  var n = 0u;
  if (in_range) {
    let pixel = cell_jobs[job];
    draw.disc = u32(pixel.x);
    // At most 8 × 8 cells: the jobs hold no record of more (`discCellJobs`).
    n = min(u32(disc_row(3u).x), 8u);
    if (k < n * n) {
      let i = k / n;
      let j = k % n;
      let centre_px = vec2f(f32(pixel.y), f32(pixel.z)) + 0.5;
      let offset = (vec2f(f32(i), f32(j)) + 0.5) / f32(n) - 0.5;
      job_cells[k] = cell_sum(body_of(), centre_px + offset, n);
    }
  }
  workgroupBarrier();
  if (!in_range || k != 0u) {
    return;
  }
  var radiance = vec3f(0.0);
  var coverage = 0.0;
  var lit = 0u;
  var shaded = 0u;
  for (var c = 0u; c < n * n; c = c + 1u) {
    let one = job_cells[c];
    radiance = radiance + one.radiance;
    coverage = coverage + one.coverage;
    lit = lit + one.lit;
    shaded = shaded + one.shaded;
  }
  let count = f32(n * n);
  cell_sums[2u * job] = vec4f(radiance / count, coverage / count);
  cell_sums[2u * job + 1u] = vec4f(f32(lit), f32(shaded), 0.0, 0.0);
}
