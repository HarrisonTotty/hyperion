import { describe, expect, it } from "vitest";

import { SURVEY_PASS_PENDING, surveyPassStatement } from "./surveyPassStatement";

const BODY = "0200080020000000.0300";

describe("the inspection's survey pass on its label block (the owner's request)", () => {
  it("states SURVEY PASS: PENDING while the pass is out", () => {
    expect(surveyPassStatement(null)).toBe(SURVEY_PASS_PENDING);
  });

  it("states nothing once the pass is recorded, the survey line reading its coverage", () => {
    expect(surveyPassStatement({ kind: "recorded", body: BODY, level: 8, revision: 1 })).toBeNull();
  });

  it("states a refusal of what was asked in the text's colour, with its remedy", () => {
    expect(
      surveyPassStatement({
        kind: "refused",
        body: BODY,
        reason: "no such body",
        standing: "refused",
      }),
    ).toEqual({
      text: "SURVEY PASS REJECTED: no such body, reopen with VIEW BODY to retry",
      standing: "refused",
    });
  });

  it("states the server's own failure as a fault", () => {
    expect(
      surveyPassStatement({ kind: "refused", body: BODY, reason: " ", standing: "fault" }),
    ).toEqual({ text: "SURVEY PASS REJECTED: reopen with VIEW BODY to retry", standing: "fault" });
  });
});
