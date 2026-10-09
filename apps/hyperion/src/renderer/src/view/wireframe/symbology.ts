import { norm, normalise, scale, type Vec3 } from "../../geometry/vec3";
import type { ColourToken } from "../../spatial/drawList";
import { RETICLE_SHIFTS, RING_SHIFTS } from "../../lib/strokes";
import { CONTACT_SIZE_CLASS } from "../../lib/system/bodySymbols";
import {
  bracketArmPx,
  destinationChevrons,
  destinationLabelRisePx,
  type OffsetPx,
  SIZE_CLASS_REM,
  symbolOutline,
  unitInradius,
} from "../../spatial/symbols";
import { type ProjectionCamera, project, type Viewport } from "../camera/projection";
import type { CameraTarget } from "../camera/state";
import type { BodyMarkSymbol } from "../scene/model";
import { isSymbolSized } from "./bodies";

/** A point on the view, px from the top left. */
export interface ScreenPx {
  readonly xPx: number;
  readonly yPx: number;
}

/** A straight stroke on the view, between two points, px. */
export type ScreenSegment = readonly [ScreenPx, ScreenPx];

/**
 * A mark of the symbology drawn over the view: its strokes in screen pixels, the colour token they
 * take, what it marks and where (for picking and the DOM labels; no text is drawn into the canvas).
 */
export interface ScreenMark {
  /** What the mark is. */
  readonly kind: "selection" | "destination" | "target" | "flight_path" | "body_symbol";
  /** The body or craft it marks, or `null` for the flight path marker. */
  readonly target: CameraTarget | null;
  /** The token its strokes are drawn in (plan 05's `readTokens` names). */
  readonly token: ColourToken;
  /**
   * Its strokes, each drawn at the view's mark outline width (`ViewStrokes.markStrokePx`), their
   * places already moved out by its shift.
   */
  readonly segments: ReadonlyArray<ScreenSegment>;
  /** The point it marks. */
  readonly anchor: ScreenPx;
}

/**
 * The margin between a mark and the brackets about it, and between the selection's brackets and
 * the destination's chevrons' apices outside them, rem: 0.25 each, as the spatial view's reticles
 * are placed (`spatial/drawList.ts`, half of its 0.5 rem margin).
 */
export const BRACKET_MARGIN_REM = 0.25;

/**
 * A mark's label's place beside it as R02.T15 built it, rem, before the reticles' growth: its
 * `--surface-0` plate's near edge 0.75 rem from the mark's anchor, beyond the selection's bracket
 * about a craft.
 */
export const LABEL_PLACE_REM = 0.75;

/**
 * How far a label's plate stands beyond the selection's bracket's line about its mark, rem, before
 * the outline's shift: 0.125, a craft's label beyond its bracket's centreline as R02.T15 built it
 * (0.75 rem less the bracket's 0.375 + 0.25). The plate's `0.25rem` padding then starts its text
 * 0.375 rem less 0.75 CSS px outside the bracket's outer edge, past the 0.25 rem every display's
 * labels keep (`LABEL_TEXT_CLEARANCE_REM`; decision-r07-quality-and-destination, addendum A).
 */
export const LABEL_CLEARANCE_REM = 0.125;

/**
 * The flight path marker's circle radius, wing length and fin height, rem. A choice of this plan,
 * after the head-up display's marker: a circle about the size of a class-4 symbol, with wings of
 * twice its radius.
 */
export const FLIGHT_PATH_MARKER_REM = { radius: 0.375, wing: 0.5, fin: 0.3125 } as const;

/** Four corner brackets of the square of half-width `halfPx` about `at`. */
function corners(at: ScreenPx, halfPx: number): ScreenSegment[] {
  const arm = bracketArmPx(halfPx);
  const segments: ScreenSegment[] = [];
  for (const sx of [-1, 1] as const) {
    for (const sy of [-1, 1] as const) {
      const corner = { xPx: at.xPx + sx * halfPx, yPx: at.yPx + sy * halfPx };
      segments.push([corner, { xPx: corner.xPx - sx * arm, yPx: corner.yPx }]);
      segments.push([corner, { xPx: corner.xPx, yPx: corner.yPx - sy * arm }]);
    }
  }
  return segments;
}

