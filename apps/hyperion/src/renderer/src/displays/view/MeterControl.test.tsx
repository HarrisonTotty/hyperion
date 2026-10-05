import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { viewId } from "../../view/camera/state";
import { DEFAULT_EXPOSURE, type ExposureControl } from "../../view/photometry/exposure";
import type { ExposureReading } from "../../view/post/autoExposure";
import type { MeterMode } from "../../view/post/meter";
import { MeterControl } from "./MeterControl";

const READING: ExposureReading = {
  ev100: 9.6,
  triple: { aperture: 1.4, shutterS: 1 / 30, iso: 7.4 },
  control: { kind: "auto", ev100: 9.6 },
  meter: "average",
  source: viewId("view"),
};

/** The meter's reading, an `output` set in parts (`readingParts`), by its whole text. */
function readingOutput(text: string): HTMLElement {
  return screen.getByText(
    (_, element) => element?.tagName === "OUTPUT" && element.textContent === text,
  );
}

describe("MeterControl", () => {
  it("shows the exposure with its level, the meter and the view it meters, by its name", () => {
    render(
      <MeterControl
        meter="average"
        reading={READING}
        meteredEv100={9.6}
        onMeter={() => undefined}
      />,
    );
    expect(readingOutput("EV100 9.6 AUTO")).toBeInTheDocument();
    expect(screen.getByRole("status", { name: "METER" })).toHaveTextContent("AVG");
    expect(screen.getByRole("status", { name: "SOURCE" })).toHaveTextContent("PRIMARY");
    expect(screen.getByRole("button", { name: "AVG" })).toHaveAttribute("aria-pressed", "true");
  });

  it("selects each meter from the keyboard", async () => {
    const user = userEvent.setup();
    const onMeter = vi.fn<(mode: MeterMode) => void>();
    render(<MeterControl meter="average" reading={READING} meteredEv100={9.6} onMeter={onMeter} />);
    await user.tab();
    expect(screen.getByRole("button", { name: "AVG" })).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(onMeter).toHaveBeenLastCalledWith("average");
    await user.tab();
    expect(screen.getByRole("button", { name: "LIT" })).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(onMeter).toHaveBeenLastCalledWith("lit");
    await user.tab();
    expect(screen.getByRole("button", { name: "DARK" })).toHaveFocus();
    await user.keyboard(" ");
    expect(onMeter).toHaveBeenLastCalledWith("dark");
    expect(onMeter).toHaveBeenCalledTimes(3);
  });

  it("keeps the meters choosable while nothing is metered, saying why", async () => {
    const user = userEvent.setup();
    const onMeter = vi.fn<(mode: MeterMode) => void>();
    render(<MeterControl meter="lit" reading={null} meteredEv100={null} onMeter={onMeter} />);
    expect(screen.getByText("NO IMAGE TO METER")).toBeInTheDocument();
    const lit = screen.getByRole("button", { name: "LIT" });
    expect(lit).not.toHaveAttribute("aria-disabled");
    expect(lit).toHaveAccessibleDescription("NO IMAGE TO METER");
    // The operator's meter still shows while nothing is metered.
    expect(lit).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("status", { name: "METER" })).toHaveTextContent("LIT");
    await user.click(screen.getByRole("button", { name: "AVG" }));
    expect(onMeter).toHaveBeenLastCalledWith("average");
  });

  it("describes only the chosen meter with the reason while nothing is metered", () => {
    render(
      <MeterControl meter="lit" reading={null} meteredEv100={null} onMeter={() => undefined} />,
    );
    expect(
      ["AVG", "LIT", "DARK"].map(
        (name) => screen.getByRole("button", { name }).getAttribute("aria-describedby") !== null,
      ),
    ).toEqual([false, true, false]);
    // Choosing another meter is the remedy, not a refused command.
    expect(screen.getByRole("button", { name: "AVG" })).not.toHaveAccessibleDescription();
    expect(screen.getByRole("button", { name: "DARK" })).not.toHaveAccessibleDescription();
  });

  it("shows the meter's own value as METERED while the exposure does not follow it (R07.T13.d)", () => {
    const renderAt = (control: ExposureControl): (() => void) =>
      render(
        <MeterControl
          meter="average"
          reading={{ ...READING, control, ev100: -1 }}
          meteredEv100={9.64}
          onMeter={() => undefined}
        />,
      ).unmount;
    for (const control of [
      DEFAULT_EXPOSURE,
      { kind: "inhibited", ev100: -1, reason: "operator" } as const,
    ]) {
      const unmount = renderAt(control);
      expect(screen.getByRole("status", { name: "METERED" })).toHaveTextContent("EV100 9.6");
      unmount();
    }
  });

  it("shows no METERED while the exposure follows the meter, under AUTO or a system inhibit", () => {
    for (const control of [
      { kind: "auto", ev100: -1 } as const,
      { kind: "inhibited", ev100: -1, reason: "no_image_to_meter" } as const,
    ]) {
      const { unmount } = render(
        <MeterControl
          meter="average"
          reading={{ ...READING, control, ev100: -1 }}
          meteredEv100={9.64}
          onMeter={() => undefined}
        />,
      );
      expect(screen.queryByRole("status", { name: "METERED" })).toBeNull();
      unmount();
    }
  });

  it("sets its reading in parts, so that it breaks at its middle dot (R07.T19.b's follow-up)", () => {
    render(
      <MeterControl
        meter="average"
        reading={{ ...READING, control: { kind: "inhibited", ev100: 9.6, reason: "operator" } }}
        meteredEv100={9.6}
        onMeter={() => undefined}
      />,
    );
    const output = readingOutput("EV100 9.6 INHIBITED · OPERATOR");
    expect(
      [...output.querySelectorAll(".view-label__part")].map((part) => part.textContent),
    ).toEqual(["EV100 9.6 INHIBITED", "OPERATOR"]);
  });

  it("shows NO IMAGE TO METER, not a reading, while nothing is metered", () => {
    render(
      <MeterControl meter="average" reading={null} meteredEv100={null} onMeter={() => undefined} />,
    );
    expect([
      screen.getByText("NO IMAGE TO METER").tagName,
      screen.queryByText(/^EV100/u),
      document.querySelector(".view-label__part"),
    ]).toEqual(["P", null, null]);
  });
});
