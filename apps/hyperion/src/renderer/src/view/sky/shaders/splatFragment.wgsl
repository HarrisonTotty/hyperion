// The bake splat's fragment stage: the star's illuminance and its count, added by the splat's
// one-one blend (plan R06, T13.g).

@fragment
fn main(@location(0) @interpolate(flat) light : vec4f) -> @location(0) vec4f {
  return light;
}
