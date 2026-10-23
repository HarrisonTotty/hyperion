import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, onTestFinished, vi } from "vitest";

import { useState } from "react";

import { AU_PER_LY, formatScaleLength, SCALE_AU_BELOW_LY } from "../lib/format";
import { reticleStrokesCssPx } from "../lib/strokes";
import { FakeResizeObserver } from "../test/FakeResizeObserver";
import { fakeFramesAndTimeouts } from "../test/fakeFramesAndTimeouts";
import { stubMatchMedia } from "../test/stubMatchMedia";
import { type RecordingContext2D, stubCanvas } from "../test/RecordingContext2D";
import { type CameraAngles, PRESETS, project, viewBasis } from "./camera";
import { easeOut, TRANSITION_MS, tweenCamera } from "./transition";
import { localFrameAt, planeFrame } from "../geometry/frame";
import { type BoxPx, segmentBoxGapPx } from "./furniture";
import type { AnnulusMark, PointMark, SpatialScene } from "./marks";
import type { SpatialMode } from "./ModeReading";
import type { ScaleUnit } from "./scale";
import { SpatialView, type SpatialViewProps } from "./SpatialView";
import { stylesheetRule } from "../test/stylesheet";
import { bracketBoxPx, textSizeRem } from "./labels";
import { boxGapPx } from "./symbols";
import { add, dot, scale, vec3, type Vec3 } from "../geometry/vec3";
import { statusLine } from "../test/statusLine";

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
      "AZM 030° ELV +30° ARROWS ROTATE +/- ZOOM Z FIT",
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

/**
 * The default mark's symbol's radius as painted at jsdom's ratio of 1: 5.25 px, moved out 0.25 px
 * by its outline's shift (R07.T16.f).
 */
const MARK_ARC_PX = 5.5;

/** The centre of the default mark's symbol as last painted. */
function markCentre(recorder: RecordingContext2D): ReadonlyArray<unknown> {
  return (
    recorder
      .calls("arc")
      .findLast(({ args }) => args[2] === MARK_ARC_PX)
      ?.args.slice(0, 2) ?? []
  );
}

