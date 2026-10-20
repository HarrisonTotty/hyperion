/**
 * The `INSTRUMENTS` panel's held slots (plan R10, R10.T12.b; decision-r10-view-block-room §3): one
 * reason line for every slot the stage has no room for, open and held or closed without room, the
 * held slots' `CONTROLS` buttons held back by it, and, folded, the slots held whole stated under its
 * title row. The panel's other rows are tested on the display, in `InstrumentView.test.tsx`.
 */
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { describedText, statusLine } from "../../test/statusLine";
import {
  InstrumentsPanel,
  type InstrumentsPanelFold,
  type InstrumentsPanelSlot,
} from "./InstrumentsPanel";
import type { ViewId } from "../../view/camera/state";
import { instrumentViewId, PRIMARY_NAME, PRIMARY_VIEW_ID } from "./viewNames";

const ONE = instrumentViewId(1);
const TWO = instrumentViewId(2);

/** A slot as the panel shows it: closed with room unless given otherwise. */
function slot(
  id: ViewId,
  name: string,
  standing: Partial<Omit<InstrumentsPanelSlot, "id" | "name">> = {},
): InstrumentsPanelSlot {
  return { id, name, open: false, room: true, held: null, ...standing };
}

/** The panel with `slots`, `CONTROLS` at `PRIMARY`, unfolded unless `fold` is given. */
function renderPanel(
  slots: ReadonlyArray<InstrumentsPanelSlot>,
  options: { readonly fold?: InstrumentsPanelFold; readonly onOperate?: () => void } = {},
): void {
  render(
    <InstrumentsPanel
      fold={options.fold}
      primary={{ id: PRIMARY_VIEW_ID, name: PRIMARY_NAME }}
      slots={slots}
      operated={PRIMARY_VIEW_ID}
      unavailable={false}
      onOpen={() => undefined}
      onClose={() => undefined}
      onOperate={options.onOperate ?? (() => undefined)}
    />,
  );
}

function controlsButton(name: string): HTMLElement {
  return within(screen.getByRole("group", { name: "CONTROLS" })).getByRole("button", { name });
}

function openButton(name: string): HTMLElement {
  return within(screen.getByRole("group", { name })).getByRole("button", { name: "OPEN" });
}

/** The panel's reason lines on show, in no hidden part. */
function reasonsOnShow(): Array<string | null> {
  return [...document.querySelectorAll(".view-instruments-panel__reason")]
    .filter((line) => line.closest("[hidden]") === null)
    .map((line) => line.textContent);
}

const HELD_ONE = "NOT AVAILABLE: no room for INSTRUMENT 1 at this window size";
const FOLDED: InstrumentsPanelFold = { folded: true, onToggle: () => undefined };

