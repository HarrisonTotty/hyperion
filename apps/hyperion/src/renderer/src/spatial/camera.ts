import type { LocalFrame } from "./frame";
import { add, dot, scale, type Vec3 } from "./vec3";

/** The direction a spatial view is seen from, without its zoom. */
export interface CameraAngles {
  /**
   * Bearing of the direction of view in the reference plane, [0, 360): `000°` looks coreward,
   * `090°` spinward, `180°` rimward, `270°` antispinward.
   */
  readonly azimuthDeg: number;
  /** Angle of the camera above the reference plane, [−90, 90]; +90 looks south onto the plane. */
  readonly elevationDeg: number;
}

/** The orbit camera of a spatial view: two angles and a zoom, and no roll. */
export interface Camera extends CameraAngles {
  /** Screen pixels per scene unit; the same for the whole picture, since the view is orthographic. */
  readonly pxPerUnit: number;
}

/** The camera's screen axes as unit vectors in the scene's frame. */
export interface ViewBasis {
  /** Towards the right edge of the screen. */
  readonly right: Vec3;
  /** Towards the top edge of the screen. */
  readonly up: Vec3;
  /** From the viewer into the screen; larger depths are further away. */
  readonly forward: Vec3;
}

/** The drawing area of a spatial view, in CSS pixels. */
export interface Viewport {
  readonly widthPx: number;
  readonly heightPx: number;
  /** CSS pixels per `rem`, which follows the interface scale. */
  readonly remPx: number;
}

/** A scene point on the screen. */
export interface Projected {
  readonly xPx: number;
  readonly yPx: number;
  /** Distance along the direction of view from the view centre; larger is further away. */
  readonly depth: number;
}

/** Names of the preset views. */
export type PresetName = "top" | "side" | "front" | "oblique";

/**
 * The preset views, defined by their direction of view (plan 05, design note D10).
 *
 * @remarks
 * `TOP` looks south onto the plane with coreward up, `SIDE` looks coreward along the plane, `FRONT`
 * looks spinward along it, and the oblique view is the default.
 */
export const PRESETS: Readonly<Record<PresetName, CameraAngles>> = {
  top: { azimuthDeg: 0, elevationDeg: 90 },
  side: { azimuthDeg: 0, elevationDeg: 0 },
  front: { azimuthDeg: 90, elevationDeg: 0 },
  oblique: { azimuthDeg: 30, elevationDeg: 30 },
};

/** How far the zoom may move from the scale that fits the query sphere, as factors of it. */
export const ZOOM_RANGE = { min: 0.5, max: 100 } as const;

/** Bounds on a camera's zoom, in pixels per scene unit. */
export interface ZoomLimits {
  readonly minPxPerUnit: number;
  readonly maxPxPerUnit: number;
}

const DEG = Math.PI / 180;
const ANGLE_MATCH_TOLERANCE_DEG = 1e-6;

// Exact at multiples of 90°, where `Math.cos(Math.PI / 2)` is 6e-17 rather than 0, so that the
// preset views are exactly axis-aligned and depth ties seen from the top are exact.
function cosSinDeg(angleDeg: number): [number, number] {
  const quarterTurns = angleDeg / 90;
  if (Number.isInteger(quarterTurns)) {
    const exact: ReadonlyArray<[number, number]> = [
      [1, 0],
      [0, 1],
      [-1, 0],
      [0, -1],
    ];
    return exact[((quarterTurns % 4) + 4) % 4] ?? [1, 0];
  }
  return [Math.cos(angleDeg * DEG), Math.sin(angleDeg * DEG)];
}

/** Maps an azimuth in degrees into [0, 360). */
export function wrapAzimuthDeg(azimuthDeg: number): number {
  const wrapped = ((azimuthDeg % 360) + 360) % 360;
  // Adding 360 to a tiny negative remainder can round up to 360 itself.
  return wrapped >= 360 ? 0 : wrapped;
}

/** Clamps an elevation in degrees to [−90, 90]. */
export function clampElevationDeg(elevationDeg: number): number {
  return Math.min(90, Math.max(-90, elevationDeg));
}

