import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { lineScale } from "../../lib/strokes";
import { SEGMENT_DASH_PX, SEGMENT_TICK_REM } from "../../spatial/drawList";
import { aCensus } from "../../test/galaxyFixtures";
import { stylesheetRule } from "../../test/stylesheet";
import { layerBands, OBSERVER_LABEL } from "./chartModel";
import { APPARENT_LEGEND, SymbolLegend } from "./SymbolLegend";

function legend(): HTMLElement {
  return screen.getByRole("group", { name: "Chart legend" });
}

/** CSS px to a rem at 100%. */
const REM_PX = 16;

/** The largest line factor in use, CSS px drawn for each CSS px of a line: 2.56 at 0.78125. */
const LARGEST_LINE_FACTOR = lineScale(0.78125) / 0.78125;

/** A straight path's `M x y H x` or `M x y V y`, as numbers. */
function straightPath(element: Element | null): {
  readonly x: number;
  readonly y: number;
  readonly axis: string;
  readonly to: number;
} {
  const match = /^M (\S+) (\S+) ([HV]) (\S+)$/u.exec(element?.getAttribute("d") ?? "");
  if (match === null) {
    throw new Error("the sample's path is not one straight line");
  }
  const [, x = "", y = "", axis = "", to = ""] = match;
  return { x: Number(x), y: Number(y), axis, to: Number(to) };
}

/**
 * The apparent position's drawn sample, which is `aria-hidden` and so has no role: the words beside
 * it are its name.
 */
function apparentSample() {
  render(<SymbolLegend bands={layerBands(aCensus().layers)} observed />);
  const svg = legend().querySelector(".symbol-legend__apparent");
  if (!(svg instanceof SVGSVGElement)) {
    throw new Error("the legend draws no apparent position's sample");
  }
  const [, , viewWidth = 0] = (svg.getAttribute("viewBox") ?? "").split(" ").map(Number);
  // The box's user units to a CSS px at 100%.
  const unitsPerPx = viewWidth / (Number.parseFloat(svg.style.width) * REM_PX);
  return {
    joiner: straightPath(svg.querySelector(".symbol-legend__joiner")),
    tick: straightPath(svg.querySelector(".symbol-legend__tick")),
    unitsPerPx,
  };
}

describe("SymbolLegend in observed mode (plan 12, P12.T11)", () => {
  it("names the observer's mark and an apparent position's tick and joiner in words", () => {
    render(<SymbolLegend bands={layerBands(aCensus().layers)} observed />);

    expect(within(legend()).getByText(OBSERVER_LABEL)).toBeInTheDocument();
    expect(within(legend()).getByText(APPARENT_LEGEND)).toBeInTheDocument();
    expect(APPARENT_LEGEND).toBe("TICK APPARENT POSITION, DASHED TO PRESENT");
  });

  it("names neither in now, which draws neither", () => {
    render(<SymbolLegend bands={layerBands(aCensus().layers)} />);

    expect(within(legend()).queryByText(OBSERVER_LABEL)).toBeNull();
    expect(within(legend()).queryByText(APPARENT_LEGEND)).toBeNull();
  });
});

describe("SymbolLegend's apparent position sample (P12.T11.c)", () => {
  it("dashes its joiner as the chart does, at the line scale", () => {
    expect(stylesheetRule(".symbol-legend__apparent .symbol-legend__joiner")).toContain(
      `stroke-dasharray: calc(${SEGMENT_DASH_PX.onPx}px * var(--line-scale)) ` +
        `calc(${SEGMENT_DASH_PX.offPx}px * var(--line-scale));`,
    );
  });

  it("holds two whole dashes and the gap between them at a line factor of 2.56", () => {
    const { joiner, unitsPerPx } = apparentSample();
    const { onPx, offPx } = SEGMENT_DASH_PX;

    expect(LARGEST_LINE_FACTOR).toBeCloseTo(2.56, 9);
    expect(joiner.axis).toBe("H");
    const lengthPx = Math.abs(joiner.to - joiner.x) / unitsPerPx;
    expect(lengthPx).toBeGreaterThanOrEqual((onPx + offPx + onPx) * LARGEST_LINE_FACTOR);
  });

  it("starts its joiner at the tick, so that a dash meets it", () => {
    const { joiner, tick } = apparentSample();

    expect(tick.axis).toBe("V");
    expect(joiner.x).toBe(tick.x);
    // Across the joiner's line, centred on it.
    expect((tick.y + tick.to) / 2).toBe(joiner.y);
  });

  it("draws the tick at the chart's length, half a rem", () => {
    const { tick, unitsPerPx } = apparentSample();

    expect(Math.abs(tick.to - tick.y) / unitsPerPx).toBe(SEGMENT_TICK_REM * REM_PX);
  });
});
