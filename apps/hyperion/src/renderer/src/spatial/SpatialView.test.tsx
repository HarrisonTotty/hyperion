import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, onTestFinished, vi } from "vitest";

import { useState } from "react";

import { AU_PER_LY, formatScaleLength, SCALE_AU_BELOW_LY } from "../lib/format";
import { FakeResizeObserver } from "../test/FakeResizeObserver";
import { fakeFramesAndTimeouts } from "../test/fakeFramesAndTimeouts";
import { stubMatchMedia } from "../test/stubMatchMedia";
import { type RecordingContext2D, stubCanvas } from "../test/RecordingContext2D";
import { type CameraAngles, PRESETS, project, viewBasis } from "./camera";
import { easeOut, TRANSITION_MS, tweenCamera } from "./transition";
import { localFrameAt, planeFrame } from "./frame";
import type { PointMark, SpatialScene } from "./marks";
import type { ScaleUnit } from "./scale";
import { SpatialView, type SpatialViewProps } from "./SpatialView";
import { add, scale, vec3 } from "./vec3";

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
      centre={[
        { label: "RADIUS", value: "26,000.0", unit: "ly", widthCh: 9 },
        { label: "ANGLE", value: "045.0°", unit: "", widthCh: 6 },
        { label: "HEIGHT", value: "+12.0", unit: "ly", widthCh: 9 },
      ]}
      time={{ label: "UT", value: "+0.00", unit: "yr", widthCh: 9 }}
      coreDistance={{ value: "26,000.0", unit: "ly" }}
      accessibleName="Local chart"
      stale={false}
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

  it("is described by its camera's angles and its visible key legend", () => {
    stubLayout();
    renderView();

    expect(screen.getByRole("application", { name: "Local chart" })).toHaveAccessibleDescription(
      "AZM 030° ELV +30° ARROWS ROTATE +/− ZOOM Z FIT",
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

  it("raises the camera 5° with the down arrow, as a drag down does", async () => {
    const { user, recorder, canvas } = setup();
    await user.click(canvas);

    await user.keyboard("{ArrowDown}");
    nextFrame();

    expect(markCentre(recorder)).toEqual(markAt({ azimuthDeg: 30, elevationDeg: 35 }));
  });

  it("lowers the camera 5° with the up arrow", async () => {
    const { user, recorder, canvas } = setup();
    await user.click(canvas);

    await user.keyboard("{ArrowUp}");
    nextFrame();

    expect(markCentre(recorder)).toEqual(markAt({ azimuthDeg: 30, elevationDeg: 25 }));
  });

  it("raises the camera no further than +90°", async () => {
    const { user, recorder, canvas } = setup();
    await user.click(canvas);
    await user.keyboard("{ArrowDown>13/}");
    nextFrame();

    await user.keyboard("{ArrowUp}");
    nextFrame();

    // Stopped at +90°, one press down is +85°, not the +90° an unclamped +95° would give.
    expect(markCentre(recorder)).toEqual(markAt({ azimuthDeg: 30, elevationDeg: 85 }));
  });

  it("paints nothing for a press against a limit", async () => {
    const { user, recorder, canvas } = setup();
    await user.click(canvas);
    await user.keyboard("{ArrowDown>13/}");
    nextFrame();
    recorder.clear();

    await user.keyboard("{ArrowDown}");
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

    await user.keyboard("{ArrowDown>13/}{ArrowUp}");
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

  it("fits a new fit radius, a chosen zoom notwithstanding, when a fit is requested with it", async () => {
    const { user, recorder, rerender } = setup();
    await user.keyboard("+");
    nextFrame();

    rerender({
      fitRadius: 25,
      fitRequest: 1,
      scene: aScene({ spheres: [{ radius: 25, role: "data_edge", label: "QUERY EDGE 25 ly" }] }),
    });

    // A zoom preset's press: the new radius fits the view as 50 ly did, 118 px.
    expect(edgeRadius(recorder)).toBeCloseTo(118, 9);
  });

  it("fits the view again with Z", async () => {
    const { user, recorder } = setup();
    await user.keyboard("++");
    nextFrame();

    await user.keyboard("z");
    nextFrame();

    expect(edgeRadius(recorder)).toBeCloseTo(118, 9);
  });

  it("tells onFitChange of a zoom by hand and of Z, and of no turn", async () => {
    const onFitChange = vi.fn<(fitting: boolean) => void>();
    const { user, canvas } = setup({ onFitChange });

    await user.keyboard("+");
    await user.keyboard("z");
    act(() => {
      canvas.focus();
    });
    await user.keyboard("{ArrowLeft}");

    expect(onFitChange.mock.calls).toEqual([[false], [true]]);
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

  it("tells onFitChange that the wheel zoomed by hand", () => {
    const onFitChange = vi.fn<(fitting: boolean) => void>();
    const { canvas } = setup({ onFitChange });

    fireEvent.wheel(canvas, { deltaY: -400, deltaMode: 0 });

    expect(onFitChange.mock.calls).toEqual([[false]]);
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

/** The value a readout of the view gives for a label: `AZM` reads `030°`. */
function readout(label: string): string | null {
  return screen.getByText(label, { selector: "dt" }).nextElementSibling?.textContent ?? null;
}

/** Lets `ms` of fake time pass, running the frames and timers that fall in it. */
function wait(ms: number): void {
  act(() => {
    vi.advanceTimersByTime(ms);
  });
}

/** The north axis of the view's triad, which says how it ends. */
function triadNorth(): Element | null {
  return screen.getByRole("img", { name: "Axis triad" }).querySelector("[data-axis='north']");
}

describe("SpatialView's readouts", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  /**
   * Renders the view with frames and timeouts faked, since the readouts' 4 Hz hold is timed with
   * `setTimeout`.
   */
  function setup(props: Partial<SpatialViewProps> = {}) {
    const advanceTimers = fakeFramesAndTimeouts();
    stubLayout(400, 300);
    const user = userEvent.setup({ advanceTimers });
    stubCanvas();
    const { container } = render(viewOf(props));
    return {
      user,
      container,
      canvas: screen.getByRole("application", { name: "Local chart" }),
    };
  }

  it("reads the camera's azimuth and elevation as AZM 030° and ELV +30°", () => {
    setup();

    expect(readout("AZM")).toBe("030°");
    expect(readout("ELV")).toBe("+30°");
  });

  it("turns the azimuth readout from 030° to 035° with the right arrow", async () => {
    const { user, canvas } = setup();
    await user.click(canvas);

    await user.keyboard("{ArrowRight}");
    wait(300);

    expect(readout("AZM")).toBe("035°");
  });

  it("stops the elevation readout at +90° after 13 presses of the down arrow", async () => {
    const { user, canvas } = setup();
    await user.click(canvas);

    await user.keyboard("{ArrowDown>13/}");
    wait(300);

    expect(readout("ELV")).toBe("+90°");
  });

  it("reads AZM 000° and ELV +90° once turned to TOP", async () => {
    const { user } = setup();

    await user.click(screen.getByRole("button", { name: "T TOP" }));
    wait(500);

    expect(readout("AZM")).toBe("000°");
    expect(readout("ELV")).toBe("+90°");
  });

  it("changes the readout once for ten turns inside 100 ms, then reads the last", async () => {
    const { user, canvas } = setup();
    await user.click(canvas);
    const shown = [readout("AZM")];

    // Ten presses 9 ms apart, each turn applied on the next 16 ms frame.
    for (let turn = 0; turn < 10; turn += 1) {
      // Each press must meet its frames before the next, as an operator's do: they cannot be sent
      // together.
      // oxlint-disable-next-line no-await-in-loop
      await user.keyboard("{ArrowRight}");
      wait(9);
      if (shown.at(-1) !== readout("AZM")) {
        shown.push(readout("AZM"));
      }
    }

    // The first frame took the first two presses; the rest were held back.
    expect(shown).toEqual(["030°", "040°"]);
    wait(250);
    expect(readout("AZM")).toBe("080°");
  });

  it("puts nothing in a live region, so that a drag is not announced", async () => {
    const { user, canvas, container } = setup();

    await user.pointer([
      { keys: "[MouseLeft>]", target: canvas, coords: { clientX: 100, clientY: 100 } },
      { coords: { clientX: 164, clientY: 132 } },
      { keys: "[/MouseLeft]" },
    ]);
    wait(300);

    expect(readout("AZM")).toBe("062°");
    expect(
      container.querySelectorAll(
        "[aria-live], output, [role='status'], [role='log'], [role='alert'], [role='timer'], [role='marquee']",
      ),
    ).toHaveLength(0);
  });

  it("turns the axis triad with the camera: north towards the viewer from TOP", async () => {
    const { user } = setup();

    await user.click(screen.getByRole("button", { name: "T TOP" }));
    wait(500);

    expect(triadNorth()).toHaveAttribute("data-end", "towards");
  });

  it("points the core arrow up from TOP, labelled with the distance to the axis", async () => {
    const { user, container } = setup();

    await user.click(screen.getByRole("button", { name: "T TOP" }));
    wait(500);

    const line = container.querySelector("[data-core='arrow'] line");
    expect(Number(line?.getAttribute("x2"))).toBeCloseTo(200, 9);
    expect(Number(line?.getAttribute("y2"))).toBeLessThan(Number(line?.getAttribute("y1")));
    expect(
      screen.getByText(
        (_, element) =>
          element instanceof HTMLSpanElement && element.textContent === "CORE 26,000.0 ly",
      ),
    ).toBeInTheDocument();
  });

  it("replaces the core arrow on the galactic axis and names the fallback directions", () => {
    const { container } = setup({ scene: aScene({ frame: localFrameAt(vec3(0, 0, 0)) }) });

    expect(screen.getByText("DIRECTIONS UNDEFINED: grid aligned to -X")).toBeInTheDocument();
    expect(container.querySelector("[data-core]")).toBeNull();
    expect(within(screen.getByRole("img", { name: "Axis triad" })).getByText("-X")).toBeVisible();
  });

  it("shortens the triad to a stage under its usual box, which the overlay would clip", () => {
    // The local chart's stage at 1280 × 688 with its census table shown: 798 × 74 px.
    fakeFramesAndTimeouts();
    stubLayout(798, 74);
    stubCanvas();
    render(viewOf({}));

    const triad = screen.getByRole("img", { name: "Axis triad" });
    expect(triad.style.height).toBe("4.625rem");
    for (const name of ["COREWARD", "SPINWARD", "NORTH"]) {
      expect(within(triad).getByText(name)).toBeVisible();
    }
  });
});

/** Light-years down to 0.01 ly, then astronomical units, as the local chart reads its scale. */
const LY_THEN_AU: ReadonlyArray<ScaleUnit> = [
  { perSceneUnit: 1, minSceneLength: SCALE_AU_BELOW_LY },
  { perSceneUnit: AU_PER_LY, minSceneLength: 0 },
];

/** The view's scale bar, named with its length. */
function scaleBarElement(): HTMLElement {
  return screen.getByRole("img", { name: /^Scale bar/u });
}

/** The scale bar's width as drawn, in CSS pixels: its length and one pixel for the end ticks. */
function scaleBarWidthPx(): number {
  const bar = scaleBarElement().querySelector<HTMLElement>(".scale-bar__bar");
  return Number.parseFloat(bar?.style.width ?? "");
}

/** Props for a view fitted to a query sphere of `radiusLy`, its scale read in ly and AU. */
function fittedTo(radiusLy: number): Partial<SpatialViewProps> {
  return {
    fitRadius: radiusLy,
    scene: aScene({
      spheres: [{ radius: radiusLy, role: "data_edge", label: `QUERY EDGE ${radiusLy} ly` }],
    }),
    formatLength: formatScaleLength,
    scaleUnits: LY_THEN_AU,
  };
}

describe("SpatialView's scale bar, frame, centre and time", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  function setup(props: Partial<SpatialViewProps> = {}) {
    const advanceTimers = fakeFramesAndTimeouts();
    stubLayout(400, 300);
    const user = userEvent.setup({ advanceTimers });
    stubCanvas();
    const result = render(viewOf(props));
    return {
      user,
      canvas: screen.getByRole("application", { name: "Local chart" }),
      rerender: (next: Partial<SpatialViewProps>) => {
        result.rerender(viewOf(next));
      },
    };
  }

  it("keeps the scale bar one width however long the bar, so that its row never moves", async () => {
    const { user } = setup(fittedTo(25));
    const bar = screen.getByRole("img", { name: /^Scale bar/u });
    const slot = bar.querySelector<HTMLElement>(".scale-bar__slot");

    await user.keyboard("+");
    nextFrame();

    // 6 rem and the end ticks' pixel, at every length.
    expect(slot?.style.width).toBe("97px");
  });

  it("draws a 1-2-5 scale bar no longer than 6 rem, labelled with its length", () => {
    setup(fittedTo(50));

    // 118 px over 50 ly: 96 px holds 40.7 ly, so the bar is 20 ly, 47.2 px.
    expect(screen.getByRole("img", { name: "Scale bar, 20 ly" })).toHaveTextContent("20 ly");
    expect(scaleBarWidthPx()).toBeCloseTo(47.2 + 1, 9);
  });

  it("changes the scale bar's label when + zooms in", async () => {
    const { user } = setup(fittedTo(25));
    expect(screen.getByRole("img", { name: "Scale bar, 20 ly" })).toBeInTheDocument();

    await user.keyboard("+");
    nextFrame();

    expect(screen.getByRole("img", { name: "Scale bar, 10 ly" })).toHaveTextContent("10 ly");
  });

  it("changes the scale bar when the wheel zooms", () => {
    const { canvas } = setup(fittedTo(25));

    fireEvent.wheel(canvas, { deltaY: -400, deltaMode: 0 });
    nextFrame();

    expect(screen.getByRole("img", { name: "Scale bar, 10 ly" })).toHaveTextContent("10 ly");
  });

  /**
   * Zooms in press by press, 25 presses fitted to 50 ly and 25 more fitted to 0.5 ly, calling
   * `seen` after each: zoom stops at 100 times the fit, so the query radius steps down on the way,
   * as the operator would step it, and the zoom chosen is kept.
   */
  async function zoomDownToAstronomicalUnits(seen: () => void): Promise<void> {
    const { user, rerender } = setup(fittedTo(50));
    seen();
    const zoomIn = async (): Promise<void> => {
      for (let press = 0; press < 25; press += 1) {
        // Each press must reach its frame before the next, so that every step of the bar is seen.
        // oxlint-disable-next-line no-await-in-loop
        await user.keyboard("+");
        nextFrame();
        seen();
      }
    };
    await zoomIn();
    rerender(fittedTo(0.5));
    await zoomIn();
  }

  it("steps its label down through 0.01 ly to 500 AU as it zooms in", async () => {
    const labels: string[] = [];

    await zoomDownToAstronomicalUnits(() => {
      const label = scaleBarElement().textContent;
      if (labels.at(-1) !== label) {
        labels.push(label);
      }
    });

    expect(labels).toEqual([
      "20 ly",
      "10 ly",
      "5 ly",
      "2 ly",
      "1 ly",
      "0.5 ly",
      "0.2 ly",
      "0.1 ly",
      "0.05 ly",
      "0.02 ly",
      "0.01 ly",
      "500 AU",
      "200 AU",
    ]);
  });

  it("never draws the bar longer than 6 rem as it zooms in", async () => {
    let widestPx = 0;

    await zoomDownToAstronomicalUnits(() => {
      widestPx = Math.max(widestPx, scaleBarWidthPx());
    });

    // 96 px, and the pixel that puts the end ticks' centres the length apart.
    expect(widestPx).toBeLessThanOrEqual(96 + 1);
  });

  it("shows the frame, the centre and the time, each with its unit", () => {
    setup();

    expect(readout("FRAME")).toBe("GALACTIC");
    expect(readout("RADIUS")).toBe("26,000.0 ly");
    expect(readout("ANGLE")).toBe("045.0°");
    expect(readout("HEIGHT")).toBe("+12.0 ly");
    expect(readout("UT")).toBe("+0.00 yr");
  });

  it("gives each value a slot of fixed width, so that a new value moves nothing", () => {
    setup();

    const radius = screen.getByText("RADIUS", { selector: "dt" }).nextElementSibling;
    if (!(radius instanceof HTMLElement)) {
      throw new Error("RADIUS has no value");
    }
    expect(within(radius).getByText("26,000.0").style.minWidth).toBe("9ch");
  });

  it("shows a missing value as an em dash, without its unit", () => {
    setup({
      centre: [
        { label: "RADIUS", value: "0.0", unit: "ly", widthCh: 9 },
        { label: "ANGLE", value: null, unit: "", widthCh: 6 },
        { label: "HEIGHT", value: "+0.0", unit: "ly", widthCh: 9 },
      ],
    });

    expect(readout("ANGLE")).toBe("—");
    expect(screen.getByText("—")).toHaveClass("readout__missing");
  });
});

/** Marks spread across the view, each with a label and a priority: `M00` lowest. */
function spreadMarks(count: number): PointMark[] {
  return Array.from({ length: count }, (_, index) =>
    aMark(`m${String(index).padStart(2, "0")}`, {
      // One to a cell of a grid, so that no label covers another.
      position: vec3(-40 + (index % 4) * 25, -40 + Math.floor(index / 4) * 16, 0),
      label: `M${String(index).padStart(2, "0")}`,
      labelPriority: index,
    }),
  );
}

/** The transform that places a label over the view. */
function placement(element: HTMLElement): string {
  return element.style.transform;
}

/** A label over the view, whose numbers are set apart in figures of their own. */
function overlayLabel(text: string): HTMLElement {
  return screen.getByText(
    (_, element) => element instanceof HTMLSpanElement && element.textContent === text,
  );
}

describe("SpatialView's labels", () => {
  beforeEach(() => {
    stubLayout(1200, 900);
  });

  it("labels the selected mark, however low its priority", () => {
    renderView({ scene: aScene({ points: spreadMarks(20), selectedId: "m00" }) });

    expect(screen.getByText("M00")).toBeInTheDocument();
  });

  it("labels no more than nine marks: eight and the selection", () => {
    renderView({ scene: aScene({ points: spreadMarks(20), selectedId: "m00" }) });

    // Room for every one of them, so the cap is what holds the count down: the eight of highest
    // priority, M19 down to M12, and the selection.
    const labels = screen.getAllByText(/^M\d\d$/u);
    expect(labels.map((label) => label.textContent).toSorted()).toEqual([
      "M00",
      "M12",
      "M13",
      "M14",
      "M15",
      "M16",
      "M17",
      "M18",
      "M19",
    ]);
  });

  it("hides the labels from assistive technology, which has the list and the readout", () => {
    renderView({ scene: aScene({ points: spreadMarks(3), selectedId: "m00" }) });

    expect(screen.getByText("M00").closest("[aria-hidden='true']")).not.toBeNull();
  });

  it("labels a mark within the set range in the colour of its symbol", () => {
    renderView({
      scene: aScene({
        points: [aMark("a", { status: "available" }), aMark("b", { position: vec3(20, 20, 0) })],
      }),
    });

    // The stylesheet, which jsdom does not apply, gives the class `--accent`.
    expect(screen.getByText("A")).toHaveClass("spatial-label--available");
    expect(screen.getByText("B")).not.toHaveClass("spatial-label--available");
  });

  it("labels the range sphere and the plane ring apart", () => {
    renderView({
      fitRadius: 80,
      scene: aScene({
        spheres: [
          { radius: 50, role: "range", label: "RANGE 50 ly SET" },
          { radius: 80, role: "data_edge", label: "QUERY EDGE 80 ly" },
        ],
        plane: { spacing: 20, extent: 80, rings: [{ radius: 50, label: "PLANE 50 ly" }] },
      }),
    });

    const range = overlayLabel("RANGE 50 ly SET");
    const plane = overlayLabel("PLANE 50 ly");
    expect(overlayLabel("QUERY EDGE 80 ly")).toBeInTheDocument();
    expect(placement(range)).not.toBe(placement(plane));
  });

  it("keeps a circle's labels in a narrow view after the zoom takes its top out of sight", async () => {
    vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame"] });
    onTestFinished(() => {
      vi.useRealTimers();
    });
    stubLayout(360, 300);
    const user = userEvent.setup();
    renderView({
      scene: aScene({
        spheres: [
          { radius: 50, role: "range", label: "RANGE 50 ly SET" },
          { radius: 50, role: "data_edge", label: "QUERY EDGE 50 ly" },
        ],
      }),
    });

    await user.keyboard("++");
    nextFrame();

    // The circle, 184 px in radius about the view's centre, has left the top and the sides.
    const outside = ["RANGE 50 ly SET", "QUERY EDGE 50 ly"].filter((text) => {
      const match = /translate\((-?[\d.]+)rem, (-?[\d.]+)rem\)/u.exec(
        placement(overlayLabel(text)),
      );
      const [leftRem, topRem] = [Number(match?.[1]), Number(match?.[2])];
      // Estimated as the view estimates it: 0.72 em a character at 0.875 rem, 1.25 lines.
      const inside =
        leftRem >= 0 &&
        topRem >= 0 &&
        leftRem * 16 + text.length * 0.72 * 14 <= 360 &&
        topRem * 16 + 17.5 <= 300;
      return !inside;
    });
    expect(outside).toEqual([]);
  });

  it("gives both labels to one circle when the query radius equals the range", () => {
    renderView({
      scene: aScene({
        spheres: [
          { radius: 50, role: "range", label: "RANGE 50 ly SET" },
          { radius: 50, role: "data_edge", label: "QUERY EDGE 50 ly" },
        ],
      }),
    });

    const range = overlayLabel("RANGE 50 ly SET");
    const edge = overlayLabel("QUERY EDGE 50 ly");
    expect(placement(range)).not.toBe(placement(edge));
  });
});

/** Clicks the view's canvas at a point, in client coordinates. */
async function clickCanvasAt(
  user: ReturnType<typeof userEvent.setup>,
  clientX: number,
  clientY: number,
): Promise<void> {
  await user.pointer([
    {
      keys: "[MouseLeft]",
      target: screen.getByRole("application", { name: "Local chart" }),
      coords: { clientX, clientY },
    },
  ]);
}

/** The colours of the reticles in the latest paint: paths of four corner brackets. */
function reticleColours(recorder: RecordingContext2D): string[] {
  const colours: string[] = [];
  const records = recorder.records;
  const lastClear = records.findLastIndex(
    (record) => record.type === "call" && record.name === "fillRect",
  );
  let moves = 0;
  let stroke = "";
  for (const record of records.slice(lastClear)) {
    if (record.type === "set" && record.name === "strokeStyle") {
      stroke = String(record.value);
    } else if (record.type === "call" && record.name === "beginPath") {
      moves = 0;
    } else if (record.type === "call" && record.name === "moveTo") {
      moves += 1;
    } else if (record.type === "call" && record.name === "stroke" && moves === 4) {
      colours.push(stroke);
    }
  }
  return colours;
}

interface SelectingProps {
  readonly destinationId: string | null;
}

/** The view with its selection owned by a parent, as the chart owns it. */
function Selecting({ destinationId }: SelectingProps) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  return viewOf({ scene: aScene({ selectedId, destinationId }), onSelect: setSelectedId });
}

describe("SpatialView picking", () => {
  beforeEach(() => {
    stubLayout(400, 300);
  });

  it("selects a mark clicked 12 px away, within 1 rem", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn<(id: string) => void>();
    renderView({ onSelect });
    const { clientX, clientY } = markPoint();

    await clickCanvasAt(user, clientX + 12, clientY);

    expect(onSelect).toHaveBeenCalledWith("a");
  });

  it("selects nothing for a click 24 px from the mark", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn<(id: string) => void>();
    renderView({ onSelect });
    const { clientX, clientY } = markPoint();

    await clickCanvasAt(user, clientX + 24, clientY);

    expect(onSelect).not.toHaveBeenCalled();
  });

  it("selects the nearer of two marks that coincide on the screen", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn<(id: string) => void>();
    const forward = viewBasis(FRAME, PRESETS.oblique).forward;
    // "a" lies 10 ly behind "b" along the line of sight, and would win a tie by ID.
    renderView({
      onSelect,
      scene: aScene({
        points: [
          aMark("a", { position: add(MARK_POSITION, scale(forward, 10)) }),
          aMark("b", { position: MARK_POSITION }),
        ],
      }),
    });
    const { clientX, clientY } = markPoint();

    await clickCanvasAt(user, clientX, clientY);

    expect(onSelect).toHaveBeenCalledExactlyOnceWith("b");
  });

  it("paints the accent reticle about a mark once it is selected", async () => {
    const user = userEvent.setup();
    const recorder = stubCanvas();
    render(<Selecting destinationId={null} />);
    expect(reticleColours(recorder)).toEqual([]);
    const { clientX, clientY } = markPoint();

    await clickCanvasAt(user, clientX, clientY);

    expect(reticleColours(recorder)).toEqual(["#5cc8e6"]);
  });

  it("paints the target reticle about the destination, outside the selection's", async () => {
    const user = userEvent.setup();
    const recorder = stubCanvas();
    render(<Selecting destinationId="a" />);
    expect(reticleColours(recorder)).toEqual(["#e879f9"]);
    const { clientX, clientY } = markPoint();

    await clickCanvasAt(user, clientX, clientY);

    expect(reticleColours(recorder)).toEqual(["#5cc8e6", "#e879f9"]);
  });

  it("takes a finger's 6 px wobble on a tap for a tap, which selects", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn<(id: string) => void>();
    renderView({ onSelect });
    const { clientX, clientY } = markPoint();

    await user.pointer([
      {
        keys: "[TouchA>]",
        target: screen.getByRole("application", { name: "Local chart" }),
        coords: { clientX, clientY },
      },
      { pointerName: "TouchA", coords: { clientX: clientX + 6, clientY } },
      { keys: "[/TouchA]" },
    ]);

    expect(onSelect).toHaveBeenCalledWith("a");
  });

  it("takes a mouse's 6 px movement for a drag, which selects nothing", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn<(id: string) => void>();
    renderView({ onSelect });
    const { clientX, clientY } = markPoint();

    await user.pointer([
      {
        keys: "[MouseLeft>]",
        target: screen.getByRole("application", { name: "Local chart" }),
        coords: { clientX, clientY },
      },
      { coords: { clientX: clientX + 6, clientY } },
      { keys: "[/MouseLeft]" },
    ]);

    expect(onSelect).not.toHaveBeenCalled();
  });
});

describe("SpatialView on a system's plane, with the galactic axes given apart", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  /** A plane tilted 30° to the galactic one about `axis`, its coreward laid from galactic coreward. */
  function tiltedAbout(axis: "coreward" | "spinward") {
    const lean = axis === "coreward" ? FRAME.spinward : FRAME.coreward;
    return planeFrame(
      add(scale(FRAME.north, Math.cos(Math.PI / 6)), scale(lean, 0.5)),
      FRAME.coreward,
    );
  }

  async function renderTurnedTo(button: string, props: Partial<SpatialViewProps>) {
    const advanceTimers = fakeFramesAndTimeouts();
    stubLayout(400, 300);
    const user = userEvent.setup({ advanceTimers });
    stubCanvas();
    const { container } = render(viewOf(props));
    await user.click(screen.getByRole("button", { name: button }));
    wait(500);
    return container;
  }

  /** Where the north axis's end symbol is drawn, in the triad's SVG units from its origin. */
  function northTip(): readonly [number, number] {
    const circle = triadNorth()?.querySelector("circle");
    return [Number(circle?.getAttribute("cx")), Number(circle?.getAttribute("cy"))];
  }

  it("points NORTH at the viewer from TOP without axes, along the plane's own normal", async () => {
    await renderTurnedTo("T TOP", { scene: aScene({ frame: tiltedAbout("coreward") }) });

    expect(triadNorth()).toHaveAttribute("data-end", "towards");
    expect(northTip()[0]).toBeCloseTo(0, 9);
    expect(northTip()[1]).toBeCloseTo(0, 9);
  });

  it("points NORTH along galactic north with axes, not along the plane's normal", async () => {
    await renderTurnedTo("T TOP", {
      scene: aScene({ frame: tiltedAbout("coreward") }),
      axes: FRAME,
    });

    // The plane leans 30° towards spinward, which is right from its TOP, so galactic north leans
    // left, half a turn of the axis's length out of the screen.
    expect(triadNorth()).toHaveAttribute("data-end", "towards");
    expect(northTip()[0]).toBeLessThan(-1);
    expect(northTip()[1]).toBeCloseTo(0, 9);
  });

  it("shows the plane's coreward along the line of sight from SIDE without axes", async () => {
    // From SIDE the view looks along the plane's coreward.
    const container = await renderTurnedTo("S SIDE", {
      scene: aScene({ frame: tiltedAbout("spinward") }),
    });

    expect(container.querySelector("[data-core='away']")).not.toBeNull();
  });

  it("points the core arrow along galactic coreward with axes, 30° above the plane's", async () => {
    const container = await renderTurnedTo("S SIDE", {
      scene: aScene({ frame: tiltedAbout("spinward") }),
      axes: FRAME,
    });

    const line = container.querySelector("[data-core='arrow'] line");
    expect(Number(line?.getAttribute("x2"))).toBeCloseTo(200, 9);
    expect(Number(line?.getAttribute("y2"))).toBeLessThan(Number(line?.getAttribute("y1")));
  });

  /** Renders a view on a tilted plane whose galactic axes are the galactic axis's fallback. */
  function renderOnTheAxis() {
    fakeFramesAndTimeouts();
    stubLayout(400, 300);
    stubCanvas();
    return render(
      viewOf({
        scene: aScene({ frame: tiltedAbout("coreward") }),
        axes: localFrameAt(vec3(0, 0, 0)),
      }),
    );
  }

  it("names the fallback directions from the axes given, which are the galaxy's", () => {
    renderOnTheAxis();

    expect(screen.getByText("DIRECTIONS UNDEFINED: grid aligned to -X")).toBeInTheDocument();
    expect(within(screen.getByRole("img", { name: "Axis triad" })).getByText("-X")).toBeVisible();
  });

  it("draws no core arrow when the axes given lie on the galactic axis", () => {
    const { container } = renderOnTheAxis();

    expect(container.querySelector("[data-core]")).toBeNull();
  });
});
