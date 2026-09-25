import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { ChartResult } from "../../lib/galaxy/model";
import { toChartResult } from "../../lib/galaxy/wire";
import { PENDING, type RequestState } from "../../lib/useServerRequest";
import { aSystemsInRange } from "../../test/galaxyFixtures";
import { announcements } from "../../test/liveRegions";
import { CensusReadout } from "./CensusReadout";
import { filterSystems, type StarFilter } from "./chartModel";

const CENTRE = [26_000, 0, 0] as const;

function aChart(spec: Parameters<typeof aSystemsInRange>[0] = {}): ChartResult {
  return toChartResult(aSystemsInRange({ centreLy: CENTRE, ...spec }));
}

const TWO_SYSTEMS = aChart({
  systems: [
    { relLy: [10, 0, 0], layer: "a" },
    { relLy: [60, 0, 0], layer: "a" },
  ],
  radiusLy: 80,
});

function renderReadout(
  result: ChartResult | null,
  state: RequestState<"systems_in_range"> = { kind: "ok", response: aSystemsInRange() },
  { stale = false, starFilter = "all" }: { stale?: boolean; starFilter?: StarFilter } = {},
) {
  const onRetry = vi.fn<() => void>();
  const readout = (
    shownResult: ChartResult | null,
    shownState: RequestState<"systems_in_range">,
  ) => (
    <CensusReadout
      result={shownResult}
      systems={shownResult === null ? [] : filterSystems(shownResult.systems, starFilter)}
      starFilter={starFilter}
      state={shownState}
      driveRangeLy={50}
      stale={stale}
      onRetry={onRetry}
    />
  );
  const { rerender } = render(readout(result, state));
  return {
    onRetry,
    /** Shows the answer to the query that was in flight, as the chart does when it arrives. */
    answered: (shownResult: ChartResult) => {
      rerender(readout(shownResult, { kind: "ok", response: aSystemsInRange() }));
    },
    /** Shows another state of the query beside the same answer. */
    show: (shownState: RequestState<"systems_in_range">) => {
      rerender(readout(result, shownState));
    },
  };
}

/** The census summary, which is the readout's one announced region. */
function summary(): HTMLElement {
  return screen.getByRole("status", { name: "Census" });
}

/** A census row's cells, its layer letter first. */
function rowText(rows: ReadonlyArray<HTMLElement>, index: number): ReadonlyArray<string | null> {
  const row = rows[index];
  if (row === undefined) {
    throw new Error(`the census has no row ${index}`);
  }
  const header = within(row).getByRole("rowheader");
  const cells = within(row).getAllByRole("cell");
  return [header, ...cells].map((cell) => cell.textContent);
}

