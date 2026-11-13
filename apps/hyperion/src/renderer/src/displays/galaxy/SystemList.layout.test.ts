/**
 * The `SYSTEMS` list's columns hold a star's class whole where they can, and cut it visibly where
 * they cannot (P06.T24.c): at 1280 × 720 the narrow list cut E-141's `He-sdO3` to read `He-sdC`.
 * jsdom lays nothing out, so these tests hold the stylesheet's rules the columns rest on; the fit
 * was measured on the real client, hidden (stripped-class's look on look-72's harness).
 */
import { describe, expect, it } from "vitest";

import { stylesheetRule, stylesheetRuleIn } from "../../test/stylesheet";

/** The list's own container query, at the longest designation's room. */
const NARROW = "@container (max-width: 42rem)";

describe("the SYSTEMS list's columns", () => {
  it("sizes the class column for the longest class the server writes, eleven characters", () => {
    expect(stylesheetRule(".system-list__head,\n.system-list__row")).toMatch(
      /grid-template-columns: minmax\(0, 1fr\) 12\.5ch 6ch 7ch 8\.5ch 6\.5ch;/u,
    );
  });

  it("turns to two-line rows below the longest designation's room, with closer gaps", () => {
    const narrow = stylesheetRuleIn(NARROW, ".system-list__head,\n.system-list__row");

    expect(narrow).toMatch(/grid-template-columns: minmax\(0, 1fr\) 6ch 7ch 8\.5ch 6\.5ch;/u);
    expect(narrow).toMatch(/gap: 0 0\.25rem;/u);
  });

  it("keeps the designation's line whole in the narrow rows", () => {
    expect(
      stylesheetRuleIn(NARROW, ".system-list__head > :first-child,\n.system-list__designation"),
    ).toMatch(/grid-column: 1 \/ -1;/u);
  });

  it("sets the class in 0.5rem under the designation", () => {
    expect(
      stylesheetRuleIn(
        NARROW,
        ".system-list__head > :nth-child(2),\n.system-list__row > :nth-child(2)",
      ),
    ).toMatch(/padding-left: 0\.5rem;/u);
  });

  it("keeps the range apart from the mass's unit in the narrow rows' closer gaps", () => {
    expect(stylesheetRuleIn(NARROW, ".system-list__row > .system-list__range")).toMatch(
      /padding-left: 0\.125rem;/u,
    );
  });

  it("cuts a class that does not fit with an ellipsis, never silently", () => {
    const cell = stylesheetRule(".system-list__class");

    expect(cell).toMatch(/overflow: hidden;/u);
    expect(cell).toMatch(/text-overflow: ellipsis;/u);
    expect(cell).toMatch(/white-space: nowrap;/u);
  });
});
