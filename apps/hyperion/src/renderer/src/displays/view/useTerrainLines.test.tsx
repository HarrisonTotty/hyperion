import { renderHook, type RenderHookResult } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { moonLikeHeader } from "../../test/coarseFieldFixtures";
import { FIXTURE_MOON, FIXTURE_PLANET } from "../../test/viewFixtures";
import { TERRAIN_COMPUTATION_FAILED } from "../../view/terrain/annunciation";
import type { FieldStatus } from "../../view/terrain/coarseFieldStore";
import { CoverageView } from "../../view/terrain/coverage";
import { NO_SIGHT_FAR, NO_SIGHT_ROTATION, readoutLineText } from "../../view/terrain/readouts";
import type { TerrainSurvey } from "../../view/terrain/session";
import type { SurfaceGroundReadout } from "../../view/terrain/workers/messages";
import type { GroundSnapshot } from "./groundReadout";
import { standingReadout, type TerrainLines, useTerrainLines } from "./useTerrainLines";

/** No fault lines on the view. */
const NO_FAULTS: ReadonlyArray<string> = [];

/** A coverage of level 5 with its first five cells surveyed at cell code `code`. */
function coverageAt(code: number): CoverageView {
  const merged = CoverageView.empty(5).merged(
    { ranges: [{ start: 0, end: 5, resolution: code }] },
    1,
  );
  if (!merged.ok) {
    throw new Error(merged.message);
  }
  return merged.coverage;
}

/** Cell code 62, 1 cm × 2^(61 ÷ 8), about 2 m: `FINEST` in metres. */
const ABOUT_2_M = 62;

/** Cell code 134, 1 cm × 2^(133 ÷ 8), about 1011 m: within 5% of 1 km, so held in metres. */
const ABOUT_1_KM = 134;

/** A field's standing: wanted as the local body, answered, nothing failed. */
function standing(): FieldStatus {
  return { reason: "local-body", pending: false, notModelled: null, failure: null };
}

/** A survey of the moon, held and live, its first cells surveyed at about 2 m by default. */
function aSurvey(overrides: Partial<TerrainSurvey> = {}): TerrainSurvey {
  return {
    status: standing(),
    coverage: coverageAt(ABOUT_2_M),
    header: moonLikeHeader(FIXTURE_MOON),
    stale: false,
    ...overrides,
  };
}

/** The readout's answer where the line of sight meets ground the ship has not surveyed. */
const NOT_SURVEYED: SurfaceGroundReadout = { kind: "not-surveyed" };

/** A publish's snapshot of the moon: its survey held, no answer yet, no fault, by default. */
function snapshotOf(overrides: Partial<GroundSnapshot> = {}): GroundSnapshot {
  return {
    body: FIXTURE_MOON,
    surfaceUnset: false,
    survey: aSurvey(),
    readout: null,
    sessionFault: null,
    slantFromCamera: false,
    ...overrides,
  };
}

/** The hook's props: the latest snapshot. */
interface LinesProps {
  readonly snapshot: GroundSnapshot | null;
}

/** The hook rendered on `snapshot`, with no fault lines. */
function renderLines(
  snapshot: GroundSnapshot | null,
): RenderHookResult<TerrainLines | null, LinesProps> {
  return renderHook(({ snapshot: shown }: LinesProps) => useTerrainLines(shown, NO_FAULTS), {
    initialProps: { snapshot },
  });
}

/** The survey line's text, or `null` with no lines. */
function surveyText(lines: TerrainLines | null): string | null {
  const [first] = lines?.model.survey ?? [];
  return first === undefined ? null : readoutLineText(first);
}

