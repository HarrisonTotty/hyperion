import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, onTestFinished, vi } from "vitest";

import { FakeResizeObserver } from "../test/FakeResizeObserver";
import { stubMatchMedia } from "../test/stubMatchMedia";
import { type RecordingContext2D, stubCanvas } from "../test/RecordingContext2D";
import { type CameraAngles, project, viewBasis } from "./camera";
import { easeOut, TRANSITION_MS, tweenCamera } from "./transition";
import { localFrameAt } from "./frame";
import type { PointMark, SpatialScene } from "./marks";
import { SpatialView, type SpatialViewProps } from "./SpatialView";
import { vec3 } from "./vec3";

const FRAME = localFrameAt(vec3(26_000, 0, 0));

function aMark(id: string, overrides: Partial<PointMark> = {}): PointMark {
  return {
    id,
    position: vec3(-10, 5, 8),
    shape: "circle",
    sizeClass: 2,
    status: "plain",
    label: id.toUpperCase(),
    labelPriority: 1,
    ...overrides,
  };
}

function aScene(overrides: Partial<SpatialScene> = {}): SpatialScene {
  return {
    frame: FRAME,
    points: [aMark("a")],
    spheres: [{ radius: 50, role: "data_edge", label: "QUERY EDGE 50 ly" }],
    plane: { spacing: 20, extent: 50, rings: [{ radius: 20, label: "" }] },
    selectedId: null,
    destinationId: null,
    ...overrides,
  };
}

/** Lays every element out at `widthPx` by `heightPx`: the stage the view measures, and its canvas. */
function stubLayout(widthPx = 400, heightPx = 300): void {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: widthPx, height: heightPx }),
  );
}

function viewOf(props: Partial<SpatialViewProps>) {
  return (
    <SpatialView
      scene={aScene()}
      fitRadius={50}
      formatLength={(length) => `${length} ly`}
      frameName="GALACTIC"
      accessibleName="Local chart"
      onSelect={() => undefined}
      {...props}
    />
  );
}

function renderView(props: Partial<SpatialViewProps> = {}) {
  const recorder = stubCanvas();
  const result = render(viewOf(props));
  return {
    recorder,
    rerender: (next: Partial<SpatialViewProps>) => {
      result.rerender(viewOf(next));
    },
    unmount: result.unmount,
  };
}

/** How many times the view's canvas was painted: each paint begins by clearing it. */
function paints(recorder: RecordingContext2D): number {
  return recorder.calls("fillRect").length;
}

