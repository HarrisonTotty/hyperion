// A ring's particles close to as drawn (plan R11, R11.T9.d; Design note 16; `ringParticles.ts`): a
// bucket draw of a sphere template (`clusters/particles.ts`), placed by `particleInstance.wgsl`'s
// `particle_place`, opaque and writing depth, lit by the ring's star as a Lambert sphere of the
// particle's albedo under the planet's shadow where the field is and the layer above it (its
// colour's alpha), its brightness by the power law's phase over the Lambert sphere's at the view's
// phase angle (in `draw.light`), pre-exposed, its meter class in alpha. Composed after frame.wgsl,
// this material's `Draw` (`ringParticles.ts`) and `PARTICLE_INSTANCE_WGSL`. The position is `@invariant`, as every cluster path's vertex stage's is.

struct RingParticleOut {
  @builtin(position) @invariant position : vec4f,
  @location(0) normal : vec3f,
  @location(1) @interpolate(flat) colour : vec4f,
}

@vertex
fn vertexMain(@location(0) local : vec3f, @builtin(instance_index) entry : u32) -> RingParticleOut {
  let place = particle_place(entry, local);
  var out : RingParticleOut;
  out.position = frame.clipProjection * frame.viewRotation * vec4f(place.position, 1.0);
  out.normal = place.normal;
  out.colour = place.colour;
  return out;
}

@fragment
fn fragmentMain(in : RingParticleOut) -> @location(0) vec4f {
  let lambert = max(dot(normalize(in.normal), draw.sun.xyz), 0.0);
  // The colour's alpha is the share of the star's beam the layer above lets through to it.
  return vec4f(in.colour.rgb * in.colour.a * draw.light.rgb * lambert, draw.light.w);
}
