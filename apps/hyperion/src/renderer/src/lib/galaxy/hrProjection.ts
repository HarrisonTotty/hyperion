/**
 * The pure geometry of the Hertzsprung-Russell diagram of a chart's systems (plan 06, P06.T37): where
 * each primary falls by effective temperature and luminosity, what it is drawn with, and which point
 * a pointer picks.
 *
 * @remarks
 * Nothing here touches the DOM or a canvas. The x axis is log T_eff, reversed as astronomers draw it
 * so that the hottest stars are on the left, from 200,000 K, which holds the hottest white dwarfs, to
 * 1,000 K, which holds the L dwarfs and the warmest T dwarfs; the y axis is log L ÷ L☉ from −6, a
 * cool white dwarf or a brown dwarf, to 6.5, the brightest supergiants. A point beyond an axis is
 * pegged at its edge and carries the off-scale mark, an arrowhead pointing off the scale, as the
 * guide has a gauge pegged rather than clipped. Lengths are CSS pixels from the top left of the
 * diagram's canvas.
 */
import type { ObjectKindDto, SystemIdHex } from "@hyperion/protocol";

import type {
  Anchor,
  DrawList,
  DrawOp,
  ScreenPoint,
  SymbolOp,
  TicksOp,
} from "../../spatial/drawList";
import type { MarkStatus, SizeClass, SymbolShape } from "../../spatial/marks";
import { pick } from "../../spatial/pick";
import { SIZE_CLASS_REM, SYMBOL_STROKE_PX } from "../../spatial/symbols";
import type { ChartSystem } from "./model";
import { starSizeClass, starSymbol } from "./starSymbols";
import { layerIndex } from "./wire";

/** The hottest effective temperature on the diagram, at its left edge. */
export const HR_TEFF_HOT_K = 200_000;

/** The coolest effective temperature on the diagram, at its right edge. */
export const HR_TEFF_COOL_K = 1_000;

/** The faintest luminosity on the diagram, as log₁₀ L ÷ L☉, at its bottom edge. */
export const HR_LOG_L_MIN = -6;

/** The brightest luminosity on the diagram, as log₁₀ L ÷ L☉, at its top edge. */
export const HR_LOG_L_MAX = 6.5;

/** The temperatures of the x axis's major ticks, each labelled: steps of 1 and 3 in the decade. */
export const HR_TEFF_TICKS_K: ReadonlyArray<number> = [100_000, 30_000, 10_000, 3_000, 1_000];

/** The luminosities of the y axis's major ticks, as log₁₀ L ÷ L☉, each labelled. */
export const HR_LOG_L_TICKS: ReadonlyArray<number> = [-6, -4, -2, 0, 2, 4, 6];

/** A spectral class's band of effective temperature on the dwarf scale. */
export interface SpectralBand {
  readonly letter: string;
  /** The band's hot edge, or the diagram's where the class runs beyond it. */
  readonly hotK: number;
  /** The band's cool edge, or the diagram's where the class runs beyond it. */
  readonly coolK: number;
}

/**
 * The spectral classes along the top axis, hottest first, with the temperature of each class's
 * subtype 0 as its hot edge: B0V 31,400 K, A0V 9,700 K, F0V 7,220 K, G0V 5,930 K, K0V 5,270 K, M0V
 * 3,850 K, L0V 2,270 K, T0V 1,255 K.
 *
 * @remarks
 * From the mean dwarf sequence of Pecaut and Mamajek (2013, ApJS 208, 9) in Mamajek's version
 * 2022.04.16, the table the server classifies with (`hyperion-sim`, `stellar::classify::pm13`), so
 * that a star's letter in the list and its place under the letters agree for a dwarf. Giants and
 * supergiants of a class run a few hundred kelvins cooler, which the letters, a guide to the eye, do
 * not follow.
 */