/**
 * Four short open ticks about `at`, above, below, left and right, from `innerPx` outwards for
 * `lengthPx`: a target's mark, which never meets at the centre and is no closed outline of the
 * ship-wide symbol set.
 */
function cardinalTicks(at: ScreenPx, innerPx: number, lengthPx: number): ScreenSegment[] {
  const outerPx = innerPx + lengthPx;
  return [
    [
      { xPx: at.xPx, yPx: at.yPx - innerPx },
      { xPx: at.xPx, yPx: at.yPx - outerPx },
    ],
    [
      { xPx: at.xPx, yPx: at.yPx + innerPx },
      { xPx: at.xPx, yPx: at.yPx + outerPx },
    ],
    [
      { xPx: at.xPx - innerPx, yPx: at.yPx },
      { xPx: at.xPx - outerPx, yPx: at.yPx },
    ],
    [
      { xPx: at.xPx + innerPx, yPx: at.yPx },
      { xPx: at.xPx + outerPx, yPx: at.yPx },
    ],
  ];
}

/**
 * How far a reticle's outer edge moves out for an outline shift δ, device px: the 4δ it moves and
 * the δ its half-width gains (`markStrokeDevicePx` ÷ 2 against a 1.5 CSS px outline's), so that
 * what stands beside it, such as a mark's label, can stand as far off it as it did.
 */
export function reticleGrowthPx(shiftPx: number): number {
  return (RETICLE_SHIFTS + 1) * shiftPx;
}

/**
 * The selection's bracket's half-size about a mark, device px: the mark's radius and a margin,
 * moved out by four times the outlines' shift.
 *
 * @param markRadiusPx - The radius of the mark it encloses, px.
 * @param shiftPx - The marks' outline shift δ, device px (`ViewStrokes.markShiftPx`).
 */
export function bracketHalfSizePx(markRadiusPx: number, remPx: number, shiftPx: number): number {
  return markRadiusPx + BRACKET_MARGIN_REM * remPx + RETICLE_SHIFTS * shiftPx;
}

/**
 * The destination's half-size about a mark, device px, where its chevrons' apices stand: a margin
 * outside the selection's bracket's place, and at least `minGapPx` outside it, whether or not the
 * destination is also the selection, so that the two marks show together (R07.T16.g;
 * decision-r07-quality-and-destination, Q2).
 *
 * @remarks
 * The least gap is a reticle's outline and one casing (`minReticleGapDevicePx`, R07.T16.d and
 * T16.g): the destination is drawn after the selection, so that its casing would otherwise cover
 * the bracket's full-coverage core below a ratio of 4/3, where 0.25 rem is under 4 device px. The
 * chevrons come nearest the bracket near its arms' inner ends, at (H ÷ 3 + the gap) ÷ √2 with H the
 * bracket's half-size, which is the gap or more wherever H is at least 1.25 times it.
 *
 * @param shiftPx - The marks' outline shift δ, device px (`ViewStrokes.markShiftPx`).
 * @param minGapPx - The least space between the bracket's centreline and the chevrons' apices,
 *   device px.
 */
export function destinationHalfSizePx(
  markRadiusPx: number,
  remPx: number,
  shiftPx: number,
  minGapPx: number,
): number {
  return (
    bracketHalfSizePx(markRadiusPx, remPx, shiftPx) + Math.max(BRACKET_MARGIN_REM * remPx, minGapPx)
  );
}

/**
 * The device px from a mark's anchor to its label's `--surface-0` plate, to its right (R07.T16.g;
 * decision-r07-t16d-followups, items 1 and (d)): the larger of {@link LABEL_PLACE_REM} and the
 * reticles' growth, 5δ (T16.d's place), and {@link LABEL_CLEARANCE_REM} and δ beyond the line of
 * the selection's bracket about the mark.
 *
 * @remarks
 * The bracket is counted whether or not the mark is selected, so that selecting a mark never moves
 * its label, and the plate's near edge stands 0.125 rem less 0.75 CSS px beyond its outer edge, the
 * clearance of a craft's label from its bracket as built, at every size class and ratio; its
 * `0.25rem` padding then starts its text 0.375 rem less 0.75 CSS px outside that edge, past the
 * 0.25 rem every display's labels keep. Every mark up to size class 2 keeps T16.d's place; a
 * class-3 or class-4 symbol's label stands 0.0625 or 0.125 rem further out. The destination's
 * chevrons never move a label outward: its label keeps this place and stands above or below the
 * whole chevron set ({@link markLabelRisePx}; decision-r07-quality-and-destination, addendum B). A
 * craft's label at a ratio of 1 and 100% stands 13.25 px from its anchor, the destination or not.
 *
 * @param markRadiusPx - The radius of the mark, px: its symbol's, or a craft's contact's.
 * @param shiftPx - The marks' outline shift δ, device px (`ViewStrokes.markShiftPx`).
 */
