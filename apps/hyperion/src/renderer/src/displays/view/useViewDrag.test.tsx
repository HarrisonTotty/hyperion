import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { stylesheetRule } from "../../test/stylesheet";
import { dragTurn } from "../../view/camera/drag";
import type { ViewTurn } from "../../view/camera/look";
import { pressAt, pressMoved } from "./useViewDrag";
import { ViewCanvas } from "./ViewCanvas";

const SIZE = { widthPx: 640, heightPx: 360 };
const REM_PX = 16;

describe("a press on a view", () => {
  it("is a click until it strays a quarter of a rem, or half a rem for a finger", () => {
    const mouse = pressAt(1, { xPx: 100, yPx: 100 }, "mouse", REM_PX);
    const finger = pressAt(1, { xPx: 100, yPx: 100 }, "touch", REM_PX);
    expect([mouse.slopPx, finger.slopPx]).toEqual([4, 8]);
    expect(pressMoved(mouse, { xPx: 103, yPx: 102 }, SIZE, 60).turn).toBeNull();
    expect(pressMoved(finger, { xPx: 106, yPx: 104 }, SIZE, 60).turn).toBeNull();
    expect(pressMoved(mouse, { xPx: 104, yPx: 100 }, SIZE, 60).press.dragging).toBe(true);
  });

  it("turns from where it went down once it strays past the slop, then from move to move", () => {
    const press = pressAt(1, { xPx: 100, yPx: 100 }, "pen", REM_PX);
    const first = pressMoved(press, { xPx: 110, yPx: 100 }, SIZE, 60);
    const second = pressMoved(first.press, { xPx: 130, yPx: 90 }, SIZE, 60);
    expect(first.turn).toEqual(dragTurn({ xPx: 100, yPx: 100 }, { xPx: 110, yPx: 100 }, SIZE, 60));
    expect(second.turn).toEqual(dragTurn({ xPx: 110, yPx: 100 }, { xPx: 130, yPx: 90 }, SIZE, 60));
  });

  it("keeps dragging once it has, back inside the slop too", () => {
    const press = pressAt(1, { xPx: 100, yPx: 100 }, "mouse", REM_PX);
    const out = pressMoved(press, { xPx: 120, yPx: 100 }, SIZE, 60);
    expect(pressMoved(out.press, { xPx: 101, yPx: 100 }, SIZE, 60).turn).not.toBeNull();
  });
});

/** The canvas, with its turns, picks and presses recorded, and a button after it to Tab to. */
function setup() {
  const user = userEvent.setup();
  const turns: ViewTurn[] = [];
  const picks: Array<readonly [number, number]> = [];
  const onPress = vi.fn<() => void>();
  const onBlur = vi.fn<() => void>();
  render(
    <>
      <ViewCanvas
        canvasRef={() => undefined}
        stageRef={() => undefined}
        accessibleName="VIEW, WIREFRAME, PRIMARY, FREE"
        describedBy="legend"
        onKeyDown={() => undefined}
        onKeyUp={() => undefined}
        onBlur={onBlur}
        fovDeg={() => 60}
        remPx={REM_PX}
        onPress={onPress}
        onTurn={(turn) => {
          turns.push(turn);
        }}
        onPick={(xPx, yPx) => {
          picks.push([xPx, yPx]);
        }}
      >
        {null}
      </ViewCanvas>
      <button type="button">NEXT</button>
    </>,
  );
  const canvas = screen.getByRole("application");
  vi.spyOn(canvas, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: SIZE.widthPx, height: SIZE.heightPx }),
  );
  return { user, canvas, turns, picks, onPress, onBlur };
}

/** The yaws of the turns, summed. */
function yawOf(turns: ReadonlyArray<ViewTurn>): number {
  return turns.reduce((sum, turn) => sum + turn.yawRad, 0);
}

type Setup = ReturnType<typeof setup>;

/** How a press may end without its pointer's release on the canvas. */
const ENDINGS: ReadonlyArray<readonly [string, (view: Setup) => Promise<void>]> = [
  [
    "pointercancel",
    async ({ canvas }) => {
      fireEvent.pointerCancel(canvas, { pointerId: 1 });
      await Promise.resolve();
    },
  ],
  [
    "a lost capture",
    async ({ canvas }) => {
      fireEvent.lostPointerCapture(canvas, { pointerId: 1 });
      await Promise.resolve();
    },
  ],
  [
    "the canvas's blur",
    async ({ user }) => {
      await user.tab();
    },
  ],
];