export const HR_SPECTRAL_BANDS: ReadonlyArray<SpectralBand> = [
  { letter: "O", hotK: HR_TEFF_HOT_K, coolK: 31_400 },
  { letter: "B", hotK: 31_400, coolK: 9_700 },
  { letter: "A", hotK: 9_700, coolK: 7_220 },
  { letter: "F", hotK: 7_220, coolK: 5_930 },
  { letter: "G", hotK: 5_930, coolK: 5_270 },
  { letter: "K", hotK: 5_270, coolK: 3_850 },
  { letter: "M", hotK: 3_850, coolK: 2_270 },
  { letter: "L", hotK: 2_270, coolK: 1_255 },
  { letter: "T", hotK: 1_255, coolK: HR_TEFF_COOL_K },
];

/** The rectangle the diagram's axes enclose, in CSS pixels from the canvas's top left. */
export interface PlotAreaPx {
  readonly leftPx: number;
  readonly topPx: number;
  readonly widthPx: number;
  readonly heightPx: number;
}

/** Which way a point runs off the temperature axis: hotter than its left edge or cooler. */
export type TeffPeg = "hot" | "cool";

/** Which way a point runs off the luminosity axis: brighter than its top edge or fainter. */
export type LuminosityPeg = "bright" | "faint";

/** One primary on the diagram. */
export interface HrPoint {
  readonly id: SystemIdHex;
  /** Its place, pegged at the edge it runs off. */
  readonly xPx: number;
  readonly yPx: number;
  readonly shape: SymbolShape;
  /** The chart's size class for it, so that one star is the same size on both pictures. */
  readonly sizeClass: SizeClass;
  /** `available` within the drive range, drawn in `--accent` as on the chart. */
  readonly status: MarkStatus;
  /** The temperature edge it is pegged at, or `null` when on scale. */
  readonly teffPeg: TeffPeg | null;
  /** The luminosity edge it is pegged at, or `null` when on scale. */
  readonly luminosityPeg: LuminosityPeg | null;
}

/** What the diagram's caption counts, so that nothing left off the plot goes unsaid. */
export interface HrCounts {
  /** The primaries plotted, those pegged at an edge included. */
  readonly plotted: number;
  /** Of those plotted, the ones beyond an axis and pegged at its edge. */
  readonly offScale: number;
  /**
   * Neutron stars and black holes, which have no photosphere to place on the diagram: counted,
   * not plotted.
   */
  readonly noPhotosphere: number;
  /** Stars that left no remnant: listed, not drawn. */
  readonly noRemnant: number;
  /** Systems not yet formed at the chart's time, which have no primary. */
  readonly notYetFormed: number;
  /**
   * Primaries of a kind that has a photosphere whose temperature or luminosity is missing or not
   * a finite, positive value: an answer the server should not give, counted rather than hidden.
   */
  readonly noData: number;
}

/** A chart's systems placed on the diagram. */
export interface HrProjection {
  /** The points in the order of the systems given, nearest the chart centre first. */
  readonly points: ReadonlyArray<HrPoint>;
  readonly counts: HrCounts;
}

const LOG_TEFF_HOT = Math.log10(HR_TEFF_HOT_K);
const LOG_TEFF_COOL = Math.log10(HR_TEFF_COOL_K);

/**
 * The x of a temperature, unclamped: `leftPx` at 200,000 K and the right edge at 1,000 K, linear
 * in log T_eff.
 *
 * @param teffK - Finite and above zero.
 */
export function hrXPx(teffK: number, area: PlotAreaPx): number {
  const share = (LOG_TEFF_HOT - Math.log10(teffK)) / (LOG_TEFF_HOT - LOG_TEFF_COOL);
  return area.leftPx + share * area.widthPx;
}

/** The y of a luminosity, unclamped: the bottom edge at log L ÷ L☉ = −6 and `topPx` at 6.5. */
export function hrYPx(logLuminosityLsun: number, area: PlotAreaPx): number {
  const share = (HR_LOG_L_MAX - logLuminosityLsun) / (HR_LOG_L_MAX - HR_LOG_L_MIN);
  return area.topPx + share * area.heightPx;
}