export function markLabelOffsetPx(markRadiusPx: number, remPx: number, shiftPx: number): number {
  return Math.max(
    LABEL_PLACE_REM * remPx + reticleGrowthPx(shiftPx),
    bracketHalfSizePx(markRadiusPx, remPx, shiftPx) + LABEL_CLEARANCE_REM * remPx + shiftPx,
  );
}

/**
 * How far above its anchor a destination's label's plate stands, device px, its bottom edge above
 * the whole chevron set, or how far below it its top edge stands at a lower place
 * (decision-r07-quality-and-destination, addendum B): the upper chevron's apex distance
 * ({@link destinationHalfSizePx}), its arm ends' run outward, half the mark stroke and 0.25 rem
 * (`destinationLabelRisePx`), the arms the bracket's about the mark.
 *
 * @remarks
 * The label keeps its place beside the bracket ({@link markLabelOffsetPx}) and stands above or below
 * all four chevrons, so that none stands on its line or beside its text. The view's
 * `markLabelPlaces` chooses its place among the four. At a ratio of 1 and 100% the plate's near
 * horizontal edge stands 22.24, 25.19 and 28.13 px from the anchor of a class-0 mark, a craft and a
 * class-4 mark: for a craft, 15 + 5.19 + 1 + 4.
 *
 * @param markRadiusPx - The radius of the mark, px: its symbol's, or a craft's contact's.
 * @param shiftPx - The marks' outline shift δ, device px (`ViewStrokes.markShiftPx`).
 * @param minGapPx - The least space between the bracket's centreline and the chevrons' apices,
 *   device px (`ViewStrokes.minReticleGapPx`).
 * @param markStrokePx - The marks' outline width, device px (`ViewStrokes.markStrokePx`).
 */
export function markLabelRisePx(
  markRadiusPx: number,
  remPx: number,
  shiftPx: number,
  minGapPx: number,
  markStrokePx: number,
): number {
  return destinationLabelRisePx(
    bracketHalfSizePx(markRadiusPx, remPx, shiftPx),
    destinationHalfSizePx(markRadiusPx, remPx, shiftPx, minGapPx),
    markStrokePx,
    remPx,
  );
}

/**
 * The bracket reticle about the selection, in `--accent`, as the spatial view's: its half-size the
 * marked symbol's radius and a margin, moved out by four times the outlines' shift
 * ({@link bracketHalfSizePx}).
 *
 * @param markRadiusPx - The radius of the mark it encloses, px.
 * @param shiftPx - The marks' outline shift δ, device px (`ViewStrokes.markShiftPx`).
 */
export function bracketReticle(
  target: CameraTarget,
  at: ScreenPx,
  markRadiusPx: number,
  remPx: number,
  shiftPx: number,
): ScreenMark {
  return {
    kind: "selection",
    target,
    token: "accent",
    segments: corners(at, bracketHalfSizePx(markRadiusPx, remPx, shiftPx)),
    anchor: at,
  };
}

/**
 * The mark of a commanded destination, in `--target`: four open chevrons, one on each screen axis of
 * the mark, each pointing at it (`destinationChevrons`; decision-r07-quality-and-destination, Q2;
 * R07.T16.h). Their apices stand at the destination's half-size ({@link destinationHalfSizePx}),
 * selected or not, and each arm is as long as the bracket's arm about the same mark, so that where
 * the destination is also the selection the bracket keeps the corners and the apices stand over the
 * open middle of its sides. Both move out with the bracket, by four times the outlines' shift.
 *
 * @param shiftPx - The marks' outline shift δ, device px (`ViewStrokes.markShiftPx`).
 * @param minGapPx - The least space between the bracket's centreline and the apices, device px.
 */
