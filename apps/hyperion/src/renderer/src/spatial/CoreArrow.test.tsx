import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { type CameraAngles, PRESETS, type Viewport } from "./camera";
import { CoreArrow } from "./CoreArrow";
import { localFrameAt } from "./frame";
import { coreArrowLayout, coreLabelText } from "./furniture";
import { vec3 } from "./vec3";

const FRAME = localFrameAt(vec3(26_000, 0, 0));
const VIEWPORT: Viewport = { widthPx: 400, heightPx: 300, remPx: 16 };
const DISTANCE = { value: "26,000.0", unit: "ly" };

function renderArrow(angles: CameraAngles, frame = FRAME) {
  const layout = coreArrowLayout(frame, angles, VIEWPORT, coreLabelText(DISTANCE), []);
  const { container } = render(
    <CoreArrow layout={layout} viewport={VIEWPORT} distance={DISTANCE} />,
  );
  return container;
}

/** What the arrow draws: the arrow itself, or the away or towards symbol. */
function drawn(container: HTMLElement): Element {
  const found = container.querySelector("[data-core]");
  if (found === null) {
    throw new Error("the core arrow draws nothing");
  }
  return found;
}

/** The arrow's shaft, from its tail to its head, in CSS pixels. */
function shaft(container: HTMLElement): { x1: number; y1: number; x2: number; y2: number } {
  const line = drawn(container).querySelector("line");
  const read = (name: string): number => Number(line?.getAttribute(name));
  return { x1: read("x1"), y1: read("y1"), x2: read("x2"), y2: read("y2") };
}

describe("CoreArrow", () => {
  it("points up from the top of the view seen from the top, where coreward is up", () => {
    const container = renderArrow(PRESETS.top);

    const { x1, y1, x2, y2 } = shaft(container);
    expect(drawn(container)).toHaveAttribute("data-core", "arrow");
    expect(x1).toBeCloseTo(200, 9);
    expect(x2).toBeCloseTo(200, 9);
    expect(y2).toBeLessThan(y1);
    // Its head half a rem inside the top edge.
    expect(y2).toBeCloseTo(8, 9);
  });

  it("points along the projected coreward direction to the rim", () => {
    // From 090°, looking spinward along the plane, coreward is to the left.
    const container = renderArrow(PRESETS.front);

    const { x1, y1, x2, y2 } = shaft(container);
    expect(x2).toBeCloseTo(8, 9);
    expect(x2).toBeLessThan(x1);
    expect(y2).toBeCloseTo(150, 9);
    expect(y1).toBeCloseTo(150, 9);
  });

  it("is labelled CORE with the distance to the axis, the digits set apart", () => {
    renderArrow(PRESETS.top);

    const label = screen.getByText(
      (_, element) =>
        element instanceof HTMLSpanElement && element.textContent === "CORE 26,000.0 ly",
    );
    expect(label).toContainElement(screen.getByText("26,000.0"));
  });

  it("becomes the away symbol when the view looks coreward", () => {
    const container = renderArrow(PRESETS.side);

    expect(drawn(container)).toHaveAttribute("data-core", "away");
    expect(drawn(container).querySelectorAll("path")).toHaveLength(1);
  });

  it("becomes the towards symbol when the view looks rimward, within 5° of the line of sight", () => {
    const container = renderArrow({ azimuthDeg: 180, elevationDeg: 4 });

    expect(drawn(container)).toHaveAttribute("data-core", "towards");
    expect(drawn(container).querySelectorAll("circle")).toHaveLength(2);
  });

  it("draws its symbol larger than any mark's, 1.25 rem across", () => {
    const container = renderArrow(PRESETS.side);

    expect(Number(drawn(container).querySelector("circle")?.getAttribute("r"))).toBe(10);
  });

  it("stays an arrow when coreward is more than 5° off the line of sight", () => {
    const container = renderArrow({ azimuthDeg: 180, elevationDeg: 6 });

    expect(drawn(container)).toHaveAttribute("data-core", "arrow");
  });

  it("draws nothing on the galactic axis, where there is no coreward", () => {
    const container = renderArrow(PRESETS.top, localFrameAt(vec3(0, 0, 0)));

    expect(container).toBeEmptyDOMElement();
  });
});
