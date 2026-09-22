import type { CameraAngles } from "./camera";
import type { LocalFrame } from "./frame";
import { TRIAD_BOX_REM, TRIAD_MARKER_REM, type TriadAxis, triadLayout } from "./furniture";

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
  const tipX = units(axis.tip.x);
  const tipY = units(axis.tip.y);
  const reach = length(axis.tip);
  const markerR = units(TRIAD_MARKER_REM);
  // The line stops at the symbol's circle, so that it never runs into the cross or the dot.
  const lineReach = axis.end === "across" ? reach : reach - TRIAD_MARKER_REM;
  const direction = reach > 0 ? { x: axis.tip.x / reach, y: axis.tip.y / reach } : { x: 0, y: 0 };
  const lineEnd = {
    x: direction.x * Math.max(0, lineReach),
    y: direction.y * Math.max(0, lineReach),
  };
  const cross = markerR * Math.SQRT1_2;
  const head = units(HEAD_REM);
  const barb = (sign: number): string => {
    const angle = Math.atan2(direction.y, direction.x) + Math.PI + (sign * Math.PI) / 6;
    return `${tipX + head * Math.cos(angle)},${tipY + head * Math.sin(angle)}`;
  };
  return (
    <g data-axis={axis.name} data-end={axis.end}>
      {lineReach > 0 ? <line x1="0" y1="0" x2={units(lineEnd.x)} y2={units(lineEnd.y)} /> : null}
      {axis.end === "across" ? (
        <polyline points={`${barb(1)} ${tipX},${tipY} ${barb(-1)}`} />
      ) : (
        <circle cx={tipX} cy={tipY} r={markerR} />
      )}
      {axis.end === "towards" ? (
        <circle className="axis-triad__dot" cx={tipX} cy={tipY} r={units(DOT_REM)} />
      ) : null}
      {axis.end === "away" ? (
        <path
          d={`M${tipX - cross} ${tipY - cross}L${tipX + cross} ${tipY + cross}M${tipX - cross} ${tipY + cross}L${tipX + cross} ${tipY - cross}`}
        />
      ) : null}
    </g>
  );
}

/** Props of {@link AxisTriad}. */
export interface AxisTriadProps {
  /** The frame whose directions the triad shows. */
  readonly frame: LocalFrame;
  /** Where the camera looks from. */
  readonly angles: CameraAngles;
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
 * labelled `−X` and `+Y`, as the frame falls back to them (design note D11). The labels are DOM
 * text, in B612; the picture is one image to assistive technology, named `Axis triad`.
 */
export function AxisTriad({ frame, angles }: AxisTriadProps) {
  const axes = triadLayout(frame, angles);
  const halfWidth = units(TRIAD_BOX_REM.width / 2);
  const halfHeight = units(TRIAD_BOX_REM.height / 2);
  return (
    <div
      className="axis-triad"
      // An `img` element cannot hold the drawn axes and their labels, set in B612 as DOM text, so
      // the whole is an image by role.
      // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
      role="img"
      aria-label="Axis triad"
      style={{ width: `${TRIAD_BOX_REM.width}rem`, height: `${TRIAD_BOX_REM.height}rem` }}
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
            transform: `translate(${TRIAD_BOX_REM.width / 2 + axis.labelAt.x}rem, ${TRIAD_BOX_REM.height / 2 + axis.labelAt.y}rem) translate(-50%, -50%)`,
          }}
        >
          {axis.label}
        </span>
      ))}
    </div>
  );
}
