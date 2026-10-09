import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { viewId } from "../../view/camera/state";
import { DEFAULT_EXPOSURE, type ExposureControl } from "../../view/photometry/exposure";
import type { Metering } from "../../view/post/autoExposure";
import type { MeterMode } from "../../view/post/meter";
import { MeterControl } from "./MeterControl";

const AUTO: ExposureControl = { kind: "auto", ev100: 9.6 };

/** The primary view, the exposure's source. */
const SOURCE = viewId("view");

/** A meter holding EV100 9.6, or 9.64 where its own value is shown. */
const METERED: Metering = { kind: "metered", ev100: 9.6 };

/** A meter whose image is not arriving. */
const NO_IMAGE: Metering = { kind: "no-image" };

/** The meter's reading, an `output` set in parts (`readingParts`), by its whole text. */
function readingOutput(text: string): HTMLElement {
  return screen.getByText(
    (_, element) => element?.tagName === "OUTPUT" && element.textContent === text,
  );
}

/** The meter's reason, by its whole text. */
function reasonOf(text: string): HTMLElement {
  return screen.getByText((_, element) => element?.tagName === "P" && element.textContent === text);
}

describe("MeterControl", () => {
  it("shows the exposure with its level, the meter and the view it meters, by its name", () => {
    render(
      <MeterControl
        meter="average"
        exposure={AUTO}
        source={SOURCE}
        metering={METERED}
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
    render(
      <MeterControl
        meter="average"
        exposure={AUTO}
        source={SOURCE}
        metering={METERED}
        onMeter={onMeter}
      />,
    );
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
    render(
      <MeterControl
        meter="lit"
        exposure={AUTO}
        source={SOURCE}
        metering={NO_IMAGE}
        onMeter={onMeter}
      />,
    );
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
      <MeterControl
        meter="lit"
        exposure={AUTO}
        source={SOURCE}
        metering={NO_IMAGE}
        onMeter={() => undefined}
      />,
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
          exposure={control}
          source={SOURCE}
          metering={{ kind: "metered", ev100: 9.64 }}
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
          exposure={control}
          source={SOURCE}
          metering={{ kind: "metered", ev100: 9.64 }}
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
        exposure={{ kind: "inhibited", ev100: 9.6, reason: "operator" }}
        source={SOURCE}
        metering={METERED}
        onMeter={() => undefined}
      />,
    );
    const output = readingOutput("EV100 9.6 INHIBITED · OPERATOR");
    expect(
      [...output.querySelectorAll(".view-label__part")].map((part) => part.textContent),
    ).toEqual(["EV100 9.6 INHIBITED", "OPERATOR"]);
  });

  it("shows NO IMAGE TO METER, not a reading, while no image arrives", () => {
    render(
      <MeterControl
        meter="average"
        exposure={AUTO}
        source={SOURCE}
        metering={NO_IMAGE}
        onMeter={() => undefined}
      />,
    );
    expect([
      reasonOf("NO IMAGE TO METER").tagName,
      screen.queryByText(/^EV100/u),
      document.querySelector(".view-label__part"),
    ]).toEqual(["P", null, null]);
  });
});

describe("MeterControl's own statuses (R07.T16.b)", () => {
  const INHIBITED: ExposureControl = {
    kind: "inhibited",
    ev100: 9.6,
    reason: "nothing_weighed",
    meter: "lit",
  };

  it.each([
    ["lit", "LIT", "NO LIT SIDE: choose AVG, or bring a sunlit body into view"],
    ["dark", "DARK", "NO DARK SIDE: choose AVG, or bring a night side into view"],
    ["average", "AVG", "STAR DISC ONLY: widen the view or turn from the star"],
  ] as const)(
    "says under %s that it weighs nothing, with what to do, in the reading's place",
    (meter, label, words) => {
      render(
        <MeterControl
          meter={meter}
          exposure={INHIBITED}
          source={SOURCE}
          metering={{ kind: "nothing-weighed", meter }}
          onMeter={() => undefined}
        />,
      );
      expect([
        reasonOf(words).tagName,
        screen.queryByText(/^EV100/u),
        screen.queryByText(/NO IMAGE TO METER/u),
      ]).toEqual(["P", null, null]);
      expect(screen.getByRole("button", { name: label })).toHaveAccessibleDescription(words);
    },
  );

  it("describes only the chosen meter with its status, and keeps every meter choosable", async () => {
    const user = userEvent.setup();
    const onMeter = vi.fn<(mode: MeterMode) => void>();
    render(
      <MeterControl
        meter="lit"
        exposure={INHIBITED}
        source={SOURCE}
        metering={{ kind: "nothing-weighed", meter: "lit" }}
        onMeter={onMeter}
      />,
    );
    expect(
      ["AVG", "LIT", "DARK"].map(
        (name) => screen.getByRole("button", { name }).getAttribute("aria-describedby") !== null,
      ),
    ).toEqual([false, true, false]);
    await user.click(screen.getByRole("button", { name: "AVG" }));
    expect(onMeter).toHaveBeenLastCalledWith("average");
  });

  it("holds its status phrase unbroken", () => {
    render(
      <MeterControl
        meter="average"
        exposure={INHIBITED}
        source={SOURCE}
        metering={{ kind: "nothing-weighed", meter: "average" }}
        onMeter={() => undefined}
      />,
    );
    const reason = reasonOf("STAR DISC ONLY: widen the view or turn from the star");
    expect([...reason.querySelectorAll(".view-label__run")].map((run) => run.textContent)).toEqual([
      "STAR DISC ONLY",
    ]);
  });

  it("shows its reading as it stands, with no status, before the first histogram", () => {
    render(
      <MeterControl
        meter="lit"
        exposure={INHIBITED}
        source={SOURCE}
        metering={{ kind: "acquiring" }}
        onMeter={() => undefined}
      />,
    );
    expect([
      readingOutput("EV100 9.6 INHIBITED · NO LIT SIDE").tagName,
      screen.getByRole("status", { name: "SOURCE" }).textContent,
      screen.queryByRole("status", { name: "METERED" }),
      screen.queryByText(/choose AVG|turn from the star|NO IMAGE TO METER/u),
      screen.getByRole("button", { name: "LIT" }).getAttribute("aria-describedby"),
    ]).toEqual(["OUTPUT", "PRIMARY", null, null, null]);
  });

  it("shows METERED missing before the first histogram, where the exposure does not follow it", () => {
    for (const exposure of [
      DEFAULT_EXPOSURE,
      { kind: "inhibited", ev100: 9.6, reason: "operator" } as const,
    ]) {
      const { unmount } = render(
        <MeterControl
          meter="average"
          exposure={exposure}
          source={SOURCE}
          metering={{ kind: "acquiring" }}
          onMeter={() => undefined}
        />,
      );
      const metered = screen.getByRole("status", { name: "METERED" });
      expect([
        metered.textContent,
        metered.querySelector(".readout__missing")?.textContent,
      ]).toEqual(["—", "—"]);
      unmount();
    }
  });

  it("sets its status in a live region, since it comes by itself", () => {
    render(
      <MeterControl
        meter="lit"
        exposure={INHIBITED}
        source={SOURCE}
        metering={{ kind: "nothing-weighed", meter: "lit" }}
        onMeter={() => undefined}
      />,
    );
    expect(
      screen
        .getAllByRole("status")
        .map((status) => status.textContent)
        .includes("NO LIT SIDE: choose AVG, or bring a sunlit body into view"),
    ).toBe(true);
  });
});
