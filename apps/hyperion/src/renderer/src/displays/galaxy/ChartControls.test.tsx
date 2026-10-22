import type { MassLayer } from "@hyperion/protocol";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactElement } from "react";
import { describe, expect, it, vi } from "vitest";

import { aCensus } from "../../test/galaxyFixtures";
import { ChartControls } from "./ChartControls";
import { layerBands, type StarFilter } from "./chartModel";
import { describedText, statusLine } from "../../test/statusLine";

const BANDS = layerBands(aCensus().layers);

type ControlsProps = Parameters<typeof ChartControls>[0];

/**
 * Renders the controls, inside a page of `pageClass` where one is given (`local-chart
 * local-chart--compact`), as the `LOCAL CHART` page holds them.
 */
function renderControls(overrides: Partial<ControlsProps> = {}, pageClass?: string) {
  const onQueryRadius = vi.fn<(radiusLy: number) => void>();
  const onMinLayer = vi.fn<(layer: MassLayer) => void>();
  const onDriveRange = vi.fn<(rangeLy: number) => void>();
  const onTime = vi.fn<(timeYr: number) => void>();
  const onStarFilter = vi.fn<(filter: StarFilter) => void>();
  const props: ControlsProps = {
    radiusChoiceLy: null,
    queryRadiusLy: 50,
    onQueryRadius,
    minLayer: "a",
    onMinLayer,
    driveRangeLy: 50,
    onDriveRange,
    timeYr: 0,
    onTime,
    bands: BANDS,
    starFilter: "all",
    onStarFilter,
    heldBack: null,
    ...overrides,
  };
  const page = (controls: ReactElement): ReactElement =>
    pageClass === undefined ? controls : <div className={pageClass}>{controls}</div>;
  const { rerender } = render(page(<ChartControls {...props} />));
  /** Renders the controls again with some props changed, as the display would. */
  const update = (changes: Partial<ControlsProps>): void => {
    rerender(page(<ChartControls {...props} {...changes} />));
  };
  return { onQueryRadius, onMinLayer, onDriveRange, onTime, onStarFilter, update };
}

/** A number field by its label: `DRIVE RANGE` or `CHART TIME`. */
function numberField(name: "DRIVE RANGE" | "CHART TIME"): HTMLInputElement {
  const element = screen.getByRole("textbox", { name });
  if (!(element instanceof HTMLInputElement)) {
    throw new Error(`the ${name} field is not an input`);
  }
  return element;
}

function radius(): HTMLSelectElement {
  const select = screen.getByLabelText("QUERY RADIUS");
  if (!(select instanceof HTMLSelectElement)) {
    throw new Error("QUERY RADIUS is not a select");
  }
  return select;
}

