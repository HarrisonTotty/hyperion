// The mesh regime's figure shaded once a pixel (plan R07, the owner's Mac;
// `GeometrySettings.figureShading` `screen`): its resolve. In `bodyDiscDraw.wgsl`'s place of its
// fragment stage, after its vertex stage: the body's disc rectangle, its corners at the near plane
// (depth 1 under reversed-Z), so that the depth test passes every pixel, with no depth written. Each
// pixel whose figure ID (`smoothMeshIds.wgsl`'s raster, the scene's depth tested and written as the
// forward draw's) is this record's takes the light the forward draw gives it (`figure_colour`);
// every other is discarded. A 2 × 2 quad's helper lanes then fall at the figure's outline alone.

// The view's figure IDs: each pixel's record, or 0xffff_ffff where no figure drew.
@group(2) @binding(18) var figure_ids : texture_2d<u32>;

// The pixel whose centre is `position` as `disc_pixel`'s first draw leaves a pixel it draws: its
// interior's cells, the light as their mean over its covered share, the meter class in alpha.
fn figure_colour(position : vec2f) -> vec4f {
  let body = body_of();
  let counts = disc_row(3u);
  // One call of the shading (`CellSamples`' note).
  let sum = pixel_cells(body, position, u32(counts.x));
  let light = select(vec3f(0.0), sum.radiance / sum.coverage, sum.coverage > 0.0);
  // The pure-pixel class: lit where every shaded point is, unlit where none is, `other` where
  // they are mixed or no star lights the body.
  var meter = METER_OTHER;
  if (u32(counts.z) > 0u && sum.shaded > 0u) {
    if (sum.lit == sum.shaded) {
      meter = METER_LIT_BODY;
    } else if (sum.lit == 0u) {
      meter = METER_UNLIT_BODY;
    }
  }
  return vec4f(min(light, vec3f(HDR_MAX)), meter);
}

@fragment
fn fragmentMain(v : DiscVarying) -> @location(0) vec4f {
  // WGSL's `discard` demotes the invocation to a helper, which runs on to the stage's end (Tint
  // emulates it so on Metal and Direct3D, with a flag that discards at the end): a discarded
  // fragment would shade in full. So the light is formed only where the fragment is kept, in a loop
  // of at most one pass, not an `if`, as the composite's reads are (`composite.wgsl`): SwiftShader,
  // the checks' adapter, runs both sides of a branch under its lanes' mask, and a loop only while a
  // lane is in it. The pixels kept draw what they drew, bit for bit.
  let own = textureLoad(figure_ids, vec2i(v.position.xy), 0).r == draw.disc;
  var colour = vec4f(0.0);
  for (var shade = own; shade; shade = false) {
    colour = figure_colour(v.position.xy);
  }
  if (!own) {
    discard;
  }
  return colour;
}
