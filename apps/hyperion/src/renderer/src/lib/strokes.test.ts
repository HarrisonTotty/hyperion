import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import STYLES from "../styles.css?raw";
import { stylesheetRule } from "../test/stylesheet";
import {
  lineScale,
  markShiftDevicePx,
  markStrokeDevicePx,
  minReticleGapDevicePx,
  MIN_STROKE_DEVICE_PX,
  RETICLE_SHIFTS,
  reticleGrowthCssPx,
  RING_SHIFTS,
  strokeProperties,
  useStrokeMetrics,
  watchStrokeProperties,
} from "./strokes";

/** The ratios in use: the development machine's and the UHD 620's, 100%, a Retina display, and 3. */
const RATIOS = [0.78125, 1, 2, 3] as const;

/** A figure to the hundredth, as the rulings give their figures. */
function hundredths(value: number): number {
  // Plus zero, so that a shift a hair under zero reads as none rather than -0.
  return Math.round(value * 100) / 100 + 0;
}

describe("the console's stroke widths (decision-thin-line-contrast, item 2)", () => {
  it("draws no line or outline under 2 device px", () => {
    expect(MIN_STROKE_DEVICE_PX).toBe(2);
  });

  it("scales a line by the larger of the ratio and 2: 2, 2, 2 and 3", () => {
    expect(RATIOS.map(lineScale)).toEqual([2, 2, 2, 3]);
  });

  it("draws a mark's outline at the larger of 1.5 times the ratio and 2: 2, 2, 3 and 4.5", () => {
    expect(RATIOS.map(markStrokeDevicePx)).toEqual([2, 2, 3, 4.5]);
  });

  it("moves a mark's outline out by half what the floor adds: 0.41, 0.25, 0 and 0 px", () => {
    expect(RATIOS.map(markShiftDevicePx)).toEqual([0.4140625, 0.25, 0, 0]);
  });

  it("moves an outline 0.06 px at a ratio of 1.25 and none from 4/3 up", () => {
    expect([
      markShiftDevicePx(1.25),
      Math.abs(markShiftDevicePx(4 / 3)) < 1e-12,
      markShiftDevicePx(1.5),
    ]).toEqual([0.0625, true, 0]);
  });

  it("stands two reticles about one mark an outline and a casing apart: 4, 4, 5 and 7.5 px", () => {
    expect(RATIOS.map(minReticleGapDevicePx)).toEqual([4, 4, 5, 7.5]);
  });

  it("takes a ratio that is not a positive number as 1", () => {
    expect(
      [Number.NaN, 0, -2, Number.POSITIVE_INFINITY].map((ratio) => [
        lineScale(ratio),
        markStrokeDevicePx(ratio),
        markShiftDevicePx(ratio),
        minReticleGapDevicePx(ratio),
        reticleGrowthCssPx(ratio),
      ]),
    ).toEqual(Array.from({ length: 4 }, () => [2, 2, 0.25, 4, 1.25]));
  });
});

describe("the outlines' shift multiples (R07.T16.f)", () => {
  it("moves a ringed circle's ring three shifts out and a reticle four", () => {
    expect([RING_SHIFTS, RETICLE_SHIFTS]).toEqual([3, 4]);
  });

  it("grows a reticle's outer edge by 5δ: 2.65, 1.25, 0 and 0 CSS px", () => {
    expect(RATIOS.map((ratio) => hundredths(reticleGrowthCssPx(ratio)))).toEqual([
      2.65, 1.25, 0, 0,
    ]);
  });
});

describe("strokeProperties (R07.T16.f)", () => {
  it("gives the line scale and the mark stroke in CSS px: 2.56, 2, 1 and 1, and 2.56, 2, 1.5 and 1.5 px", () => {
    expect(RATIOS.map(strokeProperties)).toEqual([
      { "--line-scale": "2.56", "--mark-stroke": "2.56px" },
      { "--line-scale": "2", "--mark-stroke": "2px" },
      { "--line-scale": "1", "--mark-stroke": "1.5px" },
      { "--line-scale": "1", "--mark-stroke": "1.5px" },
    ]);
  });

  it("draws every property at no less than 2 device px", () => {
    for (const ratio of RATIOS) {
      const properties = strokeProperties(ratio);
      expect(Number(properties["--line-scale"]) * ratio).toBeGreaterThanOrEqual(2);
      expect(Number.parseFloat(properties["--mark-stroke"]) * ratio).toBeGreaterThanOrEqual(2);
    }
  });

  it("holds its values at a ratio of 2 on the stylesheet's root, before main.tsx sets them", () => {
    const root = stylesheetRule(":root");

    expect(root).toContain("--line-scale: 1;");
    expect(root).toContain("--mark-stroke: 1.5px;");
    expect(strokeProperties(2)).toEqual({ "--line-scale": "1", "--mark-stroke": "1.5px" });
  });
});

