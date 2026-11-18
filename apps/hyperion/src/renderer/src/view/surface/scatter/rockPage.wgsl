// The authoritative rocks' mesh pages (plan R11, T4.b; plan R14, R14.T5.c): what every rock draw
// reads of a finest patch's rock mesh (`rockPages.ts`), its clusters held in the rocks' page pool
// and culled by the cluster path into a bucket's list (`clusters/buckets.ts`), each template vertex
// of the mesh shape's one template (`clusters/templates.ts`' `meshTemplate`, 128 triangles by index
// pulling) decoded from its page as `meshOccluderCluster.wgsl` decodes a corner: the cluster's
// template reference from the pool's records, its first vertex from the page's cluster table, its
// triangle's 8-bit index, its vertex's 21-bit grid coordinates times the page's step about the
// page's origin, exactly in `f32`, its octahedral normal and its rock's index in its fourth word. A
// template vertex past the cluster's last triangle is placed outside the clip volume by its draw.
// Composed by the lit draw (`rock.wgsl`), the wireframe's (`rockWire.wgsl`), the depth's and the
// visibility path's (`rockMaterial.ts`).

// The buckets' lists, one `u32` an entry, (instance << 8) | cluster, each bucket from its base.
@group(2) @binding(22) var<storage, read> cluster_lists : array<u32>;

// A rock instance (`rockDraw.ts`' ROCK_INSTANCE_BYTES): one resident page of a drawn patch's rock
// mesh. Its pages' origin from the camera, m, in the non-rotating axes, and its pool slot, the
// cull's first four words; then its page's first word in the pool's pages, the patch's terrain
// slot, whose class layers and horizon map shade it, and its first rock in the view's rock records;
// then its pages' origin about the patch's origin, body-fixed m, which the rocks' records are
// taken about.
struct RockInstance {
  originM : vec3f,
  slot : u32,
  pageWord : u32,
  terrainSlot : u32,
  firstRock : u32,
  spare : u32,
  pageOriginM : vec3f,
  pad : u32,
}

// A rock's record (`rockPages.ts`' ROCK_RECORD_WORDS): its base centre less its patch's origin,
// body-fixed m; the ground's height at its axis, m; the datum's normal there; its place in the
// patch, (x, y) ÷ 64, as two unorm16.
struct RockRecord {
  baseM : vec3f,
  groundM : f32,
  datum : vec3f,
  place : u32,
}

// The rocks' bindings, past the lit terrain's.
@group(2) @binding(25) var<storage, read> rock_pages : array<u32>;
@group(2) @binding(26) var<storage, read> rock_page_records : array<u32>;
@group(2) @binding(27) var<storage, read> rock_instances : array<RockInstance>;
@group(2) @binding(28) var<storage, read> rock_records : array<RockRecord>;

// A slot's records and a record's words (`clusters/pages.ts`).
const ROCK_PAGE_CLUSTERS : u32 = 128u;
const ROCK_RECORD_WORDS : u32 = 8u;

// A page header's words (`page.rs`): the grid's step, then the cluster table's, the vertices' and
// the triangles' byte offsets in the page.
const ROCK_PAGE_STEP_WORD : u32 = 14u;
const ROCK_PAGE_TABLE_WORD : u32 = 20u;
const ROCK_PAGE_VERTICES_WORD : u32 = 21u;
const ROCK_PAGE_TRIANGLES_WORD : u32 = 22u;

// One grid coordinate's bits.
const ROCK_GRID_MASK : u32 = 0x1fffffu;

// The unit vector of an octahedral pair, as the bake's `decode_octahedral` and
// `terrainPatch.wgsl`'s `decodeOctahedral`.
fn rock_octahedral(e : vec2f) -> vec3f {
  let z = 1.0 - abs(e.x) - abs(e.y);
  var xy = e;
  if (z < 0.0) {
    xy = (1.0 - abs(e.yx)) * select(vec2f(1.0), vec2f(-1.0), e < vec2f(0.0));
  }
  return normalize(vec3f(xy, z));
}

// A decoded corner: its offset from its page's origin and its normal, body-fixed, and its rock.
struct RockCorner {
  offset : vec3f,
  normal : vec3f,
  rock : u32,
  drawn : bool,
}

// The corner `corner` of triangle `triangle` of `cluster` in the page of `instance`.
fn rock_corner(instance : RockInstance, cluster : u32, triangle : u32, corner : u32) -> RockCorner {
  let templateRef = rock_page_records[(instance.slot * ROCK_PAGE_CLUSTERS + cluster) * ROCK_RECORD_WORDS + 6u];
  let firstTriangle = templateRef & 0xffffffu;
  let triangleCount = templateRef >> 24u;
  let page = instance.pageWord;
  let table = page + rock_pages[page + ROCK_PAGE_TABLE_WORD] / 4u;
  let vertices = page + rock_pages[page + ROCK_PAGE_VERTICES_WORD] / 4u;
  let triangles = page + rock_pages[page + ROCK_PAGE_TRIANGLES_WORD] / 4u;
  let indices = rock_pages[triangles + firstTriangle + min(triangle, max(triangleCount, 1u) - 1u)];
  let vertex = rock_pages[table + cluster * 4u] + ((indices >> (8u * corner)) & 0xffu);
  let w0 = rock_pages[vertices + vertex * 4u];
  let w1 = rock_pages[vertices + vertex * 4u + 1u];
  let w2 = rock_pages[vertices + vertex * 4u + 2u];
  let grid = vec3u(w0 & ROCK_GRID_MASK, ((w0 >> 21u) | (w1 << 11u)) & ROCK_GRID_MASK, w1 >> 10u);
  var out : RockCorner;
  out.offset = vec3f(grid) * bitcast<f32>(rock_pages[page + ROCK_PAGE_STEP_WORD]);
  out.normal = rock_octahedral(unpack2x16snorm(w2));
  out.rock = rock_pages[vertices + vertex * 4u + 3u];
  out.drawn = triangle < triangleCount;
  return out;
}