export function destinationReticle(
  target: CameraTarget,
  at: ScreenPx,
  markRadiusPx: number,
  remPx: number,
  shiftPx: number,
  minGapPx: number,
): ScreenMark {
  const point = (offset: OffsetPx): ScreenPx => ({
    xPx: at.xPx + offset.xPx,
    yPx: at.yPx + offset.yPx,
  });
  const chevrons = destinationChevrons(
    destinationHalfSizePx(markRadiusPx, remPx, shiftPx, minGapPx),
    bracketArmPx(bracketHalfSizePx(markRadiusPx, remPx, shiftPx)),
  );
  const segments = chevrons.flatMap(([end, apex, other]): ScreenSegment[] => [
    [point(end), point(apex)],
    [point(apex), point(other)],
  ]);
  return { kind: "destination", target, token: "target", segments, anchor: at };
}

/** A target's mark, with its range and closure rate for the DOM label beside it. */
export interface TargetMark extends ScreenMark {
  /** The target's range, m, from the own ship or, with none, from the camera. */
  readonly rangeM: number;
  /** Its closure rate, m/s, positive closing; `null` where there is no own ship to close on. */
  readonly closureMPerS: number | null;
}

/**
 * A target's closure rate, m/s, positive closing: the rate its range shrinks, −(r · v) ÷ |r|; `null`
 * where the relative velocity is not known or the range is zero.
 *
 * @param relativeM - The target from the own ship, m.
 * @param relativeVelocityMPerS - The target's velocity relative to the own ship, m/s, or `null`.
 */
export function closureRateMPerS(
  relativeM: Vec3,
  relativeVelocityMPerS: Vec3 | null,
): number | null {
  const rangeM = norm(relativeM);
  return relativeVelocityMPerS === null || !(rangeM > 0)
    ? null
    : -(
        relativeM.x * relativeVelocityMPerS.x +
        relativeM.y * relativeVelocityMPerS.y +
        relativeM.z * relativeVelocityMPerS.z
      ) / rangeM;
}

/**
 * A target's mark in `--text`: four open cardinal ticks outside the mark's radius, each as long as
 * a bracket's arm, so that corner brackets mean the selection alone (state is never shown by colour
 * alone; decided 2026-09-30 under the owner's delegation); with its range and closure rate (plan
 * R02, R02.T12.c). It is the craft's contact mark, so its ticks move out by the outlines' shift, as
 * a symbol's line does, and the open centre stays as built.
 *
 * @param relativeM - The target from the range's origin (the own ship, or the camera), m.
 * @param relativeVelocityMPerS - The target's velocity relative to the own ship, m/s, or `null`.
 * @param shiftPx - The marks' outline shift δ, device px (`ViewStrokes.markShiftPx`).
 */
export function targetMark(
  target: CameraTarget,
  at: ScreenPx,
  markRadiusPx: number,
  relativeM: Vec3,
  relativeVelocityMPerS: Vec3 | null,
  shiftPx: number,
): TargetMark {
  const rangeM = norm(relativeM);
  const closureMPerS = closureRateMPerS(relativeM, relativeVelocityMPerS);
  return {
    kind: "target",
    target,
    token: "text",
    segments: cardinalTicks(at, markRadiusPx + shiftPx, bracketArmPx(markRadiusPx)),
    anchor: at,
    rangeM,
    closureMPerS,
  };
}

/** A regular polygon of `sides` about `at` of circumradius `radiusPx`, as segments. */
function polygon(at: ScreenPx, radiusPx: number, sides: number): ScreenSegment[] {
  const points = Array.from({ length: sides + 1 }, (_, i) => {
    const angle = (2 * Math.PI * i) / sides;
    return { xPx: at.xPx + radiusPx * Math.cos(angle), yPx: at.yPx + radiusPx * Math.sin(angle) };
  });
  return points.slice(1).map((p, i): ScreenSegment => [points[i] ?? p, p]);
}

/**
 * The own ship's flight path marker: a circle with wings and a fin where its velocity against the
 * frame's reference points on the view, in `--text` (plan R02, R02.T12.c). Its circle moves out by
 * the outlines' shift, as a symbol's line does, and its wings and fin start from it.
 *
 * @param velocityMPerS - The own ship's velocity, m/s along the camera frame's axes; only its
 * direction is used, so a slow ship's marker shows as surely as a fast one's.
 * @param shiftPx - The marks' outline shift δ, device px (`ViewStrokes.markShiftPx`).
 * @returns `null` with no own ship or no velocity, or where the velocity points behind the camera.
 */