describe("ChartControls", () => {
  it("offers every query radius from 0.01 ly to 500 ly", () => {
    renderControls();

    const options = within(radius())
      .getAllByRole("option")
      .map((option) => option.textContent);

    expect(options).toHaveLength(15);
    expect(options[0]).toBe("0.01 ly");
    expect(options.at(-1)).toBe("500 ly");
  });

  it("reports the radius the operator chooses", async () => {
    const user = userEvent.setup();
    const { onQueryRadius } = renderControls();

    await user.selectOptions(radius(), "20");

    expect(onQueryRadius).toHaveBeenCalledWith(20);
  });

  it("follows the drive range while the operator has not chosen a radius", () => {
    renderControls({ radiusChoiceLy: null, driveRangeLy: 80, queryRadiusLy: 100 });

    expect(radius()).toHaveValue("100");
  });

  it("keeps a radius the operator chose when the drive range changes", () => {
    renderControls({ radiusChoiceLy: 20, driveRangeLy: 80, queryRadiusLy: 20 });

    expect(radius()).toHaveValue("20");
  });

  it("starts with the radius the default drive range asks for", () => {
    renderControls();

    expect(radius()).toHaveValue("50");
  });

  it("names the mass floors by the census's bands below the two substellar floors, the lightest ALL", () => {
    renderControls();

    const group = screen.getByRole("group", { name: "MIN MASS solar masses" });
    // The planets' floor draws its own `M⊕`: the letter is text, the sign an SVG.
    expect(
      within(group)
        .getAllByRole("radio")
        .map((option) => option.closest("label")?.textContent),
    ).toEqual(["ALL 0.33 M", "0.012", "0.08", "0.5", "0.75", "2.5", "8"]);
    // 0.08 no longer reads ALL; the lowest step does, and says its unit (plan 13, P13.T8.c).
    expect(screen.getByRole("radio", { name: "ALL 0.33 Earth masses" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "0.08" })).toBeInTheDocument();
  });

  it("names the stellar floors by their layers until the first census arrives", () => {
    renderControls({ bands: null });

    expect(screen.getByRole("radio", { name: "A" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "E" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "0.012" })).toBeInTheDocument();
  });

  it("reports each substellar layer when its floor is chosen, from the keyboard too", async () => {
    const user = userEvent.setup();
    const { onMinLayer } = renderControls();

    await user.click(screen.getByRole("radio", { name: "0.012" }));
    expect(onMinLayer).toHaveBeenLastCalledWith("brown_dwarf");
    // Arrow keys move within the radio group, as they do between the stellar floors.
    await user.keyboard("{ArrowUp}");
    expect(onMinLayer).toHaveBeenLastCalledWith("rogue_planet");
  });

  it("reports layer C when the floor 0.75 is chosen", async () => {
    const user = userEvent.setup();
    const { onMinLayer } = renderControls();

    await user.click(screen.getByRole("radio", { name: "0.75" }));

    expect(onMinLayer).toHaveBeenCalledWith("c");
  });

  it("enters a drive range when the field is left", async () => {
    const user = userEvent.setup();
    const { onDriveRange } = renderControls();

    await user.clear(screen.getByLabelText("DRIVE RANGE"));
    await user.type(screen.getByLabelText("DRIVE RANGE"), "80");
    await user.tab();

    expect(onDriveRange).toHaveBeenCalledWith(80);
  });

  it("refuses a drive range outside the steps it offers and reports nothing", async () => {
    const user = userEvent.setup();
    const { onDriveRange } = renderControls();

    await user.clear(screen.getByLabelText("DRIVE RANGE"));
    await user.type(screen.getByLabelText("DRIVE RANGE"), "900{Enter}");

    expect(
      screen.getByText(statusLine("DRIVE RANGE INVALID: enter 0.01 to 500 ly")),
    ).toBeInTheDocument();
    expect(onDriveRange).not.toHaveBeenCalled();
  });

  it("enters a chart time with Enter", async () => {
    const user = userEvent.setup();
    const { onTime } = renderControls();

    await user.clear(screen.getByLabelText("CHART TIME"));
    await user.type(screen.getByLabelText("CHART TIME"), "12.5{Enter}");

    expect(onTime).toHaveBeenCalledWith(12.5);
  });

  it("refuses a chart time outside the clock window and reports nothing", async () => {
    const user = userEvent.setup();
    const { onTime } = renderControls();

    await user.clear(screen.getByLabelText("CHART TIME"));
    await user.type(screen.getByLabelText("CHART TIME"), "1001{Enter}");

    expect(
      screen.getByText(statusLine("CHART TIME INVALID: enter -1000 to 1000 yr")),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("CHART TIME")).toHaveAttribute("aria-invalid", "true");
    expect(onTime).not.toHaveBeenCalled();
  });

  it("drops a refused drive range on Escape, showing the drive range again", async () => {
    const user = userEvent.setup();
    const { onDriveRange } = renderControls();
    const field = numberField("DRIVE RANGE");
    await user.clear(field);
    await user.type(field, "900{Enter}");

    await user.keyboard("{Escape}");
    await user.tab();

    expect(field).toHaveValue("50");
    expect(field).not.toHaveAttribute("aria-invalid");
    expect(screen.queryByText(statusLine("DRIVE RANGE INVALID: enter 0.01 to 500 ly"))).toBeNull();
    expect(onDriveRange).not.toHaveBeenCalled();
  });

  it("enters nothing for a chart time emptied and left, and refuses nothing", async () => {
    const user = userEvent.setup();
    const { onTime } = renderControls({ timeYr: 12.5 });
    await user.clear(numberField("CHART TIME"));

    await user.tab();

    expect(numberField("CHART TIME")).toHaveValue("+12.50");
    expect(numberField("CHART TIME")).not.toHaveAttribute("aria-invalid");
    expect(screen.queryByText(statusLine("CHART TIME INVALID: enter -1000 to 1000 yr"))).toBeNull();
    expect(onTime).not.toHaveBeenCalled();
  });

  it("enters nothing for a drive range emptied with Enter", async () => {
    const user = userEvent.setup();
    const { onDriveRange } = renderControls();
    await user.clear(numberField("DRIVE RANGE"));

    await user.keyboard("{Enter}");

    expect(numberField("DRIVE RANGE")).toHaveValue("50");
    expect(numberField("DRIVE RANGE")).not.toHaveAttribute("aria-invalid");
    expect(onDriveRange).not.toHaveBeenCalled();
  });

  it("enters nothing for a chart time of spaces only, and refuses nothing", async () => {
    const user = userEvent.setup();
    const { onTime } = renderControls({ timeYr: 12.5 });
    await user.clear(numberField("CHART TIME"));

    await user.type(numberField("CHART TIME"), "   {Enter}");

    expect(numberField("CHART TIME")).toHaveValue("+12.50");
    expect(numberField("CHART TIME")).not.toHaveAttribute("aria-invalid");
    expect(onTime).not.toHaveBeenCalled();
  });

  it("selects the drive range a refusal's Escape restores, so that typing replaces it", async () => {
    const user = userEvent.setup();
    const { onDriveRange } = renderControls();
    const field = numberField("DRIVE RANGE");
    await user.clear(field);
    await user.type(field, "900{Enter}");

    await user.keyboard("{Escape}");
    const selection = [field.selectionStart, field.selectionEnd];
    await user.keyboard("5");

    expect([selection, field.value]).toEqual([[0, "50".length], "5"]);
    expect(onDriveRange).not.toHaveBeenCalled();
  });

  it("selects the drive range again when the emptied field is entered with Enter", async () => {
    const user = userEvent.setup();
    const { onDriveRange } = renderControls();
    const field = numberField("DRIVE RANGE");
    await user.clear(field);

    await user.keyboard("{Enter}");
    const selection = [field.selectionStart, field.selectionEnd];
    await user.keyboard("5");

    expect([selection, field.value]).toEqual([[0, "50".length], "5"]);
    expect(onDriveRange).not.toHaveBeenCalled();
  });

  it("selects the chart time on Escape with nothing typed, wherever the caret was", async () => {
    const user = userEvent.setup();
    renderControls({ timeYr: 12.5 });
    const field = numberField("CHART TIME");
    await user.pointer({ keys: "[MouseLeft]", target: field, offset: 2 });

    await user.keyboard("{Escape}");

    expect([field.selectionStart, field.selectionEnd]).toEqual([0, "+12.50".length]);
  });

  it("enters what is typed after Escape on leaving, not the chart time with it appended", async () => {
    const user = userEvent.setup();
    const { onTime } = renderControls({ timeYr: 12.5 });
    await user.click(numberField("CHART TIME"));
    await user.keyboard("9{Escape}3");

    await user.tab();

    expect(onTime.mock.calls).toEqual([[3]]);
  });

  it("drops a drive range emptied before the link's hold arrived when it is left", async () => {
    const user = userEvent.setup();
    const { onDriveRange, update } = renderControls();
    await user.clear(numberField("DRIVE RANGE"));
    update({ heldBack: "NO CARRIER" });

    await user.tab();

    expect(numberField("DRIVE RANGE")).toHaveValue("50");
    expect(onDriveRange).not.toHaveBeenCalled();
  });

  it("shows the chart time in universe time", () => {
    renderControls({ timeYr: 12.5 });

    expect(screen.getByLabelText("CHART TIME")).toHaveValue("+12.50");
    expect(screen.getByText("UT")).toBeInTheDocument();
  });

  it("holds every control back with the link's reason while the link is down", async () => {
    const user = userEvent.setup();
    const { onMinLayer, onQueryRadius, onTime } = renderControls({ heldBack: "NO CARRIER" });

    await user.click(screen.getByRole("radio", { name: "0.75" }));
    await user.clear(screen.getByLabelText("CHART TIME"));

    expect(screen.getByLabelText("QUERY RADIUS")).toHaveAccessibleDescription("NO CARRIER");
    expect(screen.getByRole("radio", { name: "0.75" })).toHaveAttribute("aria-disabled", "true");
    expect(onMinLayer).not.toHaveBeenCalled();
    expect(onQueryRadius).not.toHaveBeenCalled();
    expect(onTime).not.toHaveBeenCalled();
  });

  it("offers the STARS filters with the key that steps through them", () => {
    renderControls({ starFilter: "living" });

    const group = screen.getByRole("group", { name: "K STARS" });
    expect(
      within(group)
        .getAllByRole("radio")
        .map((option) => option.closest("label")?.textContent),
    ).toEqual(["ALL", "LIVING", "REMNANTS"]);
    expect(within(group).getByRole("radio", { name: "LIVING" })).toBeChecked();
    expect(group).toHaveAttribute("aria-keyshortcuts", "K");
  });

  it("reports the STARS filter the operator chooses", async () => {
    const user = userEvent.setup();
    const { onStarFilter } = renderControls();

    await user.click(screen.getByRole("radio", { name: "REMNANTS" }));

    expect(onStarFilter).toHaveBeenCalledWith("remnants");
  });

  it("steps to the next STARS filter when K is pressed", async () => {
    const user = userEvent.setup();
    const { onStarFilter } = renderControls({ starFilter: "remnants" });

    await user.keyboard("k");

    expect(onStarFilter).toHaveBeenCalledWith("all");
  });

  it("leaves K to a text field that has the focus", async () => {
    const user = userEvent.setup();
    const { onStarFilter } = renderControls();

    await user.click(screen.getByLabelText("DRIVE RANGE"));
    await user.keyboard("k");

    expect(onStarFilter).not.toHaveBeenCalled();
  });

  it("keeps the STARS filter acting while the link is down, since it asks nothing", async () => {
    const user = userEvent.setup();
    const { onStarFilter } = renderControls({ heldBack: "NO CARRIER" });

    await user.click(screen.getByRole("radio", { name: "LIVING" }));

    expect(onStarFilter).toHaveBeenCalledWith("living");
    expect(screen.getByRole("radio", { name: "LIVING" })).not.toHaveAttribute("aria-disabled");
  });

  it("writes DRIVE RANGE's hint as a span in words", () => {
    renderControls();

    expect(describedText(numberField("DRIVE RANGE"))).toBe("0.01 to 500 ly");
  });
});

