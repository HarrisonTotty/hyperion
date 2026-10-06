import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import {
  DEFAULT_EXPOSURE,
  type ExposureControl,
  programTriple,
  VIEW_CAMERA,
} from "../../view/photometry/exposure";
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

  it("wraps each label with its own value as one member, in the setting's order (R07.T19.b)", () => {
    // The compact layout sets two members a row; the captures measure the spacing.
    renderAt({ kind: "auto", ev100: 9.6 });
    expect(
      screen.getAllByRole("term").map((term) => {
        const member = term.parentElement;
        const value = member?.querySelector("dd");
        return [
          term.textContent,
          member?.classList.contains("view-exposure__member"),
          member?.children.length,
          value === undefined || value === null ? null : value.textContent,
        ];
      }),
    ).toEqual([
      ["APERTURE", true, 2, "f/1.4"],
      ["SHUTTER", true, 2, "0.00253 s"],
      ["ND", true, 2, "CLEAR"],
      ["ISO", true, 2, "100"],
    ]);
  });
});

/** The parts of the panel's reading, as `readingParts` sets them. */
function readingPartsShown(): string[] {
  const output = screen.getAllByRole("status")[0];
  if (output === undefined) {
    throw new Error("the panel has no reading");
  }
  return [...output.querySelectorAll(".view-label__part")].map((part) => part.textContent);
}