export function flightPathMarker(
  velocityMPerS: Vec3 | null,
  camera: ProjectionCamera,
  viewport: Viewport,
  remPx: number,
  shiftPx: number,
): ScreenMark | null {
  if (velocityMPerS === null || !(norm(velocityMPerS) > 0)) {
    return null;
  }
  // A point a kilometre along the direction, well beyond the near plane.
  const projected = project(scale(normalise(velocityMPerS), 1e3), camera, viewport);
  if (!projected.inFront) {
    return null;
  }
  const at = { xPx: projected.xPx, yPx: projected.yPx };
  const radius = FLIGHT_PATH_MARKER_REM.radius * remPx + shiftPx;
  const wing = FLIGHT_PATH_MARKER_REM.wing * remPx;
  const fin = FLIGHT_PATH_MARKER_REM.fin * remPx;
  const wings: ScreenSegment[] = [
    [
      { xPx: at.xPx - radius, yPx: at.yPx },
      { xPx: at.xPx - radius - wing, yPx: at.yPx },
    ],
    [
      { xPx: at.xPx + radius, yPx: at.yPx },
      { xPx: at.xPx + radius + wing, yPx: at.yPx },
    ],
    [
      { xPx: at.xPx, yPx: at.yPx - radius },
      { xPx: at.xPx, yPx: at.yPx - radius - fin },
    ],
  ];
  return {
    kind: "flight_path",
    target: null,
    token: "text",
    segments: [...polygon(at, radius, 24), ...wings],
    anchor: at,
  };
}

/** A body symbol's radius on the view, px: half its size class's diameter. */
export function symbolRadiusPx(symbol: BodyMarkSymbol, remPx: number): number {
  return (SIZE_CLASS_REM[symbol.sizeClass] * remPx) / 2;
}

/**
 * A body's mark from the ship-wide set where it is under 3 device px across (Design note 13), in
 * `--text`, or `null` where it is drawn as a sphere.
 *
 * @remarks
 * Its outline moves out by the outlines' shift δ, so that its inner edge stays where a 1.5 CSS px
 * outline's would be (decision-thin-line-contrast, item 2): a circle's radius by δ, a polygon's
 * sides each by δ, and a ringed circle's disc by δ and its ring by 3δ, so that the disc's hole and
 * the gap round it both stay.
 *
 * @param remPx - The interface's rem, px, which the symbol sizes follow.
 * @param shiftPx - The marks' outline shift δ, device px (`ViewStrokes.markShiftPx`).
 */
export function bodySymbolMark(
  target: CameraTarget,
  symbol: BodyMarkSymbol,
  at: ScreenPx,
  diameterPx: number,
  remPx: number,
  shiftPx: number,
): ScreenMark | null {
  if (!isSymbolSized(diameterPx)) {
    return null;
  }
  const radiusPx = symbolRadiusPx(symbol, remPx);
  const outline = symbolOutline(symbol.shape);
  let segments: ScreenSegment[];
  switch (outline.kind) {
    case "circle":
      segments = polygon(at, radiusPx + shiftPx, 24);
      break;
    case "ringed-circle":
      // The ring at the unit radius and the disc inside it, both outlined.
      segments = [
        ...polygon(at, radiusPx + RING_SHIFTS * shiftPx, 24),
        ...polygon(at, radiusPx * outline.discRadius + shiftPx, 16),
      ];
      break;
    case "polygon": {
      // Each side moved out by the shift: its corners by the shift over the unit inradius.
      const cornerPx = radiusPx + shiftPx / unitInradius(outline.points);
      const points = outline.points.map((p) => ({
        xPx: at.xPx + p.x * cornerPx,
        yPx: at.yPx + p.y * cornerPx,
      }));
      segments = points.slice(1).map((p, i): ScreenSegment => [points[i] ?? p, p]);
      break;
    }
  }
  return { kind: "body_symbol", target, token: "text", segments, anchor: at };
}

