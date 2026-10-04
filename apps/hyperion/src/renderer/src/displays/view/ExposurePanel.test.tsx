import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { DEFAULT_EXPOSURE } from "../../view/photometry/exposure";
import { ExposurePanel } from "./ExposurePanel";

describe("ExposurePanel", () => {
  it("holds ENABLE back under AUTO, saying the exposure is AUTO", () => {
    render(
      <ExposurePanel
        exposure={{ kind: "auto", ev100: 3 }}
        meteredEv100={3}
        onChange={() => undefined}
      />,
    );
    const enable = screen.getByRole("button", { name: "ENABLE" });
    expect(enable).toHaveAttribute("aria-disabled", "true");
    expect(enable).toHaveAccessibleDescription("NOT AVAILABLE: the exposure is AUTO");
  });

  it("holds INHIBIT back under MAN, saying the exposure is MAN", () => {
    render(
      <ExposurePanel exposure={DEFAULT_EXPOSURE} meteredEv100={3} onChange={() => undefined} />,
    );
    const inhibit = screen.getByRole("button", { name: "INHIBIT" });
    expect(inhibit).toHaveAttribute("aria-disabled", "true");
    expect(inhibit).toHaveAccessibleDescription("NOT AVAILABLE: the exposure is MAN");
  });
});