/**
 * What a primary of this kind shows on the diagram: plotted, or which count it goes in instead.
 *
 * @remarks
 * Exhaustive over the wire's kinds, with no default, so that a new kind is a type error here until
 * it is placed. A white dwarf has a photosphere and is plotted; a neutron star's surface emits,
 * but at 1E-5 L☉ and 1E6 K it would sit off both scales and says nothing of its evolution, so it
 * is counted with the black holes.
 */
function placing(kind: ObjectKindDto): "plot" | "noPhotosphere" | "noRemnant" {
  let place: "plot" | "noPhotosphere" | "noRemnant";
  switch (kind) {
    case "protostar":
    case "pre_main_sequence":
    case "dwarf":
    case "subgiant":
    case "giant":
    case "supergiant":
    case "wolf_rayet":
    case "hot_subdwarf":
    case "white_dwarf":
    case "substellar":
      place = "plot";
      break;
    case "neutron_star":
    case "black_hole":
      place = "noPhotosphere";
      break;
    case "no_remnant":
      place = "noRemnant";
      break;
  }
  return place;
}

/**
 * Places a chart's systems on the diagram and counts those it cannot plot.
 *
 * @remarks
 * Each point takes its primary's symbol and the chart's size class for it (`starSymbol`,
 * `starSizeClass`), and is `available` when its system lies within the drive range, as on the
 * chart. A point beyond an axis is pegged at that edge and counted as off scale.
 *
 * @param driveRangeLy - The range a system counts as available within (plan 05, design note D9).
 */
export function projectHr(
  systems: ReadonlyArray<ChartSystem>,
  area: PlotAreaPx,
  driveRangeLy: number,
): HrProjection {
  const points: HrPoint[] = [];
  let offScale = 0;
  let noPhotosphere = 0;
  let noRemnant = 0;
  let notYetFormed = 0;
  let noData = 0;
  for (const system of systems) {
    const star = system.star;
    if (star === null) {
      notYetFormed += 1;
      continue;
    }
    const place = placing(star.kind);
    const shape = starSymbol(star.kind);
    if (place === "noPhotosphere") {
      noPhotosphere += 1;
      continue;
    }
    if (place === "noRemnant" || shape === null) {
      noRemnant += 1;
      continue;
    }
    const { teffK, logLuminosityLsun: logL } = star;
    if (
      teffK === null ||
      logL === null ||
      !(teffK > 0) ||
      !Number.isFinite(teffK) ||
      !Number.isFinite(logL)
    ) {
      noData += 1;
      continue;
    }
    let teffPeg: TeffPeg | null = null;
    if (teffK > HR_TEFF_HOT_K) {
      teffPeg = "hot";
    } else if (teffK < HR_TEFF_COOL_K) {
      teffPeg = "cool";
    }
    let luminosityPeg: LuminosityPeg | null = null;
    if (logL > HR_LOG_L_MAX) {
      luminosityPeg = "bright";
    } else if (logL < HR_LOG_L_MIN) {
      luminosityPeg = "faint";
    }
    if (teffPeg !== null || luminosityPeg !== null) {
      offScale += 1;
    }
    const shownTeffK = Math.min(HR_TEFF_HOT_K, Math.max(HR_TEFF_COOL_K, teffK));
    const shownLogL = Math.min(HR_LOG_L_MAX, Math.max(HR_LOG_L_MIN, logL));
    points.push({
      id: system.id,
      xPx: hrXPx(shownTeffK, area),
      yPx: hrYPx(shownLogL, area),
      shape,
      sizeClass: starSizeClass(shape, layerIndex(system.layer)),
      status: system.distanceLy <= driveRangeLy ? "available" : "plain",
      teffPeg,
      luminosityPeg,
    });
  }
  return {
    points,
    counts: {
      plotted: points.length,
      offScale,
      noPhotosphere,
      noRemnant,
      notYetFormed,
      noData,
    },
  };
}

/** Where a spectral letter stands along the top axis: the middle of its band in log T_eff. */
export interface SpectralLetterPlace {
  readonly letter: string;
  readonly xPx: number;
}