/** A pickable mark as the symbology sees it: what it is, where, and its symbol if a body. */
export interface SymbologyAnchor {
  /** The body or craft. */
  readonly target: CameraTarget;
  /** Where it falls on the view. */
  readonly at: ScreenPx;
  /** A body's symbol and apparent diameter, px; `null` for a craft. */
  readonly body: { readonly symbol: BodyMarkSymbol; readonly diameterPx: number } | null;
  /**
   * A craft's place and motion for its target brackets: from the own ship where there is one, else
   * from the camera (the range is then `FROM CAMERA`, Design note 17); `null` for a body.
   */
  readonly craft: {
    readonly relativeM: Vec3;
    readonly relativeVelocityMPerS: Vec3 | null;
  } | null;
}

/** The radius of a craft's brackets, as an unresolved contact's symbol: size class 2. */
export function contactRadiusPx(remPx: number): number {
  return (SIZE_CLASS_REM[CONTACT_SIZE_CLASS] * remPx) / 2;
}

/**
 * The radius of the mark its reticles and its label stand about, px: a body's symbol's, or a
 * craft's contact's.
 */
export function anchorRadiusPx(anchor: Pick<SymbologyAnchor, "body">, remPx: number): number {
  return anchor.body === null ? contactRadiusPx(remPx) : symbolRadiusPx(anchor.body.symbol, remPx);
}

/** What the symbology marks beyond the anchors themselves. */
export interface SymbologyInput {
  /** The marks in sight, each once. */
  readonly anchors: ReadonlyArray<SymbologyAnchor>;
  /** The selected target, or `null`. */
  readonly selection: CameraTarget | null;
  /** The destination as the server last reported it, or `null` (`DrawOptions.destination`). */
  readonly destination: CameraTarget | null;
  /** The own ship's velocity along the camera frame's axes, m/s, or `null`. */
  readonly ownVelocityMPerS: Vec3 | null;
  /** The interface's rem, px. */
  readonly remPx: number;
  /** The marks' outline shift δ, device px (`ViewStrokes.markShiftPx`). */
  readonly markShiftPx: number;
  /**
   * The least space between the selection's reticle and the destination's about one mark, device
   * px: a reticle's outline and one casing (`ViewStrokes.minReticleGapPx`,
   * {@link destinationHalfSizePx}).
   */
  readonly minReticleGapPx: number;
}

function sameTarget(a: CameraTarget | null, b: CameraTarget): boolean {
  if (a === null) {
    return false;
  }
  return a.kind === "body"
    ? b.kind === "body" && a.body === b.body
    : b.kind === "craft" && a.craft === b.craft;
}

/**
 * The view's symbology (plan R02, R02.T12.c): each body under 3 px its symbol, each craft its target
 * mark (four ticks) with range and closure (for the DOM label beside it), the selection's
 * bracket reticle in `--accent`, the destination's chevrons in `--target` outside it
 * ({@link destinationReticle}), and the own ship's flight path marker, in that order.
 */
export function symbologyMarks(
  input: SymbologyInput,
  camera: ProjectionCamera,
  viewport: Viewport,
): ScreenMark[] {
  const marks: ScreenMark[] = [];
  for (const anchor of input.anchors) {
    const markRadiusPx = anchorRadiusPx(anchor, input.remPx);
    if (anchor.craft !== null) {
      marks.push(
        targetMark(
          anchor.target,
          anchor.at,
          markRadiusPx,
          anchor.craft.relativeM,
          anchor.craft.relativeVelocityMPerS,
          input.markShiftPx,
        ),
      );
    }
    if (anchor.body !== null) {
      const symbol = bodySymbolMark(
        anchor.target,
        anchor.body.symbol,
        anchor.at,
        anchor.body.diameterPx,
        input.remPx,
        input.markShiftPx,
      );
      if (symbol !== null) {
        marks.push(symbol);
      }
    }
    if (sameTarget(input.selection, anchor.target)) {
      marks.push(
        bracketReticle(anchor.target, anchor.at, markRadiusPx, input.remPx, input.markShiftPx),
      );
    }
    if (sameTarget(input.destination, anchor.target)) {
      marks.push(
        destinationReticle(
          anchor.target,
          anchor.at,
          markRadiusPx,
          input.remPx,
          input.markShiftPx,
          input.minReticleGapPx,
        ),
      );
    }
  }
  const marker = flightPathMarker(
    input.ownVelocityMPerS,
    camera,
    viewport,
    input.remPx,
    input.markShiftPx,
  );
  if (marker !== null) {
    marks.push(marker);
  }
  return marks;
}