/** A `matchMedia` that answers resolution queries, and fires a query's `change` on demand. */
function stubResolutionQueries() {
  const listeners = new Map<string, Set<() => void>>();
  const listenersOf = (query: string): Set<() => void> => {
    const found = listeners.get(query) ?? new Set<() => void>();
    listeners.set(query, found);
    return found;
  };
  vi.stubGlobal("matchMedia", (query: string): MediaQueryList => {
    const list = {
      media: query,
      matches: false,
      onchange: null,
      addEventListener: (type: string, listener: () => void): void => {
        if (type === "change") {
          listenersOf(query).add(listener);
        }
      },
      removeEventListener: (type: string, listener: () => void): void => {
        if (type === "change") {
          listenersOf(query).delete(listener);
        }
      },
      addListener: (): void => undefined,
      removeListener: (): void => undefined,
      dispatchEvent: (): boolean => true,
    };
    // A stand-in for the browser's list, which jsdom lacks; it has every member the code reads.
    // oxlint-disable-next-line typescript/no-unsafe-type-assertion
    return list as unknown as MediaQueryList;
  });
  return {
    /** Moves the window to a display of `ratio`, telling the query of the ratio it leaves. */
    moveTo(ratio: number): void {
      const left = `(resolution: ${String(window.devicePixelRatio)}dppx)`;
      vi.stubGlobal("devicePixelRatio", ratio);
      // Each listener takes itself off the query it leaves, which a set's iteration allows.
      for (const listener of listenersOf(left)) {
        listener();
      }
    },
    /** How many listeners are armed, over every query. */
    armed(): number {
      return [...listeners.values()].reduce((sum, set) => sum + set.size, 0);
    },
  };
}

describe("watchStrokeProperties (R07.T16.f)", () => {
  it("sets the root's properties at once, at the window's ratio", () => {
    stubResolutionQueries();
    vi.stubGlobal("devicePixelRatio", 0.78125);
    const root = document.createElement("div");

    const stop = watchStrokeProperties(root);

    expect([
      root.style.getPropertyValue("--line-scale"),
      root.style.getPropertyValue("--mark-stroke"),
    ]).toEqual(["2.56", "2.56px"]);
    stop();
  });

  it("sets them again on a change of ratio, and each change after", () => {
    const display = stubResolutionQueries();
    vi.stubGlobal("devicePixelRatio", 0.78125);
    const root = document.createElement("div");
    const stop = watchStrokeProperties(root);

    display.moveTo(2);
    const atTwo = root.style.getPropertyValue("--mark-stroke");
    display.moveTo(1);

    expect([atTwo, root.style.getPropertyValue("--mark-stroke")]).toEqual(["1.5px", "2px"]);
    expect(root.style.getPropertyValue("--line-scale")).toBe("2");
    stop();
  });

  it("stops when disposed, leaving no listener armed", () => {
    const display = stubResolutionQueries();
    vi.stubGlobal("devicePixelRatio", 0.78125);
    const root = document.createElement("div");
    const stop = watchStrokeProperties(root);

    stop();
    display.moveTo(2);

    expect(root.style.getPropertyValue("--mark-stroke")).toBe("2.56px");
    expect(display.armed()).toBe(0);
  });
});

describe("useStrokeMetrics (R07.T16.f)", () => {
  it("gives the window's ratio and the root's rem", () => {
    stubResolutionQueries();
    vi.stubGlobal("devicePixelRatio", 0.78125);

    const { result } = renderHook(() => useStrokeMetrics());

    expect(result.current).toEqual({ devicePixelRatio: 0.78125, remPx: 16 });
  });

  it("follows a change of ratio", () => {
    const display = stubResolutionQueries();
    vi.stubGlobal("devicePixelRatio", 0.78125);
    const { result } = renderHook(() => useStrokeMetrics());

    act(() => {
      display.moveTo(2);
    });

    expect(result.current.devicePixelRatio).toBe(2);
  });

  it("follows a change of the root's rem, which a resize reports", () => {
    stubResolutionQueries();
    const { result } = renderHook(() => useStrokeMetrics());

    document.documentElement.style.fontSize = "20px";
    act(() => {
      window.dispatchEvent(new Event("resize"));
    });
    const remPx = result.current.remPx;
    document.documentElement.style.fontSize = "";

    expect(remPx).toBe(20);
  });
});

