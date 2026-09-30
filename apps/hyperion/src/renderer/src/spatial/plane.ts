import type { LocalFrame } from "./frame";
import type { PlaneSpec } from "./marks";
import { add, scale, type Vec3 } from "./vec3";

/** A straight line between two points, in scene units about the view centre. */
export interface Segment {
  readonly from: Vec3;
  readonly to: Vec3;
}

/** The reference grid: chords of its disc, in two families aligned to the local directions. */
export interface GridLines {
  /** Chords running along coreward, one per multiple of the spacing to spinward. */
  readonly alongCoreward: ReadonlyArray<Segment>;
  /** Chords running along spinward, one per multiple of the spacing to coreward. */
  readonly alongSpinward: ReadonlyArray<Segment>;
}

// A chord at the very rim of the disc has no length; it is left out rather than drawn as a dot.
const RIM_TOLERANCE = 1e-9;

function chords(
  extent: number,
  spacing: number,
  along: Vec3,
  across: Vec3,
): ReadonlyArray<Segment> {
  const segments: Segment[] = [];
  const count = Math.floor((extent * (1 - RIM_TOLERANCE)) / spacing);
  for (let k = -count; k <= count; k += 1) {
    const offset = k * spacing;
    const halfLength = Math.sqrt(extent * extent - offset * offset);
    if (!(halfLength > extent * RIM_TOLERANCE)) {
      continue;
    }
    const middle = scale(across, offset);
    segments.push({
      from: add(middle, scale(along, -halfLength)),
      to: add(middle, scale(along, halfLength)),
    });
  }
  return segments;
}

/**
 * The grid lines of the reference plane: chords of the disc of radius `extent`, parallel to
 * coreward and to spinward, at multiples of `spacing` from the centre.
 *
 * @remarks
 * The grid aligns to the local directions at the view centre, not to the galactic x and y, so a
 * `TOP` view can be measured against it along coreward and spinward.
 */
export function gridLines(plane: PlaneSpec, frame: LocalFrame): GridLines {
  if (!(plane.spacing > 0) || !(plane.extent > 0)) {
    return { alongCoreward: [], alongSpinward: [] };
  }
  return {
    alongCoreward: chords(plane.extent, plane.spacing, frame.coreward, frame.spinward),
    alongSpinward: chords(plane.extent, plane.spacing, frame.spinward, frame.coreward),
  };
}

/**
 * A ring in the reference plane about the view centre, as a closed polyline.
 *
 * @remarks
 * The first point is the ring's coreward point, where its label goes, and the last repeats it.
 *
 * @param segments - Straight pieces around the ring.
 */
export function ringPolyline(
  radius: number,
  frame: LocalFrame,
  segments = 96,
): ReadonlyArray<Vec3> {
  const points: Vec3[] = [];
  for (let i = 0; i <= segments; i += 1) {
    const angle = i === segments ? 0 : (2 * Math.PI * i) / segments;
    points.push(
      add(
        scale(frame.coreward, radius * Math.cos(angle)),
        scale(frame.spinward, radius * Math.sin(angle)),
      ),
    );
  }
  return points;
}
