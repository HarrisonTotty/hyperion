import { render, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { moonLikeHeader } from "../../test/coarseFieldFixtures";
import { FIXTURE_MOON } from "../../test/viewFixtures";
import type { FieldFailure, FieldStatus } from "../../view/terrain/coarseFieldStore";
import { CoverageView } from "../../view/terrain/coverage";
import {
  NO_READOUT_UNITS,
  type SightGap,
  sightGapLine,
  tallestTerrainLines,
  type TerrainLabelInput,
  terrainLabelModel,
} from "../../view/terrain/readouts";
import type { TerrainSurvey } from "../../view/terrain/session";
import type { SurfaceGroundReadout } from "../../view/terrain/workers/messages";
import { terrainLowerParts } from "./terrainPlate";
import type { TerrainLines } from "./useTerrainLines";

/** A coverage of level 5 with its first 77 cells surveyed at about 2 m (cell code 62). */
function coverageOf77(): CoverageView {
  const merged = CoverageView.empty(5).merged(
    { ranges: [{ start: 0, end: 77, resolution: 62 }] },
    1,
  );
  if (!merged.ok) {
    throw new Error(merged.message);
  }
  return merged.coverage;
}

/** A field's standing: wanted as the local body, answered, nothing failed, by default. */
function standing(overrides: Partial<FieldStatus> = {}): FieldStatus {
  return { reason: "local-body", pending: false, notModelled: null, failure: null, ...overrides };
}

/** A survey of the moon, a spheroid with no sea: held, live, 77 cells surveyed, by default. */
function aSurvey(overrides: Partial<TerrainSurvey> = {}): TerrainSurvey {
  return {
    status: standing(),
    coverage: coverageOf77(),
    header: moonLikeHeader(FIXTURE_MOON),
    stale: false,
    ...overrides,
  };
}

/** No survey held, and `status` its standing. */
function noSurvey(status: FieldStatus): TerrainSurvey {
  return aSurvey({ coverage: null, header: null, status });
}

/** The readout's answer where the line of sight meets ground the ship has not surveyed. */
const NOT_SURVEYED: SurfaceGroundReadout = { kind: "not-surveyed" };

/** The terrain lines `useTerrainLines` makes of `input`: a held survey, no answer, by default. */
function linesOf(input: Partial<TerrainLabelInput> = {}): TerrainLines {
  const full: TerrainLabelInput = {
    survey: aSurvey(),
    readout: null,
    faults: [],
    slantFromCamera: false,
    padWidthM: 1,
    ...input,
  };
  const model = terrainLabelModel(full, NO_READOUT_UNITS);
  return {
    model,
    header: full.survey?.header ?? null,
    readout: model.readout.length > 0 ? full.readout : null,
  };
}

describe("the lower plate's terrain parts (R10.T12; decision-r10-view-block-room §3)", () => {
  it("puts the survey line, DATUM and the readout in that order while the readout stands", () => {
    const parts = terrainLowerParts(linesOf({ readout: NOT_SURVEYED }));
    expect(parts.map((part) => part.kind)).toEqual(["survey", "datum", "readout"]);
  });

  it("puts a failure's state after the survey line where a survey is held", () => {
    const failure: FieldFailure = { kind: "invalid", check: "coverage unusable" };
    const parts = terrainLowerParts(
      linesOf({
        survey: aSurvey({ stale: true, status: standing({ failure }) }),
        readout: NOT_SURVEYED,
      }),
    );
    expect(parts.map((part) => part.kind)).toEqual(["survey", "state", "datum", "readout"]);
  });

  it.each([
    ["PENDING", null, "SURVEY COVERAGE: PENDING"],
    [
      "NOT YET MODELLED",
      noSurvey(standing({ notModelled: "surface" })),
      "TERRAIN: NOT YET MODELLED",
    ],
    [
      "REJECTED",
      noSurvey(
        standing({
          failure: {
            kind: "rejected",
            code: "unknown_body",
            message: "no such body",
            refusal: true,
          },
        }),
      ),
      "SURVEY COVERAGE REJECTED: no such body, reopen VIEW to retry",
    ],
    [
      "INVALID",
      noSurvey(standing({ failure: { kind: "invalid", check: "payload cut short" } })),
      "SURVEY DATA INVALID: payload cut short, reopen VIEW to retry",
    ],
  ])("gives %s as the one part, a state keyed by its text, which it reads", (_, survey, text) => {
    const parts = terrainLowerParts(linesOf({ survey, readout: NOT_SURVEYED }));
    const [first] = parts;
    const { container } = render(<>{first?.node}</>);
    expect({
      parts: parts.map((part) => [part.kind, part.kind === "state" ? part.key : null]),
      reads: container.textContent,
    }).toEqual({ parts: [["state", text]], reads: text });
  });

  it("gives TERRAIN: NOT YET MODELLED as the one part where a kept scene leaves the surface unset", () => {
    const parts = terrainLowerParts(linesOf({ surfaceUnset: true, readout: NOT_SURVEYED }));
    expect(parts.map((part) => [part.kind, part.kind === "state" ? part.key : null])).toEqual([
      ["state", "TERRAIN: NOT YET MODELLED"],
    ]);
  });

  it.each([
    ["survey", "ABOVE 99.9 %"],
    ["datum", "SEA LEVEL"],
    ["readout", "GRAZING"],
  ] as const)(
    "gives the %s part its tallest form, which reads %s where the part as it stands does not",
    (kind, text) => {
      const part = terrainLowerParts(linesOf({ readout: NOT_SURVEYED })).find(
        (each) => each.kind === kind,
      );
      if (part === undefined || part.kind === "state") {
        throw new Error(`no ${kind} part was made`);
      }
      const tallest = render(<>{part.tallest}</>);
      const asItStands = render(<>{part.node}</>);
      expect({
        tallest: within(tallest.container).getByText(text),
        asItStands: within(asItStands.container).queryByText(text),
      }).toEqual({ tallest: expect.any(HTMLElement), asItStands: null });
    },
  );

  it("gives no parts where the view states no terrain lines", () => {
    expect(terrainLowerParts(null)).toEqual([]);
  });

  it.each<SightGap>(["rotationNotModelled", "rotationNotResolved", "far", "centre"])(
    "gives the readout part where no line of sight forms (%s), reckoned at the full readout's tallest form (R10.T12.a)",
    (gap) => {
      const parts = terrainLowerParts(linesOf({ sightGap: gap }));
      const readout = parts.find((part) => part.kind === "readout");
      if (readout === undefined || readout.kind === "state") {
        throw new Error("no readout part was made");
      }
      const asItStands = render(<>{readout.node}</>);
      const tallest = render(<>{readout.tallest}</>);
      expect({
        kinds: parts.map((part) => part.kind),
        reads: asItStands.container.textContent,
        tallest: within(tallest.container)
          .getAllByRole("term")
          .map((term) => term.textContent),
      }).toEqual({
        kinds: ["survey", "datum", "readout"],
        reads: `GROUND AT LINE OF SIGHT${sightGapLine(gap)}`,
        tallest: tallestTerrainLines(moonLikeHeader(FIXTURE_MOON), 1).readout.flatMap((line) =>
          line.kind === "reading" ? [line.label] : [],
        ),
      });
    },
  );
});
