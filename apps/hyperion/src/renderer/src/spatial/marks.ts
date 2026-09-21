import type { LocalFrame } from "./frame";
import { dot, type Vec3 } from "./vec3";

/**
 * The outline of a mark, which encodes its type.
 *
 * @remarks
 * One meaning per shape on every spatial display. `circle` is a star system; `diamond`, `square`
 * and `triangle` are drawn now and given their meaning by plan 06. Later plans add their own
 * shapes to this union.
 */
export type SymbolShape = "circle" | "diamond" | "square" | "triangle";

/** The size of a mark, 0 (smallest) to 4, which encodes a class and never depth. */
export type SizeClass = 0 | 1 | 2 | 3 | 4;

/** What a mark's colour says: `available` is drawn in `--accent`, `plain` in `--text`. */
export type MarkStatus = "plain" | "available";

/** A mark at a point, such as a star system. */
export interface PointMark {
  /** Stable identity, used for selection, picking ties and draw-order ties. */
  readonly id: string;
  /** Position relative to the view centre, in scene units and the scene's axes. */
  readonly position: Vec3;
  readonly shape: SymbolShape;
  readonly sizeClass: SizeClass;
  readonly status: MarkStatus;
  /** Text of the mark's label when one is shown. */
  readonly label: string;
  /** Higher values are labelled first when not every mark can be. */
  readonly labelPriority: number;
}

/** A sphere about the view centre, drawn as its outline: a circle of the same radius. */
export interface SphereMark {
  /** Radius in scene units. */
  readonly radius: number;
  /** `range` is what can be reached; `data_edge` is where fetched data ends. */
  readonly role: "range" | "data_edge";
  readonly label: string;
}

/** A ring on the reference plane about the view centre. */
export interface PlaneRing {
  /** Radius in scene units. */
  readonly radius: number;
  /** Label at the ring's coreward point; empty for an unlabelled grid ring. */
  readonly label: string;
}

/** The reference plane through the view centre, with its grid and rings. */
export interface PlaneSpec {
  /** Distance between grid lines, in scene units. */
  readonly spacing: number;
  /** Radius of the disc the grid covers, in scene units. */
  readonly extent: number;
  readonly rings: ReadonlyArray<PlaneRing>;
}

/** Everything a spatial view draws, in scene units about the view centre. */
export interface SpatialScene {
  readonly frame: LocalFrame;
  readonly points: ReadonlyArray<PointMark>;
  readonly spheres: ReadonlyArray<SphereMark>;
  readonly plane: PlaneSpec;
  /** The selected mark, drawn with the `--accent` bracket reticle. */
  readonly selectedId: string | null;
  /** The commanded destination, drawn with the `--target` reticle. */
  readonly destinationId: string | null;
}

/**
 * Whether a position lies on the north side of the reference plane, where marks are filled.
 *
 * @remarks
 * Derived, never passed in. A mark exactly on the plane counts as above, and so is filled.
 */
export function isAbovePlane(frame: LocalFrame, position: Vec3): boolean {
  return dot(position, frame.north) >= 0;
}