/** The spectral letters along the top axis, each at the middle of its band in log T_eff. */
export function hrSpectralLetters(area: PlotAreaPx): ReadonlyArray<SpectralLetterPlace> {
  return HR_SPECTRAL_BANDS.map((band) => ({
    letter: band.letter,
    xPx: hrXPx(Math.sqrt(band.hotK * band.coolK), area),
  }));
}

const GRID_WIDTH_PX = 1;
const FRAME_WIDTH_PX = 1;
const RETICLE_WIDTH_PX = 1.5;
const PEG_WIDTH_PX = 1.5;
/** How much larger than its symbol the reticle is, in `rem`, as on the chart. */
const RETICLE_MARGIN_REM = 0.5;
/** Length of a class boundary's tick down from the top axis, in `rem`. */
const CLASS_TICK_REM = 0.25;
/** How far past its symbol's rim the off-scale arrowhead's tip stands, and its arms' length. */
const PEG_GAP_REM = 0.375;
const PEG_ARM_REM = 0.25;

function outerRadiusPx(point: HrPoint, remPx: number): number {
  return (SIZE_CLASS_REM[point.sizeClass] * remPx) / 2;
}

/** A unit vector on the screen pointing off the scale the way a point is pegged. */
function pegDirections(point: HrPoint): ReadonlyArray<ScreenPoint> {
  const directions: ScreenPoint[] = [];
  if (point.teffPeg !== null) {
    directions.push({ xPx: point.teffPeg === "hot" ? -1 : 1, yPx: 0 });
  }
  if (point.luminosityPeg !== null) {
    directions.push({ xPx: 0, yPx: point.luminosityPeg === "bright" ? -1 : 1 });
  }
  return directions;
}

/** The off-scale mark: an arrowhead beyond the symbol's rim, pointing off each scale it runs off. */
function pegOp(point: HrPoint, remPx: number): TicksOp | null {
  const directions = pegDirections(point);
  if (directions.length === 0) {
    return null;
  }
  const reachPx = outerRadiusPx(point, remPx) + PEG_GAP_REM * remPx;
  const armPx = PEG_ARM_REM * remPx;
  const segments: Array<TicksOp["segments"][number]> = [];
  for (const { xPx: dx, yPx: dy } of directions) {
    const tip = { xPx: point.xPx + dx * reachPx, yPx: point.yPx + dy * reachPx };
    // The two arms run back from the tip at 45° either side of the direction.
    for (const side of [-1, 1]) {
      segments.push({
        from: tip,
        to: {
          xPx: tip.xPx - dx * armPx + side * dy * armPx,
          yPx: tip.yPx - dy * armPx + side * dx * armPx,
        },
      });
    }
  }
  return {
    kind: "ticks",
    segments,
    stroke: point.status === "available" ? "accent" : "text",
    widthPx: PEG_WIDTH_PX,
  };
}

// Plain points first and those within range over them, as the chart's accent marks read; at a tie
// the lower ID is drawn last, on top, because it is the one `pick` chooses.
function drawOrder(a: HrPoint, b: HrPoint): number {
  if (a.status !== b.status) {
    return a.status === "plain" ? -1 : 1;
  }
  if (a.id === b.id) {
    return 0;
  }
  return a.id < b.id ? 1 : -1;
}

/**
 * The draw list of the diagram: its `--line` grid at the major ticks, the class boundaries' ticks
 * under the top axis, the axes' frame in `--text-muted`, each point's symbol filled in `--text`,
 * or `--accent` within the drive range, with the off-scale mark where pegged, and the chart's
 * bracket reticle on the selected point.
 *
 * @remarks
 * The painter of the spatial views paints it, so the diagram's symbols are the chart's. There is
 * no text: the tick values, the class letters and the axis labels are DOM text beside the canvas.
 * Fill means only the side of a reference plane on a spatial display, and this is a graph with no
 * plane, so every symbol is filled; a ringed circle's disc alone, as always.
 *
 * @param remPx - CSS pixels in one `rem`, which the symbols' sizes follow.
 */
