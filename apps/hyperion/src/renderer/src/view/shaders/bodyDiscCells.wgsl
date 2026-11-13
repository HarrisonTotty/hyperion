// The `disc cells` pass (plan R07, T8.d–g; decision-r07-small-disc-cost §1.2,
// decision-r08-t11-frame §2): every cell of every pixel of a disc under 32 px, and of a larger
// disc's limb pixels and its face's near-limb band, shaded at once, one invocation a cell, so that
// their cost is throughput, not the serial chain of `pixel_sum`'s n² cells of up to nine shades
// each. Composed after frame.wgsl, litBody.wgsl and bodyDisc.wgsl; `frame` is the `discs` pass's
// own (the scene target's size and projection), so that each cell is the one the pixel's draw would
// shade.
//
// A job is one pixel of one record: every pixel of a small disc's rectangle,
// [⌊left⌋, ⌈right⌉) × [⌊top⌋, ⌈bottom⌉), in row order, or of a large disc's limb spans and then its
// face band's (`discCellJobs`), so that job j's sums are the j-th in `cell_sums`. The jobs are laid
// out by the set of textures their records bind (a body's air, a class map), one dispatch a set
// (T8.e). A workgroup takes a run of a range's jobs of one cell count n (T8.g; `cell_groups`, the
// set's from `first`): ⌊64 ÷ n²⌋ pixels, the last of a run fewer, so that 64 one-cell pixels of a
// face's band share one, four of 4 × 4 cells, one of 8 × 8. Invocation k takes the pixel ⌊k ÷ n²⌋
// of the run and evaluates `cell_sum` at the cell `pixel_sum` takes c-th, c = k mod n²
// (i = ⌊c ÷ n⌋ outer, j = c mod n inner, `sampleOffset`); after the barrier, each pixel's first
// invocation adds its n² cells in that order, as `pixel_sum` does, and writes the pixel's two
// `vec4f`: the light and the coverage as means over its cells, then its lit and shaded points. The
// draws read them (`pixel_cells`); nothing in `shade` or `cell_sum` changes.

struct Draw {
  // The job's record in `discs`, which `disc_row` reads.
  disc : u32,
}

// Set from each job, so that the library's `disc_row` is unchanged.
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

// Invocations a workgroup (`discShading.ts`' CELL_INVOCATIONS): the 8 × 8 cells of a pixel under
// 4 px, or as many pixels of fewer cells as fit.
const CELL_INVOCATIONS : u32 = 64u;

// The workgroups a dispatch lays along x at most (WebGPU's maxComputeWorkgroupsPerDimension).
const GROUPS_PER_ROW : u32 = 65535u;

var<workgroup> job_cells : array<CellSum, CELL_INVOCATIONS>;

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
  if (taken) {
    let pixel = cell_jobs[job];
    draw.disc = u32(pixel.x);
    let i = c / n;
    let j = c % n;
    let centre_px = vec2f(f32(pixel.y), f32(pixel.z)) + 0.5;
    let offset = (vec2f(f32(i), f32(j)) + 0.5) / f32(n) - 0.5;
    job_cells[k] = cell_sum(body_of(), centre_px + offset, n);
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
