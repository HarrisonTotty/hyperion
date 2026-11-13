import { describe, expect, it } from "vitest";

import { terrainStatementOf } from "../../displays/view/viewFaults";
import {
  isTerrainFault,
  TERRAIN_COMPUTATION_FAILED,
  TERRAIN_NOT_AVAILABLE,
} from "../terrain/annunciation";
import { terrainTestGolden } from "../scenes/terrainScene";
import type { FieldFailure } from "../terrain/coarseFieldStore";
import { CoverageView } from "../terrain/coverage";
import {
  NO_READOUT_UNITS,
  readoutLineText,
  surveyLines,
  terrainLabelModel,
  terrainReadoutStands,
} from "../terrain/readouts";
import type { TerrainSurvey } from "../terrain/session";
import { SPIKE_RETRY, SPIKE_TERRAIN_FAULTS, spikeFaultLine } from "./spikeFaults";

/** A survey that holds no field and failed with `failure`. */
function failedSurvey(failure: FieldFailure): TerrainSurvey {
  return {
    status: { reason: "local-body", pending: false, notModelled: null, failure },
    coverage: null,
    header: null,
    stale: true,
  };
}

/** The survey's three failures: a refusal, the server's own failure, and a check that failed. */
const FAILURES: ReadonlyArray<FieldFailure> = [
  { kind: "rejected", code: "unknown_body", message: "no such body", refusal: true },
  { kind: "rejected", code: "internal", message: "out of memory", refusal: false },
  { kind: "invalid", check: "payload cut short" },
];

/** The survey line's texts and colours for `survey` with `remedy`. */
function shown(survey: TerrainSurvey, remedy?: string): Array<readonly [string, string]> {
  return surveyLines(survey, NO_READOUT_UNITS, remedy).lines.map(
    (line) => [readoutLineText(line), line.kind === "status" ? line.colour : "reading"] as const,
  );
}

describe("the descent spike's remedy (decision-r10-t14c-guide Q2)", () => {
  it("reads the terrain's fault of either cause with relaunch to retry", () => {
    expect([
      SPIKE_TERRAIN_FAULTS[TERRAIN_NOT_AVAILABLE],
      SPIKE_TERRAIN_FAULTS[TERRAIN_COMPUTATION_FAILED],
    ]).toEqual([
      "TERRAIN NOT AVAILABLE: height computation failed, relaunch to retry",
      "TERRAIN NOT AVAILABLE: terrain computation failed, relaunch to retry",
    ]);
  });

  it("shows VIEW's terrain faults in the spike's words, and any other fault as it is", () => {
    const graphics = "GRAPHICS STYLE REFUSED: photorealistic style not created, relaunch to retry";
    expect(
      [TERRAIN_NOT_AVAILABLE, TERRAIN_COMPUTATION_FAILED, graphics].map(spikeFaultLine),
    ).toEqual([
      "TERRAIN NOT AVAILABLE: height computation failed, relaunch to retry",
      "TERRAIN NOT AVAILABLE: terrain computation failed, relaunch to retry",
      graphics,
    ]);
  });

  it("keeps VIEW's texts where the faults are tested, so that each test holds on the spike", () => {
    const golden = terrainTestGolden();
    const held: TerrainSurvey = {
      status: { reason: "local-body", pending: false, notModelled: null, failure: null },
      coverage: CoverageView.empty(golden.header.level),
      header: golden.header,
      stale: false,
    };
    const answered = { kind: "no-ground" } as const;
    expect([
      terrainStatementOf("TERRAIN: STREAMING", []),
      terrainReadoutStands(held, [], answered),
      ...[TERRAIN_NOT_AVAILABLE, TERRAIN_COMPUTATION_FAILED].map((fault) => [
        isTerrainFault(fault),
        terrainStatementOf("TERRAIN: STREAMING", [fault]),
        terrainReadoutStands(held, [fault], answered),
      ]),
    ]).toEqual(["TERRAIN: STREAMING", true, [true, null, false], [true, null, false]]);
  });

  it("ends the survey line's REJECTED and INVALID lines with relaunch to retry, in their colours", () => {
    expect(FAILURES.map((failure) => shown(failedSurvey(failure), SPIKE_RETRY))).toEqual([
      [["SURVEY COVERAGE REJECTED: no such body, relaunch to retry", "text"]],
      [["SURVEY COVERAGE REJECTED: out of memory, relaunch to retry", "status-caution"]],
      [["SURVEY DATA INVALID: payload cut short, relaunch to retry", "status-caution"]],
    ]);
  });

  it("gives the remedy through the label model, VIEW's own where none is given", () => {
    const survey = failedSurvey({ kind: "invalid", check: "payload cut short" });
    const model = (remedy?: string): string =>
      terrainLabelModel(
        { survey, readout: null, faults: [], slantFromCamera: false, padWidthM: 1, remedy },
        NO_READOUT_UNITS,
      ).survey.map(readoutLineText)[0] ?? "";
    expect([model(SPIKE_RETRY), model()]).toEqual([
      "SURVEY DATA INVALID: payload cut short, relaunch to retry",
      "SURVEY DATA INVALID: payload cut short, reopen VIEW to retry",
    ]);
  });
});
