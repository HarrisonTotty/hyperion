import { describe, expect, it } from "vitest";

import { childPacing, pagehideRelease, resizedFrom } from "./childWindow";

const at = (ms: number, n = 100): number[] => Array.from({ length: n }, () => ms);

describe("the child's pacing", () => {
  it("is its own display's where the displays' rates differ", () => {
    const pacing = childPacing(at(1000 / 75), at(1000 / 60), {
      frameName: "child",
      mainHz: 60,
      childHz: 75,
      hidden: false,
    });
    expect(pacing.pass).toBe(true);
    expect(pacing.detail).toContain("the displays' rates differ");
  });

  it("fails a child paced by the opener's display instead of its own", () => {
    expect(
      childPacing(at(1000 / 60), at(1000 / 60), {
        frameName: "child",
        mainHz: 60,
        childHz: 75,
        hidden: false,
      }).pass,
    ).toBe(false);
  });

  it("says when one rate on both displays cannot tell them apart", () => {
    expect(
      childPacing(at(1000 / 60), at(1000 / 60), {
        frameName: "child",
        mainHz: 60,
        childHz: 60,
        hidden: false,
      }).detail,
    ).toContain("cannot tell");
  });

  it("cannot tell displays whose periods lie within the band apart", () => {
    expect(
      childPacing(at(1000 / 60), at(1000 / 60), {
        frameName: "child",
        mainHz: 60,
        childHz: 61.5,
        hidden: false,
      }).detail,
    ).toContain("cannot tell");
  });

  it("reads a hidden child as offscreen pacing", () => {
    const pacing = childPacing(at(16.7), at(16.7), {
      frameName: "child",
      mainHz: 60,
      childHz: 60,
      hidden: true,
    });
    expect(pacing.pass).toBe(true);
    expect(pacing.detail).toContain("offscreen pacing");
  });

  it("fails a child that drew no frame", () => {
    expect(
      childPacing([], at(16.7), { frameName: "child", mainHz: 60, childHz: 75, hidden: false })
        .pass,
    ).toBe(false);
  });
});

describe("the child's release on its pagehide", () => {
  it("holds where the pagehide came before the opener's next frame", () => {
    expect(pagehideRelease(120, 120)).toEqual({
      pass: true,
      detail: "before the opener's next frame after the close",
    });
  });

  it("counts the opener's frames that drew the closing child first, the errors judging them", () => {
    expect(pagehideRelease(120, 122)).toEqual({
      pass: true,
      detail:
        "2 of the opener's frames after the close, each still drawing the closing child (see the uncaptured GPU errors)",
    });
  });

  it("fails where no pagehide came", () => {
    expect(pagehideRelease(120, null).pass).toBe(false);
  });
});

describe("the child's resize", () => {
  it("shows where a size after the resize differs from the last before it", () => {
    expect(resizedFrom(["640 × 480", "800 × 600"], 1)).toBe(true);
  });

  it("does not show where only sizes before the resize differ", () => {
    expect(resizedFrom(["1 × 1", "640 × 480"], 2)).toBe(false);
  });

  it("does not show before any size was drawn", () => {
    expect(resizedFrom(["800 × 600"], 0)).toBe(false);
  });
});
