// The sea at its shore (plan R11, R11.T8.d; Design note 12): the depth-limited height of a random
// sea, the surf zone's foam by depth and the bed's slope, and the depth fade to the bed over the
// pixel's reach along that slope; a library of functions with no binding of its own, which
// oceanNear.wgsl composes. Its CPU twin, in f64, is ocean/shoreline.ts, whose remarks give each
// figure's source; every constant here is its twin's.

// SURF_HEIGHT_RATIO, BREAKER_INDEX, SURF_SIMILARITY_BOUNDS and SHORE_FADE_MIN_M (shoreline.ts).
const SHORE_SURF_HEIGHT_RATIO : f32 = 0.42;
const SHORE_BREAKER_INDEX : f32 = 0.73;
const SHORE_SPILLING : f32 = 0.5;
const SHORE_SURGING : f32 = 3.3;
const SHORE_FADE_MIN : f32 = 0.05;

// The share of every component's amplitude kept at a water depth (`shoreAmplitudeScale`).
fn shore_amplitude_scale(depth : f32, rms_height : f32) -> f32 {
  if (!(rms_height > 0.0)) {
    return 1.0;
  }
  return clamp(SHORE_SURF_HEIGHT_RATIO * max(depth, 0.0) / rms_height, 0.0, 1.0);
}

// The shore's foam share at a water depth over a bed of slope `slope_tan` (`shoreFoamShare`).
fn shore_foam_share(depth : f32, slope_tan : f32, rms_height : f32, deep_wavelength : f32) -> f32 {
  if (!(rms_height > 0.0)) {
    return 0.0;
  }
  let ratio = SHORE_BREAKER_INDEX * max(depth, 0.0) / rms_height;
  let breaking = exp(-ratio * ratio);
  var xi = 1e30;
  if (deep_wavelength > 0.0) {
    xi = abs(slope_tan) / sqrt(rms_height / deep_wavelength);
  }
  let kind = clamp((SHORE_SURGING - xi) / (SHORE_SURGING - SHORE_SPILLING), 0.0, 1.0);
  return breaking * kind;
}

// A pixel's reach on the sea along the bed's slope, m (`shorePixelReachM`).
fn shore_pixel_reach(
  distance : f32,
  pixel_angle : f32,
  view_cosine : f32,
  slope_along_view : f32,
  min_cosine : f32,
) -> f32 {
  let along = slope_along_view / max(abs(view_cosine), min_cosine);
  let across = sqrt(max(1.0 - slope_along_view * slope_along_view, 0.0));
  return distance * pixel_angle * length(vec2f(along, across));
}

// The near sea's opacity at its shore (`shoreFade`).
fn shore_fade(depth : f32, footprint : f32, slope_tan : f32) -> f32 {
  let span = max(SHORE_FADE_MIN, footprint * abs(slope_tan));
  let x = clamp(depth / span, 0.0, 1.0);
  return x * x * (3.0 - 2.0 * x);
}