describe("a view's canvas under the pointer (R07.T19.f)", () => {
  it("captures the pointer at a press and makes the view the one acted on", async () => {
    const { user, canvas, onPress } = setup();
    await user.pointer({
      keys: "[MouseLeft>]",
      target: canvas,
      coords: { clientX: 50, clientY: 60 },
    });
    expect([onPress.mock.calls.length, canvas.hasPointerCapture(1)]).toEqual([1, true]);
  });

  it("picks at the press point on a click, and turns nothing", async () => {
    const { user, canvas, turns, picks } = setup();
    await user.pointer([
      { keys: "[MouseLeft>]", target: canvas, coords: { clientX: 50, clientY: 60 } },
      { coords: { clientX: 52, clientY: 61 } },
      { keys: "[/MouseLeft]" },
    ]);
    expect([picks, turns, canvas.hasPointerCapture(1)]).toEqual([[[50, 60]], [], false]);
  });

  it("turns the camera on a drag, by the drag's whole travel, and picks nothing", async () => {
    const { user, canvas, turns, picks } = setup();
    await user.pointer([
      { keys: "[MouseLeft>]", target: canvas, coords: { clientX: 100, clientY: 180 } },
      { coords: { clientX: 140, clientY: 180 } },
      { coords: { clientX: 300, clientY: 180 } },
      { keys: "[/MouseLeft]" },
    ]);
    const whole = dragTurn({ xPx: 100, yPx: 180 }, { xPx: 300, yPx: 180 }, SIZE, 60);
    expect(yawOf(turns)).toBeCloseTo(whole.yawRad, 12);
    expect([picks, canvas.hasPointerCapture(1)]).toEqual([[], false]);
  });

  it("turns by the whole travel of a drag that runs past the canvas's edge", async () => {
    const { user, canvas, turns } = setup();
    await user.pointer([
      { keys: "[MouseLeft>]", target: canvas, coords: { clientX: 600, clientY: 180 } },
      { coords: { clientX: 900, clientY: 180 } },
    ]);
    expect(yawOf(turns)).toBeCloseTo(
      dragTurn({ xPx: 600, yPx: 180 }, { xPx: 900, yPx: 180 }, SIZE, 60).yawRad,
      12,
    );
  });

  it.each(ENDINGS)("ends a press without a pick or a turn on %s", async (_name, end) => {
    const view = setup();
    const { user, canvas, turns, picks } = view;
    await user.pointer([
      { keys: "[MouseLeft>]", target: canvas, coords: { clientX: 100, clientY: 100 } },
      { coords: { clientX: 101, clientY: 100 } },
    ]);
    await end(view);
    const captured = canvas.hasPointerCapture(1);
    await user.pointer([
      { target: canvas, coords: { clientX: 300, clientY: 100 } },
      { keys: "[/MouseLeft]" },
    ]);
    expect([captured, picks, turns]).toEqual([false, [], []]);
  });

  it("releases the held flight keys as it ends a drag on the canvas's blur", async () => {
    const { user, canvas, onBlur } = setup();
    await user.pointer({
      keys: "[MouseLeft>]",
      target: canvas,
      coords: { clientX: 1, clientY: 1 },
    });
    await user.tab();
    expect(onBlur).toHaveBeenCalledTimes(1);
  });

  it("ignores the secondary button", async () => {
    const { user, canvas, picks, onPress } = setup();
    await user.pointer({
      keys: "[MouseRight]",
      target: canvas,
      coords: { clientX: 100, clientY: 100 },
    });
    expect([picks, onPress.mock.calls.length]).toEqual([[], 0]);
  });

  it("ignores a second pointer while one is down", async () => {
    const { user, canvas, turns, picks } = setup();
    await user.pointer([
      { keys: "[TouchA>]", target: canvas, coords: { clientX: 100, clientY: 100 } },
      { keys: "[TouchB>]", target: canvas, coords: { clientX: 300, clientY: 100 } },
      { pointerName: "TouchB", coords: { clientX: 500, clientY: 100 } },
      { keys: "[/TouchB]" },
      { keys: "[/TouchA]" },
    ]);
    expect([turns, picks]).toEqual([[], [[100, 100]]]);
  });
});

describe("a view's canvas as it is drawn (R07.T19.f)", () => {
  it("shows its focus ring whenever it holds the focus, after a press too", () => {
    expect(stylesheetRule(".view__canvas:focus")).toContain("outline: 2px solid var(--accent)");
  });

  it("takes the grab cursor, grabbing while pressed, and gives a finger's drag to the view", () => {
    expect([
      stylesheetRule(".view__canvas").includes("cursor: grab;"),
      stylesheetRule(".view__canvas").includes("touch-action: none;"),
      stylesheetRule(".view__canvas:active").includes("cursor: grabbing;"),
    ]).toEqual([true, true, true]);
  });
});
