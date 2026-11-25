// Decorative scatter's cull against the bodies at the ground (plan R11, R11.T6.a; Design note 7):
// the twin of `scatterCull.ts`' `intersectsBody`. A library composed by concatenation, with no
// binding of its own: its caller holds the bodies, each an oriented box in the passes' frame, and
// tests each instance against every one. A box comes only from a body whose bounding volume lies
// within 0.2 m of the ground (`scatterCull.ts`' `nearGround`, on the CPU).

// A body's box (`scatterCull.ts`' `packScatterBoxes`): its centre less the camera, m, in the passes'
// frame (w unused), then its three unit axes in that frame, each with its half extent along it, m,
// as w.
struct ScatterBox {
  centre : vec4f,
  axis0 : vec4f,
  axis1 : vec4f,
  axis2 : vec4f,
}

// Whether the sphere of `radius` m about `centre` (the passes' frame, less the camera) meets `box`:
// its centre inside the box widened by the radius on every axis, `intersectsBody`'s test.
fn scatter_box_meets(box : ScatterBox, centre : vec3f, radius : f32) -> bool {
  let d = centre - box.centre.xyz;
  return abs(dot(d, box.axis0.xyz)) <= box.axis0.w + radius &&
    abs(dot(d, box.axis1.xyz)) <= box.axis1.w + radius &&
    abs(dot(d, box.axis2.xyz)) <= box.axis2.w + radius;
}
