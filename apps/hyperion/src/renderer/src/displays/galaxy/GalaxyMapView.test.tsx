import type { MapPopulation, MapView } from "@hyperion/protocol";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { CentreLy } from "../../lib/galaxy/model";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { aDensityMap, UNIVERSE_ID } from "../../test/galaxyFixtures";
import { type RecordingContext2D, stubCanvas } from "../../test/RecordingContext2D";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { GalaxyMapView } from "./GalaxyMapView";

/** An 8 × 8 face-on map: the background, then every code up to the ceiling. */
const FACE_ON_CODES = Array.from({ length: 64 }, (_, pixel) => pixel * 4);

/** An 8 × 4 edge-on map. */
const EDGE_ON_CODES = Array.from({ length: 32 }, (_, pixel) => pixel * 8);

/** Plays the server's side, letting the outcomes it settles reach React. */
async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

interface ViewSpec {
  readonly view?: MapView;
  readonly population?: MapPopulation;
  readonly pictureWidthPx?: number;
  readonly devicePixelRatio?: number;
  readonly cursorLy?: CentreLy;
  readonly onCursor?: (cursorLy: CentreLy) => void;
  readonly centreLy?: CentreLy | null;
}

function viewOf({
  view = "face_on",
  population = "all",
  pictureWidthPx = 400,
  devicePixelRatio = 1,
  cursorLy = [0, 0, 0],
  onCursor = () => undefined,
  centreLy = null,
}: ViewSpec) {
  return (
    <ServerLinkHarness>
      <GalaxyMapView
        universe={UNIVERSE_ID}
        view={view}
        population={population}
        pictureWidthPx={pictureWidthPx}
        devicePixelRatio={devicePixelRatio}
        cursorLy={cursorLy}
        onCursor={onCursor}
        centreLy={centreLy}
      />
    </ServerLinkHarness>
  );
}

/** The pixels a recorded `putImageData` put, as RGBA bytes. */
function putPixels(recorder: RecordingContext2D, index = 0): Uint8ClampedArray {
  const image = recorder.calls("putImageData")[index]?.args[0];
  if (!isImage(image)) {
    throw new Error(`no putImageData ${index} with an image`);
  }
  return image.data;
}

function renderView(spec: ViewSpec = {}) {
  const recorder = stubCanvas();
  const result = render(viewOf(spec));
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  return {
    socket,
    recorder,
    rerender: (next: ViewSpec) => {
      result.rerender(viewOf(next));
    },
  };
}

/** Answers the face-on request with an 8 × 8 map whose one bright pixel is at the ceiling. */
async function answerBrightFaceOn(socket: FakeWebSocket): Promise<void> {
  const codes = Array<number>(64).fill(1);
  codes[7] = 255;
  await server(() => {
    socket.serverAnswers("density_map", () => aDensityMap({ codes, widthPx: 8, heightPx: 8 }));
  });
}

async function answerFaceOn(socket: FakeWebSocket): Promise<void> {
  await server(() => {
    socket.serverAnswers("density_map", (body) =>
      aDensityMap({ codes: FACE_ON_CODES, widthPx: 8, heightPx: 8, population: body.population }),
    );
  });
}

function faceOn(): HTMLElement {
  return screen.getByRole("region", { name: "FACE-ON FROM NORTH" });
}

function isImage(value: unknown): value is { readonly data: Uint8ClampedArray } {
  return (
    typeof value === "object" &&
    value !== null &&
    "data" in value &&
    value.data instanceof Uint8ClampedArray
  );
}

/**
 * The straight strokes of a drawn mark, each as its run across and down, read from its path's
 * `M x y` and `L x y` or `H x`, `V y` commands.
 */
function strokes(mark: HTMLElement): Array<[number, number]> {
  const d = mark.querySelector(".map-view__mark-line")?.getAttribute("d") ?? "";
  const runs: Array<[number, number]> = [];
  let at: [number, number] = [0, 0];
  for (const [, command = "", args = ""] of d.matchAll(/([MLHV])([^MLHV]*)/g)) {
    const numbers = args
      .trim()
      .split(/[\s,]+/)
      .map(Number);
    const [first = 0, second = 0] = numbers;
    let next: [number, number];
    switch (command) {
      case "H":
        next = [first, at[1]];
        break;
      case "V":
        next = [at[0], first];
        break;
      default:
        next = [first, second];
    }
    if (command !== "M") {
      runs.push([next[0] - at[0], next[1] - at[1]]);
    }
    at = next;
  }
  return runs;
}