describe("SpatialView", () => {
  it("is an application named for its view, reached with Tab", async () => {
    const user = userEvent.setup();
    stubLayout();
    renderView();

    // Past its four preset views, which come first.
    await user.tab();
    await user.tab();
    await user.tab();
    await user.tab();
    await user.tab();

    expect(screen.getByRole("application", { name: "Local chart" })).toHaveFocus();
  });

  it("puts its preset views before its canvas in the Tab order", async () => {
    const user = userEvent.setup();
    stubLayout();
    renderView();

    await user.tab();

    expect(screen.getByRole("button", { name: "T TOP" })).toHaveFocus();
  });

  it("is described by its visible key legend", () => {
    stubLayout();
    renderView();

    expect(screen.getByRole("application", { name: "Local chart" })).toHaveAccessibleDescription(
      "ARROWS ROTATE +/− ZOOM T S F O VIEWS Z FIT",
    );
  });

  it("paints once when it is mounted and laid out", () => {
    stubLayout();
    const { recorder } = renderView();

    expect(paints(recorder)).toBe(1);
  });

  it("paints nothing before it is laid out", () => {
    const { recorder } = renderView();

    expect(paints(recorder)).toBe(0);
  });

  it("fits a sphere of the fit radius into the view's shorter side at first", () => {
    stubLayout(400, 300);
    const { recorder } = renderView();

    // Half the shorter side, 150 px, less the 2 rem margin: the query edge is 118 px in radius.
    const circle = recorder
      .calls("arc")
      .find(({ args }) => args[0] === 200 && args[1] === 150 && args[2] !== 0);
    expect(circle?.args[2]).toBeCloseTo(118, 9);
  });

  it("fits a new fit radius while the operator has not zoomed", () => {
    stubLayout(400, 300);
    const { recorder, rerender } = renderView();
    recorder.clear();

    rerender({
      fitRadius: 25,
      scene: aScene({ spheres: [{ radius: 25, role: "data_edge", label: "QUERY EDGE 25 ly" }] }),
    });

    const circle = recorder
      .calls("arc")
      .find(({ args }) => args[0] === 200 && args[1] === 150 && args[2] !== 0);
    expect(circle?.args[2]).toBeCloseTo(118, 9);
  });

  it("backs the canvas at the device pixel ratio", () => {
    vi.stubGlobal("devicePixelRatio", 2);
    stubLayout(400, 300);
    const { recorder } = renderView();

    const canvas = screen.getByRole("application", { name: "Local chart" });
    expect(canvas).toHaveAttribute("width", "800");
    expect(canvas).toHaveAttribute("height", "600");
    expect(recorder.calls("setTransform").at(-1)?.args).toEqual([2, 0, 0, 2, 0, 0]);
  });

  it("paints nothing more when rendered again with the same props", () => {
    stubLayout();
    const scene = aScene();
    const { recorder, rerender } = renderView({ scene });

    rerender({ scene });

    expect(paints(recorder)).toBe(1);
  });

  it("paints once for a new scene", () => {
    stubLayout();
    const { recorder, rerender } = renderView();
    recorder.clear();

    rerender({ scene: aScene({ points: [aMark("b")] }) });

    expect(paints(recorder)).toBe(1);
  });

  it("paints again at its new size when it is resized", () => {
    stubLayout(400, 300);
    const { recorder } = renderView();
    recorder.clear();

    stubLayout(500, 300);
    act(() => {
      FakeResizeObserver.resizeAll();
    });

    expect(paints(recorder)).toBe(1);
    expect(screen.getByRole("application", { name: "Local chart" })).toHaveAttribute(
      "width",
      "500",
    );
  });

  it("draws its furniture over the canvas", () => {
    stubLayout();
    renderView({ children: <p>SYMBOLS NOT TO SCALE</p> });

    expect(screen.getByText("SYMBOLS NOT TO SCALE")).toBeInTheDocument();
  });

  it("stops observing its size when unmounted", () => {
    stubLayout();
    const { unmount } = renderView();
    expect(FakeResizeObserver.instances.some((observer) => observer.observed.size > 0)).toBe(true);

    unmount();

    expect(FakeResizeObserver.instances.every((observer) => observer.observed.size === 0)).toBe(
      true,
    );
  });
});

/** The mark's position in the default scene, which the keyboard tests follow. */
const MARK_POSITION = vec3(-10, 5, 8);

/** The zoom that fits the 50 ly default fit radius into a 400 × 300 view: 118 px over 50 ly. */
const FITTED_PX_PER_LY = 118 / 50;

/**
 * Where a camera at `angles` and `pxPerUnit` puts the default scene's mark on a 400 × 300 view, as
 * a matcher for the painted centre.
 */
function markAt(angles: CameraAngles, pxPerUnit = FITTED_PX_PER_LY): unknown[] {
  const projected = project(
    MARK_POSITION,
    viewBasis(FRAME, angles),
    { ...angles, pxPerUnit },
    { widthPx: 400, heightPx: 300, remPx: 16 },
  );
  return [expect.closeTo(projected.xPx, 9), expect.closeTo(projected.yPx, 9)];
}

/** The centre of the default mark's symbol (5.25 px in radius) as last painted. */
function markCentre(recorder: RecordingContext2D): ReadonlyArray<unknown> {
  return (
    recorder
      .calls("arc")
      .findLast(({ args }) => args[2] === 5.25)
      ?.args.slice(0, 2) ?? []
  );
}

/** The radius of the query edge, a circle about the view centre, as last painted. */
function edgeRadius(recorder: RecordingContext2D): unknown {
  return recorder
    .calls("arc")
    .findLast(({ args }) => args[0] === 200 && args[1] === 150 && args[2] !== 5.25)?.args[2];
}

/** Lets the frame that input asked for run. */
function nextFrame(): void {
  act(() => {
    vi.advanceTimersToNextFrame();
  });
}

