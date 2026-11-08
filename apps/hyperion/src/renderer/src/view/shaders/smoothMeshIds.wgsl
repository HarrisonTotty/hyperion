// The mesh regime's figure shaded once a pixel (plan R07, the owner's Mac;
// `GeometrySettings.figureShading` `screen`): its raster. In `smoothMesh.wgsl`'s place of its
// fragment stage, after its vertex stage, which places every vertex as the forward draw does
// (`@invariant`), so that the figure writes the scene's depth to the same bits. Each fragment
// discards where the forward draw's does, beneath surveyed terrain (`surveyed_beneath`) and where
// the body does not cover the pixel wholly (`disc_pixel`'s first draw's test, `figure_covers`), and
// elsewhere writes its record's index into the view's `r32uint` figure IDs, which the resolve
// (`figureResolve.wgsl`) shades once a pixel. No light is formed here, so a 2 × 2 quad's helper
// lanes along the figure's small triangles cost the tests alone.

// Whether the body covers the pixel whose centre is `position` wholly, the pixels `disc_pixel`
// draws on its first draw (`edge_pass` 0): its centre's ray in front, then its four corners inside
// the limb by the same angles, the same gradient and the same tests, in the same order.
fn figure_covers(position : vec2f) -> bool {
  let body = body_of();
  let pixel = floor(position);
  if (dot(body.centre, view_ray(pixel + 0.5)) <= 0.0) {
    return false;
  }
  let c00 = angle_at(body, pixel);
  let c10 = angle_at(body, pixel + vec2f(1.0, 0.0));
  let c01 = angle_at(body, pixel + vec2f(0.0, 1.0));
  let c11 = angle_at(body, pixel + vec2f(1.0, 1.0));
  let gradient = length(vec2f(c10 + c11 - c00 - c01, c01 + c11 - c00 - c10) * 0.5);
  let centre = (c00 + c10 + c01 + c11) * 0.25;
  if (!(gradient > 0.0) || centre / gradient > OUTSIDE_PX) {
    return false;
  }
  let corners = vec4f(c00, c10, c01, c11);
  return all(corners < vec4f(0.0));
}

@fragment
fn fragmentMain(v : MeshVarying) -> @location(0) u32 {
  // WGSL's `discard` demotes the invocation to a helper, which runs on to the stage's end (Tint
  // emulates it so on Metal and Direct3D): the cover's test is taken only where the pixel is not
  // surveyed beneath, in a loop of at most one pass, not an `if` (`smoothMesh.wgsl`'s note).
  var covers = false;
  for (var test = !surveyed_beneath(v.position.xy); test; test = false) {
    covers = figure_covers(v.position.xy);
  }
  if (!covers) {
    discard;
  }
  return draw.disc;
}
