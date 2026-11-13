// The DAG consumers' occluder faces in the wireframe (plan R14, R14.T5.f): a vertex stage over a
// bucket's list (`clusters/buckets.ts`) and the mesh shape's one template (`clusters/templates.ts`'
// `meshTemplate`), 128 triangles by index pulling, which decodes each corner from a mesh page in
// the pool (`clusters/pageFormat.ts`, `tools/meshbuild/src/page.rs`): the cluster's template
// reference from the pool's records, its first vertex from the page's cluster table, its
// triangle's 8-bit index and its vertex's 21-bit grid coordinates, times the page's step, exactly
// in `f32`, about the page's origin. A template vertex past the cluster's last triangle is placed
// outside the clip volume, one point for all three corners, so that it draws nothing. A corner is
// placed as occluder.wgsl places a hull's, the origin from the camera plus its offset, the
// position `@invariant`, and then pushed away from the camera along its ray by its cluster's own
// error as built (the pool's `errors`), so that a coarser cut's faces lie no nearer the camera
// than its leaves' and hide no edge the finest cut shows; a leaf's error is 0, and its corner is
// placed to the bit (R14.T5.f). The fragment stage is occluder.wgsl's, to the byte. Composed after
// frame.wgsl and occluder.wgsl, its `vertexMain` taken out and its `Draw` declaring `clusterBase`
// (`wireframe/submit.ts`' MESH_OCCLUDER_MATERIAL, by `occluderClusterForm`).

// A mesh occluder's instance (`meshOccluders.ts`' MESH_OCCLUDER_INSTANCE_BYTES): its pages' origin
// from the camera, m, and its slot, the cull's first four words; then its page's first word in the
// pool's pages.
struct MeshInstance {
  originM : vec3f,
  slot : u32,
  pageWord : u32,
  spare0 : u32,
  spare1 : u32,
  spare2 : u32,
}

@group(2) @binding(1) var<storage, read> mesh_instances : array<MeshInstance>;

// The buckets' lists, one `u32` an entry, (instance << 8) | cluster, each bucket from its base.
@group(2) @binding(2) var<storage, read> cluster_lists : array<u32>;

// The pool's pages, word by word, each slot's page from its first word.
@group(2) @binding(3) var<storage, read> mesh_pages : array<u32>;

// The pool's records, eight words a record, 128 records a slot (`record.ts`' ClusterRecord).
@group(2) @binding(4) var<storage, read> mesh_records : array<u32>;

// Each record's own error as its page holds it, m, never rewritten (`clusters/pages.ts`' errors).
@group(2) @binding(5) var<storage, read> mesh_errors : array<f32>;

// A slot's records and a record's words.
const MESH_PAGE_CLUSTERS : u32 = 128u;
const MESH_RECORD_WORDS : u32 = 8u;

// A page header's words (`page.rs`): the grid's step, then the cluster table's, the vertices' and
// the triangles' byte offsets in the page.
const PAGE_STEP_WORD : u32 = 14u;
const PAGE_TABLE_WORD : u32 = 20u;
const PAGE_VERTICES_WORD : u32 = 21u;
const PAGE_TRIANGLES_WORD : u32 = 22u;

// One grid coordinate's bits.
const GRID_MASK : u32 = 0x1fffffu;

@vertex
fn vertexMain(@location(0) local : vec3f, @builtin(instance_index) entry : u32) -> @builtin(position) @invariant vec4f {
  let item = cluster_lists[draw.clusterBase + entry];
  let mesh = mesh_instances[item >> 8u];
  let cluster = item & 0xffu;
  let templateRef = mesh_records[(mesh.slot * MESH_PAGE_CLUSTERS + cluster) * MESH_RECORD_WORDS + 6u];
  let firstTriangle = templateRef & 0xffffffu;
  let triangleCount = templateRef >> 24u;
  // local is (corner, triangle in the cluster, 0).
  let triangle = u32(local.y);
  let drawn = triangle < triangleCount;
  let page = mesh.pageWord;
  let table = page + mesh_pages[page + PAGE_TABLE_WORD] / 4u;
  let vertices = page + mesh_pages[page + PAGE_VERTICES_WORD] / 4u;
  let triangles = page + mesh_pages[page + PAGE_TRIANGLES_WORD] / 4u;
  let indices = mesh_pages[triangles + firstTriangle + min(triangle, max(triangleCount, 1u) - 1u)];
  let corner = mesh_pages[table + cluster * 4u] + ((indices >> (8u * u32(local.x))) & 0xffu);
  let w0 = mesh_pages[vertices + corner * 4u];
  let w1 = mesh_pages[vertices + corner * 4u + 1u];
  let grid = vec3u(w0 & GRID_MASK, ((w0 >> 21u) | (w1 << 11u)) & GRID_MASK, w1 >> 10u);
  let p = vec3f(grid) * bitcast<f32>(mesh_pages[page + PAGE_STEP_WORD]);
  let placed = mesh.originM + p;
  let error = mesh_errors[mesh.slot * MESH_PAGE_CLUSTERS + cluster];
  let pushed = select(placed, placed * (1.0 + error / length(placed)), error > 0.0);
  let clip = frame.clipProjection * frame.viewRotation * vec4f(pushed, 1.0);
  return select(vec4f(2.0, 2.0, 2.0, 1.0), clip, drawn);
}