/** The index in the recording of the first record that matches. */
function indexOf(recorder: RecordingContext2D, matches: (name: string, value: unknown) => boolean) {
  return recorder.records.findIndex((record) =>
    matches(record.name, record.type === "set" ? record.value : record.args),
  );
}

describe("GalaxyMapView", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("asks for the view and population of the universe's map at 8 bits", () => {
    const { socket } = renderView({ view: "edge_on", population: "young" });

    expect(socket.requestsOfKind("density_map").map(({ body }) => body)).toEqual([
      {
        kind: "density_map",
        universe: UNIVERSE_ID,
        view: "edge_on",
        population: "young",
        resolution: 512,
        bits: 8,
      },
    ]);
  });

  it.each([
    [100, 1, 128],
    [244, 1, 256],
    [320, 1, 256],
    [330, 1, 512],
    [562, 1, 512],
    [300, 2, 512],
    [733, 1.5, 1_024],
  ])(
    "asks, for a picture %i px wide at a device pixel ratio of %f, for the %i-pixel map",
    (pictureWidthPx, devicePixelRatio, resolution) => {
      const { socket } = renderView({ pictureWidthPx, devicePixelRatio });

      expect(socket.requestsOfKind("density_map").map(({ body }) => body.resolution)).toEqual([
        resolution,
      ]);
    },
  );

  it("asks again for the resolution that fits when the picture grows past it", () => {
    const { socket, rerender } = renderView({ pictureWidthPx: 244 });
    const [first] = socket.requestsOfKind("density_map");

    rerender({ pictureWidthPx: 330 });

    expect(socket.cancelledIds()).toEqual([first?.id]);
    expect(socket.requestsOfKind("density_map").map(({ body }) => body.resolution)).toEqual([
      256, 512,
    ]);
  });

  it("asks for nothing before the display is laid out", () => {
    const { socket } = renderView({ pictureWidthPx: 0 });

    expect(socket.requestsOfKind("density_map")).toEqual([]);
  });

  it("shows PENDING until the map arrives", () => {
    renderView();

    expect(within(faceOn()).getByRole("status")).toHaveTextContent("PENDING");
    expect(
      screen.queryByRole("application", { name: "Galaxy map, face-on" }),
    ).not.toBeInTheDocument();
  });

  it("puts the map's pixels once and draws them once, without smoothing", async () => {
    const { socket, recorder } = renderView();

    await answerFaceOn(socket);

    const canvas = screen.getByRole("application", { name: "Galaxy map, face-on" });
    const [put] = recorder.calls("putImageData");
    const [draw] = recorder.calls("drawImage");
    expect(recorder.calls("putImageData")).toHaveLength(1);
    expect(recorder.calls("drawImage")).toHaveLength(1);
    // Painted on a canvas of the map's own size, which is drawn onto the visible one.
    expect(put?.canvas).not.toBe(canvas);
    expect(put?.canvas).toMatchObject({ width: 8, height: 8 });
    expect(draw?.canvas).toBe(canvas);
    expect(draw?.args).toEqual([put?.canvas, 0, 0, 400, 400]);
    const smoothingOff = indexOf(
      recorder,
      (name, value) => name === "imageSmoothingEnabled" && value === false,
    );
    expect(smoothingOff).toBeGreaterThanOrEqual(0);
    expect(smoothingOff).toBeLessThan(indexOf(recorder, (name) => name === "drawImage"));
    expect(within(faceOn()).queryByRole("status")).not.toBeInTheDocument();
  });

  it("paints code 0 exactly in the background token", async () => {
    const { socket, recorder } = renderView();

    await answerFaceOn(socket);

    // The raster's first pixel, code 0, is the turned picture's top right: --surface-0, opaque.
    expect(Array.from(putPixels(recorder).subarray(7 * 4, 8 * 4))).toEqual([0x05, 0x08, 0x0d, 255]);
  });

  it("draws the face-on map turned, +x down and +y to the right", async () => {
    // One bright pixel at the raster's right-hand edge (the most +x) in its top row (the most +y).
    const codes = Array<number>(64).fill(0);
    codes[7] = 255;
    const { socket, recorder } = renderView();

    await server(() => {
      socket.serverAnswers("density_map", () => aDensityMap({ codes, widthPx: 8, heightPx: 8 }));
    });

    // On screen it is the bottom right-hand pixel: +x is down and +y to the right.
    const pixels = putPixels(recorder);
    const brightAt = Array.from({ length: 64 }, (_, pixel) => pixels[pixel * 4] ?? 0).findIndex(
      (red) => red === 0xc8,
    );
    expect(brightAt).toBe(63);
  });

  it("draws the edge-on map as sent, x across and z up", async () => {
    // The raster's top left-hand pixel: the most -x and the most +z.
    const codes = Array<number>(32).fill(0);
    codes[0] = 255;
    const { socket, recorder } = renderView({ view: "edge_on" });

    await server(() => {
      socket.serverAnswers("density_map", () => aDensityMap({ codes, view: "edge_on" }));
    });

    expect(putPixels(recorder)[0]).toBe(0xc8);
  });

  it("backs the picture at the device pixel ratio, in its view's proportions", async () => {
    const { socket } = renderView({ view: "edge_on", pictureWidthPx: 400, devicePixelRatio: 2 });

    await server(() => {
      socket.serverAnswers("density_map", () =>
        aDensityMap({ codes: EDGE_ON_CODES, view: "edge_on" }),
      );
    });

    const canvas = screen.getByRole("application", { name: "Galaxy map, edge-on" });
    expect(canvas).toHaveAttribute("width", "800");
    expect(canvas).toHaveAttribute("height", "400");
  });

  it("reduces a map wider than its picture by averaging, keeping a disc one pixel thick", async () => {
    // A 512 × 256 edge-on map at its floor but for one bright row, the young disc, drawn into a
    // picture 244 device pixels wide: nearest-pixel drawing would skip the row.
    const codes = Array<number>(512 * 256).fill(1);
    codes.fill(255, 128 * 512, 129 * 512);
    const { socket, recorder } = renderView({ view: "edge_on", pictureWidthPx: 244 });

    await server(() => {
      socket.serverAnswers("density_map", () =>
        aDensityMap({ codes, widthPx: 512, heightPx: 256, view: "edge_on", floorLog10PerLy2: -6 }),
      );
    });

    const [put] = recorder.calls("putImageData");
    const [draw] = recorder.calls("drawImage");
    // Reduced to the backing store and drawn onto it pixel for pixel.
    expect(put?.canvas).toMatchObject({ width: 244, height: 122 });
    expect(draw?.args.slice(1)).toEqual([0, 0, 244, 122]);
    const pixels = putPixels(recorder);
    const reds = Array.from({ length: 122 }, (_, row) => pixels[(row * 244 + 100) * 4] ?? 0);
    // The disc's row is near the top of the ramp; the floor around it is one level up from --surface-0.
    expect(Math.max(...reds)).toBeGreaterThan(0xb0);
  });

  it("states the title, axes, rotation and legend face-on", async () => {
    const { socket } = renderView();

    await answerFaceOn(socket);

    const view = faceOn();
    expect(within(view).getByRole("heading", { level: 3 })).toHaveTextContent("FACE-ON FROM NORTH");
    // +y across to the right, +x down the side, as the picture is turned.
    expect(within(view).getByText("+Y")).toHaveClass("map-view__axis--across");
    expect(within(view).getByText("+X")).toHaveClass("map-view__axis--bottom");
    expect(within(view).getByText("ROTATION COUNTER-CLOCKWISE")).toBeInTheDocument();
    expect(within(view).getByText("COLUMN DENSITY")).toBeInTheDocument();
    expect(
      within(view).getByText("FLOOR 1.00E-4: AT OR BELOW SHOWN AS BACKGROUND"),
    ).toBeInTheDocument();
  });

  it("labels the vertical axis +Z NORTH edge-on, and gives no rotation sense", async () => {
    const { socket } = renderView({ view: "edge_on" });

    await server(() => {
      socket.serverAnswers("density_map", () =>
        aDensityMap({ codes: EDGE_ON_CODES, view: "edge_on", floorLog10PerLy2: -6 }),
      );
    });

    const view = screen.getByRole("region", { name: "EDGE-ON ALONG +Y" });
    expect(within(view).getByText("+Z NORTH")).toHaveClass("map-view__axis--top");
    expect(within(view).getByText("+X")).toHaveClass("map-view__axis--across");
    expect(within(view).queryByText("ROTATION COUNTER-CLOCKWISE")).not.toBeInTheDocument();
    expect(
      within(view).getByText("FLOOR 1.00E-6: AT OR BELOW SHOWN AS BACKGROUND"),
    ).toBeInTheDocument();
  });

  it("does not paint again when re-rendered with the same map", async () => {
    const { socket, recorder, rerender } = renderView();
    await answerFaceOn(socket);
    recorder.clear();

    rerender({});

    expect(recorder.calls("putImageData")).toEqual([]);
    expect(recorder.calls("drawImage")).toEqual([]);
  });

  it("draws the same pixels again, without putting them again, when the picture is resized", async () => {
    const { socket, recorder, rerender } = renderView();
    await answerFaceOn(socket);
    recorder.clear();

    // 400 and 450 device pixels both take the 512-pixel map.
    rerender({ pictureWidthPx: 450 });

    expect(recorder.calls("putImageData")).toEqual([]);
    expect(recorder.calls("drawImage").map(({ args }) => args.slice(1))).toEqual([
      [0, 0, 450, 450],
    ]);
  });

  it("draws again at the new backing size when the device pixel ratio changes", async () => {
    const { socket, recorder, rerender } = renderView({ pictureWidthPx: 400 });
    await answerFaceOn(socket);
    recorder.clear();

    // 600 device pixels still take the 512-pixel map, so nothing is asked for again.
    rerender({ pictureWidthPx: 400, devicePixelRatio: 1.5 });

    const canvas = screen.getByRole("application", { name: "Galaxy map, face-on" });
    expect(canvas).toHaveAttribute("width", "600");
    expect(recorder.calls("drawImage").map(({ args }) => args.slice(1))).toEqual([
      [0, 0, 600, 600],
    ]);
    expect(recorder.calls("putImageData")).toEqual([]);
    expect(socket.requestsOfKind("density_map")).toHaveLength(1);
  });

  it("keeps the picture, stale, while a map of the new resolution is on its way", async () => {
    const { socket, rerender } = renderView({ pictureWidthPx: 300 });
    await answerFaceOn(socket);

    // 300 device pixels take the 256-pixel map, 400 the 512-pixel one.
    rerender({ pictureWidthPx: 400 });

    expect(socket.requestsOfKind("density_map").map(({ body }) => body.resolution)).toEqual([
      256, 512,
    ]);
    expect(
      screen.getByRole("application", { name: "Galaxy map, face-on, stale" }),
    ).toBeInTheDocument();
    expect(within(faceOn()).getByRole("status")).toHaveTextContent("PENDING");
  });

  it("draws the stale picture at the new size", async () => {
    const { socket, recorder, rerender } = renderView({ pictureWidthPx: 300 });
    await answerFaceOn(socket);
    recorder.clear();

    rerender({ pictureWidthPx: 400 });

    expect(recorder.calls("drawImage").map(({ args }) => args.slice(1))).toEqual([
      [0, 0, 400, 400],
    ]);
  });

  it("paints the stale picture up to --text-muted, not --text", async () => {
    const { socket, recorder, rerender } = renderView({ pictureWidthPx: 300 });
    await answerBrightFaceOn(socket);
    recorder.clear();

    rerender({ pictureWidthPx: 400 });

    // The ceiling's pixel is --text-muted, #8a9db3, where a current picture has --text, #c8d6e5.
    const pixels = putPixels(recorder);
    const reds = Array.from({ length: 64 }, (_, pixel) => pixels[pixel * 4] ?? 0);
    expect(Math.max(...reds)).toBe(0x8a);
  });

  it("marks the stale picture with a trailing S, and its density reading too", async () => {
    const { socket, rerender } = renderView({ pictureWidthPx: 300 });
    await answerFaceOn(socket);

    rerender({ pictureWidthPx: 400 });

    const view = faceOn();
    expect(within(view).getAllByText("S")).toHaveLength(2);
    expect(within(view).getByText("CURSOR DENSITY").parentElement).toHaveTextContent(/S\s*stale$/);
  });

  it("replaces the stale picture when the new map arrives", async () => {
    const { socket, recorder, rerender } = renderView({ pictureWidthPx: 300 });
    await answerBrightFaceOn(socket);
    rerender({ pictureWidthPx: 400 });
    recorder.clear();

    await answerBrightFaceOn(socket);

    expect(screen.getByRole("application", { name: "Galaxy map, face-on" })).toBeInTheDocument();
    expect(within(faceOn()).queryByText("S")).not.toBeInTheDocument();
    expect(within(faceOn()).queryByRole("status")).not.toBeInTheDocument();
    const reds = Array.from({ length: 64 }, (_, pixel) => putPixels(recorder)[pixel * 4] ?? 0);
    expect(Math.max(...reds)).toBe(0xc8);
  });

  it("keeps the stale picture beside a failure of the new map, with RETRY", async () => {
    const { socket, rerender } = renderView({ pictureWidthPx: 300 });
    await answerFaceOn(socket);
    rerender({ pictureWidthPx: 400 });
    const [, second] = socket.requestsOfKind("density_map");

    await server(() => {
      socket.serverRejects(second?.id ?? -1, {
        code: "queue_full",
        message: "the bulk queue is full",
        field: null,
      });
    });

    expect(
      screen.getByRole("application", { name: "Galaxy map, face-on, stale" }),
    ).toBeInTheDocument();
    expect(within(faceOn()).getByRole("status")).toHaveTextContent(
      "REJECTED: the bulk queue is full",
    );
    expect(within(faceOn()).getByRole("button", { name: "RETRY" })).toBeInTheDocument();
  });

  it("keeps no picture of another population, which is other data", async () => {
    const { socket, rerender } = renderView();
    await answerFaceOn(socket);

    rerender({ population: "young" });

    expect(
      screen.queryByRole("application", { name: /Galaxy map, face-on/ }),
    ).not.toBeInTheDocument();
    expect(within(faceOn()).getByRole("status")).toHaveTextContent("PENDING");
  });

  it("cancels the request in flight and asks again when the population changes", () => {
    const { socket, rerender } = renderView();
    const [first] = socket.requestsOfKind("density_map");

    rerender({ population: "young" });

    expect(socket.cancelledIds()).toEqual([first?.id]);
    expect(socket.requestsOfKind("density_map").map(({ body }) => body.population)).toEqual([
      "all",
      "young",
    ]);
    expect(within(faceOn()).getByRole("status")).toHaveTextContent("PENDING");
  });

  it("reads MAP DATA INVALID when the map's data is cut short", async () => {
    const { socket } = renderView();

    await server(() => {
      socket.serverAnswers("density_map", () => ({
        ...aDensityMap({ codes: FACE_ON_CODES, widthPx: 8, heightPx: 8 }),
        data_base64: btoa("\u0001\u0002"),
      }));
    });

    expect(within(faceOn()).getByRole("status")).toHaveTextContent(
      "MAP DATA INVALID: pixel codes unreadable",
    );
    expect(
      screen.queryByRole("application", { name: "Galaxy map, face-on" }),
    ).not.toBeInTheDocument();
  });

  it("reads MAP DATA INVALID when the map's density range cannot be drawn", async () => {
    const { socket } = renderView();

    await server(() => {
      socket.serverAnswers("density_map", () =>
        aDensityMap({
          codes: FACE_ON_CODES,
          widthPx: 8,
          heightPx: 8,
          floorLog10PerLy2: 1,
          ceilingLog10PerLy2: -4,
        }),
      );
    });

    expect(within(faceOn()).getByRole("status")).toHaveTextContent(
      "MAP DATA INVALID: density range unusable",
    );
  });

  it("reads MAP DATA INVALID when the map's scale cannot be drawn", async () => {
    const { socket } = renderView();

    await server(() => {
      socket.serverAnswers("density_map", () => ({
        ...aDensityMap({ codes: FACE_ON_CODES, widthPx: 8, heightPx: 8 }),
        ly_per_px: 0,
      }));
    });

    expect(within(faceOn()).getByRole("status")).toHaveTextContent(
      "MAP DATA INVALID: size or scale unusable",
    );
  });

  it("asks again from PENDING when RETRY follows invalid data", async () => {
    const user = userEvent.setup();
    const { socket } = renderView();
    await server(() => {
      socket.serverAnswers("density_map", () => ({
        ...aDensityMap({ codes: FACE_ON_CODES, widthPx: 8, heightPx: 8 }),
        data_base64: "",
      }));
    });

    await user.click(within(faceOn()).getByRole("button", { name: "RETRY" }));

    expect(socket.requestsOfKind("density_map")).toHaveLength(2);
    expect(within(faceOn()).getByRole("status")).toHaveTextContent("PENDING");
    await answerFaceOn(socket);
    expect(screen.getByRole("application", { name: "Galaxy map, face-on" })).toBeInTheDocument();
  });

  it("asks again from PENDING when RETRY follows a rejection", async () => {
    const user = userEvent.setup();
    const { socket } = renderView();
    const [request] = socket.requestsOfKind("density_map");
    await server(() => {
      socket.serverRejects(request?.id ?? -1, {
        code: "queue_full",
        message: "the bulk queue is full",
        field: null,
      });
    });
    expect(within(faceOn()).getByRole("status")).toHaveTextContent(
      "REJECTED: the bulk queue is full",
    );

    await user.click(within(faceOn()).getByRole("button", { name: "RETRY" }));

    expect(socket.requestsOfKind("density_map")).toHaveLength(2);
    expect(within(faceOn()).getByRole("status")).toHaveTextContent("PENDING");
  });

  it("reads MAP DATA INVALID when the map is of another view than asked", async () => {
    const { socket } = renderView();

    await server(() => {
      socket.serverAnswers("density_map", () =>
        aDensityMap({ codes: EDGE_ON_CODES, view: "edge_on" }),
      );
    });

    expect(within(faceOn()).getByRole("status")).toHaveTextContent(
      "MAP DATA INVALID: not the map requested",
    );
  });

  it("waits two minutes for a map before reporting it timed out", () => {
    vi.useFakeTimers();
    const { socket } = renderView();

    act(() => {
      vi.advanceTimersByTime(119_000);
    });
    expect(within(faceOn()).getByRole("status")).toHaveTextContent("PENDING");
    act(() => {
      vi.advanceTimersByTime(1_000);
    });

    expect(within(faceOn()).getByRole("status")).toHaveTextContent("TIMED OUT");
    expect(socket.cancelledIds()).toHaveLength(1);
  });

  it("moves the cursor ten map pixels along +y with Shift and the right arrow face-on", async () => {
    const user = userEvent.setup();
    const onCursor = vi.fn<(cursorLy: CentreLy) => void>();
    const { socket } = renderView({ onCursor });
    await server(() => {
      socket.serverAnswers("density_map", () =>
        aDensityMap({ codes: Array<number>(32 * 32).fill(1), widthPx: 32, heightPx: 32 }),
      );
    });
    screen.getByRole("application", { name: "Galaxy map, face-on" }).focus();

    await user.keyboard("{Shift>}{ArrowRight}{/Shift}");

    // 131,072 ly over 32 pixels is 4,096 ly a pixel; right is +y on the turned picture.
    expect(onCursor).toHaveBeenCalledWith([0, 40_960, 0]);
  });

  it("moves the cursor along +x with the down arrow face-on", async () => {
    const user = userEvent.setup();
    const onCursor = vi.fn<(cursorLy: CentreLy) => void>();
    const { socket } = renderView({ onCursor });
    await answerFaceOn(socket);
    screen.getByRole("application", { name: "Galaxy map, face-on" }).focus();

    await user.keyboard("{ArrowDown}");

    expect(onCursor).toHaveBeenCalledWith([16_384, 0, 0]);
  });

  it("reads no density, and pegs the cursor to the edge, where the cursor is off the map", async () => {
    const { socket } = renderView({ view: "edge_on", cursorLy: [0, 0, 50_000] });

    await server(() => {
      socket.serverAnswers("density_map", () =>
        aDensityMap({ codes: EDGE_ON_CODES, view: "edge_on" }),
      );
    });

    const view = screen.getByRole("region", { name: "EDGE-ON ALONG +Y" });
    expect(within(view).getByText("CURSOR DENSITY").parentElement).toHaveTextContent(
      "CURSOR DENSITY —",
    );
    expect(
      within(view).getByRole("img", { name: "Cursor, off the map above" }),
    ).toBeInTheDocument();
  });

  it("reads the density under the cursor out as it changes, while the picture has focus", async () => {
    const { socket } = renderView();
    await answerFaceOn(socket);

    act(() => {
      screen.getByRole("application", { name: "Galaxy map, face-on" }).focus();
    });

    const reading = within(faceOn()).getByText("CURSOR DENSITY").parentElement;
    expect(reading).toHaveAttribute("aria-live", "polite");
    expect(reading).toHaveAttribute("aria-atomic", "true");
  });

  it("stops announcing the density once the picture loses focus", async () => {
    const user = userEvent.setup();
    const { socket } = renderView();
    await answerFaceOn(socket);
    await user.click(screen.getByRole("application", { name: "Galaxy map, face-on" }));

    // Focus that moved on to the other view would otherwise leave both announcing (ruling 16).
    await user.click(document.body);

    expect(within(faceOn()).getByText("CURSOR DENSITY").parentElement).toHaveAttribute(
      "aria-live",
      "off",
    );
  });

  it("announces no density from a new picture after a focused one was taken away", async () => {
    const { socket, rerender } = renderView();
    await answerFaceOn(socket);
    act(() => {
      screen.getByRole("application", { name: "Galaxy map, face-on" }).focus();
    });

    // Another population is other data, waited for from PENDING: the focused canvas goes, and no
    // blur tells the view so.
    rerender({ population: "young" });
    await answerFaceOn(socket);

    expect(within(faceOn()).getByText("CURSOR DENSITY").parentElement).toHaveAttribute(
      "aria-live",
      "off",
    );
  });

  it("announces no density while the picture has not got focus", async () => {
    const { socket } = renderView();

    await answerFaceOn(socket);

    // The other view's reading follows the same cursor, so a view that is not being moved over
    // would announce a second time for one key press (the orchestrator's ruling 16).
    expect(within(faceOn()).getByText("CURSOR DENSITY").parentElement).toHaveAttribute(
      "aria-live",
      "off",
    );
  });

  it("marks the chart's centre with a mark of its own, apart from the cursor", async () => {
    const { socket } = renderView({ cursorLy: [0, 0, 0], centreLy: [32_768, -32_768, 0] });

    await answerFaceOn(socket);

    // y = -32,768 ly a quarter of the way across; x = 32,768 ly three quarters of the way down.
    expect(within(faceOn()).getByRole("img", { name: "Chart centre" })).toHaveStyle({
      left: "25%",
      top: "75%",
    });
    expect(within(faceOn()).getByRole("img", { name: "Cursor" })).toHaveStyle({
      left: "50%",
      top: "50%",
    });
  });

  it("draws the chart's centre as a small diagonal cross, not the cursor's upright one", async () => {
    const { socket } = renderView({ cursorLy: [0, 0, 0], centreLy: [0, 0, 0] });

    await answerFaceOn(socket);

    // Every stroke of the centre's cross runs corner to corner; the cursor's run across and down.
    const centre = strokes(within(faceOn()).getByRole("img", { name: "Chart centre" }));
    const cursor = strokes(within(faceOn()).getByRole("img", { name: "Cursor" }));
    expect(centre).toHaveLength(2);
    expect(centre.every(([dx, dy]) => Math.abs(dx) === Math.abs(dy) && dx !== 0)).toBe(true);
    expect(cursor.every(([dx, dy]) => dx === 0 || dy === 0)).toBe(true);
  });

  it("marks no chart centre before one is chosen", async () => {
    const { socket } = renderView();

    await answerFaceOn(socket);

    expect(within(faceOn()).queryByRole("img", { name: "Chart centre" })).not.toBeInTheDocument();
  });
});
