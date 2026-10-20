import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { aViewScene, FIXTURE_MOON, FIXTURE_PLANET } from "../../test/viewFixtures";
import type { ViewScene } from "../../view/scene/model";
import type { DrawnSurveyEdge } from "../../view/wireframe/drawList";
import { markRows, rangesFromCamera, startServerRun, surveyEdgeNotes } from "./viewRun";
import { ViewMarkList } from "./ViewMarkList";

/** Both bodies' edges in view, the moon's nearer. */
const BOTH: ReadonlyArray<DrawnSurveyEdge> = [
  { body: FIXTURE_PLANET, nearestRangeM: 9_000 },
  { body: FIXTURE_MOON, nearestRangeM: 4_210 },
];

/** The list of `scene`'s targets with the notes of `edges`. */
function renderList(scene: ViewScene, edges: ReadonlyArray<DrawnSurveyEdge>, stale = false): void {
  const run = startServerRun(scene);
  render(
    <ViewMarkList
      rows={markRows(run)}
      notes={surveyEdgeNotes(scene, edges)}
      fromCamera={rangesFromCamera(scene)}
      stale={stale}
      selectedKey={null}
      onSelect={() => undefined}
    />,
  );
}

describe("the view list's survey-edge notes (R10.T16.c; decision-r10-t16-review §1)", () => {
  it("lists one note a body, nearest first, after the options", () => {
    renderList(aViewScene(), BOTH);
    expect(screen.getAllByRole("note").map((note) => note.textContent)).toEqual([
      "SURVEY EDGE TEST MOON 4.21 km",
      "SURVEY EDGE TEST PLANET 9.00 km",
    ]);
  });

  it("names a note in the options' form", () => {
    renderList(aViewScene(), BOTH);
    expect(
      screen.getByRole("note", { name: "TEST MOON, survey edge, range 4.21 km" }),
    ).toBeInTheDocument();
  });

  it("names a note FROM CAMERA in a scene with no own ship", () => {
    renderList(aViewScene({ craft: [], ownShip: null }), [
      { body: FIXTURE_MOON, nearestRangeM: 412 },
    ]);
    expect(
      screen.getByRole("note", { name: "TEST MOON, survey edge, range 412 m FROM CAMERA" }),
    ).toHaveTextContent("SURVEY EDGE TEST MOON 412 m FROM CAMERA");
  });

  it("names a note stale while the list's ranges are", () => {
    renderList(aViewScene(), BOTH, true);
    expect(
      screen.getByRole("note", { name: "TEST MOON, survey edge, range 4.21 km, stale" }),
    ).toBeInTheDocument();
  });

  it("mutes a stale note's range with its S", () => {
    renderList(aViewScene(), BOTH, true);
    const note = screen.getByRole("note", { name: /^TEST MOON/u });
    expect(within(note).getByText("4.21 km").closest(".stale")).not.toBeNull();
    expect(within(note).getByText("S")).toHaveClass("stale-mark");
  });

  it("sets no note inside the listbox, nor as an option", () => {
    renderList(aViewScene(), BOTH);
    const list = screen.getByRole("listbox", { name: "Marks in view" });
    expect(within(list).queryAllByRole("note")).toEqual([]);
  });

  it("makes no note focusable", () => {
    renderList(aViewScene(), BOTH);
    expect(screen.getAllByRole("note").filter((note) => note.tabIndex >= 0)).toEqual([]);
  });

  it("leaves the notes out of the list's keys", async () => {
    const user = userEvent.setup();
    const scene = aViewScene();
    const rows = markRows(startServerRun(scene));
    const selected: string[] = [];
    render(
      <ViewMarkList
        rows={rows}
        notes={surveyEdgeNotes(scene, BOTH)}
        fromCamera={false}
        selectedKey={null}
        onSelect={(row) => {
          selected.push(row.key);
        }}
      />,
    );
    screen.getByRole("listbox", { name: "Marks in view" }).focus();
    await user.keyboard("{End}");
    expect(selected).toEqual([rows.at(-1)?.key]);
  });

  it("lists no note where no edge is in view", () => {
    renderList(aViewScene(), [{ body: FIXTURE_MOON, nearestRangeM: null }]);
    expect(screen.queryByRole("note")).toBeNull();
  });
});