describe("the stylesheet's SVG strokes (R07.T16.f; decision-thin-line-contrast, items 2 and 3)", () => {
  it.each([
    [".map-view__mark-line", "stroke-width: var(--mark-stroke);"],
    [".map-view__mark-casing", "stroke-width: calc(var(--mark-stroke) + 2px * var(--line-scale));"],
    [".symbol-legend__mark", "stroke-width: var(--mark-stroke);"],
    [".orbit-legend__path--reference path", "stroke-width: calc(1px * var(--line-scale));"],
    [".orbit-legend__path--selected path", "stroke-width: calc(2px * var(--line-scale));"],
    [".density-legend__tick-mark", "stroke-width: calc(1px * var(--line-scale));"],
    [".glyph--disclosure polyline", "stroke-width: max(var(--mark-stroke), 0.105em);"],
  ])("draws %s at its property", (selector, declaration) => {
    expect(stylesheetRule(selector)).toContain(declaration);
  });

  it("draws the axis triad and the core arrow at the mark stroke, in CSS px", () => {
    expect(STYLES).toMatch(
      /\.axis-triad__axes,\n\.core-arrow__mark \{[^}]*stroke-width: var\(--mark-stroke\);/u,
    );
    expect(STYLES).toMatch(
      /\.axis-triad__axes :is\(line, polyline, circle, path\),\n\.core-arrow__mark :is\(line, polyline, circle, path\) \{\n {2}vector-effect: non-scaling-stroke;/u,
    );
  });

  it("keeps the chevron's width in CSS px, out of its box's units", () => {
    expect(stylesheetRule(".glyph--disclosure polyline")).toContain(
      "vector-effect: non-scaling-stroke;",
    );
  });

  it("writes no other stroke width in a number of px", () => {
    const widths = [...STYLES.matchAll(/stroke-width: ([^;]+);/gu)].map(([, value]) => value);

    expect(widths.filter((value) => !(value?.includes("var(--") ?? false))).toEqual([]);
  });
});

/** The selectors of every rule that paints `--surface-2`, as the stylesheet writes them. */
function surface2Rules(): string[] {
  return [...STYLES.matchAll(/(?:^|\n)([^\n{}]+) \{([^}]*)\}/gu)]
    .filter(([, , body]) => body?.includes("background: var(--surface-2);") ?? false)
    .map(([, selector]) => selector?.trim() ?? "");
}

describe("the strokes changed here and --surface-2 (decision-thin-line-contrast, item 2)", () => {
  // Only `--text-muted` on `--surface-2` (6.04:1) comes within 1% of 6:1 at 2 device px, so a
  // stroke there would need its pixels read. The strokes changed here stand on the spatial view's
  // `--surface-0` canvas, over the map's raster on their casing, in the legends' panels, and in a
  // control; these are the rules that paint `--surface-2`, so that a new one is looked at again.
  it("paints --surface-2 only on hovered or selected tabs, controls and rows, and inputs", () => {
    expect(surface2Rules()).toEqual([
      ".console__tab:hover",
      ".control:hover",
      ".command:hover",
      ".form-field__input",
      ".galaxy-pages__tab:hover",
      ".system-list__row:hover",
      '.system-list__row[aria-selected="true"]',
      ".body-list__row:hover",
      '.body-list__row[aria-selected="true"]',
      ".view-list__row:hover",
      '.view-list__row[aria-selected="true"]',
    ]);
  });

  it("draws the one changed stroke a --surface-2 rule can stand under, the chevron, in --accent", () => {
    // The chevron is `currentColor` in a `.control`: `--accent` (8.68:1 on `--surface-2`). A
    // control held back is `--text-muted`, and its hover paints no surface.
    expect(stylesheetRule(".control")).toContain("color: var(--accent);");
    expect(stylesheetRule('.control[aria-disabled="true"]')).toContain("color: var(--text-muted);");
    expect(STYLES).toMatch(
      /\.control\[aria-disabled="true"\]:hover,\n\.control\[aria-disabled="true"\]:active \{\n {2}background: none;/u,
    );
  });
});
