import type { FeatureIdHex } from "@hyperion/protocol";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { toChartFeatures } from "../../lib/galaxy/featureWire";
import type { ChartFeature } from "../../lib/galaxy/model";
import { aFeaturesInRange, ASSOCIATION, STAR_FORMING_REGION } from "../../test/featureFixtures";
import { FeatureList } from "./FeatureList";
import { type ChartDistanceUnit, distanceUnitFor } from "./chartModel";

/** Twenty rows of 2rem at the test root font size, the window the list is measured at. */
const VIEWPORT_PX = 20 * 32;

/** The golden's three features nearest the Sun-like point, nearest first. */
const GOLDEN: ReadonlyArray<ChartFeature> = toChartFeatures(aFeaturesInRange()).features;

/** `count` features at 1, 2, … light-years out, as a dense chart holds them. */
function manyFeatures(count: number): ReadonlyArray<ChartFeature> {
  return Array.from({ length: count }, (_, index) => {
    const [feature] = GOLDEN;
    if (feature === undefined) {
      throw new Error("the golden holds no feature");
    }
    return {
      ...feature,
      id: `e859e0678${index.toString(16).padStart(7, "0")}`,
      designation: `GPF F${String(index)}`,
      distanceLy: index + 1,
    };
  });
}

function renderList(
  features: ReadonlyArray<ChartFeature>,
  {
    selectedId = null,
    stale = false,
    distanceUnit = distanceUnitFor(50),
  }: {
    selectedId?: FeatureIdHex | null;
    stale?: boolean;
    distanceUnit?: ChartDistanceUnit;
  } = {},
) {
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(VIEWPORT_PX);
  const onSelect = vi.fn<(id: FeatureIdHex) => void>();
  const view = render(
    <FeatureList
      features={features}
      selectedId={selectedId}
      onSelect={onSelect}
      distanceUnit={distanceUnit}
      stale={stale}
    />,
  );
  const rerenderWith = (id: FeatureIdHex | null): void => {
    view.rerender(
      <FeatureList
        features={features}
        selectedId={id}
        onSelect={onSelect}
        distanceUnit={distanceUnit}
        stale={stale}
      />,
    );
  };
  return { onSelect, rerenderWith };
}

function list(): HTMLElement {
  return screen.getByRole("listbox", { name: "Features by distance" });
}

describe("FeatureList (plan 09, P09.T41.c)", () => {
  it("names each feature with its kind in words, its distance, radius and mass in their units", () => {
    renderList(GOLDEN);

    expect(
      within(list())
        .getAllByRole("option")
        .map((row) => row.getAttribute("aria-label")),
    ).toEqual([
      `GPF F207, OPEN CLUSTER, ${GOLDEN[0]?.distanceLy.toFixed(2) ?? ""} ly, radius 26.9 ly, 268 solar masses`,
      `GPG F336, STAR-FORMING REGION, ${GOLDEN[1]?.distanceLy.toFixed(2) ?? ""} ly, radius 10.8 ly, 112 solar masses`,
      `GPG F530, OB ASSOCIATION, ${GOLDEN[2]?.distanceLy.toFixed(2) ?? ""} ly, radius 52.8 ly, 12,300 solar masses`,
    ]);
  });

  it("reads its distances in AU, under DIST AU, on a chart of the centre's fine steps (plan 09, P09.T42)", () => {
    const [feature] = GOLDEN;
    if (feature === undefined) {
      throw new Error("the golden holds no feature");
    }
    // 0.003 ly is 189.72 AU.
    renderList([{ ...feature, distanceLy: 0.003 }], { distanceUnit: distanceUnitFor(0.002) });

    expect(screen.getByText("DIST AU")).toBeInTheDocument();
    const [row] = within(list()).getAllByRole("option");
    expect(row).toHaveAccessibleName(
      "GPF F207, OPEN CLUSTER, 189.7 AU, radius 26.9 ly, 268 solar masses",
    );
    expect(row?.querySelectorAll(".system-list__number")[0]).toHaveTextContent("189.7");
  });

  it("shows each kind by the abbreviation its mark carries", () => {
    renderList(GOLDEN);

    const kinds = within(list())
      .getAllByRole("option")
      .map((row) => row.querySelector(".system-list__class")?.textContent);
    expect(kinds).toEqual(["OC", "SFR", "OB"]);
  });

  it("selects with the system list's keys: ArrowDown, End and Home", async () => {
    const user = userEvent.setup();
    const { onSelect, rerenderWith } = renderList(manyFeatures(40));
    list().focus();

    await user.keyboard("{ArrowDown}");
    expect(onSelect).toHaveBeenLastCalledWith("e859e06780000001");
    rerenderWith("e859e06780000001");
    await user.keyboard("{End}");
    expect(onSelect).toHaveBeenLastCalledWith("e859e06780000027");
    rerenderWith("e859e06780000027");
    await user.keyboard("{Home}");
    expect(onSelect).toHaveBeenLastCalledWith("e859e06780000000");
  });

  it("moves one window with PageDown, and windows its rows", async () => {
    const user = userEvent.setup();
    const { onSelect } = renderList(manyFeatures(400), { selectedId: "e859e06780000000" });
    list().focus();

    expect(within(list()).getAllByRole("option").length).toBeLessThan(400);
    await user.keyboard("{PageDown}");
    expect(onSelect).toHaveBeenLastCalledWith("e859e06780000014");
  });

  it("selects the row the operator clicks, and marks the selected row", async () => {
    const user = userEvent.setup();
    const { onSelect } = renderList(GOLDEN, { selectedId: STAR_FORMING_REGION.id });

    expect(within(list()).getByRole("option", { selected: true })).toHaveAccessibleName(
      /^GPG F336,/u,
    );
    await user.click(within(list()).getByRole("option", { name: /^GPG F530,/u }));

    expect(onSelect).toHaveBeenCalledWith(ASSOCIATION.id);
  });

  it("trails each row with one S and names it stale while the chart is stale", () => {
    renderList(GOLDEN.slice(0, 1), { stale: true });

    const [row] = within(list()).getAllByRole("option");
    expect(row).toHaveAccessibleName(/, stale$/u);
    // One mark to a row, after the mass's unit, which stands for each of its values.
    expect(row?.querySelector(".feature-list__mass")).toHaveTextContent(/^268 M S\s*stale$/u);
    expect(row?.querySelectorAll(".stale-mark")).toHaveLength(1);
  });
});