/** The two number fields, which can refuse an entry. */
const NUMBER_FIELDS = ["DRIVE RANGE", "CHART TIME"] as const;

type NumberFieldName = (typeof NUMBER_FIELDS)[number];

/** Each field's refusal: what is typed, the hint, and what the refusal says is valid. */
const REFUSALS: Readonly<
  Record<NumberFieldName, { readonly typed: string; readonly hint: string; readonly says: string }>
> = {
  "DRIVE RANGE": { typed: "900", hint: "0.01 to 500 ly", says: "enter 0.01 to 500 ly" },
  "CHART TIME": { typed: "1001", hint: "±1000 yr", says: "enter -1000 to 1000 yr" },
};

/** Refuses an entry in both number fields, so that both refusals stand at once. */
async function refuseBoth(pageClass?: string): Promise<void> {
  const user = userEvent.setup();
  renderControls({}, pageClass);
  // One field after the other, as the operator types: the second's focus leaves the first refused.
  await user.clear(numberField("DRIVE RANGE"));
  await user.type(numberField("DRIVE RANGE"), `${REFUSALS["DRIVE RANGE"].typed}{Enter}`);
  await user.clear(numberField("CHART TIME"));
  await user.type(numberField("CHART TIME"), `${REFUSALS["CHART TIME"].typed}{Enter}`);
}