/** The radius of the query edge, a circle about the view centre, as last painted. */
function edgeRadius(recorder: RecordingContext2D): unknown {
  return recorder
    .calls("arc")
    .findLast(({ args }) => args[0] === 200 && args[1] === 150 && args[2] !== MARK_ARC_PX)?.args[2];
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

    expect(
      screen.getByText(statusLine("DIRECTIONS UNDEFINED: grid aligned to -X")),
    ).toBeInTheDocument();
    expect(container.querySelector("[data-core]")).toBeNull();
    expect(within(screen.getByRole("img", { name: "Axis triad" })).getByText("-X")).toBeVisible();
  });

  it("tracks DIRECTIONS UNDEFINED: in the axis note, and leaves the grid's sentence untracked", () => {
    setup({ scene: aScene({ frame: localFrameAt(vec3(0, 0, 0)) }) });

    const state = screen.getByText("DIRECTIONS UNDEFINED:");
    expect([
      state.className,
      state.nextElementSibling?.className,
      state.parentElement?.className,
    ]).toEqual(["status-text__state", "status-text__sentence", "spatial-view__axis-note"]);
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

/** The observer the furniture's tests observe from, written as the centre is. */
const OBSERVED_FROM: SpatialMode = {
  name: "OBSERVED FROM",
  from: [
    { label: "RADIUS", value: "10,000.0", unit: "ly", widthCh: 9 },
    { label: "ANGLE", value: "036.9°", unit: "", widthCh: 6 },
    { label: "HEIGHT", value: "-12.3", unit: "ly", widthCh: 9 },
  ],
};

/** A centre on the galactic axis, whose `ANGLE` is missing. */
const CENTRE_ON_AXIS: SpatialViewProps["centre"] = [
  { label: "RADIUS", value: "0.0", unit: "ly", widthCh: 9 },
  { label: "ANGLE", value: null, unit: "", widthCh: 6 },
  { label: "HEIGHT", value: "+100.0", unit: "ly", widthCh: 9 },
];

/** The view's furniture, the row under its canvas, which its `FRAME` reading opens. */
function furniture(): HTMLElement {
  const row = screen.getByText("FRAME", { selector: "dt" }).closest(".spatial-view__furniture");
  if (!(row instanceof HTMLElement)) {
    throw new Error("the view has no furniture");
  }
  return row;
}

/** A reading's value in the furniture, by its term's label, the `n`th where the label repeats. */
function furnitureValue(label: string, n = 0): HTMLElement {
  const value = screen.getAllByText(label, { selector: ".spatial-view__furniture dt" })[n]
    ?.nextElementSibling;
  if (!(value instanceof HTMLElement)) {
    throw new Error(`the furniture has no ${label} value`);
  }
  return value;
}

/** A value's stale slot, or `null` where it has none. */
function slotOf(value: HTMLElement): Element | null {
  return value.querySelector(":scope > .stale-slot");
}

describe("SpatialView's stale furniture, one S per value (plan 12, P12.T11.d)", () => {
  function setup(props: Partial<SpatialViewProps> = {}): void {
    fakeFramesAndTimeouts();
    stubLayout(400, 300);
    stubCanvas();
    render(viewOf(props));
  }

  afterEach(() => {
    vi.useRealTimers();
  });

  it("trails an S after each of the centre's values and the time while stale", () => {
    setup({ stale: true });

    for (const label of ["RADIUS", "ANGLE", "HEIGHT", "UT"]) {
      expect(slotOf(furnitureValue(label))).toHaveTextContent(/^S\s*stale$/u);
    }
  });

  it("trails an S after each of the observer's values in observed mode", () => {
    setup({ stale: true, mode: OBSERVED_FROM });

    for (const label of ["RADIUS", "ANGLE", "HEIGHT"]) {
      // The observer's are the second of each label, nested in the mode's value.
      expect(slotOf(furnitureValue(label, 1))).toHaveTextContent(/^S\s*stale$/u);
    }
    // The centre's three, the time and the observer's three: seven marks, and no others.
    expect(screen.getAllByText("stale")).toHaveLength(7);
  });

  it("marks neither the frame's name nor the mode's name, which are words", () => {
    setup({ stale: true, mode: OBSERVED_FROM });

    expect(furnitureValue("FRAME")).toHaveTextContent(/^GALACTIC$/u);
    expect(furnitureValue("MODE").firstChild?.textContent).toBe("OBSERVED FROM");
  });

  it("keeps an em dash's slot empty while stale", () => {
    setup({ stale: true, centre: CENTRE_ON_AXIS });

    expect(furnitureValue("ANGLE")).toHaveTextContent(/^—$/u);
    expect(slotOf(furnitureValue("ANGLE"))).toBeEmptyDOMElement();
  });

  it("marks nothing in the scale bar, which is the camera's", () => {
    setup({ stale: true, mode: OBSERVED_FROM });

    expect(within(scaleBarElement()).queryByText("stale")).not.toBeInTheDocument();
  });

  it("leaves no mark outside a value's slot, after the scale bar or anywhere else", () => {
    setup({ stale: true, mode: OBSERVED_FROM });

    for (const mark of furniture().querySelectorAll(".stale-mark")) {
      expect(mark.closest(".stale-slot")).not.toBeNull();
    }
  });

  it("keeps each value's slot, empty, while live", () => {
    setup({ mode: OBSERVED_FROM });

    for (const [label, n] of [
      ["RADIUS", 0],
      ["ANGLE", 0],
      ["HEIGHT", 0],
      ["UT", 0],
      ["RADIUS", 1],
      ["ANGLE", 1],
      ["HEIGHT", 1],
    ] as const) {
      const slot = slotOf(furnitureValue(label, n));
      expect(slot).not.toBeNull();
      expect(slot).toBeEmptyDOMElement();
    }
    expect(screen.queryByText("stale")).not.toBeInTheDocument();
  });

  it.each([
    ["live", false],
    ["stale", true],
  ])("gives the camera's AZM and ELV no slot and no mark while %s", (_, stale) => {
    setup({ stale });

    for (const label of ["AZM", "ELV"]) {
      const value = screen.getByText(label, { selector: "dt" }).nextElementSibling;
      expect(value).toHaveTextContent(/^[+\-\d]+°$/u);
      expect(value?.querySelector(".stale-slot")).toBeNull();
    }
  });

  it("stands the scale bar after the time and before the mode, in a dl of its own", () => {
    setup({ mode: OBSERVED_FROM });

    const time = furnitureValue("UT").parentElement;
    const mode = furnitureValue("MODE").closest("dl");
    const bar = scaleBarElement();
    if (time === null || mode === null) {
      throw new Error("the furniture has no time or no mode");
    }
    expect(time.compareDocumentPosition(bar)).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(bar.compareDocumentPosition(mode)).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(mode).toHaveClass("spatial-view__readings");
    expect(mode.parentElement).toBe(furniture());
    expect(mode.children).toHaveLength(1);
  });

  it("keeps one dl, then the scale bar, on a view with no mode", () => {
    setup();

    const lists = furniture().querySelectorAll(":scope > dl");
    expect(lists).toHaveLength(1);
    expect([...(lists[0]?.querySelectorAll("dt") ?? [])].map((term) => term.textContent)).toEqual([
      "FRAME",
      "RADIUS",
      "ANGLE",
      "HEIGHT",
      "UT",
    ]);
    expect(lists[0]?.nextElementSibling).toBe(scaleBarElement());
  });

  it("gives a time the console keeps itself no slot and no S while stale, marked as its own", () => {
    setup({
      stale: true,
      time: { label: "DISPLAY TIME UT", value: "+0 yr 000/00:00:00", unit: "", widthCh: 21 },
      timeIsOwn: true,
    });

    const time = furnitureValue("DISPLAY TIME UT");
    expect(time).toHaveTextContent(/^\+0 yr 000\/00:00:00$/u);
    expect(slotOf(time)).toBeNull();
    expect(time.parentElement).toHaveClass("spatial-reading--own");
    // The centre's values are the answer's, and stay marked.
    expect(slotOf(furnitureValue("HEIGHT"))).toHaveTextContent(/^S\s*stale$/u);
  });

  it("keeps a time the console keeps itself in --text while the view is stale", () => {
    expect(stylesheetRule(".spatial-view--stale .spatial-reading--own")).toMatch(
      /color: var\(--text\);/u,
    );
  });

  it("still names its canvas stale", () => {
    setup({ stale: true, mode: OBSERVED_FROM });

    expect(screen.getByRole("application", { name: "Local chart, stale" })).toBeInTheDocument();
  });

  it("still mutes the furniture's readings, both lists of them, while stale", () => {
    expect(
      stylesheetRule(
        ".spatial-view--stale :is(.spatial-view__readings, .spatial-view__labels),\n.census-readout--stale,\n.systems-panel--stale :is(.system-list, .system-readout)",
      ),
    ).toMatch(/color: var\(--text-muted\);/u);
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

/**
 * The places of the marks about a mark that the last paint drew, in order, CSS px: a bracket's
 * half-size, half the span of its first two corners; or the destination's chevrons' apex distance,
 * half the span of the upper and lower apices, each its subpath's first `lineTo`.
 */
function reticlePlaces(recorder: RecordingContext2D): Array<{ stroke: string; placePx: number }> {
  const places: Array<{ stroke: string; placePx: number }> = [];
  const records = recorder.records;
  const lastClear = records.findLastIndex(
    (record) => record.type === "call" && record.name === "fillRect",
  );
  let stroke = "";
  let moves: number[] = [];
  let apices: number[] = [];
  for (const record of records.slice(lastClear)) {
    if (record.type === "set" && record.name === "strokeStyle") {
      stroke = String(record.value);
    } else if (record.type === "call" && record.name === "beginPath") {
      moves = [];
      apices = [];
    } else if (record.type === "call" && record.name === "moveTo") {
      moves.push(Number(record.args[0]));
    } else if (record.type === "call" && record.name === "lineTo" && apices.length < moves.length) {
      apices.push(Number(record.args[1]));
    } else if (record.type === "call" && record.name === "stroke" && moves.length === 4) {
      places.push({
        stroke,
        placePx:
          stroke === "#e879f9"
            ? ((apices[1] ?? 0) - (apices[0] ?? 0)) / 2
            : ((moves[1] ?? 0) - (moves[0] ?? 0)) / 2,
      });
    }
  }
  return places;
}

/**
 * The highest point the last paint traced for the destination's chevrons, CSS px: the upper
 * chevron's arm ends. The chevrons are one path of four subpaths, above, below, left and right.
 */
function chevronsTopPx(recorder: RecordingContext2D): number {
  const records = recorder.records;
  const lastClear = records.findLastIndex(
    (record) => record.type === "call" && record.name === "fillRect",
  );
  let stroke = "";
  let subpaths: number[][] = [];
  let top = Number.NaN;
  for (const record of records.slice(lastClear)) {
    if (record.type === "set" && record.name === "strokeStyle") {
      stroke = String(record.value);
    } else if (record.type === "call" && record.name === "beginPath") {
      subpaths = [];
    } else if (record.type === "call" && record.name === "moveTo") {
      subpaths.push([Number(record.args[1])]);
    } else if (record.type === "call" && record.name === "lineTo") {
      subpaths.at(-1)?.push(Number(record.args[1]));
    } else if (record.type === "call" && record.name === "stroke" && stroke === "#e879f9") {
      top = Math.min(...subpaths.flat());
    }
  }
  return top;
}

/** Mark "a"'s label's place, CSS px from the view's top left, in a view of the scene with `overrides`. */
function labelPlaceIn(overrides: Partial<SpatialScene>): [number, number] {
  const { unmount } = renderView({ scene: aScene(overrides) });
  const place: [number, number] = [labelLeftPx("A"), labelTopPx("A")];
  unmount();
  return place;
}

/** A label's left edge, CSS px, from its transform in `rem` at a rem of 16 px. */
function labelLeftPx(text: string): number {
  const transform = screen.getByText(text).style.transform;
  return 16 * Number(/^translate\(([-\d.e]+)rem/u.exec(transform)?.[1]);
}

/** A label's top edge, CSS px, from its transform in `rem` at a rem of 16 px. */
function labelTopPx(text: string): number {
  const transform = screen.getByText(text).style.transform;
  return 16 * Number(/, ([-\d.e]+)rem\)$/u.exec(transform)?.[1]);
}

describe("SpatialView at the display's ratio (R07.T16.f)", () => {
  beforeEach(() => {
    stubLayout(1200, 900);
  });

  it("stands the destination's chevrons the least gap outside the bracket, selected or not: 5.12 px at 0.78125", () => {
    vi.stubGlobal("devicePixelRatio", 0.78125);
    const { recorder, unmount } = renderView({
      scene: aScene({ selectedId: "a", destinationId: "a" }),
    });
    const [bracket, destination] = reticlePlaces(recorder);
    unmount();
    const lone = renderView({ scene: aScene({ destinationId: "a" }) });

    expect([
      bracket?.stroke,
      destination?.stroke,
      Math.round(((destination?.placePx ?? 0) - (bracket?.placePx ?? 0)) * 100) / 100,
      reticlePlaces(lone.recorder),
    ]).toEqual(["#5cc8e6", "#e879f9", 5.12, [destination]]);
  });

  it("never moves a mark's label when it is selected or deselected, the destination or not", () => {
    vi.stubGlobal("devicePixelRatio", 0.78125);

    expect([
      labelPlaceIn({ selectedId: "a" }),
      labelPlaceIn({ selectedId: "a", destinationId: "a" }),
    ]).toEqual([labelPlaceIn({}), labelPlaceIn({ destinationId: "a" })]);
  });

  it("keeps the destination's label beside the bracket", () => {
    vi.stubGlobal("devicePixelRatio", 0.78125);
    const [besideLeftPx] = labelPlaceIn({});
    renderView({ scene: aScene({ destinationId: "a" }) });

    expect(labelLeftPx("A")).toBe(besideLeftPx);
  });

  it("stands the destination's label 0.25 rem above its painted chevrons' ink", () => {
    vi.stubGlobal("devicePixelRatio", 0.78125);
    const { recorder } = renderView({ scene: aScene({ destinationId: "a" }) });

    // The upper chevron's arm ends, less half its 2.56 CSS px stroke at this ratio, less the label's
    // bottom edge, its top and one line of 1.25 × 0.875 rem: 0.25 rem at a rem of 16 px.
    expect(chevronsTopPx(recorder) - 2.56 / 2 - (labelTopPx("A") + 17.5)).toBeCloseTo(4, 6);
  });

  it("stands a mark's label the bracket's growth further out at 1 than at 2: 1.25 px", () => {
    vi.stubGlobal("devicePixelRatio", 2);
    const { unmount } = renderView({ scene: aScene({ selectedId: "a" }) });
    const atTwo = labelLeftPx("A");
    unmount();
    vi.stubGlobal("devicePixelRatio", 1);
    renderView({ scene: aScene({ selectedId: "a" }) });

    expect(labelLeftPx("A") - atTwo).toBeCloseTo(1.25, 9);
  });
});

/** The default camera's scale in a 1200 × 900 stage: the fit radius of 50 in its shorter side, 2 rem in. */
const FIT_PX_PER_UNIT = (450 - 32) / 50;

/** The point on the reference plane that the default camera, `OBLIQUE` at the fit, draws at a place. */
function onPlaneAt(xPx: number, yPx: number) {
  const basis = viewBasis(FRAME, PRESETS.oblique);
  const across = (xPx - 600) / FIT_PX_PER_UNIT;
  const up = (450 - yPx) / FIT_PX_PER_UNIT;
  const rc = dot(FRAME.coreward, basis.right);
  const rs = dot(FRAME.spinward, basis.right);
  const uc = dot(FRAME.coreward, basis.up);
  const us = dot(FRAME.spinward, basis.up);
  const det = rc * us - rs * uc;
  return add(
    scale(FRAME.coreward, (across * us - rs * up) / det),
    scale(FRAME.spinward, (rc * up - across * uc) / det),
  );
}

/** Where the default camera draws a point, CSS px. */
function drawnAt(position: Vec3) {
  return project(
    position,
    viewBasis(FRAME, PRESETS.oblique),
    { ...PRESETS.oblique, pxPerUnit: FIT_PX_PER_UNIT },
    { widthPx: 1200, heightPx: 900, remPx: 16 },
  );
}

describe("SpatialView's labels off the observer's eye (plan 12, P12.T11.c)", () => {
  beforeEach(() => {
    stubLayout(1200, 900);
  });

  // A mark 40 px left of the view's centre, its label running right across the centre, from about
  // 576 to 654 px, where the eye stands: unlabelled, so that only the eye's box can drop the label.
  const near = aMark("m", { position: onPlaneAt(560, 450), label: "MARK NEAR" });
  const eye = { position: onPlaneAt(600, 450), label: "" };

  it("drops a system's label that would cover the eye", () => {
    renderView({ scene: aScene({ points: [near], observer: eye }) });

    expect(screen.queryByText("MARK NEAR")).toBeNull();
  });

  it("puts the selection's label on its other side, clear of the eye", () => {
    renderView({ scene: aScene({ points: [near], selectedId: "m", observer: eye }) });

    expect(placement(overlayLabel("MARK NEAR"))).toContain("- 100%");
  });

  it("labels the system as before in now, with no observer", () => {
    renderView({ scene: aScene({ points: [near], selectedId: "m" }) });

    expect(placement(overlayLabel("MARK NEAR"))).not.toContain("- 100%");
  });

  it("labels the system as before with the observer outside the view", () => {
    renderView({
      scene: aScene({ points: [near], observer: { position: onPlaneAt(-200, 450), label: "" } }),
    });

    expect(screen.getByText("MARK NEAR")).toBeInTheDocument();
  });
});

describe("SpatialView's labels with no side beside their mark (plan 12, P12.T11.e)", () => {
  const DESIGNATION = "GDG 0E0V00 E-141";
  // At the view's centre, a class-4 symbol whose 16-character label, 138.88 px from 18 px out, has
  // room on neither side in the compact chart's 256 px stage, and on the right in the stacked one.
  const centred = aMark("s", { position: vec3(0, 0, 0), sizeClass: 4, label: DESIGNATION });

  it("gives the selection at the view's centre no label in the compact chart's 256 px stage", () => {
    stubLayout(256, 262);
    renderView({ scene: aScene({ points: [centred], selectedId: "s" }) });

    expect(
      screen.queryAllByText(
        (_text, element) =>
          element instanceof HTMLSpanElement && element.textContent === DESIGNATION,
      ),
    ).toHaveLength(0);
  });

  it("gives the selection at the view's centre its label in the stacked page's 1036 px stage", () => {
    stubLayout(1036, 600);
    renderView({ scene: aScene({ points: [centred], selectedId: "s" }) });

    expect(overlayLabel(DESIGNATION)).toBeInTheDocument();
  });
});

describe("SpatialView's furniture text clear of the marks (R07.T16.j)", () => {
  beforeEach(() => {
    stubLayout(1200, 900);
  });

  it("gives the curve labels the marks' symbols to keep clear of: a ring's label leaves a mark on its first place", () => {
    // The plane ring's label would stand from 8 px right of and 4 px below its coreward point; a
    // mark on the plane sits 30 px right of and 12 px below it.
    const ring = drawnAt(scale(FRAME.coreward, 20));
    const at = onPlaneAt(ring.xPx + 30, ring.yPx + 12);
    renderView({
      scene: aScene({
        points: [aMark("m", { position: at })],
        plane: { spacing: 20, extent: 50, rings: [{ radius: 20, label: "PLANE 20 ly" }] },
      }),
    });
    const [leftRem = Number.NaN, topRem = Number.NaN] = (
      /^translate\(([-\d.e]+)rem, ([-\d.e]+)rem\)$/u.exec(placement(overlayLabel("PLANE 20 ly"))) ??
      []
    )
      .slice(1)
      .map(Number);
    const size = textSizeRem("PLANE 20 ly", 0.1);
    const box = {
      leftPx: 16 * leftRem,
      topPx: 16 * topRem,
      widthPx: size.widthRem * 16,
      heightPx: size.heightRem * 16,
    };
    const mark = drawnAt(at);
    // The class-2 symbol's outline reaches 6.5 px from its centre at a ratio of 1.
    const gapPx = Math.hypot(
      Math.max(0, box.leftPx - mark.xPx, mark.xPx - box.leftPx - box.widthPx),
      Math.max(0, box.topPx - mark.yPx, mark.yPx - box.topPx - box.heightPx),
    );

    const atFirstPlace =
      Math.abs(box.leftPx - ring.xPx - 8) < 1e-6 && Math.abs(box.topPx - ring.yPx - 4) < 1e-6;

    expect([atFirstPlace, gapPx >= 6.5 + 2 - 1e-6]).toEqual([false, true]);
  });

  it.each([
    ["0.375 rem below the query edge's is not drawn", 40.75, false],
    ["0.625 rem below it is drawn", 44.75, true],
  ] as const)(
    "gives the marks' labels the curve labels to stand 0.5 rem from: a label %s",
    (_, yPx, drawn) => {
      // The query edge's label stands above the top of its circle, at (600, 32), from 608 px across
      // and 8.5 to 26 px down; a mark's label at its right, from 610 px across, centred on it.
      renderView({ scene: aScene({ points: [aMark("m", { position: onPlaneAt(594, yPx) })] }) });

      expect(screen.queryByText("M") !== null).toBe(drawn);
    },
  );

  it("keeps the destination's label where it was on selecting a mark outside the eight labelled, and flips the selection's", () => {
    // Eight marks of higher priority along the foot of the view; the selection, of the lowest, 20 px
    // left of and 50 px above the destination, its long label at the right running into the
    // destination's at the upper right.
    const heavy = Array.from({ length: 8 }, (_, index) =>
      aMark(`h${String(index)}`, {
        position: onPlaneAt(150 + index * 120, 820),
        labelPriority: 10 + index,
      }),
    );
    const points = [
      ...heavy,
      aMark("d", { position: onPlaneAt(600, 400), label: "DEST", labelPriority: 0 }),
      aMark("s", { position: onPlaneAt(580, 350), label: "SELECTED MARK", labelPriority: -1 }),
    ];
    const { unmount } = renderView({ scene: aScene({ points, destinationId: "d" }) });
    const before = placement(overlayLabel("DEST"));
    unmount();

    renderView({ scene: aScene({ points, destinationId: "d", selectedId: "s" }) });

    expect([placement(overlayLabel("DEST")), placement(overlayLabel("SELECTED MARK"))]).toEqual([
      before,
      expect.stringContaining("- 100%"),
    ]);
  });
});

/**
 * A label's box over the view, from where it is placed and its estimated size, CSS px: a curve
 * label's set with the overlay's 0.1 em letter spacing, a mark's on its right with none.
 */
function boxOfLabel(element: HTMLElement, text: string, letterSpacingEm: number): BoxPx {
  const [leftRem = Number.NaN, topRem = Number.NaN] = (
    /^translate\(([-\d.e]+)rem, ([-\d.e]+)rem\)$/u.exec(placement(element)) ?? []
  )
    .slice(1)
    .map(Number);
  const size = textSizeRem(text, letterSpacingEm);
  return {
    leftPx: 16 * leftRem,
    topPx: 16 * topRem,
    widthPx: size.widthRem * 16,
    heightPx: size.heightRem * 16,
  };
}

/** The box of a label that must be drawn over the view ({@link boxOfLabel}). */
function drawnBox(text: string, letterSpacingEm: number): BoxPx {
  return boxOfLabel(overlayLabel(text), text, letterSpacingEm);
}

/** The box of an annulus's label, `null` where it is not drawn, as it may not be. */
function annulusBox(text: string): BoxPx | null {
  const element = screen.queryByText(
    (_, candidate) => candidate instanceof HTMLSpanElement && candidate.textContent === text,
  );
  return element === null ? null : boxOfLabel(element, text, 0.1);
}

/** Whether a box stands at another's place, to within rounding. */
function sameBox(a: BoxPx, b: BoxPx): boolean {
  return Math.abs(a.leftPx - b.leftPx) < 1e-6 && Math.abs(a.topPx - b.topPx) < 1e-6;
}

/** The box of the selection's bracket's place about a class-2 mark, 12 px about its centre at 1. */
function bracketAbout(xPx: number, yPx: number): BoxPx {
  return bracketBoxPx({ id: "m", xPx, yPx, depth: 0, radiusPx: 6 }, 16, reticleStrokesCssPx(1));
}

/** A habitable zone 14 to 20 ly about the view centre, labelled at its outer rimward point. */
const ZONE: AnnulusMark = {
  id: "hz",
  innerRadius: 14,
  outerRadius: 20,
  ticks: false,
  label: "HABITABLE ZONE",
};
/** The zone's outer edge's rimward point, where the default camera draws it. */
const RIMWARD = drawnAt(scale(FRAME.coreward, -20));
/** The zone's label's first place: 0.5 rem right of the point, 0.25 rem below. */
const FIRST: BoxPx = {
  leftPx: RIMWARD.xPx + 8,
  topPx: RIMWARD.yPx + 4,
  widthPx: textSizeRem("HABITABLE ZONE", 0.1).widthRem * 16,
  heightPx: 17.5,
};

describe("SpatialView's annulus labels, placed after the marks' (plan 14, P14.T42.d)", () => {
  beforeEach(() => {
    stubLayout(1200, 900);
  });

  it("draws a lesser mark's label over an annulus label's first place, which stands 0.5 rem off it", () => {
    // A mark 10 px left of and 12 px below the rimward point, clear of the zone's label's first
    // place by its symbol, its label `M` across that place: dropped for it before (D4).
    const at = onPlaneAt(RIMWARD.xPx - 10, RIMWARD.yPx + 12);
    renderView({ scene: aScene({ points: [aMark("m", { position: at })], annuli: [ZONE] }) });

    const mark = drawnBox("M", 0);
    const zone = annulusBox("HABITABLE ZONE");
    expect([
      boxGapPx(mark, FIRST) === 0,
      zone === null || boxGapPx(zone, mark) >= 8 - 1e-6,
      zone === null || boxGapPx(zone, bracketAbout(RIMWARD.xPx - 10, RIMWARD.yPx + 12)) >= 4 - 1e-6,
    ]).toEqual([true, true, true]);
  });

  it("keeps the label of a selection on an annulus's rimward point at its right", () => {
    renderView({
      scene: aScene({
        points: [aMark("s", { position: onPlaneAt(RIMWARD.xPx, RIMWARD.yPx), label: "S /256" })],
        selectedId: "s",
        annuli: [ZONE],
      }),
    });

    const selection = drawnBox("S /256", 0);
    const zone = annulusBox("HABITABLE ZONE");
    expect([
      placement(overlayLabel("S /256")),
      zone === null || boxGapPx(zone, selection) >= 8 - 1e-6,
    ]).toEqual([expect.not.stringContaining("- 100%"), true]);
  });

  it("moves an annulus's label 0.25 rem outside a mark's bracket's box, 0.125 rem of which stood beside its first place", () => {
    // A mark right of the zone's label's first place, its bracket's box 2 px from it, its symbol
    // 7.5 px, its label 30 px: the first place met every rule but the bracket's before.
    const xPx = FIRST.leftPx + FIRST.widthPx + 2 + 12;
    const yPx = FIRST.topPx + FIRST.heightPx / 2;
    renderView({
      scene: aScene({ points: [aMark("m", { position: onPlaneAt(xPx, yPx) })], annuli: [ZONE] }),
    });

    const zone = drawnBox("HABITABLE ZONE", 0.1);
    expect([
      boxGapPx(FIRST, bracketAbout(xPx, yPx)),
      sameBox(zone, FIRST),
      boxGapPx(zone, bracketAbout(xPx, yPx)) >= 4 - 1e-6,
    ]).toEqual([expect.closeTo(2, 6), false, true]);
  });

  it("moves an annulus's label off a belt's ticks across its first place", () => {
    // A belt 5 to 25 ly about a point 44 ly rimward, ticked every 10°, whose ticks, and not its
    // edges, cross the zone's label's first place.
    const centre = scale(FRAME.coreward, -44);
    const belt: AnnulusMark = {
      id: "belt",
      centre,
      innerRadius: 5,
      outerRadius: 25,
      ticks: true,
      label: "",
    };
    const onBelt = (radius: number, angleDeg: number) => {
      const angle = (angleDeg * Math.PI) / 180;
      return drawnAt(
        add(
          centre,
          add(
            scale(FRAME.coreward, radius * Math.cos(angle)),
            scale(FRAME.spinward, radius * Math.sin(angle)),
          ),
        ),
      );
    };
    // Each 1 px wide, drawn 2 px wide at a ratio of 1.
    const across = (box: BoxPx): number =>
      Array.from({ length: 36 }, (_, index) => index * 10).filter(
        (angleDeg) => segmentBoxGapPx(onBelt(5, angleDeg), onBelt(25, angleDeg), box) <= 1,
      ).length;
    renderView({ scene: aScene({ points: [], annuli: [ZONE, belt] }) });

    const zone = drawnBox("HABITABLE ZONE", 0.1);
    expect([across(FIRST) > 0, sameBox(zone, FIRST), across(zone)]).toEqual([true, false, 0]);
  });
});

/** A label over the view, `null` where it is not drawn, as a lesser mark's may not be. */
function labelOrNull(text: string): HTMLElement | null {
  return screen.queryByText(
    (_, element) => element instanceof HTMLSpanElement && element.textContent === text,
  );
}

describe("SpatialView's selected annulus's label, placed as the selection's (plan 14, P14.T42.e)", () => {
  beforeEach(() => {
    stubLayout(1200, 900);
  });

  /**
   * A selected annulus at the zone's place, unticked and labelled as a belt: the pass tells a belt
   * from a zone by nothing but its flags.
   */
  const BELT: AnnulusMark = { ...ZONE, id: "belt", label: "BELT 1", selected: true };
  /** The belt's label's first place, where the zone's starts. */
  const BELT_FIRST: BoxPx = { ...FIRST, widthPx: textSizeRem("BELT 1", 0.1).widthRem * 16 };
  /**
   * A lesser mark 10 px left of and 12 px below the rimward point, as in T42.d's first test: its
   * label `M` covers the first place, which its bracket's box stands 6 px left of.
   */
  const LESSER = aMark("m", { position: onPlaneAt(RIMWARD.xPx - 10, RIMWARD.yPx + 12) });
  const LESSER_BRACKET = bracketAbout(RIMWARD.xPx - 10, RIMWARD.yPx + 12);

  it("keeps a selected belt's label at its first place, where a lesser mark's label gives way", () => {
    renderView({ scene: aScene({ points: [LESSER], annuli: [BELT] }) });

    expect([
      boxGapPx(BELT_FIRST, LESSER_BRACKET),
      sameBox(drawnBox("BELT 1", 0.1), BELT_FIRST),
      labelOrNull("M"),
    ]).toEqual([expect.closeTo(6, 6), true, null]);
  });

  it("still draws a lesser mark's label over an unselected belt's first place, as T42.d built", () => {
    renderView({ scene: aScene({ points: [LESSER], annuli: [{ ...BELT, selected: false }] }) });

    const mark = drawnBox("M", 0);
    const belt = annulusBox("BELT 1");
    expect([boxGapPx(mark, BELT_FIRST), belt === null || boxGapPx(belt, mark) >= 8 - 1e-6]).toEqual(
      [0, true],
    );
  });

  it("keeps a selected belt's label at its first place, which a zone's label earlier in the scene would take", () => {
    renderView({ scene: aScene({ points: [], annuli: [ZONE, BELT] }) });

    const belt = drawnBox("BELT 1", 0.1);
    const zone = annulusBox("HABITABLE ZONE");
    expect([
      boxGapPx(FIRST, BELT_FIRST),
      sameBox(belt, BELT_FIRST),
      zone === null || boxGapPx(zone, belt) >= 8 - 1e-6,
    ]).toEqual([0, true, true]);
  });

  it("leaves out a selected belt's label whose every place stands within 0.25 rem of a mark's bracket's box", () => {
    // A belt 0.1 to 0.3 ly about a mark, within 2.6 px of its centre on the screen: every place
    // beside its outer edge, 0.5 rem to one side and 0.25 rem above or below, starts inside the
    // mark's bracket's box, 12 px about its centre. The mark's own label stands beside it.
    const at = onPlaneAt(800, 300);
    renderView({
      scene: aScene({
        points: [aMark("m", { position: at })],
        annuli: [{ ...BELT, centre: at, innerRadius: 0.1, outerRadius: 0.3 }],
      }),
    });

    expect([annulusBox("BELT 1"), overlayLabel("M").textContent]).toEqual([null, "M"]);
  });

  it("places a selected belt's scattered disc's label with the belt's, before the marks' labels", () => {
    // The belt 5 to 10 ly, labelled at its spinward point, and its scattered disc at the zone's
    // place, both selected, as `populationAnnuli` gives them: the lesser mark's label covers the
    // disc's first place, which `HABITABLE ZONE`'s length makes `FIRST`.
    const belt: AnnulusMark = {
      id: "belt",
      innerRadius: 5,
      outerRadius: 10,
      ticks: false,
      label: "BELT 1",
      labelSpinward: true,
      selected: true,
    };
    const disc: AnnulusMark = {
      ...ZONE,
      id: "belt:scattered",
      label: "SCATTERED DISC",
      selected: true,
    };
    renderView({ scene: aScene({ points: [LESSER], annuli: [belt, disc] }) });

    // The belt's label, placed before it in the scene's order, stands 0.5 rem off it.
    const placed = drawnBox("SCATTERED DISC", 0.1);
    const beltLabel = annulusBox("BELT 1");
    expect([
      sameBox(placed, FIRST),
      labelOrNull("M"),
      beltLabel === null || boxGapPx(beltLabel, placed) >= 8 - 1e-6,
    ]).toEqual([true, null, true]);
  });

  it("keeps a selected belt's label 0.5 rem from a curve label placed before it", () => {
    // A belt 15 to 20 ly about a point 40 ly coreward, whose outer edge's rimward point is the
    // plane ring's coreward point: the ring's label, placed first, stands at the belt's first place.
    const centre = scale(FRAME.coreward, 40);
    const point = drawnAt(scale(FRAME.coreward, 20));
    const beltFirst: BoxPx = { ...BELT_FIRST, leftPx: point.xPx + 8, topPx: point.yPx + 4 };
    renderView({
      scene: aScene({
        points: [],
        plane: { spacing: 20, extent: 50, rings: [{ radius: 20, label: "PLANE 20 ly" }] },
        annuli: [{ ...BELT, centre, innerRadius: 15, outerRadius: 20 }],
      }),
    });

    const ring = drawnBox("PLANE 20 ly", 0.1);
    const belt = annulusBox("BELT 1");
    expect([sameBox(ring, beltFirst), belt === null || boxGapPx(belt, ring) >= 8 - 1e-6]).toEqual([
      true,
      true,
    ]);
  });
});

describe("SpatialView's annulus labels, crossed by no tick (plan 14, P14.T42.f)", () => {
  beforeEach(() => {
    stubLayout(1200, 900);
  });

  it("leaves out a zone's label whose every place a belt's ticks cross, and draws it with the belt switched off", () => {
    // A zone 1 to 2 ly about the view centre, its outer edge 17 px or less from it on the screen,
    // inside a belt 0.5 to 45 ly about the same centre, ticked every 10° from 4 to 376 px out:
    // every place beside the zone's outer edge, 8.8 rem long, lies across that fan of ticks. Before
    // P14.T42.f, which makes the ticks a rule and not a preference, the label stood across them.
    const zone: AnnulusMark = { ...ZONE, innerRadius: 1, outerRadius: 2 };
    const belt: AnnulusMark = {
      id: "belt",
      innerRadius: 0.5,
      outerRadius: 45,
      ticks: true,
      label: "",
    };
    const view = renderView({ scene: aScene({ points: [], annuli: [zone, belt] }) });
    const withBelt = annulusBox("HABITABLE ZONE");
    view.rerender({ scene: aScene({ points: [], annuli: [zone] }) });

    expect([withBelt, annulusBox("HABITABLE ZONE") === null]).toEqual([null, false]);
  });
});

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

  it("paints the target chevrons about the destination, outside the selection's bracket", async () => {
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

    expect(
      screen.getByText(statusLine("DIRECTIONS UNDEFINED: grid aligned to -X")),
    ).toBeInTheDocument();
    expect(within(screen.getByRole("img", { name: "Axis triad" })).getByText("-X")).toBeVisible();
  });

  it("draws no core arrow when the axes given lie on the galactic axis", () => {
    const { container } = renderOnTheAxis();

    expect(container.querySelector("[data-core]")).toBeNull();
  });
});
