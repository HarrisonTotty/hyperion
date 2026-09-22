import type { ReactNode } from "react";

import type { Viewport } from "./camera";
import { CORE_HEAD_REM, CORE_SYMBOL_REM, type CoreArrowLayout } from "./furniture";
import type { SpatialQuantity } from "./Reading";

/** The radius of the towards symbol's dot, in `rem`. */
const DOT_REM = 0.125;

/**
 * What the fallback frame of design note D11 means for the operator, shown in place of the core
 * arrow on the galactic axis.
 */
export const AXIS_FALLBACK_MESSAGE = "DIRECTIONS UNDEFINED AT AXIS: GRID ALIGNED TO −X";

interface CoreMarkProps {
  readonly layout: CoreArrowLayout;
  readonly remPx: number;
}

/** The arrow, or the away or towards symbol, in the view's CSS pixels. */
function CoreMark({ layout, remPx }: CoreMarkProps) {
  const symbolR = CORE_SYMBOL_REM * remPx;
  const cross = symbolR * Math.SQRT1_2;
  let shape: ReactNode;
  switch (layout.kind) {
    case "arrow": {
      const { tip, tail } = layout;
      const heading = Math.atan2(tip.yPx - tail.yPx, tip.xPx - tail.xPx);
      const headPx = CORE_HEAD_REM * remPx;
      const barb = (sign: number): string => {
        const angle = heading + Math.PI + (sign * Math.PI) / 6;
        return `${tip.xPx + headPx * Math.cos(angle)},${tip.yPx + headPx * Math.sin(angle)}`;
      };
      shape = (
        <g data-core="arrow">
          <line x1={tail.xPx} y1={tail.yPx} x2={tip.xPx} y2={tip.yPx} />
          <polyline points={`${barb(1)} ${tip.xPx},${tip.yPx} ${barb(-1)}`} />
        </g>
      );
      break;
    }
    case "away": {
      const { xPx: x, yPx: y } = layout.centre;
      shape = (
        <g data-core="away">
          <circle cx={x} cy={y} r={symbolR} />
          <path
            d={`M${x - cross} ${y - cross}L${x + cross} ${y + cross}M${x - cross} ${y + cross}L${x + cross} ${y - cross}`}
          />
        </g>
      );
      break;
    }
    case "towards": {
      const { xPx: x, yPx: y } = layout.centre;
      shape = (
        <g data-core="towards">
          <circle cx={x} cy={y} r={symbolR} />
          <circle className="core-arrow__dot" cx={x} cy={y} r={DOT_REM * remPx} />
        </g>
      );
      break;
    }
    case "undefined":
      shape = null;
      break;
  }
  return shape;
}

/** Props of {@link CoreArrow}. */
export interface CoreArrowProps {
  /** Where the arrow and its label go, from `coreArrowLayout`. */
  readonly layout: CoreArrowLayout;
  readonly viewport: Viewport;
  /**
   * The distance from the view centre to the galactic axis, as the centre's `RADIUS` reads it:
   * `26,000.0` `ly`.
   */
  readonly distance: SpatialQuantity;
}

/**
 * The arrow towards the galactic core at the rim of a spatial view, labelled with the distance to
 * the axis: `CORE 26,000.0 ly` (plan 05, T10.f).
 *
 * @remarks
 * The arrow points along the projected coreward direction, its head where that direction leaves
 * the view or meets other furniture. When coreward lies within 5° of the line of sight it becomes
 * the away symbol (an open circle with a cross) or the towards symbol (a circle with a dot) at the
 * top of the view, larger than any mark's symbol. The label is DOM text, the distance in
 * monospaced figures. On the galactic axis there is no coreward and nothing is drawn: the view
 * says why ({@link AXIS_FALLBACK_MESSAGE}).
 */
export function CoreArrow({ layout, viewport, distance }: CoreArrowProps) {
  if (layout.kind === "undefined") {
    return null;
  }
  const remPx = viewport.remPx;
  return (
    <div className="core-arrow">
      <svg
        className="core-arrow__mark"
        viewBox={`0 0 ${viewport.widthPx} ${viewport.heightPx}`}
        aria-hidden="true"
        focusable="false"
      >
        <CoreMark layout={layout} remPx={remPx} />
      </svg>
      <span
        className="core-arrow__label"
        style={{
          transform: `translate(${layout.label.leftPx / remPx}rem, ${layout.label.topPx / remPx}rem)`,
        }}
      >
        CORE <span className="core-arrow__value">{distance.value}</span>
        {distance.unit.length > 0 ? ` ${distance.unit}` : null}
      </span>
    </div>
  );
}
