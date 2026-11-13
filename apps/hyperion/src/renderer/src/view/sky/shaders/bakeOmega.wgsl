// A face texel's solid angle as the bake's kernels read it (plan R06, T13.g): the face's lower
// triangle of rows, `cube.ts`'s `texelSolidAnglesLowerF32`, read at `lowerTriangleIndex`, so that a
// texel above the diagonal reads its mirror's. Composed after a kernel that binds the triangle as
// `omega`.

fn solidAngleAt(texel : vec2u) -> f32 {
  let row = max(texel.x, texel.y);
  let at = row * (row + 1u) / 2u + min(texel.x, texel.y);
  return omega[min(at, arrayLength(&omega) - 1u)];
}
