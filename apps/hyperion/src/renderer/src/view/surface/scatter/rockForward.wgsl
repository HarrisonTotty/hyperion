// The authoritative rocks' forward entry points (plan R11, T4.b; `rock.wgsl`): each vertex placed by
// `rock_vertex`, each fragment shaded by `rock_fragment` with its geometric normal from its place's
// fine screen derivatives, oriented to the datum's side.

@vertex
fn vertexMain(@location(0) local : vec3f, @builtin(instance_index) entry : u32) -> RockVertexOut {
  return rock_vertex(local, entry);
}

@fragment
fn fragmentMain(in : RockVertexOut) -> @location(0) vec4f {
  let geometric = cascade_geometric_normal(
    dpdxFine(in.fromCameraBf), dpdyFine(in.fromCameraBf), normalize(in.datum)
  );
  return rock_fragment(in, geometric);
}
