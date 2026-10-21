import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { RenderStyle } from "../../view/camera/state";
import type { SlopeBaselineM } from "../../view/terrain/workers/messages";
import { StyleControl } from "./StyleControl";
import { describedText, statusLine } from "../../test/statusLine";

const BOTH = { wireframe: null, photorealistic: null } as const;
const SOFTWARE = {
  wireframe: null,
  photorealistic: "GRAPHICS SOFTWARE ADAPTER: photorealistic style not available",
} as const;

describe("StyleControl", () => {
  it("shows a fault of the graphics in the caution colour's class", () => {
    render(
      <StyleControl
        renderStyle="wireframe"
        refusals={{ wireframe: null, photorealistic: "GRAPHICS STYLE REFUSED: not made" }}
        faulted
        onStyle={() => undefined}
      />,
    );
    expect(screen.getByText(statusLine("GRAPHICS STYLE REFUSED: not made"))).toHaveClass(
      "view-style__reason--fault",
    );
  });

  it("tracks a refusal's state and leaves its sentence untracked, the fault's colour on both", () => {
    render(
      <StyleControl
        renderStyle="wireframe"
        refusals={{ wireframe: null, photorealistic: "GRAPHICS STYLE REFUSED: not made" }}
        faulted
        onStyle={() => undefined}
      />,
    );
    const state = screen.getByText("GRAPHICS STYLE REFUSED:");
    expect([
      state.className,
      state.nextElementSibling?.className,
      state.parentElement?.className,
    ]).toEqual([
      "status-text__state",
      "status-text__sentence",
      "view-style__reason view-style__reason--fault",
    ]);
  });

  it("shows the view's style as the pressed button", () => {
    render(
      <StyleControl
        renderStyle="wireframe"
        refusals={BOTH}
        faulted={false}
        onStyle={() => undefined}
      />,
    );
    expect(screen.getByRole("button", { name: "WIREFRAME" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("button", { name: "PHOTOREALISTIC" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
  });

  it("asks for the photorealistic style when it is offered", async () => {
    const user = userEvent.setup();
    const onStyle = vi.fn<(style: RenderStyle) => void>();
    render(
      <StyleControl renderStyle="wireframe" refusals={BOTH} faulted={false} onStyle={onStyle} />,
    );
    await user.click(screen.getByRole("button", { name: "PHOTOREALISTIC" }));
    expect(onStyle).toHaveBeenCalledWith("photorealistic");
  });

  it("holds the photorealistic style back on a software adapter and says why", async () => {
    const user = userEvent.setup();
    const onStyle = vi.fn<(style: RenderStyle) => void>();
    render(
      <StyleControl
        renderStyle="wireframe"
        refusals={SOFTWARE}
        faulted={false}
        onStyle={onStyle}
      />,
    );
    const photorealistic = screen.getByRole("button", { name: "PHOTOREALISTIC" });
    expect(photorealistic).toHaveAttribute("aria-disabled", "true");
    expect(describedText(photorealistic)).toBe(
      "GRAPHICS SOFTWARE ADAPTER: photorealistic style not available",
    );
    await user.click(photorealistic);
    expect(onStyle).not.toHaveBeenCalled();
  });

  it("shows the key that toggles the style on the group", () => {
    render(
      <StyleControl
        renderStyle="wireframe"
        refusals={BOTH}
        faulted={false}
        onStyle={() => undefined}
      />,
    );
    expect(screen.getByRole("group", { name: "4 STYLE" })).toHaveAttribute(
      "aria-keyshortcuts",
      "4",
    );
  });
});

/** The `BASELINE` row's buttons: each one's name and whether it is pressed. */
function baselineButtons(): Array<[string, string | null]> {
  return within(screen.getByRole("group", { name: "BASELINE" }))
    .getAllByRole("button")
    .map((button) => [button.textContent, button.getAttribute("aria-pressed")]);
}

describe("StyleControl's BASELINE row (R10.T12.a; decision-r10-t12a-scope §3.1)", () => {
  it("offers 2 m, 5 m and 100 m, the chosen one pressed", () => {
    render(
      <StyleControl
        renderStyle="wireframe"
        refusals={BOTH}
        faulted={false}
        onStyle={() => undefined}
        baseline={{ chosen: 5, onBaseline: () => undefined }}
      />,
    );
    expect(baselineButtons()).toEqual([
      ["2 m", "false"],
      ["5 m", "true"],
      ["100 m", "false"],
    ]);
  });

  it("calls back with the metres of the baseline chosen, at once", async () => {
    const user = userEvent.setup();
    const onBaseline = vi.fn<(baselineM: SlopeBaselineM) => void>();
    render(
      <StyleControl
        renderStyle="wireframe"
        refusals={BOTH}
        faulted={false}
        onStyle={() => undefined}
        baseline={{ chosen: 2, onBaseline }}
      />,
    );
    await user.click(screen.getByRole("button", { name: "100 m" }));
    await user.click(screen.getByRole("button", { name: "5 m" }));
    expect(onBaseline.mock.calls).toEqual([[100], [5]]);
  });

  it("sets its buttons as the presets are, display controls underlined while pressed", () => {
    render(
      <StyleControl
        renderStyle="wireframe"
        refusals={BOTH}
        faulted={false}
        onStyle={() => undefined}
        baseline={{ chosen: 2, onBaseline: () => undefined }}
      />,
    );
    const group = screen.getByRole("group", { name: "BASELINE" });
    expect({
      group: group.className,
      legend: group.querySelector("legend")?.className,
      button: within(group).getByRole("button", { name: "2 m" }).className,
      key: group.getAttribute("aria-keyshortcuts"),
    }).toEqual({
      group: "preset-buttons view-style__baseline",
      legend: "field__label view-style__baseline-label",
      button: "control preset-buttons__button view-style__baseline-button",
      key: null,
    });
  });

  it("leaves the row out without its state, as for a view that states no readout", () => {
    render(
      <StyleControl
        renderStyle="wireframe"
        refusals={BOTH}
        faulted={false}
        onStyle={() => undefined}
      />,
    );
    expect([
      screen.queryByRole("group", { name: "BASELINE" }),
      screen.getAllByRole("button").map((button) => button.textContent),
    ]).toEqual([null, ["WIREFRAME", "PHOTOREALISTIC"]]);
  });

  it("comes after the style's buttons and their reason, in the tab order and on the page", async () => {
    const user = userEvent.setup();
    render(
      <StyleControl
        renderStyle="wireframe"
        refusals={SOFTWARE}
        faulted={false}
        onStyle={() => undefined}
        baseline={{ chosen: 2, onBaseline: () => undefined }}
      />,
    );
    const order: Array<string | null> = [];
    for (let step = 0; step < 5; step += 1) {
      // Each tab moves on from where the one before left the focus.
      // oxlint-disable-next-line no-await-in-loop
      await user.tab();
      order.push(document.activeElement?.textContent ?? null);
    }
    const reason = screen.getByText(
      statusLine("GRAPHICS SOFTWARE ADAPTER: photorealistic style not available"),
    );
    const row = screen.getByRole("group", { name: "BASELINE" });
    expect({
      order,
      after: (reason.compareDocumentPosition(row) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0,
    }).toEqual({ order: ["WIREFRAME", "PHOTOREALISTIC", "2 m", "5 m", "100 m"], after: true });
  });
});
