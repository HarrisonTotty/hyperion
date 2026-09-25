import type { SystemIdHex } from "@hyperion/protocol";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { ChartSystem } from "../../lib/galaxy/model";
import { toChartResult } from "../../lib/galaxy/wire";
import { aStellarBrief, aSystemsInRange } from "../../test/galaxyFixtures";
import { SystemList } from "./SystemList";

const CENTRE = [26_000, 0, 0] as const;

/** Twenty rows of 2rem at the test root font size, the window the list is measured at. */
const VIEWPORT_PX = 20 * 32;

/** `count` systems at 1, 2, … light-years from the centre, alternating layers. */
function manySystems(count: number): ReadonlyArray<ChartSystem> {
  return toChartResult(
    aSystemsInRange({
      centreLy: CENTRE,
      radiusLy: 500,
      systems: Array.from({ length: count }, (_, index) => ({
        relLy: [index + 1, 0, 0] as const,
        layer: index % 2 === 0 ? ("a" as const) : ("e" as const),
      })),
    }),
  ).systems;
}

/** Lays the list's scrolling box out at twenty rows, which jsdom otherwise leaves at no height. */
function stubViewport(): void {
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(VIEWPORT_PX);
}

function renderList(
  systems: ReadonlyArray<ChartSystem>,
  {
    selectedId = null,
    driveRangeLy = 50,
  }: { selectedId?: SystemIdHex | null; driveRangeLy?: number } = {},
) {
  const onSelect = vi.fn<(id: SystemIdHex) => void>();
  const view = render(
    <SystemList
      systems={systems}
      selectedId={selectedId}
      onSelect={onSelect}
      driveRangeLy={driveRangeLy}
      distanceDecimals={2}
    />,
  );
  const rerenderWith = (id: SystemIdHex | null): void => {
    view.rerender(
      <SystemList
        systems={systems}
        selectedId={id}
        onSelect={onSelect}
        driveRangeLy={driveRangeLy}
        distanceDecimals={2}
      />,
    );
  };
  return { onSelect, rerenderWith };
}

/** Presses ArrowDown and gives the list back the selection it reported, as its owner would. */
async function pressDown(
  user: ReturnType<typeof userEvent.setup>,
  onSelect: { readonly mock: { readonly calls: ReadonlyArray<readonly [SystemIdHex]> } },
  rerenderWith: (id: SystemIdHex | null) => void,
): Promise<void> {
  await user.keyboard("{ArrowDown}");
  rerenderWith(onSelect.mock.calls.at(-1)?.[0] ?? null);
}

function list(): HTMLElement {
  return screen.getByRole("listbox", { name: "Systems by distance" });
}

/** The list's first row. */
function firstOption(): HTMLElement | undefined {
  return within(screen.getByRole("listbox")).getAllByRole("option")[0];
}

