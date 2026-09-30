import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { formatMassMearth } from "../lib/format";
import { EarthMassUnit } from "./EarthMassUnit";

const EARTH_SIGN = String.fromCodePoint(0x2295);

describe("EarthMassUnit", () => {
  it("is announced as Earth masses", () => {
    render(
      <p>
        {formatMassMearth(17.1)} <EarthMassUnit />
      </p>,
    );

    expect(screen.getByRole("img", { name: "Earth masses" })).toBeInTheDocument();
  });

  it("draws the Earth sign instead of typing it", () => {
    const { container } = render(<EarthMassUnit />);

    expect(container.textContent).not.toContain(EARTH_SIGN);
    expect(document.body.textContent).not.toContain(EARTH_SIGN);
  });

  it("hides the drawn sign from assistive technology, which hears the unit's name", () => {
    const { container } = render(<EarthMassUnit />);

    expect(container.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });

  it("draws the sign as a circle with a cross, in the colour of the surrounding text", () => {
    const { container } = render(<EarthMassUnit />);

    const circle = container.querySelector("svg circle");
    const cross = container.querySelector("svg path");
    expect(circle).toHaveAttribute("stroke", "currentColor");
    expect(circle).toHaveAttribute("fill", "none");
    expect(cross).toHaveAttribute("stroke", "currentColor");
    // One stroke down and one across, each the circle's diameter, through its centre.
    expect(cross).toHaveAttribute("d", "M5 1V9M1 5H9");
  });
});