describe("CensusReadout", () => {
  it("says what the census is complete above, with its drawn unit", () => {
    renderReadout(aChart({ minLayer: "b" }));

    expect(screen.getByText(/COMPLETE ABOVE/u)).toHaveTextContent("COMPLETE ABOVE 0.50");
    expect(screen.getByRole("img", { name: "solar masses" })).toBeInTheDocument();
  });

  it("says what to do when nothing fits, as a limit reached", () => {
    renderReadout(aChart({ overLimit: ["a", "b", "c", "d", "e"] }));

    expect(screen.getByText("NOTHING FITS: reduce radius")).toHaveClass(
      "census-readout__line--caution",
    );
  });

  it("counts the systems returned and those within the drive range", () => {
    renderReadout(TWO_SYSTEMS);

    expect(screen.getByText("SYSTEMS").nextElementSibling).toHaveTextContent("2");
    expect(screen.getByText("IN RANGE").nextElementSibling).toHaveTextContent("1");
  });

  it("says what the STARS filter hides, and counts in range among the systems shown", () => {
    renderReadout(TWO_SYSTEMS, undefined, { starFilter: "remnants" });

    expect(screen.getByText("SYSTEMS").nextElementSibling).toHaveTextContent(
      "0 OF 2 SHOWN: REMNANTS",
    );
    expect(screen.getByText("IN RANGE").nextElementSibling).toHaveTextContent("0");
  });

  it("asks for a smaller radius when a layer was over the limit", () => {
    renderReadout(aChart({ overLimit: ["a"] }));

    expect(
      screen.getByText("DENSE REGION: reduce radius before raising MIN MASS"),
    ).toBeInTheDocument();
  });

  it("folds the census table, and shows a line for every layer once asked", async () => {
    const user = userEvent.setup();
    renderReadout(aChart({ minLayer: "c", systems: [{ relLy: [1, 0, 0], layer: "c" }] }));

    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "CENSUS BY LAYER" }));

    const rows = within(screen.getByRole("table")).getAllByRole("row").slice(1);
    expect(rows).toHaveLength(5);
    expect(rowText(rows, 0)).toEqual(["A", "0.08-0.5", "1.00E4", "0", "BELOW MIN MASS"]);
    expect(rowText(rows, 2)).toEqual(["C", "0.75-2.5", "1.4", "1", "INCLUDED"]);
  });

  it("names a layer over the census limit in words", async () => {
    const user = userEvent.setup();
    renderReadout(aChart({ overLimit: ["a"] }));

    await user.click(screen.getByRole("button", { name: "CENSUS BY LAYER" }));

    expect(rowText(within(screen.getByRole("table")).getAllByRole("row").slice(1), 0)).toEqual([
      "A",
      "0.08-0.5",
      "1.00E4",
      "0",
      "OVER LIMIT",
    ]);
  });

  it("names a layer dropped for the server's cell budget in words", async () => {
    const user = userEvent.setup();
    renderReadout(aChart({ overCellBudget: ["e"] }));

    await user.click(screen.getByRole("button", { name: "CENSUS BY LAYER" }));

    expect(rowText(within(screen.getByRole("table")).getAllByRole("row").slice(1), 4)).toEqual([
      "E",
      "8-150",
      "1.00E4",
      "0",
      "OVER CELL BUDGET",
    ]);
  });

  it("keeps the answer on show with PENDING beside it while a newer query is in flight", () => {
    renderReadout(TWO_SYSTEMS, PENDING);

    expect(summary()).toHaveTextContent("PENDING");
    expect(screen.getByText("SYSTEMS").nextElementSibling).toHaveTextContent("2");
  });

  it("announces the query and then what came back", async () => {
    const { answered } = renderReadout(null, PENDING);
    expect(summary()).toHaveTextContent("PENDING");

    const announced = await announcements(() => {
      answered(TWO_SYSTEMS);
      return Promise.resolve();
    });

    expect(announced).toEqual([summary()]);
  });

  it("gives the answer in place of PENDING once it comes back", () => {
    const { answered } = renderReadout(null, PENDING);

    answered(TWO_SYSTEMS);

    expect(summary()).toHaveTextContent("COMPLETE ABOVE 0.08");
    expect(summary()).not.toHaveTextContent("PENDING");
  });

  it("reads the whole summary when any part of it changes", () => {
    renderReadout(TWO_SYSTEMS, PENDING);

    // Atomic, so that a count is read with the line it belongs to (the orchestrator's ruling 11).
    expect(summary()).toHaveAttribute("aria-atomic", "true");
  });

  it("holds its list in an element whose content may be a list, not in an output", () => {
    renderReadout(TWO_SYSTEMS, PENDING);

    // A rule of HTML's content models, so held as markup: an `output` holds phrasing content only,
    // and the summary holds a `p` and a `dl`, so it is a `div` with the status role (ruling 14).
    expect(summary().tagName).toBe("DIV");
  });

  it("holds no region of its own inside the summary", () => {
    renderReadout(TWO_SYSTEMS, PENDING);

    // A live region nested in a live region is read differently by every screen reader, so the
    // request's state is words in the summary rather than a `StatusLine` of its own (the
    // orchestrator's ruling 13).
    expect(within(summary()).queryByRole("status")).not.toBeInTheDocument();
  });

  it("announces a change confined to the state's words, from the summary", async () => {
    const { show } = renderReadout(TWO_SYSTEMS, PENDING);

    const announced = await announcements(() => {
      show({ kind: "link_down", reason: "NO CARRIER" });
      return Promise.resolve();
    });

    // `NO CARRIER` replacing `PENDING` changes nothing else, which is the change that a silenced
    // status line nested in the summary left unannounced (ruling 13).
    expect(announced).toEqual([summary()]);
  });

  it("keeps the table and both controls out of what is announced", async () => {
    const user = userEvent.setup();
    // Rejected, so that the state in words and its RETRY are both on show.
    renderReadout(TWO_SYSTEMS, {
      kind: "rejected",
      code: "queue_full",
      reason: "the queue is full",
    });

    await user.click(screen.getByRole("button", { name: "CENSUS BY LAYER" }));

    expect(within(summary()).queryByRole("table")).not.toBeInTheDocument();
    // A control inside a live region is announced with it, so both stand beside the region, while
    // the words that say what happened stay in it (ruling 13).
    expect(within(summary()).queryByRole("button")).not.toBeInTheDocument();
  });

  it("describes RETRY by the failure it retries", () => {
    renderReadout(TWO_SYSTEMS, {
      kind: "rejected",
      code: "queue_full",
      reason: "the queue is full",
    });

    // Outside the region, so the words are not read with it unless it names them itself.
    expect(screen.getByRole("button", { name: "RETRY" })).toHaveAccessibleDescription(
      "REJECTED: the queue is full",
    );
  });

  it("gives the server's reason and RETRY when the query is rejected", async () => {
    const user = userEvent.setup();
    const { onRetry } = renderReadout(TWO_SYSTEMS, {
      kind: "rejected",
      code: "queue_full",
      reason: "the queue is full",
    });

    await user.click(screen.getByRole("button", { name: "RETRY" }));

    expect(summary()).toHaveTextContent("REJECTED: the queue is full");
    expect(onRetry).toHaveBeenCalledOnce();
  });

  it("hands the focus to CENSUS BY LAYER beside it when RETRY is pressed", async () => {
    const user = userEvent.setup();
    const { show } = renderReadout(TWO_SYSTEMS, {
      kind: "rejected",
      code: "queue_full",
      reason: "the queue is full",
    });
    act(() => {
      screen.getByRole("button", { name: "RETRY" }).focus();
    });

    await user.keyboard("{Enter}");
    // What the chart does on RETRY: the query goes pending, and RETRY goes with the failure.
    show(PENDING);

    expect(document.activeElement).toBe(screen.getByRole("button", { name: "CENSUS BY LAYER" }));
  });

  it("offers no RETRY while the query is pending", () => {
    renderReadout(TWO_SYSTEMS, PENDING);

    expect(screen.queryByRole("button", { name: "RETRY" })).not.toBeInTheDocument();
  });

  it("offers no RETRY while the link is down, since the query goes again with the link", () => {
    renderReadout(TWO_SYSTEMS, { kind: "link_down", reason: "NO CARRIER" });

    expect(screen.queryByRole("button", { name: "RETRY" })).not.toBeInTheDocument();
  });

  it("gives the link's state in plain words, as it is no failure of the query", () => {
    renderReadout(TWO_SYSTEMS, { kind: "link_down", reason: "NO CARRIER" });

    // Exactly the plain class, so that a renamed caution class cannot pass unseen.
    expect(within(summary()).getByText("NO CARRIER")).toHaveClass("census-readout__state", {
      exact: true,
    });
  });

  it("reads a timeout in caution, as a failed system", () => {
    renderReadout(TWO_SYSTEMS, { kind: "timed_out" });

    expect(within(summary()).getByText("TIMED OUT")).toHaveClass("census-readout__state--fault");
  });

  it("reads a refusal in plain text, since nothing failed", () => {
    renderReadout(TWO_SYSTEMS, {
      kind: "rejected",
      code: "unknown_universe",
      reason: "no such universe",
    });

    expect(within(summary()).getByText("REJECTED: no such universe")).toHaveClass(
      "census-readout__state",
      { exact: true },
    );
  });

  it("reads the census layer by layer with its position and total, the head kept in view", async () => {
    const user = userEvent.setup();
    renderReadout(aChart());

    await user.click(screen.getByRole("button", { name: "CENSUS BY LAYER" }));

    // jsdom lays nothing out, so the box measures no height and the first row alone is in view.
    expect(screen.getByText("1-1 of 5")).toBeInTheDocument();
    expect(screen.getByRole("columnheader", { name: "LAYER" })).toBeInTheDocument();
  });

  it("writes an expected count within its field to one decimal", async () => {
    const user = userEvent.setup();
    renderReadout(aChart({ systems: [{ relLy: [1, 0, 0], layer: "c" }] }));

    await user.click(screen.getByRole("button", { name: "CENSUS BY LAYER" }));

    // An included layer expects a little more than it returned; a dropped one 2.5 times the limit,
    // 10,000, which no longer fits a 9ch field and reads 1.00E4 above.
    const rows = within(screen.getByRole("table")).getAllByRole("row").slice(1);
    expect(rowText(rows, 2)).toEqual(["C", "0.75-2.5", "1.4", "1", "INCLUDED"]);
  });

  it("marks the answer stale once the link no longer backs it", () => {
    renderReadout(TWO_SYSTEMS, undefined, { stale: true });

    expect(screen.getByText("IN RANGE").nextElementSibling).toHaveTextContent("1 S");
    expect(screen.getByText("stale")).toBeInTheDocument();
  });

  it("shows no census before the first answer", () => {
    renderReadout(null, PENDING);

    expect(screen.queryByText(/COMPLETE ABOVE/u)).not.toBeInTheDocument();
    expect(screen.queryByText("SYSTEMS")).not.toBeInTheDocument();
  });
});
