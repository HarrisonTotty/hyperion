import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { DEFAULT_EXPOSURE, type ExposureControl } from "../../view/photometry/exposure";
import { ExposurePanel } from "./ExposurePanel";

/** Renders the panel at a control, with a metered value. */
function renderAt(exposure: ExposureControl): void {
  render(<ExposurePanel exposure={exposure} meteredEv100={3} onChange={() => undefined} />);
}

/** The camera setting's value under its label, `APERTURE`, `SHUTTER`, `ND` or `ISO`. */
function settingOf(label: string): HTMLElement {
  const index = screen.getAllByRole("term").findIndex((term) => term.textContent === label);
  const value = screen.getAllByRole("definition")[index];
  if (value === undefined) {
    throw new Error(`no camera setting labelled ${label}`);
  }
  return value;
}

describe("ExposurePanel's camera setting", () => {
  it("shows the view camera's setting under AUTO, in the guide's order", () => {
    renderAt({ kind: "auto", ev100: 9.6 });
    expect(screen.getAllByRole("term").map((term) => term.textContent)).toEqual([
      "APERTURE",
      "SHUTTER",
      "ND",
      "ISO",
    ]);
    expect(
      ["APERTURE", "SHUTTER", "ND", "ISO"].map((label) => settingOf(label).textContent),
    ).toEqual(["f/1.4", "0.00253 s", "CLEAR", "100"]);
  });

  it("shows the default MAN triple: f/1.4, 0.0333 s, the filter clear, ISO 11,760", () => {
    renderAt(DEFAULT_EXPOSURE);
    expect(
      ["APERTURE", "SHUTTER", "ND", "ISO"].map((label) => settingOf(label).textContent),
    ).toEqual(["f/1.4", "0.0333 s", "CLEAR", "11,760"]);
  });

  it("pegs a pushed sensitivity at ISO 409,600 with the off-scale mark, announced", () => {
    renderAt({ kind: "auto", ev100: -10 });
    const iso = settingOf("ISO");
    expect(iso).toHaveTextContent("409,600 ↑ off scale high");
    expect(within(iso).getByText("↑")).toHaveAttribute("aria-hidden", "true");
    expect(within(iso).getByText("off scale high")).toHaveClass("visually-hidden");
  });

  it("shows the ND in EV at one decimal, and the shortest shutter, at a bright exposure", () => {
    renderAt({ kind: "inhibited", ev100: 20, reason: "operator" });
    expect(["SHUTTER", "ND", "ISO"].map((label) => settingOf(label).textContent)).toEqual([
      "1.25E-4 s",
      "6.1 EV",
      "100",
    ]);
  });

  it("pegs an ND beyond the filter's densest at 28.1 EV with the off-scale mark", () => {
    renderAt({
      kind: "manual",
      triple: { aperture: 1.4, shutterS: 1 / 8_000, iso: 100, ndEv: 30 },
    });
    expect(settingOf("ND")).toHaveTextContent("28.1 EV ↑ off scale high");
  });
});

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