/**
 * The camera's screen axes for a frame (plan 05, design note D10).
 *
 * @remarks
 * With c, s and n the frame's coreward, spinward and north, and h = cos α·c + sin α·s:
 * forward = cos ε·h − sin ε·n, right = cos α·s − sin α·c (h × n, never degenerate, so there is
 * no roll), and up = cos ε·n + sin ε·h (right × forward). The frame is a parameter so that the same
 * code serves a body-centred frame.
 */
export function viewBasis(frame: LocalFrame, angles: CameraAngles): ViewBasis {
  const [cosA, sinA] = cosSinDeg(angles.azimuthDeg);
  const [cosE, sinE] = cosSinDeg(clampElevationDeg(angles.elevationDeg));
  const horizontal = add(scale(frame.coreward, cosA), scale(frame.spinward, sinA));
  return {
    forward: add(scale(horizontal, cosE), scale(frame.north, -sinE)),
    right: add(scale(frame.spinward, cosA), scale(frame.coreward, -sinA)),
    up: add(scale(frame.north, cosE), scale(horizontal, sinE)),
  };
}

/**
 * Projects a point, given relative to the view centre in scene units, onto the screen.
 *
 * @remarks
 * Orthographic: x = centre + k·(d·right), y = centre − k·(d·up), depth = d·forward, with the view
 * centre at the middle of the viewport and k the camera's pixels per unit.
 */
export function project(
  relative: Vec3,
  basis: ViewBasis,
  camera: Camera,
  viewport: Viewport,
): Projected {
  return {
    xPx: viewport.widthPx / 2 + camera.pxPerUnit * dot(relative, basis.right),
    yPx: viewport.heightPx / 2 - camera.pxPerUnit * dot(relative, basis.up),
    depth: dot(relative, basis.forward),
  };
}

/** Turns the camera by the given angles, wrapping the azimuth and clamping the elevation. */
export function rotateCamera(camera: Camera, dAzimuthDeg: number, dElevationDeg: number): Camera {
  return {
    azimuthDeg: wrapAzimuthDeg(camera.azimuthDeg + dAzimuthDeg),
    elevationDeg: clampElevationDeg(camera.elevationDeg + dElevationDeg),
    pxPerUnit: camera.pxPerUnit,
  };
}

/**
 * Zooms the camera by a factor (above 1 zooms in), clamped to the limits.
 *
 * @param factor - Multiplies the pixels per unit; must be positive.
 */
export function zoomCamera(camera: Camera, factor: number, limits: ZoomLimits): Camera {
  const pxPerUnit = Math.min(
    limits.maxPxPerUnit,
    Math.max(limits.minPxPerUnit, camera.pxPerUnit * factor),
  );
  return { ...camera, pxPerUnit };
}

/** The zoom limits around a fitted scale, per {@link ZOOM_RANGE}. */
export function zoomLimits(fittedPxPerUnit: number): ZoomLimits {
  return {
    minPxPerUnit: fittedPxPerUnit * ZOOM_RANGE.min,
    maxPxPerUnit: fittedPxPerUnit * ZOOM_RANGE.max,
  };
}

/**
 * The zoom at which a sphere of `radius` about the view centre fits the viewport's shorter side.
 *
 * @param radius - Sphere radius in scene units; must be positive.
 * @param marginPx - Space kept clear between the sphere and the edge.
 * @throws RangeError when the radius is not positive.
 */
export function fitPxPerUnit(radius: number, viewport: Viewport, marginPx: number): number {
  if (!(radius > 0) || !Number.isFinite(radius)) {
    throw new RangeError(`fit radius must be positive, got ${String(radius)}`);
  }
  const availablePx = Math.max(1, Math.min(viewport.widthPx, viewport.heightPx) / 2 - marginPx);
  return availablePx / radius;
}

function azimuthDifferenceDeg(a: number, b: number): number {
  const difference = Math.abs(wrapAzimuthDeg(a) - wrapAzimuthDeg(b));
  return Math.min(difference, 360 - difference);
}

/** Whether the camera looks in a preset's direction, whatever its zoom. */
export function matchesPreset(camera: CameraAngles, preset: CameraAngles): boolean {
  return (
    azimuthDifferenceDeg(camera.azimuthDeg, preset.azimuthDeg) <= ANGLE_MATCH_TOLERANCE_DEG &&
    Math.abs(camera.elevationDeg - preset.elevationDeg) <= ANGLE_MATCH_TOLERANCE_DEG
  );
}