export function hrDrawList(
  projection: HrProjection,
  area: PlotAreaPx,
  selectedId: SystemIdHex | null,
  remPx: number,
): DrawList {
  const { leftPx, topPx, widthPx, heightPx } = area;
  const rightPx = leftPx + widthPx;
  const bottomPx = topPx + heightPx;
  const ops: DrawOp[] = [];
  for (const teffK of HR_TEFF_TICKS_K) {
    const xPx = hrXPx(teffK, area);
    ops.push({
      kind: "line",
      from: { xPx, yPx: topPx },
      to: { xPx, yPx: bottomPx },
      stroke: "line",
      widthPx: GRID_WIDTH_PX,
      markId: null,
    });
  }
  for (const logL of HR_LOG_L_TICKS) {
    const yPx = hrYPx(logL, area);
    ops.push({
      kind: "line",
      from: { xPx: leftPx, yPx },
      to: { xPx: rightPx, yPx },
      stroke: "line",
      widthPx: GRID_WIDTH_PX,
      markId: null,
    });
  }
  ops.push({
    kind: "ticks",
    segments: HR_SPECTRAL_BANDS.slice(1).map((band) => {
      const xPx = hrXPx(band.hotK, area);
      return { from: { xPx, yPx: topPx }, to: { xPx, yPx: topPx + CLASS_TICK_REM * remPx } };
    }),
    stroke: "textMuted",
    widthPx: FRAME_WIDTH_PX,
  });
  ops.push({
    kind: "polyline",
    points: [
      { xPx: leftPx, yPx: topPx },
      { xPx: rightPx, yPx: topPx },
      { xPx: rightPx, yPx: bottomPx },
      { xPx: leftPx, yPx: bottomPx },
      { xPx: leftPx, yPx: topPx },
    ],
    stroke: "textMuted",
    widthPx: FRAME_WIDTH_PX,
  });

  const anchors: Anchor[] = [];
  let selected: HrPoint | undefined;
  for (const point of projection.points.toSorted(drawOrder)) {
    const colour = point.status === "available" ? "accent" : "text";
    const radiusPx = outerRadiusPx(point, remPx);
    const symbol: SymbolOp = {
      kind: "symbol",
      id: point.id,
      centre: { xPx: point.xPx, yPx: point.yPx },
      shape: point.shape,
      radiusPx: radiusPx - SYMBOL_STROKE_PX / 2,
      stroke: colour,
      fill: colour,
      widthPx: SYMBOL_STROKE_PX,
    };
    ops.push(symbol);
    const peg = pegOp(point, remPx);
    if (peg !== null) {
      ops.push(peg);
    }
    anchors.push({ id: point.id, xPx: point.xPx, yPx: point.yPx, depth: 0, radiusPx });
    if (point.id === selectedId) {
      selected = point;
    }
  }
  if (selected !== undefined) {
    ops.push({
      kind: "reticle",
      id: selected.id,
      centre: { xPx: selected.xPx, yPx: selected.yPx },
      halfSizePx: outerRadiusPx(selected, remPx) + (RETICLE_MARGIN_REM * remPx) / 2,
      stroke: "accent",
      widthPx: RETICLE_WIDTH_PX,
    });
  }
  return { ops, anchors, curveLabels: [] };
}

/**
 * The point a pointer at `pointPx` picks on the diagram, or `null` when none is near enough.
 *
 * @remarks
 * Within 1 rem of a point's centre, half the guide's 2 rem target, or within its symbol where that
 * is larger, as on the chart; the nearest wins, and at a tie the lower ID.
 *
 * @param remPx - CSS pixels in one `rem`, which sets the tolerance.
 */
export function pickHr(
  anchors: ReadonlyArray<Anchor>,
  pointPx: ScreenPoint,
  remPx: number,
): SystemIdHex | null {
  return pick(anchors, pointPx, remPx);
}
