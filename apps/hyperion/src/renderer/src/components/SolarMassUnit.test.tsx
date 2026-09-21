import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { SolarMassUnit } from "./SolarMassUnit";

const SUN_SIGN = String.fromCodePoint(0x2609);

describe("SolarMassUnit", () => {
  it("is announced as solar masses", () => {
    render(
      <p>
        1.00 <SolarMassUnit />
      </p>,
    );

    expect(screen.getByRole("img", { name: "solar masses" })).toBeInTheDocument();
  });

  it("draws the sun sign instead of typing it", () => {
    const { container } = render(<SolarMassUnit />);

    expect(container.textContent).not.toContain(SUN_SIGN);
    expect(document.body.textContent).not.toContain(SUN_SIGN);
    expect(container.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });

  it("draws the sign in the colour of the surrounding text", () => {
    const { container } = render(<SolarMassUnit />);

    const shapes = container.querySelectorAll("svg circle");
    expect(shapes).toHaveLength(2);
    expect(shapes[0]).toHaveAttribute("stroke", "currentColor");
    expect(shapes[0]).toHaveAttribute("fill", "none");
    expect(shapes[1]).toHaveAttribute("fill", "currentColor");
  });
});
