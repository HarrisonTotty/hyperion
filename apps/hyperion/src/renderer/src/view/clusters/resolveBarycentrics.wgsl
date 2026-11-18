// The visibility buffer's resolves' barycentrics (plan R14, R14.T2.d, shared with R14.T5.e's rocks;
// `clusters/resolve.ts`): a pixel's perspective-correct weights on its triangle from the clip
// positions of its three refetched corners, each snapped as the rasteriser snapped it. Composed by
// each resolve after its `Draw`, whose `resolveLists.w` is the cells a pixel of the rasteriser's
// sub-pixel grid (`resolveUniforms`).

// Clip position `c` with its place on a frame of `size` px snapped to the nearest cell of the
// rasteriser's sub-pixel grid, the draw's `resolveLists.w` cells a pixel, as the rasteriser snaps a
// corner before it covers and interpolates; a corner behind the eye kept whole, since the
// rasteriser snaps the corners it clips to the near plane instead (`resolve.ts`' `snappedClip`).
fn resolve_snapped(c : vec4f, size : vec2f) -> vec4f {
  if (!(c.w > 0.0)) {
    return c;
  }
  let cells = draw.resolveLists.w;
  let px = vec2f((0.5 + 0.5 * c.x / c.w) * size.x, (0.5 - 0.5 * c.y / c.w) * size.y);
  let snapped = round(px * cells) / cells;
  return vec4f((2.0 * snapped.x / size.x - 1.0) * c.w, (1.0 - 2.0 * snapped.y / size.y) * c.w, c.z, c.w);
}

// The perspective-correct barycentrics of the point at normalised device coordinates `ndc` on the
// triangle of clip positions `c0`, `c1` and `c2`: b ∝ u × v, u_i = x_i − X w_i and v_i = y_i − Y w_i,
// each at least 0 and summing to 1, the first corner's alone for a triangle seen edge-on
// (`resolve.ts`' `clipBarycentrics`). Clamped to the triangle: a pixel's centre may still fall a hair
// outside it (on a grid other than the one taken, or at a corner clipped), where a triangle seen near
// edge-on would carry the weights far beyond it.
fn resolve_barycentrics(c0 : vec4f, c1 : vec4f, c2 : vec4f, ndc : vec2f) -> vec3f {
  let u = vec3f(c0.x - ndc.x * c0.w, c1.x - ndc.x * c1.w, c2.x - ndc.x * c2.w);
  let v = vec3f(c0.y - ndc.y * c0.w, c1.y - ndc.y * c1.w, c2.y - ndc.y * c2.w);
  let b = cross(u, v);
  // Oriented by the triangle's side, det(w, x, y), the same at every pixel (`clipBarycentrics`).
  let w = max(sign(dot(b, vec3f(c0.w, c1.w, c2.w))) * b, vec3f(0.0));
  let total = w.x + w.y + w.z;
  if (total > 0.0) {
    return w / total;
  }
  return vec3f(1.0, 0.0, 0.0);
}
