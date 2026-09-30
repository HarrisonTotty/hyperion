import type { Unit } from "@hyperion/protocol";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { UnitLabel } from "./UnitLabel";

const SUN_SIGN = String.fromCodePoint(0x2609);

describe("UnitLabel", () => {
  it("draws solar masses as an image named for them, without the typed sun sign", () => {
    const { container } = render(<UnitLabel unit="msun" />);

    expect(screen.getByRole("img", { name: "solar masses" })).toBeInTheDocument();
    expect(container.textContent).not.toContain(SUN_SIGN);
  });

  it.each<[Unit, string]>([
    ["ly", "ly"],
    ["myr", "Myr"],
    ["gyr", "Gyr"],
    ["km_per_s", "km/s"],
    ["deg_per_myr", "°/Myr"],
    ["deg", "°"],
    ["per_ly3", "/ly³"],
  ])("writes %s as %s", (unit, symbol) => {
    const { container } = render(<UnitLabel unit={unit} />);

    expect(container).toHaveTextContent(symbol);
  });

  it.each<Unit>(["none", "count"])("writes nothing for %s", (unit) => {
    const { container } = render(<UnitLabel unit={unit} />);

    expect(container).toBeEmptyDOMElement();
  });
});