/** The field a number input stands in: its label, input, unit, hint and any refusal. */
function fieldOf(name: NumberFieldName): HTMLElement {
  const field = numberField(name).closest<HTMLElement>(".chart-controls__field");
  if (field === null) {
    throw new Error(`${name} stands in no field`);
  }
  return field;
}

describe("ChartControls' refusals (plan 12, P12.T11.c)", () => {
  it.each(NUMBER_FIELDS)(
    "shows %s's refusal in its own field while both stand, describing its own input",
    async (name) => {
      await refuseBoth();
      const { hint, says } = REFUSALS[name];

      expect(
        within(fieldOf(name)).getByText(statusLine(`${name} INVALID: ${says}`)),
      ).toBeInTheDocument();
      expect(describedText(numberField(name))).toBe(`${hint} ${name} INVALID: ${says}`);
      expect(fieldOf(name)).toHaveClass("chart-controls__field--refused");
    },
  );

  it.each(NUMBER_FIELDS)(
    "holds %s's name, with its trailing space, in an element the compact layout hides",
    async (name) => {
      await refuseBoth();

      const named = within(fieldOf(name)).getByText(
        (_, element) => element?.classList.contains("chart-controls__error-name") === true,
      );
      expect(named.textContent).toBe(`${name} `);
      expect(named.closest(".status-text__state")?.textContent).toBe(`${name} INVALID:`);
    },
  );

  it("says the same refusals and descriptions on a compact page as on a stacked one", async () => {
    const read = (): string[] =>
      NUMBER_FIELDS.flatMap((name) => [
        fieldOf(name).querySelector(".chart-controls__error")?.textContent ?? "",
        describedText(numberField(name)),
      ]);
    await refuseBoth("local-chart");
    const stacked = read();
    cleanup();

    await refuseBoth("local-chart local-chart--compact");

    expect(read()).toEqual(stacked);
    expect(stacked).toContain("DRIVE RANGE INVALID: enter 0.01 to 500 ly");
  });

  it("drops the refused mark with the refusal, so that the hint comes back", async () => {
    const user = userEvent.setup();
    renderControls();
    await user.clear(numberField("DRIVE RANGE"));
    await user.type(numberField("DRIVE RANGE"), "900{Enter}");

    await user.keyboard("{Escape}");

    expect(fieldOf("DRIVE RANGE")).not.toHaveClass("chart-controls__field--refused");
  });
});
