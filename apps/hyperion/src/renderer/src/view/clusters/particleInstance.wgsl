// The particles' vertex library (plan R14, R14.T5.d; `clusters/particles.ts`): a bucket draw's
// instance read from its list and its record, and a template's vertex placed for it. Composed
// after frame.wgsl and the material's own `Draw`, which declares `clusterBase`, the bucket's first
// entry in the lists (`clusters/buckets.ts`).

// A particle's instance record (`particles.ts`' PARTICLE_INSTANCE_BYTES): its centre less the camera,
// m, in the passes' frame, and its radius, m, the instance cull's bounds; its rotation, a unit
// quaternion (x, y, z, w) as four snorm16; the consumer's colour (rgba8unorm) and tag.
struct ParticleInstance {
  centre : vec3f,
  radius : f32,
  rotation : vec2u,
  colour : u32,
  tag : u32,
}

@group(2) @binding(0) var<storage, read> particle_instances : array<ParticleInstance>;

// The buckets' lists, one `u32` an entry, the instance's index, each bucket from its base.
@group(2) @binding(1) var<storage, read> particle_lists : array<u32>;

// A template's vertex placed for an instance: its place less the camera, m, its unit normal, both in
// the passes' frame, and the instance's colour and tag.
struct ParticlePlace {
  position : vec3f,
  normal : vec3f,
  colour : vec4f,
  tag : u32,
}

// `v` turned by the unit quaternion `q` (x, y, z, w).
fn particle_rotate(q : vec4f, v : vec3f) -> vec3f {
  let t = 2.0 * cross(q.xyz, v);
  return v + q.w * t + cross(q.xyz, t);
}

// The template's vertex `local` (over the instance's radius, at its template's scale) placed for the
// instance of entry `entry` of the draw's bucket, its normal the turned vertex's direction.
fn particle_place(entry : u32, local : vec3f) -> ParticlePlace {
  let instance = particle_instances[particle_lists[draw.clusterBase + entry]];
  let q = vec4f(unpack2x16snorm(instance.rotation.x), unpack2x16snorm(instance.rotation.y));
  let turned = particle_rotate(q * inverseSqrt(max(dot(q, q), 1e-12)), local);
  return ParticlePlace(instance.centre + instance.radius * turned, normalize(turned), unpack4x8unorm(instance.colour), instance.tag);
}