describe("ExposurePanel's reading (R07.T19.b's follow-up)", () => {
  it("sets the trapped reading in its two parts, so that it breaks at its middle dot", () => {
    renderAt({ kind: "inhibited", ev100: 11.7, reason: "no_image_to_meter" });
    expect([screen.getAllByRole("status")[0]?.textContent, readingPartsShown()]).toEqual([
      "EV100 11.7 INHIBITED · NO IMAGE TO METER",
      ["EV100 11.7 INHIBITED", "NO IMAGE TO METER"],
    ]);
  });

  it("sets a reading without a middle dot as one part", () => {
    renderAt({ kind: "auto", ev100: 9.6 });
    expect([screen.getAllByRole("status")[0]?.textContent, readingPartsShown()]).toEqual([
      "EV100 9.6 AUTO",
      ["EV100 9.6 AUTO"],
    ]);
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

const CONSEQUENCE = "An entry sets MAN: AUTO resumes only on ENABLE";

const REFUSAL = "EV100 INVALID: enter -14.0 to 42.0";

interface HarnessProps {
  readonly initial: ExposureControl;
  readonly meteredEv100: number | null;
  readonly onChange: (exposure: ExposureControl) => void;
}

/** The panel over a display's exposure state, as `ViewDisplay` holds it. */
function Harness({ initial, meteredEv100, onChange }: HarnessProps) {
  const [exposure, setExposure] = useState(initial);
  return (
    <ExposurePanel
      exposure={exposure}
      meteredEv100={meteredEv100}
      onChange={(next) => {
        onChange(next);
        setExposure(next);
      }}
    />
  );
}

/** Renders the panel over its own state at a control, with the commands it accepts recorded. */
function renderEntry(initial: ExposureControl, meteredEv100: number | null = 3) {
  const user = userEvent.setup();
  const onChange = vi.fn<(exposure: ExposureControl) => void>();
  render(<Harness initial={initial} meteredEv100={meteredEv100} onChange={onChange} />);
  const field = screen.getByRole("textbox", { name: "MAN" });
  if (!(field instanceof HTMLInputElement)) {
    throw new TypeError("the MAN field is not an input");
  }
  return { user, onChange, field };
}

/**
 * The note or line whose whole text is `text`: a paragraph or a span, which may hold a status phrase
 * in a run of its own.
 */
function noteOf(text: string): HTMLElement {
  return screen.getByText(
    (_, element) =>
      (element?.tagName === "P" || element?.tagName === "SPAN") && element.textContent === text,
  );
}

/** The runs a note holds unbroken, by their text. */
function runsOf(note: HTMLElement): Array<string | null> {
  return [...note.querySelectorAll(".view-label__run")].map((run) => run.textContent);
}

/** Whether `after` follows `before` in the document. */
function follows(before: Element, after: Element): boolean {
  return (before.compareDocumentPosition(after) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0;
}

/** The panel's reading, `EV100 -1.0 MAN`. */
function reading(): string {
  return screen.getAllByRole("status")[0]?.textContent ?? "";
}

describe("ExposurePanel's MAN field (R07.T13.d)", () => {
  it("states the consequence of an entry in the words that fit one line of the compact column", () => {
    renderEntry({ kind: "auto", ev100: 9.6 });
    expect(screen.getByText(/sets MAN/u)).toHaveTextContent(
      /^An entry sets MAN: AUTO resumes only on ENABLE$/u,
    );
  });

  it("reads — under AUTO, with its unit, its span and the consequence of an entry", () => {
    const { field } = renderEntry({ kind: "auto", ev100: 9.6 });
    expect(field).toHaveValue("—");
    expect(field).toHaveAccessibleDescription(`EV100 -14.0 to 42.0 ${CONSEQUENCE}`);
    expect(screen.getByText(CONSEQUENCE).tagName).toBe("P");
  });

  it("fills with the exposure as it stands on focus, and takes MAN there on Enter", async () => {
    const { user, field, onChange } = renderEntry({ kind: "auto", ev100: 9.64 });
    // Tab, then Enter: MAN at the exposure as it stands.
    await user.tab();
    expect(field).toHaveFocus();
    expect(field).toHaveValue("9.6");
    expect([field.selectionStart, field.selectionEnd]).toEqual([0, 3]);
    await user.keyboard("{Enter}");
    expect(onChange).toHaveBeenLastCalledWith({
      kind: "manual",
      triple: programTriple(VIEW_CAMERA, 9.6),
    });
    expect(reading()).toBe("EV100 9.6 MAN");
  });

  it("replaces the fill with what is typed when focused by a press, and enters it on Enter", async () => {
    const { user, field } = renderEntry({ kind: "auto", ev100: 9.6 });
    await user.click(field);
    await user.keyboard("8.6{Enter}");
    expect([reading(), field.value]).toEqual(["EV100 8.6 MAN", "8.6"]);
  });

  it("enters a typed value when it is left, with either minus sign, kept to one decimal", async () => {
    const { user, field } = renderEntry({ kind: "auto", ev100: 9.6 });
    await user.click(field);
    await user.keyboard("−3.14");
    await user.tab();
    expect(reading()).toBe("EV100 -3.1 MAN");
  });

  it("enters nothing when tabbed through, and returns to —", async () => {
    const { user, field, onChange } = renderEntry({ kind: "auto", ev100: 9.6 });
    await user.tab();
    expect(field).toHaveFocus();
    await user.tab();
    expect([onChange.mock.calls.length, field.value, reading()]).toEqual([
      0,
      "—",
      "EV100 9.6 AUTO",
    ]);
  });

  it("refuses an EV100 outside its span, saying what is valid, and leaves the exposure", async () => {
    const { user, field, onChange } = renderEntry({ kind: "auto", ev100: 9.6 });
    await user.click(field);
    await user.keyboard("-14.1{Enter}");
    expect([field.getAttribute("aria-invalid"), onChange.mock.calls.length, reading()]).toEqual([
      "true",
      0,
      "EV100 9.6 AUTO",
    ]);
    expect(field).toHaveAccessibleDescription(`EV100 -14.0 to 42.0 ${CONSEQUENCE} ${REFUSAL}`);
  });

  it("refuses an entry that is not a number, keeping what was typed", async () => {
    const { user, field, onChange } = renderEntry({ kind: "auto", ev100: 9.6 });
    await user.click(field);
    await user.keyboard("abc");
    await user.tab();
    expect([field.value, field.getAttribute("aria-invalid"), onChange.mock.calls.length]).toEqual([
      "abc",
      "true",
      0,
    ]);
    expect(screen.getByText(REFUSAL)).toBeInTheDocument();
  });

  it("takes the span's ends, -14.0 and 42.0", async () => {
    const { user, field } = renderEntry({ kind: "auto", ev100: 9.6 });
    await user.click(field);
    await user.keyboard("-14{Enter}");
    expect(reading()).toBe("EV100 -14.0 MAN");
    await user.tripleClick(field);
    await user.keyboard("+42.04{Enter}");
    expect(reading()).toBe("EV100 42.0 MAN");
  });

  it("sets MAN with nothing to meter, where ENABLE stays held back", async () => {
    const { user, field } = renderEntry(
      { kind: "inhibited", ev100: 4, reason: "no_image_to_meter" },
      null,
    );
    expect(field).not.toHaveAttribute("aria-disabled");
    await user.click(field);
    await user.keyboard("2{Enter}");
    expect(reading()).toBe("EV100 2.0 MAN");
    expect(screen.getByRole("button", { name: "ENABLE" })).toHaveAccessibleDescription(
      "NO IMAGE TO METER",
    );
  });

  it("sets MAN from the operator's inhibit, which then reads MAN", async () => {
    const { user, field } = renderEntry({ kind: "inhibited", ev100: 4, reason: "operator" });
    expect(screen.getByText(CONSEQUENCE).tagName).toBe("P");
    await user.click(field);
    await user.keyboard("5.5{Enter}");
    expect(reading()).toBe("EV100 5.5 MAN");
  });

  it("drops a refused entry when ENABLE sets the exposure elsewhere", async () => {
    const { user, field } = renderEntry(DEFAULT_EXPOSURE);
    await user.click(field);
    await user.keyboard("99");
    await user.tab();
    const refused = [field.value, field.getAttribute("aria-invalid")];
    await user.click(screen.getByRole("button", { name: "ENABLE" }));
    expect([refused, field.value, field.getAttribute("aria-invalid"), reading()]).toEqual([
      ["99", "true"],
      "—",
      null,
      "EV100 3.0 AUTO",
    ]);
    expect(screen.queryByText(REFUSAL)).toBeNull();
  });

  it("keeps the fill selected when a press focuses the field, and lets a later press place the caret", async () => {
    const { user, field } = renderEntry({ kind: "auto", ev100: 9.6 });
    const prevented: boolean[] = [];
    const onMouseUp = (event: MouseEvent): void => {
      prevented.push(event.defaultPrevented);
    };
    document.addEventListener("mouseup", onMouseUp);
    await user.click(field);
    await user.click(field);
    document.removeEventListener("mouseup", onMouseUp);
    expect(prevented).toEqual([true, false]);
  });

  it("shows MAN's value under MAN, without the consequence line", () => {
    const { field } = renderEntry(DEFAULT_EXPOSURE);
    expect(field).toHaveValue("-1.0");
    expect(screen.queryByText(CONSEQUENCE)).toBeNull();
    expect(field).toHaveAccessibleDescription("EV100 -14.0 to 42.0");
  });

  it("stands after the camera's setting, before AUTO NOT AVAILABLE and the commands", () => {
    const { field } = renderEntry(
      { kind: "inhibited", ev100: 4, reason: "no_image_to_meter" },
      null,
    );
    const iso = screen.getAllByRole("term").at(-1);
    const notAvailable = noteOf("AUTO NOT AVAILABLE: NO IMAGE TO METER");
    const enable = screen.getByRole("button", { name: "ENABLE" });
    expect([
      iso?.textContent,
      iso !== undefined && follows(iso, field),
      follows(field, notAvailable),
      follows(notAvailable, enable),
    ]).toEqual(["ISO", true, true, true]);
  });
});

describe("ExposurePanel's MAN field, its way out (R07.T13.d's follow-up)", () => {
  it("drops a refused entry on Escape, taking the fill again, and then enters nothing on leaving", async () => {
    const { user, field, onChange } = renderEntry({ kind: "auto", ev100: 9.6 });
    await user.click(field);
    await user.keyboard("abc");
    await user.tab();
    const refused = [field.value, field.getAttribute("aria-invalid")];
    await user.click(field);
    await user.keyboard("{Escape}");
    expect([refused, field.value, [field.selectionStart, field.selectionEnd]]).toEqual([
      ["abc", "true"],
      "9.6",
      [0, 3],
    ]);
    expect([field.getAttribute("aria-invalid"), screen.queryByText(REFUSAL)]).toEqual([null, null]);
    expect(field).toHaveFocus();
    await user.tab();
    expect([field.value, onChange.mock.calls.length, reading()]).toEqual([
      "—",
      0,
      "EV100 9.6 AUTO",
    ]);
  });

  it("changes nothing on Escape with nothing typed", async () => {
    const { user, field, onChange } = renderEntry({ kind: "auto", ev100: 9.6 });
    await user.tab();
    await user.keyboard("{Escape}");
    expect([
      field.value,
      [field.selectionStart, field.selectionEnd],
      field.getAttribute("aria-invalid"),
      onChange.mock.calls.length,
      reading(),
    ]).toEqual(["9.6", [0, 3], null, 0, "EV100 9.6 AUTO"]);
  });

  it("drops a refused entry emptied and left, entering nothing", async () => {
    const { user, field, onChange } = renderEntry({ kind: "auto", ev100: 9.6 });
    await user.click(field);
    await user.keyboard("99{Enter}");
    const refused = field.getAttribute("aria-invalid");
    await user.clear(field);
    await user.tab();
    expect([refused, field.value, field.getAttribute("aria-invalid")]).toEqual(["true", "—", null]);
    expect([screen.queryByText(REFUSAL), onChange.mock.calls.length]).toEqual([null, 0]);
  });

  it("takes the fill again, selected, when the emptied field is entered", async () => {
    const { user, field, onChange } = renderEntry({ kind: "auto", ev100: 9.6 });
    await user.tab();
    await user.keyboard("{Backspace}");
    const emptied = field.value;
    await user.keyboard("{Enter}");
    expect([
      emptied,
      field.value,
      [field.selectionStart, field.selectionEnd],
      field.getAttribute("aria-invalid"),
      onChange.mock.calls.length,
    ]).toEqual(["", "9.6", [0, 3], null, 0]);
  });

  it("enters nothing for spaces only, and returns to — on leaving", async () => {
    const { user, field, onChange } = renderEntry({ kind: "auto", ev100: 9.6 });
    await user.click(field);
    await user.keyboard("   ");
    await user.tab();
    expect([field.value, field.getAttribute("aria-invalid"), onChange.mock.calls.length]).toEqual([
      "—",
      null,
      0,
    ]);
  });

  it("keeps MAN's value when the field is emptied and left under MAN", async () => {
    const { user, field, onChange } = renderEntry(DEFAULT_EXPOSURE);
    await user.clear(field);
    await user.tab();
    expect([field.value, field.getAttribute("aria-invalid"), onChange.mock.calls.length]).toEqual([
      "-1.0",
      null,
      0,
    ]);
  });

  it("fills with the exposure beyond the span, refuses it there and drops it on leaving", async () => {
    const { user, field, onChange } = renderEntry({ kind: "auto", ev100: -15.3 });
    await user.tab();
    const filled = field.value;
    await user.keyboard("{Enter}");
    expect([filled, field.value, field.getAttribute("aria-invalid"), reading()]).toEqual([
      "-15.3",
      "-15.3",
      "true",
      "EV100 -15.3 AUTO",
    ]);
    await user.tab();
    expect([field.value, field.getAttribute("aria-invalid"), screen.queryByText(REFUSAL)]).toEqual([
      "—",
      null,
      null,
    ]);
    expect(onChange).not.toHaveBeenCalled();
  });

  it("enters a fill at the span's foot, -14.04 shown and taken as -14.0", async () => {
    const { user, field } = renderEntry({ kind: "auto", ev100: -14.04 });
    await user.tab();
    const filled = field.value;
    await user.keyboard("{Enter}");
    expect([filled, reading()]).toEqual(["-14.0", "EV100 -14.0 MAN"]);
  });
});

const INHIBIT_CONSEQUENCE = "Then AUTO resumes only on ENABLE";

/** Renders the panel at a control and returns its `INHIBIT` button. */
function inhibitAt(exposure: ExposureControl, meteredEv100: number | null = 3): HTMLElement {
  render(
    <ExposurePanel exposure={exposure} meteredEv100={meteredEv100} onChange={() => undefined} />,
  );
  return screen.getByRole("button", { name: "INHIBIT" });
}

describe("ExposurePanel's INHIBIT consequence (R07.T13.d's follow-up)", () => {
  it("states it beside INHIBIT under AUTO, as the offered button's description", () => {
    const inhibit = inhibitAt({ kind: "auto", ev100: 9.6 });
    expect(inhibit).not.toHaveAttribute("aria-disabled");
    expect(inhibit).toHaveAccessibleDescription(INHIBIT_CONSEQUENCE);
    expect(screen.getByText(INHIBIT_CONSEQUENCE)).toBeVisible();
  });

  it("states it under a system inhibit, which INHIBIT takes over", () => {
    const inhibit = inhibitAt({ kind: "inhibited", ev100: 4, reason: "no_image_to_meter" }, null);
    expect(inhibit).toHaveAccessibleDescription(INHIBIT_CONSEQUENCE);
    expect(screen.getByText(INHIBIT_CONSEQUENCE)).toBeVisible();
  });

  it("gives way to the held-back reason alone under MAN", () => {
    const inhibit = inhibitAt(DEFAULT_EXPOSURE);
    expect(inhibit).toHaveAccessibleDescription("NOT AVAILABLE: the exposure is MAN");
    expect(screen.queryByText(INHIBIT_CONSEQUENCE)).toBeNull();
  });

  it("leaves the MAN field's line as it reads", () => {
    inhibitAt({ kind: "auto", ev100: 9.6 });
    expect(screen.getByRole("textbox", { name: "MAN" })).toHaveAccessibleDescription(
      `EV100 -14.0 to 42.0 ${CONSEQUENCE}`,
    );
  });
});

const OPERATOR_HELD = "NOT AVAILABLE: the exposure is INHIBITED · OPERATOR";

const OPERATOR: ExposureControl = { kind: "inhibited", ev100: 4, reason: "operator" };

describe("ExposurePanel's INHIBIT under the operator's own inhibit (R07.T19.d)", () => {
  it("holds INHIBIT back, saying the exposure is INHIBITED · OPERATOR, in its consequence's place", () => {
    const inhibit = inhibitAt(OPERATOR);
    expect([
      inhibit.getAttribute("aria-disabled"),
      screen.queryByText(INHIBIT_CONSEQUENCE),
    ]).toEqual(["true", null]);
    expect(inhibit).toHaveAccessibleDescription(OPERATOR_HELD);
  });

  it("changes nothing when the held-back INHIBIT is pressed", async () => {
    const { user, onChange } = renderEntry(OPERATOR);
    await user.click(screen.getByRole("button", { name: "INHIBIT" }));
    expect([onChange.mock.calls.length, reading()]).toEqual([0, "EV100 4.0 INHIBITED · OPERATOR"]);
  });

  it("offers ENABLE there beside a metered value, with nothing beside it", () => {
    inhibitAt(OPERATOR);
    const enable = screen.getByRole("button", { name: "ENABLE" });
    expect([enable.getAttribute("aria-disabled"), enable.getAttribute("aria-describedby")]).toEqual(
      [null, null],
    );
  });

  it("holds ENABLE back there by NO IMAGE TO METER while nothing is metered", () => {
    inhibitAt(OPERATOR, null);
    const enable = screen.getByRole("button", { name: "ENABLE" });
    expect(enable).toHaveAttribute("aria-disabled", "true");
    expect(enable).toHaveAccessibleDescription("NO IMAGE TO METER");
  });

  it("sets the level phrase unbroken, so that the note breaks before it and never at its middle dot", () => {
    inhibitAt(OPERATOR);
    expect(runsOf(noteOf(OPERATOR_HELD))).toEqual(["INHIBITED · OPERATOR"]);
  });

  it("holds NO IMAGE TO METER unbroken in AUTO NOT AVAILABLE and beside ENABLE", () => {
    inhibitAt({ kind: "inhibited", ev100: 4, reason: "no_image_to_meter" }, null);
    // ENABLE's row: the button and the reason beside it.
    const enableRow = screen.getByRole("button", { name: "ENABLE" }).parentElement;
    expect([
      runsOf(noteOf("AUTO NOT AVAILABLE: NO IMAGE TO METER")),
      enableRow === null ? null : runsOf(enableRow),
    ]).toEqual([["NO IMAGE TO METER"], ["NO IMAGE TO METER"]]);
  });
});
