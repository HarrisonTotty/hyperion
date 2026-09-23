import type { LocalFrame } from "./frame";
import { dot, type Vec3 } from "./vec3";

/**
 * The outline of a mark, which encodes its type.
 *
 * @remarks
 * One meaning per shape on every spatial display. `circle` is a star system; `diamond`, `square`
 * and `triangle` are drawn now and given their meaning by plan 06. Later plans add their own
 * shapes to this union: plan 06 the `ringed-circle` (a giant), plan 13 the `triangle-down` (a
 * planet), plan 14 the `pentagon` (a moon) and the `hexagon` (an unresolved contact). Each is drawn
 * here before the display that gives it its meaning is built, and every one is closed, since a mark
 * is filled above the reference plane and open below it.
 */
export type SymbolShape =
  | "circle"
  | "diamond"
  | "square"
  | "triangle"
  | "ringed-circle"
  | "triangle-down"
  | "pentagon"
  | "hexagon";

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

/**
 * What a path is drawn as: `reference` in `--line`, as an orbit is, or `selected` in `--text`, as
 * the selected body's orbit is because it carries meaning (plan 14, T38.a).
 *
 * @remarks
 * Both are solid hairlines. Dashes are kept for predicted paths and `--target` for commanded ones,
 * which the guide gives them and which no role here draws.
 */
export type PathRole = "reference" | "selected";

/** A polyline in three dimensions, such as an orbit: a reference line, never picked. */
export interface PathMark {
  /** Stable identity, which keys its label. */
  readonly id: string;
  /**
   * Its points in order, relative to the view centre in scene units and the scene's axes; a closed
   * path repeats its first point last.
   */
  readonly points: ReadonlyArray<Vec3>;
  readonly role: PathRole;
  /** Text of its label; empty for none. */
  readonly label: string;
  /** The point its label names, in the same units and axes as its points. */
  readonly labelAt: Vec3;
}

/**
 * A band on the reference plane between two circles about one centre, such as a habitable zone or
 * a belt: drawn as its two edges in `--line`, a belt's joined by radial ticks every 10°, with no
 * fill, hatch or dots (plan 14, T38.a).
 */
export interface AnnulusMark {
  /** Stable identity, which keys its label. */
  readonly id: string;
  /**
   * Its centre, relative to the view centre in scene units; the view centre when absent. Only its
   * foot on the reference plane counts, since the band lies in the plane.
   */
  readonly centre?: Vec3 | undefined;
  /** Radius of the inner edge in scene units; an edge of radius 0 is not drawn. */
  readonly innerRadius: number;
  /** Radius of the outer edge in scene units, at least the inner's; equal radii draw one edge. */
  readonly outerRadius: number;
  /** Whether the edges are joined by radial ticks every 10°, which marks a belt. */
  readonly ticks: boolean;
  /** Text of its label, at the outer edge's rimward point; empty for none. */
  readonly label: string;
}

/** Everything a spatial view draws, in scene units about the view centre. */
export interface SpatialScene {
  readonly frame: LocalFrame;
  readonly points: ReadonlyArray<PointMark>;
  readonly spheres: ReadonlyArray<SphereMark>;
  readonly plane: PlaneSpec;
  /** Paths such as orbits, drawn but never picked; none when absent (plan 14, T40.a). */
  readonly paths?: ReadonlyArray<PathMark> | undefined;
  /** Bands on the reference plane, drawn with it and never picked; none when absent. */
  readonly annuli?: ReadonlyArray<AnnulusMark> | undefined;
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
