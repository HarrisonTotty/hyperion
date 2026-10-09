import { describe, expect, it } from "vitest";

import type { RenderView } from "../view/engine/types";
import {
  childPacing,
  type ClosingChild,
  closeRelease,
  type HeldChildView,
  holdChildView,
  resizeDue,
  resizedFrom,
} from "./childWindow";
import { frameOf } from "./harness";

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

/** A child window whose `pagehide` comes only when the test fires it, as a late one does. */
class FakeChildWindow implements ClosingChild {
  closes = 0;
  /** Runs inside `close()`, for a `pagehide` that comes at once. */
  onClose: () => void = () => {};
  readonly listeners = new Set<() => void>();

  close(): void {
    this.closes += 1;
    this.onClose();
  }
  addEventListener(_type: "pagehide", listener: () => void): void {
    this.listeners.add(listener);
  }
  removeEventListener(_type: "pagehide", listener: () => void): void {
    this.listeners.delete(listener);
  }
  firePagehide(): void {
    for (const listener of this.listeners) {
      listener();
    }
  }
}

/** A view that counts its disposals and the frames it was given. */
function countingView(): RenderView & { disposals: number; frames: number } {
  return {
    name: "child instrument",
    disposals: 0,
    frames: 0,
    resize: () => {},
    render() {
      this.frames += 1;
    },
    readBack: () => Promise.resolve(new Uint8Array(0)),
    dispose() {
      this.disposals += 1;
    },
  };
}

/** One of the opener's frames: it draws the child's view while one is held. */
function openerFrame(held: HeldChildView): void {
  held.view()?.render(frameOf("child instrument", []));
}

describe("the child's view, dropped at the first of the opener's close and its pagehide", () => {
  it("is dropped at the opener's close, before a pagehide that comes after the next frame", () => {
    const child = new FakeChildWindow();
    const view = countingView();
    const held = holdChildView(child, view);
    openerFrame(held);
    held.close();
    expect([view.disposals, held.view(), held.releasedBy(), child.closes]).toEqual([
      1,
      null,
      "close",
      1,
    ]);
    openerFrame(held);
    child.firePagehide();
    expect([view.frames, view.disposals, held.releasedBy()]).toEqual([1, 1, "close"]);
  });

  it("is dropped before the window closes, so a pagehide inside close() drops nothing", () => {
    const child = new FakeChildWindow();
    const view = countingView();
    const held = holdChildView(child, view);
    let disposalsAtClose: number | null = null;
    child.onClose = () => {
      disposalsAtClose = view.disposals;
      child.firePagehide();
    };
    held.close();
    expect([disposalsAtClose, view.disposals, held.releasedBy()]).toEqual([1, 1, "close"]);
  });

  it("is dropped on the pagehide of a close the opener did not make, and once only", () => {
    const child = new FakeChildWindow();
    const view = countingView();
    const held = holdChildView(child, view);
    child.firePagehide();
    expect([view.disposals, held.view(), held.releasedBy()]).toEqual([1, null, "pagehide"]);
    held.close();
    child.firePagehide();
    expect([view.disposals, held.releasedBy()]).toEqual([1, "pagehide"]);
  });

  it("stops listening for the pagehide once the view is dropped", () => {
    const child = new FakeChildWindow();
    const held = holdChildView(child, countingView());
    expect(child.listeners.size).toBe(1);
    held.close();
    expect(child.listeners.size).toBe(0);
  });
});

describe("the record of the child's release", () => {
  it("passes a drop at the close whose pagehide came before the opener's next frame", () => {
    expect(closeRelease("close", 120, 120)).toEqual({
      pass: true,
      detail: "at the close; the pagehide came before the opener's next frame and dropped nothing",
    });
  });

  it("passes a drop at the close whose pagehide came frames later, counting them", () => {
    expect(closeRelease("close", 120, 121)).toEqual({
      pass: true,
      detail: "at the close; the pagehide came 1 of the opener's frames later and dropped nothing",
    });
  });

  it("fails where no pagehide came", () => {
    expect(closeRelease("close", 120, null).pass).toBe(false);
  });

  it("fails where the pagehide dropped the view before the opener closed the child", () => {
    expect(closeRelease("pagehide", 120, 90).pass).toBe(false);
  });

  it("fails where the view was never dropped", () => {
    expect(closeRelease(null, 120, 120).pass).toBe(false);
  });
});

describe("the child's resize", () => {
  it("is due once, halfway through a shown child's time", () => {
    expect([
      resizeDue(false, false, 2_999, 6),
      resizeDue(false, false, 3_000, 6),
      resizeDue(false, true, 3_500, 6),
    ]).toEqual([false, true, false]);
  });

  it("is never due for a hidden child", () => {
    expect(resizeDue(true, false, 3_000, 6)).toBe(false);
  });

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
