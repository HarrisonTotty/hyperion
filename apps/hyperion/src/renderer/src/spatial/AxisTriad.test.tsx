import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { AxisTriad } from "./AxisTriad";
import { type CameraAngles, PRESETS } from "./camera";
import { localFrameAt } from "./frame";
import { TRIAD_BOX_REM, type TriadBoxRem } from "./furniture";
import { vec3 } from "./vec3";

const FRAME = localFrameAt(vec3(26_000, 0, 0));

function renderTriad(angles: CameraAngles, frame = FRAME, boxRem: TriadBoxRem = TRIAD_BOX_REM) {
  render(<AxisTriad frame={frame} angles={angles} boxRem={boxRem} />);
  return screen.getByRole("img", { name: "Axis triad" });
}

/** The drawn axis of that name, which says how it ends. */
function axis(triad: HTMLElement, name: string): Element {
  const found = triad.querySelector(`[data-axis="${name}"]`);
  if (found === null) {
    throw new Error(`the triad draws no ${name} axis`);
  }
  return found;
}

/** The label's centre, in `rem` from the triad's top left, as its transform places it. */
function labelCentreRem(label: HTMLElement): readonly [number, number] {
  const match = /translate\((-?[\d.]+)rem, (-?[\d.]+)rem\)/u.exec(label.style.transform);
  if (match === null) {
    throw new Error(`label ${label.textContent} has no position: ${label.style.transform}`);
  }
  return [Number(match[1]), Number(match[2])];
}

describe("AxisTriad", () => {
  it("is one image named Axis triad, labelled COREWARD, SPINWARD and NORTH", () => {
    const triad = renderTriad(PRESETS.oblique);

    expect(within(triad).getByText("COREWARD")).toBeInTheDocument();
    expect(within(triad).getByText("SPINWARD")).toBeInTheDocument();
    expect(within(triad).getByText("NORTH")).toBeInTheDocument();
  });

  it("ends north in the towards symbol seen from the top", () => {
    const triad = renderTriad(PRESETS.top);

    expect(axis(triad, "north")).toHaveAttribute("data-end", "towards");
  });

  it("ends north in the away symbol seen from the south, at -90°", () => {
    const triad = renderTriad({ azimuthDeg: 0, elevationDeg: -90 });

    expect(axis(triad, "north")).toHaveAttribute("data-end", "away");
  });

  it("draws the towards symbol as a circle with a dot", () => {
    const towards = axis(renderTriad(PRESETS.top), "north");

    expect(towards.querySelectorAll("circle")).toHaveLength(2);
    expect(towards.querySelectorAll("path")).toHaveLength(0);
  });

  it("draws the away symbol as an open circle with a cross", () => {
    // SIDE looks coreward, along the plane.
    const away = axis(renderTriad(PRESETS.side), "coreward");

    expect(away).toHaveAttribute("data-end", "away");
    expect(away.querySelectorAll("circle")).toHaveLength(1);
    expect(away.querySelectorAll("path")).toHaveLength(1);
  });

  it("ends an axis across the screen in an arrowhead", () => {
    const triad = renderTriad(PRESETS.side);

    expect(axis(triad, "spinward")).toHaveAttribute("data-end", "across");
    expect(axis(triad, "north")).toHaveAttribute("data-end", "across");
    expect(axis(triad, "north").querySelectorAll("polyline")).toHaveLength(1);
  });

  it("ends the axes that lean towards and away from the viewer in their symbols", () => {
    const triad = renderTriad(PRESETS.oblique);

    // From 030°, +30°: coreward and spinward lean away, north towards the camera above the plane.
    expect(axis(triad, "coreward")).toHaveAttribute("data-end", "away");
    expect(axis(triad, "spinward")).toHaveAttribute("data-end", "away");
    expect(axis(triad, "north")).toHaveAttribute("data-end", "towards");
  });

  it("puts coreward up and spinward right seen from the top", () => {
    const triad = renderTriad(PRESETS.top);
    const coreward = axis(triad, "coreward").querySelector("line");
    const spinward = axis(triad, "spinward").querySelector("line");

    expect(Number(coreward?.getAttribute("y2"))).toBeLessThan(0);
    expect(Number(coreward?.getAttribute("x2"))).toBeCloseTo(0, 9);
    expect(Number(spinward?.getAttribute("x2"))).toBeGreaterThan(0);
    expect(Number(spinward?.getAttribute("y2"))).toBeCloseTo(0, 9);
  });

  it("mirrors the view from the south: coreward down, spinward still right", () => {
    const triad = renderTriad({ azimuthDeg: 0, elevationDeg: -90 });
    const coreward = axis(triad, "coreward").querySelector("line");
    const spinward = axis(triad, "spinward").querySelector("line");

    expect(Number(coreward?.getAttribute("y2"))).toBeGreaterThan(0);
    expect(Number(spinward?.getAttribute("x2"))).toBeGreaterThan(0);
  });

  it("sets the label of an axis seen end on away from the other two", () => {
    const triad = renderTriad(PRESETS.top);
    const [northX, northY] = labelCentreRem(within(triad).getByText("NORTH"));
    const [originX, originY] = [7, 3.5];

    // Coreward is up and spinward right, so north's label goes down and to the left.
    expect(northX).toBeLessThan(originX);
    expect(northY).toBeGreaterThan(originY);
  });

  it("draws itself and every label inside a box shorter than its usual one", () => {
    // The local chart's stage at 1280 x 688 with the census table shown: 74 px, 4.625rem.
    const shortBox = { width: 15, height: 4.625 };
    const triad = renderTriad(PRESETS.oblique, FRAME, shortBox);

    expect(triad.style.height).toBe("4.625rem");
    for (const name of ["COREWARD", "SPINWARD", "NORTH"]) {
      const label = within(triad).getByText(name);
      const [, centreY] = labelCentreRem(label);
      const halfHeight = 1.09375 / 2;
      expect(centreY - halfHeight).toBeGreaterThanOrEqual(0);
      expect(centreY + halfHeight).toBeLessThanOrEqual(shortBox.height);
    }
    // Every axis ends, with its symbol, inside the box too.
    for (const name of ["coreward", "spinward", "north"]) {
      const line = axis(triad, name).querySelector("line");
      const reachRem = Math.abs(Number(line?.getAttribute("y2") ?? 0)) / 16;
      expect(reachRem + 0.25).toBeLessThanOrEqual(shortBox.height / 2);
    }
  });

  it("labels the directions -X and +Y on the galactic axis, where the frame falls back to them", () => {
    const triad = renderTriad(PRESETS.oblique, localFrameAt(vec3(0, 0, 5)));

    expect(within(triad).getByText("-X")).toBeInTheDocument();
    expect(within(triad).getByText("+Y")).toBeInTheDocument();
    expect(within(triad).getByText("NORTH")).toBeInTheDocument();
    expect(within(triad).queryByText("COREWARD")).not.toBeInTheDocument();
  });
});
