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

const CONSEQUENCE = "Entering a value sets MAN: AUTO resumes only on ENABLE";

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

/** Whether `after` follows `before` in the document. */
function follows(before: Element, after: Element): boolean {
  return (before.compareDocumentPosition(after) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0;
}

/** The panel's reading, `EV100 -1.0 MAN`. */
function reading(): string {
  return screen.getAllByRole("status")[0]?.textContent ?? "";
}

describe("ExposurePanel's MAN field (R07.T13.d)", () => {
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
    const notAvailable = screen.getByText("AUTO NOT AVAILABLE: NO IMAGE TO METER");
    const enable = screen.getByRole("button", { name: "ENABLE" });
    expect([
      iso?.textContent,
      iso !== undefined && follows(iso, field),
      follows(field, notAvailable),
      follows(notAvailable, enable),
    ]).toEqual(["ISO", true, true, true]);
  });
});
