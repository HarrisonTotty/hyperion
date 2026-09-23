import type { CameraAngles } from "./camera";
import type { LocalFrame } from "./frame";
import { TRIAD_MARKER_REM, type TriadAxis, type TriadBoxRem, triadLayout } from "./furniture";

/** SVG user units in a `rem`, so that stroke widths read as CSS pixels at 100%. */
const UNITS_PER_REM = 16;
/** The arrowhead of an axis across the screen, and the dot of the towards symbol, in `rem`. */
const HEAD_REM = 0.3;
const DOT_REM = 0.09;

function length(point: { readonly x: number; readonly y: number }): number {
  return Math.hypot(point.x, point.y);
}

/** SVG user units for a length in `rem`. */
function units(rem: number): number {
  return rem * UNITS_PER_REM;
}

interface AxisMarkProps {
  readonly axis: TriadAxis;
}

/** An axis: a line from the origin, ended by the away or towards symbol or by an arrowhead. */
function AxisMark({ axis }: AxisMarkProps) {
  // The axis's own geometry is in `rem`; what the SVG is given is in user units.
  const tipXUnits = units(axis.tip.x);
  const tipYUnits = units(axis.tip.y);
  const reachRem = length(axis.tip);
  const markerUnits = units(TRIAD_MARKER_REM);
  // The line stops at the symbol's circle, so that it never runs into the cross or the dot.
  const lineReachRem = axis.end === "across" ? reachRem : reachRem - TRIAD_MARKER_REM;
  const direction =
    reachRem > 0 ? { x: axis.tip.x / reachRem, y: axis.tip.y / reachRem } : { x: 0, y: 0 };
  const lineEndRem = {
    x: direction.x * Math.max(0, lineReachRem),
    y: direction.y * Math.max(0, lineReachRem),
  };
  const crossUnits = markerUnits * Math.SQRT1_2;
  const headUnits = units(HEAD_REM);
  const barb = (sign: number): string => {
    const angle = Math.atan2(direction.y, direction.x) + Math.PI + (sign * Math.PI) / 6;
    return `${tipXUnits + headUnits * Math.cos(angle)},${tipYUnits + headUnits * Math.sin(angle)}`;
  };
  return (
    <g data-axis={axis.name} data-end={axis.end}>
      {lineReachRem > 0 ? (
        <line x1="0" y1="0" x2={units(lineEndRem.x)} y2={units(lineEndRem.y)} />
      ) : null}
      {axis.end === "across" ? (
        <polyline points={`${barb(1)} ${tipXUnits},${tipYUnits} ${barb(-1)}`} />
      ) : (
        <circle cx={tipXUnits} cy={tipYUnits} r={markerUnits} />
      )}
      {axis.end === "towards" ? (
        <circle className="axis-triad__dot" cx={tipXUnits} cy={tipYUnits} r={units(DOT_REM)} />
      ) : null}
      {axis.end === "away" ? (
        <path
          d={`M${tipXUnits - crossUnits} ${tipYUnits - crossUnits}L${tipXUnits + crossUnits} ${tipYUnits + crossUnits}M${tipXUnits - crossUnits} ${tipYUnits + crossUnits}L${tipXUnits + crossUnits} ${tipYUnits - crossUnits}`}
        />
      ) : null}
    </g>
  );
}

/** Props of {@link AxisTriad}. */
export interface AxisTriadProps {
  /** The scene's frame, whose directions the triad shows unless `axes` is given. */
  readonly frame: LocalFrame;
  /** Where the camera looks from, in the scene's frame. */
  readonly angles: CameraAngles;
  /**
   * The box to draw in, from `triadBoxRem`: the view's own size where that is smaller than the
   * triad's usual 15 × 8 rem, so that the overlay never clips the triad.
   */
  readonly boxRem: TriadBoxRem;
  /**
   * The three named directions to show instead, as unit vectors in the scene's axes: the galactic
   * ones at the view centre, where the scene's frame is tilted to them (plan 14, D21).
   */
  readonly axes?: LocalFrame | undefined;
}

/**
 * The axis triad of a spatial view: the frame's coreward, spinward and north as the camera sees
 * them, drawn from a common origin and labelled (plan 05, T10.f).
 *
 * @remarks
 * An axis pointing away from the viewer ends in an open circle with a cross, one pointing at the
 * viewer in a circle with a dot, and one across the screen in an arrowhead, so that a view from
 * the south, which is mirrored, reads as such. An axis seen end on has its label beside its
 * symbol. On the galactic axis, where coreward and spinward are undefined, the directions are
 * labelled `-X` and `+Y`, as the frame falls back to them (design note D11). The labels are DOM
 * text, in B612; the picture is one image to assistive technology, named `Axis triad`. Given
 * `axes`, it shows those directions as the camera sees them, so that an orbit map drawn on a
 * system's own plane still points to galactic north, coreward and spinward.
 */
export function AxisTriad({ frame, angles, boxRem, axes: shown }: AxisTriadProps) {
  const axes = triadLayout(frame, angles, boxRem, shown ?? frame);
  const halfWidth = units(boxRem.width / 2);
  const halfHeight = units(boxRem.height / 2);
  return (
    <div
      className="axis-triad"
      // An `img` element cannot hold the drawn axes and their labels, set in B612 as DOM text, so
      // the whole is an image by role.
      // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
      role="img"
      aria-label="Axis triad"
      style={{ width: `${boxRem.width}rem`, height: `${boxRem.height}rem` }}
    >
      <svg
        className="axis-triad__axes"
        viewBox={`${-halfWidth} ${-halfHeight} ${2 * halfWidth} ${2 * halfHeight}`}
        aria-hidden="true"
        focusable="false"
      >
        {axes.map((axis) => (
          <AxisMark key={axis.name} axis={axis} />
        ))}
      </svg>
      {axes.map((axis) => (
        <span
          key={axis.name}
          className="axis-triad__label"
          style={{
            transform: `translate(${boxRem.width / 2 + axis.labelAt.x}rem, ${boxRem.height / 2 + axis.labelAt.y}rem) translate(-50%, -50%)`,
          }}
        >
          {axis.label}
        </span>
      ))}
    </div>
  );
}
