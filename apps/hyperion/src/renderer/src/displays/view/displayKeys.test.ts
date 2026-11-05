import { describe, expect, it } from "vitest";

import { hiddenControlKeys } from "./displayKeys";

describe("MAXIMIZE VIEW's hidden keys", () => {
  it("names ORBIT's key after FREE's where the CONTROLS view's scene offers it (plan R15, T4)", () => {
    expect([hiddenControlKeys(false, true), hiddenControlKeys(false, false)]).toEqual([
      "1 SEAT, 2 CHASE, 3 FREE, 0 ORBIT, 4 STYLE, 7 ENABLE, 8 INHIBIT, 9 LABEL BLOCK, [/] TARGET, +/- FOV",
      "1 SEAT, 2 CHASE, 3 FREE, 4 STYLE, 7 ENABLE, 8 INHIBIT, 9 LABEL BLOCK, [/] TARGET, +/- FOV",
    ]);
  });
});
