import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { viewId } from "../../view/camera/state";
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

describe("MeterControl", () => {
  it("shows the exposure with its level, the meter and the view it meters", () => {
    render(<MeterControl meter="average" reading={READING} onMeter={() => undefined} />);
    expect(screen.getByText("EV100 9.6 AUTO")).toBeInTheDocument();
    expect(screen.getByRole("status", { name: "METER" })).toHaveTextContent("AVG");
    expect(screen.getByRole("status", { name: "SOURCE" })).toHaveTextContent("VIEW");
    expect(screen.getByRole("button", { name: "AVG" })).toHaveAttribute("aria-pressed", "true");
  });

  it("selects each meter from the keyboard", async () => {
    const user = userEvent.setup();
    const onMeter = vi.fn<(mode: MeterMode) => void>();
    render(<MeterControl meter="average" reading={READING} onMeter={onMeter} />);
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
    render(<MeterControl meter="lit" reading={null} onMeter={onMeter} />);
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
});
