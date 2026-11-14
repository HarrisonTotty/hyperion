// The instance path's test particles (plan R14, R14.T5.d; `clusters/instanceScene.ts`): a bucket
// draw of a particle template (`particles.ts`), placed by `particleInstance.wgsl`, composed after
// frame.wgsl and this file's `Draw`, and lit by one light from the draw's direction with an ambient
// share, the instance's colour as its albedo. Check-only: the test field's picture, which R11's own
// materials replace. The position is `@invariant`, as every cluster path's vertex stage's is.

struct ParticleOut {
  @builtin(position) @invariant position : vec4f,
  @location(0) normal : vec3f,
  @location(1) @interpolate(flat) colour : vec4f,
}

@vertex
fn vertexMain(@location(0) local : vec3f, @builtin(instance_index) entry : u32) -> ParticleOut {
  let place = particle_place(entry, local);
  var out : ParticleOut;
  out.position = frame.clipProjection * frame.viewRotation * vec4f(place.position, 1.0);
  out.normal = place.normal;
  out.colour = place.colour;
  return out;
}

@fragment
fn fragmentMain(in : ParticleOut) -> @location(0) vec4f {
  let lit = draw.light.w + (1.0 - draw.light.w) * max(dot(normalize(in.normal), draw.light.xyz), 0.0);
  return vec4f(in.colour.rgb * lit, 1.0);
}