describe("useTerrainLines (R10.T12)", () => {
  it("keeps FINEST's unit within its hysteresis from one snapshot of a body to the next", () => {
    const hook = renderLines(snapshotOf({ survey: aSurvey({ coverage: coverageAt(ABOUT_2_M) }) }));
    hook.rerender({
      snapshot: snapshotOf({ survey: aSurvey({ coverage: coverageAt(ABOUT_1_KM) }) }),
    });
    expect(surveyText(hook.result.current)).toMatch(/FINEST 1000 m$/u);
  });

  it("starts FINEST's unit afresh for another body", () => {
    const hook = renderLines(snapshotOf({ survey: aSurvey({ coverage: coverageAt(ABOUT_2_M) }) }));
    hook.rerender({
      snapshot: snapshotOf({
        body: FIXTURE_PLANET,
        survey: aSurvey({
          header: moonLikeHeader(FIXTURE_PLANET),
          coverage: coverageAt(ABOUT_1_KM),
        }),
      }),
    });
    expect(surveyText(hook.result.current)).toMatch(/FINEST 1\.0 km$/u);
  });

  it("states the readout and DATUM from the snapshot's answer while no fault stands", () => {
    const { result } = renderLines(snapshotOf({ readout: NOT_SURVEYED }));
    expect({
      answer: result.current?.readout,
      datum: result.current?.model.datum !== null,
      readout: result.current?.model.readout.map(readoutLineText),
    }).toEqual({
      answer: NOT_SURVEYED,
      datum: true,
      readout: ["GROUND AT LINE OF SIGHT", "NOT SURVEYED"],
    });
  });

  it("states no readout or DATUM under the session's terrain fault", () => {
    const { result } = renderLines(
      snapshotOf({ readout: NOT_SURVEYED, sessionFault: TERRAIN_COMPUTATION_FAILED }),
    );
    expect({
      answer: result.current?.readout,
      datum: result.current?.model.datum,
      readout: result.current?.model.readout,
    }).toEqual({ answer: null, datum: null, readout: [] });
  });

  it("reads TERRAIN: NOT YET MODELLED alone where a kept scene leaves the surface unset", () => {
    const { result } = renderLines(snapshotOf({ surfaceUnset: true, readout: NOT_SURVEYED }));
    expect({
      survey: result.current?.model.survey.map(readoutLineText),
      datum: result.current?.model.datum,
      readout: result.current?.model.readout,
      answer: result.current?.readout,
    }).toEqual({ survey: ["TERRAIN: NOT YET MODELLED"], datum: null, readout: [], answer: null });
  });

  it("gives no lines for no snapshot, before any lines and after them", () => {
    const before = renderLines(null).result.current;
    const hook = renderLines(snapshotOf());
    hook.rerender({ snapshot: null });
    expect([before, hook.result.current]).toEqual([null, null]);
  });
});

describe("the readout the plate states, which its tripod and note follow (decision-r10-view-block-room §3)", () => {
  it("is the lines' answer while the plate's readout stands", () => {
    const { result } = renderLines(snapshotOf({ readout: NOT_SURVEYED }));
    expect(standingReadout(result.current, "stands")).toEqual(NOT_SURVEYED);
  });

  it("is none while the plate's readout gives way for room, so that no tripod or note stands", () => {
    const { result } = renderLines(snapshotOf({ readout: NOT_SURVEYED }));
    expect(standingReadout(result.current, "yields")).toBeNull();
  });

  it("is none with no lines", () => {
    expect(standingReadout(null, "stands")).toBeNull();
  });

  it("is none where no line of sight forms, so that no tripod or note stands (R10.T12.a)", () => {
    const { result } = renderLines(snapshotOf({ sightGap: "far" }));
    expect(standingReadout(result.current, "stands")).toBeNull();
  });
});

describe("useTerrainLines where no line of sight forms (R10.T12.a; decision-r10-t12a-scope §3.2)", () => {
  it("passes the snapshot's cause on: the heading and its NOT AVAILABLE line, with DATUM", () => {
    const { result } = renderLines(snapshotOf({ sightGap: "rotation" }));
    expect({
      readout: result.current?.model.readout.map(readoutLineText),
      datum: result.current?.model.datum === null ? null : "DATUM",
      answer: result.current?.readout,
    }).toEqual({
      readout: ["GROUND AT LINE OF SIGHT", NO_SIGHT_ROTATION],
      datum: "DATUM",
      answer: null,
    });
  });

  it("follows the cause from one snapshot to the next", () => {
    const hook = renderLines(snapshotOf({ sightGap: "rotation" }));
    hook.rerender({ snapshot: snapshotOf({ sightGap: "far" }) });
    expect(hook.result.current?.model.readout.map(readoutLineText)).toEqual([
      "GROUND AT LINE OF SIGHT",
      NO_SIGHT_FAR,
    ]);
  });

  it("gives the lines it gave before for a snapshot made without the cause, as the descent spike's", () => {
    // The spike's survey snapshot (`surveySnapshotOf`) carries no `sightGap` key at all.
    const spike: GroundSnapshot = {
      body: FIXTURE_MOON,
      surfaceUnset: false,
      survey: aSurvey(),
      readout: null,
      sessionFault: null,
      slantFromCamera: false,
    };
    const { result } = renderLines(spike);
    expect({
      survey: result.current?.model.survey.length,
      datum: result.current?.model.datum,
      readout: result.current?.model.readout,
      answer: result.current?.readout,
    }).toEqual({ survey: 1, datum: null, readout: [], answer: null });
  });
});
