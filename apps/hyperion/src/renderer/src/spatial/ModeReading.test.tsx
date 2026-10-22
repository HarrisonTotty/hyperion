import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ModeReading, type ModeReadingProps, type SpatialMode } from "./ModeReading";

const NOW: SpatialMode = { name: "NOW", from: [] };

const OBSERVED: SpatialMode = {
  name: "OBSERVED FROM",
  from: [
    { label: "RADIUS", value: "10,000.0", unit: "ly", widthCh: 9 },
    { label: "ANGLE", value: "036.9°", unit: "", widthCh: 6 },
    { label: "HEIGHT", value: "-12.3", unit: "ly", widthCh: 9 },
  ],
};

const OBSERVED_ON_AXIS: SpatialMode = {
  name: "OBSERVED FROM",
  from: [
    { label: "RADIUS", value: "0.0", unit: "ly", widthCh: 9 },
    { label: "ANGLE", value: null, unit: "", widthCh: 6 },
    { label: "HEIGHT", value: "+100.0", unit: "ly", widthCh: 9 },
  ],
};

/** Renders the mode's reading in the `dl` it is written for. */
function renderMode(props: ModeReadingProps): void {
  render(
    <dl>
      <ModeReading {...props} />
    </dl>,
  );
}

/** The value a term defines. */
function definition(label: string): HTMLElement {
  const value = screen.getByText(label, { selector: "dt" }).nextElementSibling;
  if (!(value instanceof HTMLElement)) {
    throw new Error(`${label} has no value`);
  }
  return value;
}

/** The mode's name as written in its value, before any observer. */
function modeName(): string | null {
  return definition("MODE").firstChild?.textContent ?? null;
}

describe("ModeReading (plan 12, P12.T11.d)", () => {
  it("reads MODE NOW, with no observer", () => {
    renderMode({ mode: NOW });

    expect(definition("MODE")).toHaveTextContent(/^NOW$/u);
    expect(definition("MODE").parentElement).toHaveClass("spatial-view__mode");
    expect(definition("MODE").querySelector("dl")).toBeNull();
  });

  it("reads MODE OBSERVED FROM with the observer's readings nested in its value", () => {
    renderMode({ mode: OBSERVED });

    expect(modeName()).toBe("OBSERVED FROM");
    const observer = definition("MODE").querySelector("dl");
    expect(observer).toHaveClass("spatial-view__observer");
    expect([...(observer?.querySelectorAll("dt") ?? [])].map((term) => term.textContent)).toEqual([
      "RADIUS",
      "ANGLE",
      "HEIGHT",
    ]);
    expect(definition("RADIUS")).toHaveTextContent(/^10,000\.0 ly$/u);
    expect(definition("ANGLE")).toHaveTextContent(/^036\.9°$/u);
    expect(definition("HEIGHT")).toHaveTextContent(/^-12\.3 ly$/u);
  });

  it("marks each of the observer's values stale, and not the mode's name", () => {
    renderMode({ mode: OBSERVED, state: "stale" });

    for (const label of ["RADIUS", "ANGLE", "HEIGHT"]) {
      expect(definition(label)).toHaveTextContent(/S\s*stale$/u);
    }
    expect(modeName()).toBe("OBSERVED FROM");
    // Three marks in all: the name, a word, carries none.
    expect(within(definition("MODE")).getAllByText("stale")).toHaveLength(3);
  });

  it("marks no missing coordinate stale", () => {
    renderMode({ mode: OBSERVED_ON_AXIS, state: "stale" });

    expect(definition("ANGLE")).toHaveTextContent(/^—$/u);
    expect(within(definition("MODE")).getAllByText("stale")).toHaveLength(2);
  });

  it("marks NOW, a word, never stale", () => {
    renderMode({ mode: NOW, state: "stale" });

    expect(definition("MODE")).toHaveTextContent(/^NOW$/u);
    expect(screen.queryByText("stale")).not.toBeInTheDocument();
  });

  it("keeps an empty slot after each of the observer's values while live", () => {
    renderMode({ mode: OBSERVED, state: "live" });

    for (const label of ["RADIUS", "ANGLE", "HEIGHT"]) {
      const slot = definition(label).querySelector(".stale-slot");
      expect(slot).not.toBeNull();
      expect(slot).toBeEmptyDOMElement();
    }
  });

  it("gives the observer's readings no slot when no state is given", () => {
    renderMode({ mode: OBSERVED });

    expect(definition("MODE").querySelector(".stale-slot")).toBeNull();
  });
});