describe("the INSTRUMENTS panel's held slots (R10.T12.b)", () => {
  it("states a slot held whole on the line under its rows, describing its OPEN", () => {
    renderPanel([
      slot(ONE, "INSTRUMENT 1", { open: true, held: "whole" }),
      slot(TWO, "INSTRUMENT 2"),
    ]);
    expect(describedText(openButton("INSTRUMENT 1"))).toBe(HELD_ONE);
  });

  it("holds a slot held whole back as the CONTROLS view", () => {
    renderPanel([
      slot(ONE, "INSTRUMENT 1", { open: true, held: "whole" }),
      slot(TWO, "INSTRUMENT 2"),
    ]);
    expect(controlsButton("INSTRUMENT 1")).toHaveAttribute("aria-disabled", "true");
  });

  it("describes a held slot's CONTROLS button by the line", () => {
    renderPanel([
      slot(ONE, "INSTRUMENT 1", { open: true, held: "whole" }),
      slot(TWO, "INSTRUMENT 2"),
    ]);
    expect(describedText(controlsButton("INSTRUMENT 1"))).toBe(HELD_ONE);
  });

  it("does not make a held slot the CONTROLS view when its button is pressed", async () => {
    const user = userEvent.setup();
    const onOperate = vi.fn<() => void>();
    renderPanel(
      [slot(ONE, "INSTRUMENT 1", { open: true, held: "whole" }), slot(TWO, "INSTRUMENT 2")],
      { onOperate },
    );
    await user.click(controlsButton("INSTRUMENT 1"));
    expect(onOperate).not.toHaveBeenCalled();
  });

  it("holds a slot standing as its stub back as the CONTROLS view, described by the line", () => {
    renderPanel([
      slot(ONE, "INSTRUMENT 1", { open: true }),
      slot(TWO, "INSTRUMENT 2", { open: true, room: false, held: "stub" }),
    ]);
    const button = controlsButton("INSTRUMENT 2");
    expect([button.getAttribute("aria-disabled"), describedText(button)]).toEqual([
      "true",
      "NOT AVAILABLE: no room for INSTRUMENT 2 at this window size",
    ]);
  });

  it("gives a held slot and a closed one without room one line", () => {
    renderPanel([
      slot(ONE, "INSTRUMENT 1", { open: true, room: false, held: "whole" }),
      slot(TWO, "INSTRUMENT 2", { room: false }),
    ]);
    expect(
      screen.getByText(
        statusLine("NOT AVAILABLE: no room for INSTRUMENT 1 and INSTRUMENT 2 at this window size"),
      ),
    ).toBeInTheDocument();
  });

  it("gives two held slots one line", () => {
    renderPanel([
      slot(ONE, "INSTRUMENT 1", { open: true, room: false, held: "whole" }),
      slot(TWO, "INSTRUMENT 2", { open: true, room: false, held: "whole" }),
    ]);
    expect(reasonsOnShow()).toEqual([
      "NOT AVAILABLE: no room for INSTRUMENT 1 and INSTRUMENT 2 at this window size",
    ]);
  });

  it("leaves a standing slot's CONTROLS button acting", () => {
    renderPanel([slot(ONE, "INSTRUMENT 1", { open: true }), slot(TWO, "INSTRUMENT 2")]);
    expect(controlsButton("INSTRUMENT 1")).not.toHaveAttribute("aria-disabled");
  });
});

describe("the folded INSTRUMENTS panel's held slots (R10.T12.b)", () => {
  it("states a slot held whole under its title row, and not a closed slot's OPEN held back", () => {
    renderPanel(
      [
        slot(ONE, "INSTRUMENT 1", { open: true, room: false, held: "whole" }),
        slot(TWO, "INSTRUMENT 2", { room: false }),
      ],
      { fold: FOLDED },
    );
    expect(reasonsOnShow()).toEqual([HELD_ONE]);
  });

  it("sets the line under the title row, out of the folded body", () => {
    renderPanel(
      [slot(ONE, "INSTRUMENT 1", { open: true, held: "whole" }), slot(TWO, "INSTRUMENT 2")],
      { fold: FOLDED },
    );
    expect(
      screen.getByText(statusLine(HELD_ONE), { selector: ".view-instruments-panel__standing" }),
    ).toBeInTheDocument();
  });

  it("states nothing under its title row for a slot standing as its stub, which the stage shows", () => {
    renderPanel(
      [
        slot(ONE, "INSTRUMENT 1", { open: true }),
        slot(TWO, "INSTRUMENT 2", { open: true, room: false, held: "stub" }),
      ],
      { fold: FOLDED },
    );
    expect(reasonsOnShow()).toEqual([]);
  });

  it("states nothing under its title row unfolded", () => {
    renderPanel(
      [slot(ONE, "INSTRUMENT 1", { open: true, held: "whole" }), slot(TWO, "INSTRUMENT 2")],
      { fold: { folded: false, onToggle: () => undefined } },
    );
    expect(document.querySelector(".view-instruments-panel__standing")).toBeNull();
  });
});