describe("SystemList", () => {
  it("renders only the rows around the window, whatever the chart holds", () => {
    stubViewport();
    renderList(manySystems(3_000));

    // Twenty rows in view and eight of overscan beyond the end the list is not scrolled to.
    expect(screen.getAllByRole("option")).toHaveLength(28);
  });

  it("gives every row its place in the whole list", () => {
    stubViewport();
    renderList(manySystems(3_000));

    const options = screen.getAllByRole("option");
    expect(options[0]).toHaveAttribute("aria-setsize", "3000");
    expect(options[0]).toHaveAttribute("aria-posinset", "1");
    expect(options.at(-1)).toHaveAttribute("aria-posinset", "28");
  });

  it("selects the fourth-nearest system after three presses of ArrowDown", async () => {
    const user = userEvent.setup();
    stubViewport();
    const systems = manySystems(3_000);
    const { onSelect, rerenderWith } = renderList(systems);

    await user.click(list());
    await pressDown(user, onSelect, rerenderWith);
    await pressDown(user, onSelect, rerenderWith);
    await pressDown(user, onSelect, rerenderWith);

    expect(onSelect).toHaveBeenLastCalledWith(systems[3]?.id);
  });

  it("selects the last system with End and reads its place", async () => {
    const user = userEvent.setup();
    stubViewport();
    const systems = manySystems(3_000);
    const { onSelect } = renderList(systems);

    await user.click(list());
    await user.keyboard("{End}");

    expect(onSelect).toHaveBeenLastCalledWith(systems.at(-1)?.id);
    expect(screen.getByText("2981-3000 of 3000")).toBeInTheDocument();
  });

  it("moves one window with PageDown", async () => {
    const user = userEvent.setup();
    stubViewport();
    const systems = manySystems(3_000);
    const { onSelect } = renderList(systems);

    await user.click(list());
    await user.keyboard("{PageDown}");

    expect(onSelect).toHaveBeenLastCalledWith(systems[20]?.id);
  });

  it("says in words whether a system is within the drive range", () => {
    stubViewport();
    renderList(manySystems(60), { driveRangeLy: 10 });

    const options = screen.getAllByRole("option");
    expect(options[0]).toHaveTextContent("IN RANGE");
    expect(options[0]).toHaveClass("system-list__row--in-range");
    // The twenty-first system is 21 ly out, beyond the drive range.
    expect(options[20]).toHaveTextContent("OUT");
    expect(options[20]).not.toHaveClass("system-list__row--in-range");
  });

  it("heads those words with the setting's one name, DRIVE RANGE", () => {
    renderList(manySystems(3));

    // `RANGE` alone is the chart's curve labels' abbreviation, and nowhere else's (ruling 15).
    expect(screen.getByText("DRIVE RANGE")).toBeInTheDocument();
    expect(screen.queryByText("RANGE", { exact: true })).not.toBeInTheDocument();
  });

  it("marks the selected row, and scrolls to it when the chart selects it", () => {
    stubViewport();
    const systems = manySystems(3_000);
    const { rerenderWith } = renderList(systems);

    rerenderWith(systems[500]?.id ?? null);

    const selected = screen.getByRole("option", { selected: true });
    expect(selected).toHaveAttribute("aria-posinset", "501");
    expect(list()).toHaveAttribute("aria-activedescendant", selected.id);
  });

  it("selects the row the operator clicks", async () => {
    const user = userEvent.setup();
    stubViewport();
    const systems = manySystems(40);
    const { onSelect } = renderList(systems);

    await user.click(screen.getAllByRole("option")[2] ?? list());

    expect(onSelect).toHaveBeenCalledWith(systems[2]?.id);
  });

  it("names each row with its distance and mass in their units", () => {
    stubViewport();
    const systems = manySystems(1);

    renderList(systems);

    expect(
      screen.getByRole("option", {
        name: "H7K 4C0RFZ A-1, DWARF M3V, 1.00 ly, 0.29 solar masses, IN RANGE",
      }),
    ).toBeInTheDocument();
  });

  it("names a system beyond the drive range OUT OF RANGE, though its column shows OUT", () => {
    stubViewport();
    renderList(manySystems(3), { driveRangeLy: 2 });

    // The third system is 3 ly out, beyond the 2 ly drive range.
    const row = screen.getByRole("option", {
      name: /, 3\.00 ly, [\d.]+ solar masses, OUT OF RANGE$/u,
    });
    expect(within(row).getByText("OUT", { exact: true })).toBeInTheDocument();
    expect(within(row).queryByText("OUT OF RANGE")).not.toBeInTheDocument();
  });

  it("shows nothing but its heading when the chart holds no system", () => {
    stubViewport();
    renderList([]);

    expect(screen.queryAllByRole("option")).toHaveLength(0);
    expect(screen.queryByText(/of/u)).not.toBeInTheDocument();
  });

  it("shows each row's spectral class in its class column", () => {
    stubViewport();
    const systems = toChartResult(
      aSystemsInRange({
        centreLy: CENTRE,
        systems: [{ relLy: [1, 0, 0], layer: "c", stellar: aStellarBrief("c", "white_dwarf") }],
      }),
    ).systems;

    renderList(systems);

    const row = screen.getByRole("option", { name: /^H7K 4C0RFZ C-1, WHITE DWARF DA4\.2, /u });
    expect(within(row).getByText("DA4.2")).toBeInTheDocument();
    expect(screen.getByText("CLASS")).toBeInTheDocument();
  });

  it("names a system not yet formed so and shows its class missing", () => {
    stubViewport();
    const systems = toChartResult(
      aSystemsInRange({
        centreLy: CENTRE,
        systems: [{ relLy: [1, 0, 0], layer: "a", stellar: null }],
      }),
    ).systems;

    renderList(systems);

    const row = screen.getByRole("option", { name: /^H7K 4C0RFZ A-1, NOT YET FORMED, /u });
    expect(within(row).getByText("—")).toBeInTheDocument();
  });

  it("shows a system where the chart time's answer puts it", () => {
    // The chart asks again when its time changes (plan 05); the answer a thousand years on has the
    // system moved by its velocity, 300 km/s outward being 1.00 ly, and the list reads it there
    // (plan 08, P08.T7.b).
    const at = (relX: number) =>
      toChartResult(
        aSystemsInRange({
          centreLy: CENTRE,
          systems: [{ relLy: [relX, 0, 0], layer: "c", velocityKmS: [300, 0, 0] }],
        }),
      ).systems;
    const onSelect = vi.fn<(id: SystemIdHex) => void>();
    const view = render(
      <SystemList
        systems={at(12)}
        selectedId={null}
        onSelect={onSelect}
        driveRangeLy={50}
        distanceDecimals={2}
      />,
    );
    expect(firstOption()).toHaveTextContent("12.00");
    view.rerender(
      <SystemList
        systems={at(13)}
        selectedId={null}
        onSelect={onSelect}
        driveRangeLy={50}
        distanceDecimals={2}
      />,
    );
    expect(firstOption()).toHaveTextContent("13.00");
    expect(firstOption()).not.toHaveTextContent("12.00");
  });
});
