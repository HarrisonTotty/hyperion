import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { aViewScene, FIXTURE_MOON, FIXTURE_PLANET } from "../../test/viewFixtures";
import type { ViewScene } from "../../view/scene/model";
import type { DrawnSurveyEdge } from "../../view/wireframe/drawList";
import { markRows, rangesFromCamera, startServerRun, surveyEdgeNotes } from "./viewRun";
import { GROUND_NOTE, GROUND_NOTE_NAME, ViewMarkList } from "./ViewMarkList";

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

/**
 * The list of the fixture scene's targets with the notes of `edges` and, where `groundNote` holds,
 * the line-of-sight mark's; each selection made pushed to `selected`.
 */
function renderWithGround(
  edges: ReadonlyArray<DrawnSurveyEdge>,
  groundNote: boolean | undefined,
  options: { readonly stale?: boolean; readonly selected?: string[] } = {},
): void {
  const scene = aViewScene();
  render(
    <ViewMarkList
      rows={markRows(startServerRun(scene))}
      notes={surveyEdgeNotes(scene, edges)}
      groundNote={groundNote}
      fromCamera={rangesFromCamera(scene)}
      stale={options.stale ?? false}
      selectedKey={null}
      onSelect={(row) => {
        options.selected?.push(row.key);
      }}
    />,
  );
}

describe("the view list's line-of-sight note (R10.T12; the guide's tripod, In the list)", () => {
  it("stands GROUND AT LINE OF SIGHT first of the notes, before the survey edges'", () => {
    renderWithGround(BOTH, true);
    expect(screen.getAllByRole("note").map((note) => note.textContent)).toEqual([
      GROUND_NOTE,
      "SURVEY EDGE TEST MOON 4.21 km",
      "SURVEY EDGE TEST PLANET 9.00 km",
    ]);
  });

  it("names the note by the mark's place", () => {
    renderWithGround(BOTH, true);
    expect(
      screen.getByRole("note", { name: "Ground at line of sight, at the view's centre" }),
    ).toHaveTextContent("GROUND AT LINE OF SIGHT");
  });

  it("names the note the same while the list's ranges are stale, since it marks the centre", () => {
    renderWithGround(BOTH, true, { stale: true });
    expect(screen.getByRole("note", { name: GROUND_NOTE_NAME })).toHaveTextContent(GROUND_NOTE);
  });

  it("stands the note with no survey edge in view", () => {
    renderWithGround([], true);
    expect(screen.getAllByRole("note").map((note) => note.textContent)).toEqual([GROUND_NOTE]);
  });

  it("sets the note outside the listbox, and not as an option", () => {
    renderWithGround(BOTH, true);
    const list = screen.getByRole("listbox", { name: "Marks in view" });
    const note = screen.getByRole("note", { name: GROUND_NOTE_NAME });
    expect([list.contains(note), within(list).queryByText(GROUND_NOTE)]).toEqual([false, null]);
  });

  it("makes the note not focusable", () => {
    renderWithGround(BOTH, true);
    const note = screen.getByRole("note", { name: GROUND_NOTE_NAME });
    expect([note.hasAttribute("tabindex"), note.tabIndex]).toEqual([false, -1]);
  });

  it("leaves the note out of the list's keys", async () => {
    const user = userEvent.setup();
    const selected: string[] = [];
    renderWithGround([], true, { selected });
    const rows = markRows(startServerRun(aViewScene()));
    screen.getByRole("listbox", { name: "Marks in view" }).focus();
    // The selection stays where the test holds it, none: each key moves from before the first row.
    await user.keyboard("{End}{ArrowDown}");
    expect(selected).toEqual([rows.at(-1)?.key, rows[0]?.key]);
  });

  it.each([false, undefined])("sets no line-of-sight note with groundNote %s", (groundNote) => {
    renderWithGround([], groundNote);
    expect(screen.queryByRole("note")).toBeNull();
  });
});
