// The hulls' occluder faces on the cluster path (plan R14, R14.T4.b): a vertex stage over a bucket's
// list (`clusters/buckets.ts`) and the mesh shape's one template (`clusters/templates.ts`'
// `meshTemplate`), 128 triangles by index pulling, in place of occluder.wgsl's instanced stage; the
// fragment stage is occluder.wgsl's, to the byte. Each list entry names a hull, its instance, and a
// cluster of its page, the hull's faces 128 at a time from its first triangle in `corners`; a
// template vertex past the cluster's last triangle is placed outside the clip volume, one point for
// all three corners, so that it draws nothing. A corner is placed as occluder.wgsl places it, the
// hull's reference point from the camera plus the corner's offset, the position `@invariant`, so
// that each pixel's depth is the instanced draw's bit for bit. Composed after frame.wgsl and
// occluder.wgsl, its `vertexMain` taken out and its `Draw` declaring `clusterBase`
// (`wireframe/hullClusters.ts`' `hullClusterSource`).

// A hull as the cull and this stage read it (`hullClusters.ts`' HULL_INSTANCE_BYTES): its
// reference point from the camera, m, and its page, the cull's first four words; then its first
// triangle in `corners` and its triangles.
struct HullInstance {
  originM : vec3f,
  page : u32,
  firstTriangle : u32,
  triangles : u32,
  spare0 : u32,
  spare1 : u32,
}

@group(2) @binding(1) var<storage, read> hull_instances : array<HullInstance>;

// The buckets' lists, one `u32` an entry, (instance << 8) | cluster, each bucket from its base.
@group(2) @binding(2) var<storage, read> cluster_lists : array<u32>;

// A mesh cluster's triangles (`record.ts`' MESH_CLUSTER_TRIANGLES).
const HULL_CLUSTER_TRIANGLES : u32 = 128u;

@vertex
fn vertexMain(@location(0) local : vec3f, @builtin(instance_index) entry : u32) -> @builtin(position) @invariant vec4f {
  let item = cluster_lists[draw.clusterBase + entry];
  let hull = hull_instances[item >> 8u];
  // local is (corner, triangle in the cluster, 0).
  let triangle = (item & 0xffu) * HULL_CLUSTER_TRIANGLES + u32(local.y);
  let drawn = triangle < hull.triangles;
  let p = corners[(hull.firstTriangle + min(triangle, hull.triangles - 1u)) * 3u + u32(local.x)].xyz;
  let clip = frame.clipProjection * frame.viewRotation * vec4f(hull.originM + p, 1.0);
  return select(vec4f(2.0, 2.0, 2.0, 1.0), clip, drawn);
}
