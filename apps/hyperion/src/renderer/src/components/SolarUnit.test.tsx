import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { SolarUnit } from "./SolarUnit";

const SUN_SIGN = String.fromCodePoint(0x2609);

describe("SolarUnit", () => {
  it.each([
    ["luminosity", "L", "solar luminosities"],
    ["radius", "R", "solar radii"],
  ] as const)("writes %s as %s and the drawn sign, announced as %s", (quantity, letter, name) => {
    render(
      <p>
        1.00 <SolarUnit quantity={quantity} />
      </p>,
    );

    const unit = screen.getByRole("img", { name });
    expect(unit).toHaveTextContent(letter);
    expect(unit.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });

  it("draws the sun sign instead of typing it", () => {
    render(<SolarUnit quantity="luminosity" />);

    expect(document.body.textContent).not.toContain(SUN_SIGN);
  });
});