/** Records, after every handler has run, whether each key's default action was prevented. */
function watchDefaults(): boolean[] {
  const prevented: boolean[] = [];
  const record = (event: KeyboardEvent): void => {
    prevented.push(event.defaultPrevented);
  };
  window.addEventListener("keydown", record);
  onTestFinished(() => {
    window.removeEventListener("keydown", record);
  });
  return prevented;
}

describe("SpatialView from the keyboard", () => {
  beforeEach(() => {
    // Frames alone are faked: `user-event` waits on real timeouts.
    vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame"] });
    stubLayout(400, 300);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  /** Renders the view beside a text field and a button that are not part of it. */
  function setup(props: Partial<SpatialViewProps> = {}) {
    const user = userEvent.setup();
    const recorder = stubCanvas();
    const siblings = (next: Partial<SpatialViewProps>) => (
      <>
        {viewOf(next)}
        <input aria-label="Name" />
        <button type="button">ELSEWHERE</button>
      </>
    );
    const result = render(siblings(props));
    return {
      user,
      recorder,
      canvas: screen.getByRole("application", { name: "Local chart" }),
      rerender: (next: Partial<SpatialViewProps>) => {
        result.rerender(siblings(next));
      },
      unmount: result.unmount,
    };
  }

  it("turns the azimuth 5° with the right arrow on the focused canvas", async () => {
    const { user, recorder, canvas } = setup();
    await user.click(canvas);

    await user.keyboard("{ArrowRight}");
    nextFrame();

    expect(markCentre(recorder)).toEqual(markAt({ azimuthDeg: 35, elevationDeg: 30 }));
  });

  it("turns 1° a press with Shift", async () => {
    const { user, recorder, canvas } = setup();
    await user.click(canvas);

    await user.keyboard("{Shift>}{ArrowLeft}{/Shift}");
    nextFrame();

    expect(markCentre(recorder)).toEqual(markAt({ azimuthDeg: 29, elevationDeg: 30 }));
  });

  it("raises the camera no further than +90°", async () => {
    const { user, recorder, canvas } = setup();
    await user.click(canvas);
    await user.keyboard("{ArrowUp>13/}");
    nextFrame();

    await user.keyboard("{ArrowDown}");
    nextFrame();

    // Stopped at +90°, one press down is +85°, not the +90° an unclamped +95° would give.
    expect(markCentre(recorder)).toEqual(markAt({ azimuthDeg: 30, elevationDeg: 85 }));
  });

  it("paints nothing for a press against a limit", async () => {
    const { user, recorder, canvas } = setup();
    await user.click(canvas);
    await user.keyboard("{ArrowUp>13/}");
    nextFrame();
    recorder.clear();

    await user.keyboard("{ArrowUp}");
    nextFrame();

    expect(paints(recorder)).toBe(0);
  });

  it("gathers the presses made before a frame into one paint", async () => {
    const { user, recorder, canvas } = setup();
    await user.click(canvas);
    recorder.clear();

    await user.keyboard("{ArrowRight}{ArrowRight}{ArrowRight}");
    nextFrame();

    expect(paints(recorder)).toBe(1);
    expect(markCentre(recorder)).toEqual(markAt({ azimuthDeg: 45, elevationDeg: 30 }));
  });

  it("applies the presses of one frame in turn, each within the limits", async () => {
    const { user, recorder, canvas } = setup();
    await user.click(canvas);

    await user.keyboard("{ArrowUp>13/}{ArrowDown}");
    nextFrame();

    expect(markCentre(recorder)).toEqual(markAt({ azimuthDeg: 30, elevationDeg: 85 }));
  });

  it("does not turn when the arrows are pressed elsewhere", async () => {
    const { user, recorder } = setup();
    await user.click(screen.getByRole("button", { name: "ELSEWHERE" }));
    recorder.clear();

    await user.keyboard("{ArrowRight}");
    nextFrame();

    expect(paints(recorder)).toBe(0);
  });

  it("keeps the page from scrolling while the canvas has focus", async () => {
    const { user, canvas } = setup();
    await user.click(canvas);
    const prevented = watchDefaults();

    await user.keyboard("{ArrowDown}");

    expect(prevented).toEqual([true]);
  });

  it("leaves the arrows elsewhere to the page", async () => {
    const { user } = setup();
    const prevented = watchDefaults();
    await user.click(screen.getByRole("button", { name: "ELSEWHERE" }));

    await user.keyboard("{ArrowDown}");

    expect(prevented).toEqual([false]);
  });

  it("ignores the arrows with Alt", async () => {
    const { user, recorder, canvas } = setup();
    await user.click(canvas);
    recorder.clear();

    await user.keyboard("{Alt>}{ArrowRight}{/Alt}");
    nextFrame();

    expect(paints(recorder)).toBe(0);
  });

  it("zooms in by 1.25 with +", async () => {
    const { user, recorder } = setup();

    await user.keyboard("+");
    nextFrame();

    expect(edgeRadius(recorder)).toBeCloseTo(118 * 1.25, 9);
  });

  it("zooms in with Shift and +, as some layouts type it", async () => {
    const { user, recorder } = setup();

    await user.keyboard("{Shift>}+{/Shift}");
    nextFrame();

    expect(edgeRadius(recorder)).toBeCloseTo(118 * 1.25, 9);
  });

  it("zooms out by 1.25 with -", async () => {
    const { user, recorder } = setup();

    await user.keyboard("-");
    nextFrame();

    expect(edgeRadius(recorder)).toBeCloseTo(118 / 1.25, 9);
  });

  it("zooms out with the minus sign", async () => {
    const { user, recorder } = setup();

    await user.keyboard("−");
    nextFrame();

    expect(edgeRadius(recorder)).toBeCloseTo(118 / 1.25, 9);
  });

  it("leaves Ctrl with = to the interface scale", async () => {
    const { user, recorder } = setup();
    const prevented = watchDefaults();
    recorder.clear();

    await user.keyboard("{Control>}={/Control}");
    nextFrame();

    expect(paints(recorder)).toBe(0);
    expect(prevented).toEqual([false, false]);
  });

  it("zooms in no further than 100 times the scale that fits", async () => {
    const { user, recorder } = setup();

    await user.keyboard("=".repeat(25));
    nextFrame();

    expect(edgeRadius(recorder)).toBeCloseTo(118 * 100, 6);
  });

  it("zooms out no further than half the scale that fits", async () => {
    const { user, recorder } = setup();

    await user.keyboard("-----");
    nextFrame();

    expect(edgeRadius(recorder)).toBeCloseTo(118 / 2, 9);
  });

  it("keeps a chosen zoom when the fit radius changes", async () => {
    const { user, recorder, rerender } = setup();
    await user.keyboard("+");
    nextFrame();

    rerender({
      fitRadius: 25,
      scene: aScene({ spheres: [{ radius: 25, role: "data_edge", label: "QUERY EDGE 25 ly" }] }),
    });

    // 1.25 times the scale that fitted 50 ly, 118 px, over the new 25 ly edge.
    expect(edgeRadius(recorder)).toBeCloseTo((118 / 50) * 1.25 * 25, 9);
  });

  it("fits the view again with Z", async () => {
    const { user, recorder } = setup();
    await user.keyboard("++");
    nextFrame();

    await user.keyboard("z");
    nextFrame();

    expect(edgeRadius(recorder)).toBeCloseTo(118, 9);
  });

  it("zooms from the fitted scale after Z in the same frame", async () => {
    const { user, recorder } = setup();

    await user.keyboard("++z+");
    nextFrame();

    expect(edgeRadius(recorder)).toBeCloseTo(118 * 1.25, 9);
  });

  it("does not fit with Shift and Z", async () => {
    const { user, recorder } = setup();
    await user.keyboard("+");
    nextFrame();

    await user.keyboard("{Shift>}Z{/Shift}");
    nextFrame();

    expect(edgeRadius(recorder)).toBeCloseTo(118 * 1.25, 9);
  });

  it("leaves the zoom keys to a text field that has focus", async () => {
    const { user, recorder } = setup();
    const field = screen.getByRole("textbox", { name: "Name" });
    await user.click(field);
    recorder.clear();

    await user.keyboard("+z");
    nextFrame();

    expect(field).toHaveValue("+z");
    expect(paints(recorder)).toBe(0);
  });

  it("cancels a frame still pending when it is unmounted", async () => {
    const { user, canvas, unmount } = setup();
    await user.click(canvas);
    await user.keyboard("{ArrowRight}");
    expect(vi.getTimerCount()).toBe(1);

    unmount();

    expect(vi.getTimerCount()).toBe(0);
  });
});

/** Where the default mark is painted at the starting camera, as client coordinates. */
function markPoint(): { readonly clientX: number; readonly clientY: number } {
  const projected = project(
    MARK_POSITION,
    viewBasis(FRAME, { azimuthDeg: 30, elevationDeg: 30 }),
    { azimuthDeg: 30, elevationDeg: 30, pxPerUnit: FITTED_PX_PER_LY },
    { widthPx: 400, heightPx: 300, remPx: 16 },
  );
  return { clientX: projected.xPx, clientY: projected.yPx };
}

describe("SpatialView from a pointer", () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame"] });
    stubLayout(400, 300);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  function setup(props: Partial<SpatialViewProps> = {}) {
    const user = userEvent.setup();
    const view = renderView(props);
    const canvas = screen.getByRole("application", { name: "Local chart" });
    /** Presses the primary button at `from`, moves to `to`, and lets go there. */
    const drag = async (
      from: readonly [number, number],
      to: readonly [number, number],
    ): Promise<void> => {
      await user.pointer([
        { keys: "[MouseLeft>]", target: canvas, coords: { clientX: from[0], clientY: from[1] } },
        { coords: { clientX: to[0], clientY: to[1] } },
        { keys: "[/MouseLeft]" },
      ]);
    };
    /** Puts two fingers down at `a` and `b`, then moves the second to `bTo`. */
    const pinch = async (
      a: readonly [number, number],
      b: readonly [number, number],
      bTo: readonly [number, number],
    ): Promise<void> => {
      await user.pointer([
        { keys: "[TouchA>]", target: canvas, coords: { clientX: a[0], clientY: a[1] } },
        { keys: "[TouchB>]", target: canvas, coords: { clientX: b[0], clientY: b[1] } },
        { pointerName: "TouchB", coords: { clientX: bTo[0], clientY: bTo[1] } },
      ]);
    };
    return { user, ...view, canvas, drag, pinch };
  }

  it("turns the azimuth 16° for a 32 px drag across, 8° a rem", async () => {
    const { recorder, drag } = setup();

    await drag([100, 100], [132, 100]);
    nextFrame();

    expect(markCentre(recorder)).toEqual(markAt({ azimuthDeg: 46, elevationDeg: 30 }));
  });

  it("raises the camera for a drag down", async () => {
    const { recorder, drag } = setup();

    await drag([100, 100], [100, 116]);
    nextFrame();

    expect(markCentre(recorder)).toEqual(markAt({ azimuthDeg: 30, elevationDeg: 38 }));
  });

  it("captures the pointer while it is down, so that a drag off the canvas still turns it", async () => {
    const { user, canvas } = setup();

    await user.pointer({
      keys: "[MouseLeft>]",
      target: canvas,
      coords: { clientX: 10, clientY: 10 },
    });

    expect(canvas.hasPointerCapture(1)).toBe(true);
  });

  it("lets the pointer go when it comes up", async () => {
    const { drag, canvas } = setup();

    await drag([10, 10], [10, 10]);

    expect(canvas.hasPointerCapture(1)).toBe(false);
  });

  it("takes a movement of 2 px for a click, which leaves the camera", async () => {
    const { recorder, drag } = setup();
    recorder.clear();

    await drag([100, 100], [102, 100]);
    nextFrame();

    expect(paints(recorder)).toBe(0);
  });

  it("reports a click on a mark", async () => {
    const onSelect = vi.fn<(id: string) => void>();
    const { drag } = setup({ onSelect });
    const { clientX, clientY } = markPoint();

    await drag([clientX, clientY], [clientX + 2, clientY]);

    expect(onSelect).toHaveBeenCalledWith("a");
  });

  it("reports no click at the end of a drag", async () => {
    const onSelect = vi.fn<(id: string) => void>();
    const { drag } = setup({ onSelect });
    const { clientX, clientY } = markPoint();

    await drag([clientX - 32, clientY], [clientX, clientY]);

    expect(onSelect).not.toHaveBeenCalled();
  });

  it("reports no click at the end of a pinch", async () => {
    const onSelect = vi.fn<(id: string) => void>();
    const { user, pinch } = setup({ onSelect });
    const { clientX, clientY } = markPoint();

    await pinch([clientX, clientY], [clientX + 100, clientY], [clientX + 150, clientY]);
    await user.pointer([{ keys: "[/TouchB]" }, { keys: "[/TouchA]" }]);

    expect(onSelect).not.toHaveBeenCalled();
  });

  it("ignores the secondary button", async () => {
    const { user, recorder, canvas } = setup();
    recorder.clear();

    await user.pointer([
      { keys: "[MouseRight>]", target: canvas, coords: { clientX: 100, clientY: 100 } },
      { coords: { clientX: 164, clientY: 100 } },
      { keys: "[/MouseRight]" },
    ]);
    nextFrame();

    expect(paints(recorder)).toBe(0);
  });

  it("zooms in as two pointers move apart, by the ratio of their separations", async () => {
    const { recorder, pinch } = setup();

    await pinch([100, 150], [300, 150], [400, 150]);
    nextFrame();

    expect(edgeRadius(recorder)).toBeCloseTo(118 * 1.5, 9);
  });

  it("does not turn the camera during a pinch", async () => {
    const { recorder, pinch } = setup();

    await pinch([100, 150], [300, 150], [400, 150]);
    nextFrame();

    expect(markCentre(recorder)).toEqual(
      markAt({ azimuthDeg: 30, elevationDeg: 30 }, (118 * 1.5) / 50),
    );
  });

  // `user-event` sends no `pointercancel`, loses no capture and has no wheel, so these use
  // `fireEvent`.
  it("ends a drag at pointercancel", () => {
    const { recorder, canvas } = setup();
    fireEvent.pointerDown(canvas, { pointerId: 1, button: 0, clientX: 100, clientY: 100 });
    fireEvent.pointerMove(canvas, { pointerId: 1, clientX: 132, clientY: 100 });
    fireEvent.pointerCancel(canvas, { pointerId: 1 });

    fireEvent.pointerMove(canvas, { pointerId: 1, clientX: 200, clientY: 100 });
    nextFrame();

    expect(markCentre(recorder)).toEqual(markAt({ azimuthDeg: 46, elevationDeg: 30 }));
  });

  it("reports no click after pointercancel", () => {
    const onSelect = vi.fn<(id: string) => void>();
    const { canvas } = setup({ onSelect });
    const { clientX, clientY } = markPoint();
    fireEvent.pointerDown(canvas, { pointerId: 1, button: 0, clientX, clientY });
    fireEvent.pointerCancel(canvas, { pointerId: 1 });

    fireEvent.pointerUp(canvas, { pointerId: 1, button: 0, clientX, clientY });

    expect(onSelect).not.toHaveBeenCalled();
  });

  it("forgets a pointer whose capture is lost without an up, so the next drag turns", () => {
    const { recorder, canvas } = setup();
    fireEvent.pointerDown(canvas, { pointerId: 5, button: 0, clientX: 300, clientY: 100 });
    fireEvent.lostPointerCapture(canvas, { pointerId: 5 });

    fireEvent.pointerDown(canvas, { pointerId: 1, button: 0, clientX: 100, clientY: 100 });
    fireEvent.pointerMove(canvas, { pointerId: 1, clientX: 132, clientY: 100 });
    nextFrame();

    expect(markCentre(recorder)).toEqual(markAt({ azimuthDeg: 46, elevationDeg: 30 }));
  });

  it("zooms in with the wheel turned up", () => {
    const { recorder, canvas } = setup();

    fireEvent.wheel(canvas, { deltaY: -400, deltaMode: 0 });
    nextFrame();

    expect(edgeRadius(recorder)).toBeCloseTo(118 * 2, 9);
  });

  it("keeps the page from scrolling under the wheel", () => {
    const { canvas } = setup();

    expect(fireEvent.wheel(canvas, { deltaY: -400, deltaMode: 0 })).toBe(false);
  });

  it("reads a wheel that counts lines as 16 px a line", () => {
    const { recorder, canvas } = setup();

    fireEvent.wheel(canvas, { deltaY: 25, deltaMode: 1 });
    nextFrame();

    expect(edgeRadius(recorder)).toBeCloseTo(118 / 2, 9);
  });

  it("reads a wheel that counts pages as the canvas's height a page", () => {
    const { recorder, canvas } = setup();
    vi.spyOn(canvas, "clientHeight", "get").mockReturnValue(200);

    fireEvent.wheel(canvas, { deltaY: 2, deltaMode: 2 });
    nextFrame();

    expect(edgeRadius(recorder)).toBeCloseTo(118 / 2, 9);
  });
});

/** The angles a turn from `from` to `to` shows `elapsedMs` after its first frame. */
function turnedAt(from: CameraAngles, to: CameraAngles, elapsedMs: number): CameraAngles {
  const { azimuthDeg, elevationDeg } = tweenCamera(
    { ...from, pxPerUnit: 1 },
    { ...to, pxPerUnit: 1 },
    easeOut(elapsedMs / TRANSITION_MS),
  );
  return { azimuthDeg, elevationDeg };
}

describe("SpatialView's preset views", () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame"] });
    stubLayout(400, 300);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  function setup() {
    const user = userEvent.setup();
    const recorder = stubCanvas();
    render(
      <>
        {viewOf({})}
        <input aria-label="Name" />
      </>,
    );
    return { user, recorder, canvas: screen.getByRole("application", { name: "Local chart" }) };
  }

  /** Runs frames until the view asks for no more, as a browser would; at most a second of them. */
  function runFrames(): number {
    let frames = 0;
    while (vi.getTimerCount() > 0 && frames < 60) {
      nextFrame();
      frames += 1;
    }
    return frames;
  }

  it("names each preset's key for assistive technology", () => {
    setup();

    expect(screen.getByRole("button", { name: "F FRONT" })).toHaveAttribute(
      "aria-keyshortcuts",
      "F",
    );
  });

  it("offers the four preset views, each with its key, the default pressed", () => {
    setup();

    const views = screen.getByRole("group", { name: "Views" });
    expect(
      within(views)
        .getAllByRole("button")
        .map((button) => [button.textContent, button.getAttribute("aria-pressed")]),
    ).toEqual([
      ["T TOP", "false"],
      ["S SIDE", "false"],
      ["F FRONT", "false"],
      ["O OBLIQUE", "true"],
    ]);
  });

  it("turns to TOP when it is clicked, and shows it pressed", async () => {
    const { user, recorder } = setup();

    await user.click(screen.getByRole("button", { name: "T TOP" }));
    runFrames();

    expect(screen.getByRole("button", { name: "T TOP" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "O OBLIQUE" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
    expect(markCentre(recorder)).toEqual(markAt({ azimuthDeg: 0, elevationDeg: 90 }));
  });

  it("turns to SIDE with the S key", async () => {
    const { user, recorder } = setup();

    await user.keyboard("s");
    runFrames();

    expect(screen.getByRole("button", { name: "S SIDE" })).toHaveAttribute("aria-pressed", "true");
    expect(markCentre(recorder)).toEqual(markAt({ azimuthDeg: 0, elevationDeg: 0 }));
  });

  it("leaves the preset keys to a text field that has focus", async () => {
    const { user } = setup();
    const field = screen.getByRole("textbox", { name: "Name" });
    await user.click(field);

    await user.keyboard("s");
    runFrames();

    expect(field).toHaveValue("s");
    expect(screen.getByRole("button", { name: "S SIDE" })).toHaveAttribute("aria-pressed", "false");
  });

  it("keeps the zoom when it turns to a preset", async () => {
    const { user, recorder } = setup();
    await user.keyboard("+");
    nextFrame();

    await user.keyboard("f");
    runFrames();

    expect(edgeRadius(recorder)).toBeCloseTo(118 * 1.25, 9);
    expect(markCentre(recorder)).toEqual(
      markAt({ azimuthDeg: 90, elevationDeg: 0 }, FITTED_PX_PER_LY * 1.25),
    );
  });

  it("turns over frames for 120 ms, then asks for none", async () => {
    const { user, recorder } = setup();
    recorder.clear();

    await user.click(screen.getByRole("button", { name: "T TOP" }));
    const frames = runFrames();

    // The first frame starts the clock; frames 16 ms apart reach 120 ms on the eighth after it.
    expect(frames).toBe(9);
    expect(paints(recorder)).toBe(8);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("asks for no frame once it has landed, so nothing drifts", async () => {
    const { user, recorder } = setup();
    await user.click(screen.getByRole("button", { name: "T TOP" }));
    runFrames();
    recorder.clear();

    act(() => {
      vi.advanceTimersByTime(1_000);
    });

    expect(paints(recorder)).toBe(0);
  });

  it("turns at once, with one paint and no frame, under reduced motion", async () => {
    stubMatchMedia(true);
    const { user, recorder } = setup();
    recorder.clear();

    await user.click(screen.getByRole("button", { name: "T TOP" }));

    expect(vi.getTimerCount()).toBe(0);
    expect(paints(recorder)).toBe(1);
    expect(markCentre(recorder)).toEqual(markAt({ azimuthDeg: 0, elevationDeg: 90 }));
  });

  it("follows a change of the reduced-motion setting", async () => {
    const reducedMotion = stubMatchMedia(false);
    const { user } = setup();

    act(() => {
      reducedMotion.set(true);
    });
    await user.click(screen.getByRole("button", { name: "T TOP" }));

    expect(vi.getTimerCount()).toBe(0);
    expect(screen.getByRole("button", { name: "T TOP" })).toHaveAttribute("aria-pressed", "true");
  });

  it("starts a turn from where the camera is, and eases along it", async () => {
    const { user, recorder, canvas } = setup();
    await user.click(canvas);
    await user.keyboard("{ArrowRight}{ArrowRight}");
    nextFrame();

    await user.keyboard("t");
    nextFrame();
    nextFrame();

    // From 040°, +30° to TOP; the second frame is 16 ms after the first, which started the clock.
    expect(markCentre(recorder)).toEqual(
      markAt(
        turnedAt({ azimuthDeg: 40, elevationDeg: 30 }, { azimuthDeg: 0, elevationDeg: 90 }, 16),
      ),
    );
  });

  it("carries a second preset on from where the first turn has got to", async () => {
    const { user, recorder } = setup();
    await user.keyboard("t");
    nextFrame();
    nextFrame();
    const partway = turnedAt(
      { azimuthDeg: 30, elevationDeg: 30 },
      { azimuthDeg: 0, elevationDeg: 90 },
      16,
    );

    await user.keyboard("f");
    nextFrame();
    nextFrame();

    expect(markCentre(recorder)).toEqual(
      markAt(turnedAt(partway, { azimuthDeg: 90, elevationDeg: 0 }, 16)),
    );
  });

  it("does not turn to a preset for its key with Shift", async () => {
    const { user } = setup();

    await user.keyboard("{Shift>}T{/Shift}");
    runFrames();

    expect(screen.getByRole("button", { name: "O OBLIQUE" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("asks for no frame to turn to the preset already on screen", async () => {
    const { user } = setup();

    await user.click(screen.getByRole("button", { name: "O OBLIQUE" }));

    expect(vi.getTimerCount()).toBe(0);
  });

  it("turns once for a preset key held down", async () => {
    const { user } = setup();

    await user.keyboard("{t>3/}");

    // One turn asked for, its first frame pending; the repeats asked for none of their own.
    expect(vi.getTimerCount()).toBe(1);
    runFrames();
    expect(screen.getByRole("button", { name: "T TOP" })).toHaveAttribute("aria-pressed", "true");
  });

  it("stops the turn when the operator moves the camera", async () => {
    const { user, canvas } = setup();
    await user.click(screen.getByRole("button", { name: "T TOP" }));
    nextFrame();
    nextFrame();

    await user.click(canvas);
    await user.keyboard("{ArrowRight}");
    runFrames();

    expect(screen.getByRole("button", { name: "T TOP" })).toHaveAttribute("aria-pressed", "false");
    expect(vi.getTimerCount()).toBe(0);
  });
});
