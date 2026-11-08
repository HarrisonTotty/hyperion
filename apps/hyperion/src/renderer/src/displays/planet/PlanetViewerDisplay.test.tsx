/**
 * The `PLANET VIEWER` display (plan R15): its inputs, its commands closed-loop over the link, and
 * its planet opened in `VIEW`'s display of its own.
 */
import type { ResponseFor } from "@hyperion/protocol";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Activity } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { UniverseProvider } from "../../components/UniverseProvider";
import {
  GraphicsStatusContext,
  GraphicsStatusStore,
  initialGraphicsStatus,
} from "../../view/engine/status";
import { requestAdapterOutcome } from "../../view/engine/platform";
import { UniversePanel } from "../galaxy/UniversePanel";
import { fakeFramesAndTimeouts } from "../../test/fakeFramesAndTimeouts";
import { FakeGpu } from "../../test/fakeGpu";
import { FakeResizeObserver } from "../../test/FakeResizeObserver";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import {
  anOpenedUniverse,
  aSystemRecord,
  aUniverse,
  aUniverseList,
} from "../../test/galaxyFixtures";
import {
  FIXTURE_MOON,
  populatedMoonInspection,
  sliceEarthInspection,
} from "../../test/inspectionFixture";
import {
  FIXTURE_EARTH,
  FIXTURE_SYSTEM,
  populatedBodies,
  sliceBodies,
} from "../../test/planetaryFixture";
import { REQUEST_TIMEOUT_MS } from "../../lib/useServerRequest";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { stubViewLayout } from "../../test/viewDisplayHarness";
import { SHARE_HELD_NOTE, SHARE_NOT_COPIED } from "./PlanetViewerBar";
import { PlanetViewerDisplay } from "./PlanetViewerDisplay";
import { nextDrawSeed, type PlanetViewerOpening } from "./usePlanetViewer";
import { formatInputsString, DEFAULT_INPUTS, toInputsDto } from "./planetInputs";

afterEach(() => {
  vi.useRealTimers();
});

/** A status store whose adapter has answered none: `VIEW` asks for no engine. */
async function noAdapterStore(): Promise<GraphicsStatusStore> {
  const store = new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));
  const outcome = await requestAdapterOutcome(new FakeGpu([]));
  store.dispatch({ kind: "adapter-outcome", outcome });
  return store;
}

/**
 * The `INPUTS` column's height, CSS px, as the client lays it out at 100% (measured hidden, R15.T6):
 * 959 in a 1920 × 1080 window, the full layout's, and 599 in a 1280 × 720 one, the floor's.
 */
const COLUMN_HEIGHT_PX = { window1920x1080: 959, window1280x720: 599 } as const;

/**
 * Lays the `INPUTS` column out `heightPx()` tall, as `heightPx` gives it at each measure, over
 * {@link stubViewLayout}'s stubs of every other element.
 */
function stubInputsColumn(heightPx: () => number): void {
  const spy = vi.spyOn(HTMLElement.prototype, "getBoundingClientRect");
  const laidOut = spy.getMockImplementation();
  spy.mockImplementation(function column(this: HTMLElement) {
    if (this.classList.contains("planet-viewer__inputs")) {
      return DOMRect.fromRect({ x: 0, y: 0, width: 416, height: heightPx() });
    }
    if (laidOut === undefined) {
      throw new Error("stubInputsColumn runs after stubViewLayout, whose stubs it wraps");
    }
    return laidOut.call(this);
  });
}

/**
 * Renders the display in its providers, a universe's panel beside it to open one, and a way to hand
 * it a planet over, as `App` does, its `INPUTS` column `columnHeightPx()` tall, 1920 × 1080's by
 * default.
 */
async function renderViewer(
  columnHeightPx: () => number = () => COLUMN_HEIGHT_PX.window1920x1080,
): Promise<{
  readonly socket: FakeWebSocket;
  readonly user: ReturnType<typeof userEvent.setup>;
  readonly handOver: (opening: PlanetViewerOpening) => void;
  /** Hides the display, as `App`'s `Activity` does when another is chosen, or shows it again. */
  readonly show: (shown: boolean) => void;
}> {
  const advanceTimers = fakeFramesAndTimeouts();
  stubViewLayout({ widthPx: 1280, heightPx: 720 }, () => ({ widthPx: 900, heightPx: 600 }));
  stubInputsColumn(columnHeightPx);
  vi.stubGlobal("WebSocket", FakeWebSocket);
  const user = userEvent.setup({ advanceTimers });
  const store = await noAdapterStore();
  let handedOver: PlanetViewerOpening | null = null;
  const tree = (opening: PlanetViewerOpening | null, shown = true) => (
    <ServerLinkHarness>
      <UniverseProvider>
        <GraphicsStatusContext value={store}>
          <UniversePanel expanded onToggle={() => undefined} />
          <Activity mode={shown ? "visible" : "hidden"}>
            <main className="console__work console__work--planet">
              <PlanetViewerDisplay
                setting="high"
                framePacing="display"
                platform="linux"
                performanceThreads={null}
                geometry={undefined}
                atmosphereLevers={undefined}
                opening={opening}
              />
            </main>
          </Activity>
        </GraphicsStatusContext>
      </UniverseProvider>
    </ServerLinkHarness>
  );
  const { rerender } = render(tree(null));
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  return {
    socket,
    user,
    handOver: (opening) => {
      handedOver = opening;
      rerender(tree(opening));
    },
    show: (shown) => {
      rerender(tree(handedOver, shown));
    },
  };
}

/** Opens the fixture's universe on the universe panel. */
async function openUniverse(
  socket: FakeWebSocket,
  user: ReturnType<typeof userEvent.setup>,
): Promise<void> {
  await act(async () => {
    socket.serverAnswers("list_universes", () => aUniverseList());
    await vi.advanceTimersByTimeAsync(0);
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(300);
  });
  await user.click(screen.getByRole("button", { name: "Open universe SURVEY 1" }));
  await act(async () => {
    socket.serverAnswers("open_universe", () => anOpenedUniverse());
    await vi.advanceTimersByTimeAsync(0);
  });
}

/** The server's planet: the slice's Earth, under the slice's system's ID. */
function planetAnswer(inputs = toInputsDto(DEFAULT_INPUTS)): ResponseFor<"viewer_planet"> {
  return {
    kind: "viewer_planet",
    universe: aUniverse().id,
    system: FIXTURE_SYSTEM,
    body: FIXTURE_EARTH,
    barycentre: { cell_ly: [0, 26_000, 0], offset_m: [0, 0, 0] },
    inputs,
    drawn: {
      formed_at_au: 1,
      day_hours: 23.9,
      tilt_deg: 22.5,
      water_rank: 0.41,
      carbon_rank: 0.52,
      nitrogen_rank: 0.63,
    },
  };
}

/** Every element whose whole text is `text`, as `StatusText` splits a line into its parts. */
function wholeText(text: string): HTMLElement[] {
  return screen.queryAllByText((_, element) => {
    const own = element?.textContent ?? "";
    return (
      own === text && [...(element?.children ?? [])].every((child) => child.textContent !== text)
    );
  });
}

/** Answers the planet's `viewer_planet` with `inputs`, then its system's bodies. */
async function showPlanet(
  socket: FakeWebSocket,
  inputs: ReturnType<typeof toInputsDto>,
): Promise<void> {
  await act(async () => {
    socket.serverAnswers("viewer_planet", () => planetAnswer(inputs));
    await vi.advanceTimersByTimeAsync(0);
  });
  await act(async () => {
    socket.serverAnswers("system_bodies", () => sliceBodies());
    await vi.advanceTimersByTimeAsync(0);
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
}

/** The slice's Earth's inputs read back, its orbit at 1.5 au, its system of one star or more. */
function readAnswer(multipleStars = false): ResponseFor<"viewer_inputs"> {
  return {
    kind: "viewer_inputs",
    universe: aUniverse().id,
    body: FIXTURE_EARTH,
    inputs: { ...toInputsDto(DEFAULT_INPUTS), semi_major_axis_au: 1.5 },
    multiple_stars: multipleStars,
  };
}

/** Opens the panel's `SHOWN`, folded as the display mounts, unless it is open. */
async function openShown(user: ReturnType<typeof userEvent.setup>): Promise<void> {
  const shown = screen.getByRole("button", { name: "SHOWN" });
  if (shown.getAttribute("aria-expanded") !== "true") {
    await user.click(shown);
  }
}

/** Types `id` into `BODY`, under `SHOWN`, and presses `OPEN BODY`. */
async function openTyped(user: ReturnType<typeof userEvent.setup>, id: string): Promise<void> {
  await openShown(user);
  const field = screen.getByRole("textbox", { name: "BODY" });
  await user.clear(field);
  await user.type(field, id);
  await user.click(screen.getByRole("button", { name: "OPEN BODY" }));
}

/** Answers an `OPEN BODY` of the slice's Earth: its inputs, its system's record, its bodies. */
async function answerOpen(socket: FakeWebSocket, multipleStars = false): Promise<void> {
  await act(async () => {
    socket.serverAnswers("viewer_inputs", () => readAnswer(multipleStars));
    await vi.advanceTimersByTimeAsync(0);
  });
  await act(async () => {
    socket.serverAnswers("resolve_system", () => ({
      kind: "resolve_system",
      universe: aUniverse().id,
      time: { seconds: 0, nanos: 0 },
      record: { ...aSystemRecord({ positionLy: [26_000, 0, 12] }), id: FIXTURE_SYSTEM },
    }));
    await vi.advanceTimersByTimeAsync(0);
  });
  await act(async () => {
    socket.serverAnswers("system_bodies", () => sliceBodies());
    await vi.advanceTimersByTimeAsync(0);
  });
  await settle();
}

/**
 * `NO INPUTS`' line as a description, its state and sentence in their own parts, which jsdom's
 * accessible name joins with no space between.
 */
const NO_INPUTS_DESCRIPTION = /^NO INPUTS: ?a moon has none, enter one to GENERATE or SHARE$/u;

/** Refuses the pending `viewer_inputs` as the server refuses a body that is no planet. */
async function refuseAsNoPlanet(socket: FakeWebSocket): Promise<void> {
  const [asked] = socket.requestsOfKind("viewer_inputs");
  await act(async () => {
    if (asked !== undefined) {
      socket.serverRejects(asked.id, {
        code: "bad_request",
        message: "body is no planet: the viewer reads a planet's inputs",
        field: "body",
      });
    }
    await vi.advanceTimersByTimeAsync(0);
  });
}

/** Answers an `OPEN BODY`'s system's record, then its bodies with `bodies`. */
async function answerSystem(
  socket: FakeWebSocket,
  bodies: ResponseFor<"system_bodies">,
): Promise<void> {
  await act(async () => {
    socket.serverAnswers("resolve_system", () => ({
      kind: "resolve_system",
      universe: aUniverse().id,
      time: { seconds: 0, nanos: 0 },
      record: { ...aSystemRecord({ positionLy: [26_000, 0, 12] }), id: FIXTURE_SYSTEM },
    }));
    await vi.advanceTimersByTimeAsync(0);
  });
  await act(async () => {
    socket.serverAnswers("system_bodies", () => bodies);
    await vi.advanceTimersByTimeAsync(0);
  });
  await settle();
}

/** Lets the requests' promises settle. */
async function settle(): Promise<void> {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(0);
  });
}

describe("PlanetViewerDisplay", () => {
  it("holds its commands back, saying why, until a universe is open", async () => {
    const { socket, user } = await renderViewer();
    expect(
      wholeText("NOT AVAILABLE: no universe open, open one on the GALAXY display").length,
    ).toBeGreaterThan(0);
    await user.click(screen.getByRole("button", { name: "G GENERATE" }));
    expect(socket.requestsOfKind("viewer_planet")).toHaveLength(0);
    expect(screen.getByText(/NO PLANET/u)).toBeTruthy();
  });

  it("generates the panel's inputs, reads the planet's system, and shows it in VIEW", async () => {
    const { socket, user } = await renderViewer();
    await openUniverse(socket, user);
    await user.click(screen.getByRole("button", { name: "G GENERATE" }));
    const [asked] = socket.requestsOfKind("viewer_planet");
    expect(asked?.body.inputs).toEqual(toInputsDto(DEFAULT_INPUTS));
    expect(screen.getByText(/GENERATE PENDING/u)).toBeTruthy();
    await act(async () => {
      socket.serverAnswers("viewer_planet", () => planetAnswer());
      await vi.advanceTimersByTimeAsync(0);
    });
    const [bodies] = socket.requestsOfKind("system_bodies");
    expect(bodies?.body.system).toBe(FIXTURE_SYSTEM);
    await act(async () => {
      socket.serverAnswers("system_bodies", () => sliceBodies());
      await vi.advanceTimersByTimeAsync(0);
    });
    await settle();
    expect(screen.queryByText(/NO PLANET/u)).toBeNull();
    expect(screen.queryByText(/GENERATE PENDING/u)).toBeNull();
    // Its drawn rotation stands in the field, in days, while the panel holds its inputs.
    expect(screen.getByRole("textbox", { name: "ROTATION" })).toHaveProperty("value", "0.9958");
    // The planet's whole body is surveyed for training as it opens.
    expect(socket.requestsOfKind("survey_pass").at(-1)?.body.scope).toBe("training");
  });

  it("asks a generated planet's sky with no system left out, the one every generated planet shares (plan R15)", async () => {
    const { socket, user } = await renderViewer();
    await openUniverse(socket, user);
    await user.click(screen.getByRole("button", { name: "G GENERATE" }));
    await showPlanet(socket, toInputsDto(DEFAULT_INPUTS));
    const skies = socket.requestsOfKind("sky").map((request) => request.body.exclude_system);
    expect([skies.length > 0, new Set(skies)]).toEqual([true, new Set([null])]);
  });

  it("puts a refusal the server gives under the field of the input it names", async () => {
    const { socket, user } = await renderViewer();
    await openUniverse(socket, user);
    await user.click(screen.getByRole("button", { name: "G GENERATE" }));
    const [asked] = socket.requestsOfKind("viewer_planet");
    await act(async () => {
      if (asked !== undefined) {
        socket.serverRejects(asked.id, {
          code: "bad_request",
          message: "the orbit's periapsis lies inside its star",
          field: "inputs.semi_major_axis_au",
        });
      }
      await vi.advanceTimersByTimeAsync(0);
    });
    const sma = screen.getByRole("textbox", { name: "SMA" });
    expect(sma.getAttribute("aria-invalid")).toBe("true");
    expect(
      screen.getAllByText(/the orbit's periapsis lies inside its star/u).length,
    ).toBeGreaterThan(0);
  });

  it("refuses an entry out of range with what is valid, and keeps the input", async () => {
    const { user } = await renderViewer();
    const mass = screen.getByRole("textbox", { name: "INIT MASS" });
    await user.clear(mass);
    await user.type(mass, "200{Enter}");
    expect(wholeText("INIT MASS INVALID: enter 0.08 to 150")).toHaveLength(1);
    await user.type(mass, "{Escape}");
    expect(mass).toHaveProperty("value", "1");
  });

  it("draws with DRAW SEED, puts the drawn inputs on the panel, and moves the seed on once shown", async () => {
    const { socket, user } = await renderViewer();
    await openUniverse(socket, user);
    await user.click(screen.getByRole("button", { name: "N RANDOMIZE" }));
    const [draw] = socket.requestsOfKind("viewer_randomize");
    expect(draw?.body.draw_seed).toBe("0000000000000001");
    expect(screen.getByText(/RANDOMIZE PENDING/u)).toBeTruthy();
    const drawn = { ...toInputsDto(DEFAULT_INPUTS), star_mass_msun: 0.42, mass_mearth: 3.1 };
    await act(async () => {
      socket.serverAnswers("viewer_randomize", () => ({
        kind: "viewer_randomize",
        universe: aUniverse().id,
        draw_seed: "0000000000000001",
        inputs: drawn,
        source: `${FIXTURE_SYSTEM}.0100`,
      }));
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(socket.requestsOfKind("viewer_planet").at(-1)?.body.inputs).toEqual(drawn);
    expect(screen.getByRole("textbox", { name: "INIT MASS" })).toHaveProperty("value", "0.42");
    // Nothing moves on before the planet is shown.
    expect(screen.getByRole("textbox", { name: "DRAW SEED" })).toHaveProperty(
      "value",
      "0000000000000001",
    );
    await showPlanet(socket, drawn);
    expect(screen.getByRole("textbox", { name: "DRAW SEED" })).toHaveProperty(
      "value",
      "0000000000000002",
    );
    const lastDraw = screen.getByText("LAST DRAW").parentElement;
    expect(within(lastDraw ?? document.body).getByText("0000000000000001")).toBeTruthy();
    expect(screen.queryByText(/RANDOMIZE PENDING/u)).toBeNull();
  });

  it("keeps DRAW SEED and LAST DRAW where a draw is refused", async () => {
    const { socket, user } = await renderViewer();
    await openUniverse(socket, user);
    await user.keyboard("n");
    const [draw] = socket.requestsOfKind("viewer_randomize");
    await act(async () => {
      if (draw !== undefined) {
        socket.serverRejects(draw.id, {
          code: "bad_request",
          message: "no single star with a present planet",
          field: "draw_seed",
        });
      }
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(screen.getByRole("textbox", { name: "DRAW SEED" })).toHaveProperty(
      "value",
      "0000000000000001",
    );
    const lastDraw = screen.getByText("LAST DRAW").parentElement;
    expect(within(lastDraw ?? document.body).getByText("—")).toBeTruthy();
    expect(screen.getAllByText(/no single star with a present planet/u).length).toBeGreaterThan(0);
  });

  it("shows GENERATE TIMED OUT when the server never answers", async () => {
    const { socket, user } = await renderViewer();
    await openUniverse(socket, user);
    await user.click(screen.getByRole("button", { name: "G GENERATE" }));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(REQUEST_TIMEOUT_MS);
    });
    expect(screen.getByText(/GENERATE TIMED OUT/u)).toBeTruthy();
    expect(socket.cancelledIds()).toContain(socket.requestsOfKind("viewer_planet")[0]?.id);
  });

  it("cancels a command in flight for a newer one, whose planet alone is shown", async () => {
    const { socket, user } = await renderViewer();
    await openUniverse(socket, user);
    await user.click(screen.getByRole("button", { name: "G GENERATE" }));
    await user.click(screen.getByRole("button", { name: "G GENERATE" }));
    const [first, second] = socket.requestsOfKind("viewer_planet");
    expect(socket.cancelledIds()).toEqual([first?.id]);
    expect(second).toBeDefined();
    await showPlanet(socket, toInputsDto(DEFAULT_INPUTS));
    expect(screen.queryByText(/NO PLANET/u)).toBeNull();
  });

  it("leaves a given input to its draw on DRAW, out of the string", async () => {
    const { user } = await renderViewer();
    const tilt = screen.getByRole("textbox", { name: "OBLIQUITY" });
    await user.clear(tilt);
    await user.type(tilt, "45{Enter}");
    const share = screen.getByRole("textbox", { name: "SHARE" });
    expect(share.getAttribute("value")).toContain("tilt_deg=45");
    await user.click(screen.getByRole("button", { name: "DRAW OBLIQUITY" }));
    expect(share.getAttribute("value")).not.toContain("tilt_deg");
    expect(tilt).toHaveProperty("value", "—");
  });

  it("opens its planet in VIEW's ORBIT camera", async () => {
    const { socket, user } = await renderViewer();
    await openUniverse(socket, user);
    await user.click(screen.getByRole("button", { name: "G GENERATE" }));
    await showPlanet(socket, toInputsDto(DEFAULT_INPUTS));
    expect(screen.getByRole("button", { name: "0 ORBIT" })).toHaveAttribute("aria-pressed", "true");
  });

  it("leaves the sun where its orbit puts it as the display mounts", async () => {
    await renderViewer();
    expect([
      screen.getByRole("button", { name: "ORBITAL" }).getAttribute("aria-pressed"),
      screen.queryByRole("textbox", { name: "LATITUDE" }),
    ]).toEqual(["true", null]);
  });

  it("holds the sun over the prime meridian on the equator once HELD is pressed", async () => {
    const { user } = await renderViewer();
    await user.click(screen.getByRole("button", { name: "HELD" }));
    expect(screen.getByRole("textbox", { name: "LATITUDE" })).toHaveProperty("value", "0.0");
  });

  it("refuses a latitude outside -90 to 90 with what is valid", async () => {
    const { user } = await renderViewer();
    await user.click(screen.getByRole("button", { name: "HELD" }));
    const latitude = screen.getByRole("textbox", { name: "LATITUDE" });
    await user.clear(latitude);
    await user.type(latitude, "95{Enter}");
    expect(wholeText("LATITUDE INVALID: enter -90.0 to 90.0")).toHaveLength(1);
  });

  it("takes a latitude in range and shows it to a tenth", async () => {
    const { user } = await renderViewer();
    await user.click(screen.getByRole("button", { name: "HELD" }));
    const latitude = screen.getByRole("textbox", { name: "LATITUDE" });
    await user.clear(latitude);
    await user.type(latitude, "30{Enter}");
    expect(latitude).toHaveProperty("value", "30.0");
  });

  it("gives the sun back to its orbit under ORBITAL", async () => {
    const { user } = await renderViewer();
    await user.click(screen.getByRole("button", { name: "HELD" }));
    await user.click(screen.getByRole("button", { name: "ORBITAL" }));
    expect(screen.queryByRole("textbox", { name: "LATITUDE" })).toBeNull();
  });

  it("offers VIEW no SCENE selector: its one scene is the planet", async () => {
    const { socket, user } = await renderViewer();
    await openUniverse(socket, user);
    await user.click(screen.getByRole("button", { name: "G GENERATE" }));
    await showPlanet(socket, toInputsDto(DEFAULT_INPUTS));
    expect(screen.queryByRole("group", { name: "Scene" })).toBeNull();
  });

  it("loads a pasted string's inputs on the panel", async () => {
    const { user } = await renderViewer();
    const share = screen.getByRole("textbox", { name: "SHARE" });
    const pasted = formatInputsString({
      ...DEFAULT_INPUTS,
      values: { ...DEFAULT_INPUTS.values, tiltDeg: 60 },
    });
    await user.clear(share);
    await user.click(share);
    await user.paste(pasted);
    await user.click(screen.getByRole("button", { name: "LOAD" }));
    expect(screen.getByRole("textbox", { name: "OBLIQUITY" })).toHaveProperty("value", "60");
    expect(share).toHaveProperty("value", pasted);
  });

  it("generates on G from anywhere but a text field", async () => {
    const { socket, user } = await renderViewer();
    await openUniverse(socket, user);
    await user.keyboard("g");
    expect(socket.requestsOfKind("viewer_planet")).toHaveLength(1);
    await user.click(screen.getByRole("textbox", { name: "SMA" }));
    await user.keyboard("g");
    expect(socket.requestsOfKind("viewer_planet")).toHaveLength(1);
  });
});

describe("nextDrawSeed", () => {
  it("moves a seed on by one, wrapping at 2^64", () => {
    expect(nextDrawSeed("00000000000000ff")).toBe("0000000000000100");
    expect(nextDrawSeed("ffffffffffffffff")).toBe("0000000000000000");
  });

  describe("a planet of the galaxy (plan R15, T5)", () => {
    it("is asked by the ID typed in BODY, in the wire's lower case", async () => {
      const { socket, user } = await renderViewer();
      await openUniverse(socket, user);
      await openTyped(user, FIXTURE_EARTH.toUpperCase());
      expect(socket.requestsOfKind("viewer_inputs").map(({ body }) => body.body)).toEqual([
        FIXTURE_EARTH,
      ]);
    });

    it("is shown with its inputs read back onto the panel", async () => {
      const { socket, user } = await renderViewer();
      await openUniverse(socket, user);
      await openTyped(user, FIXTURE_EARTH);
      await answerOpen(socket);
      expect([
        screen.queryByText(/NO PLANET/u),
        screen.getByRole("textbox", { name: "SMA" }).getAttribute("value"),
      ]).toEqual([null, "1.5"]);
    });

    it("is refused an ID of another form with what is valid, asking nothing", async () => {
      const { socket, user } = await renderViewer();
      await openUniverse(socket, user);
      await openTyped(user, "0200.03");
      expect([
        wholeText("BODY INVALID: enter 16 hexadecimal digits, a full stop and 4 more (0-9, A-F)")
          .length,
        socket.requestsOfKind("viewer_inputs").length,
      ]).toEqual([1, 0]);
    });

    it("states the server's refusal of a body that is no planet, once its system says it is no moon", async () => {
      const { socket, user } = await renderViewer();
      await openUniverse(socket, user);
      await openTyped(user, FIXTURE_EARTH);
      await refuseAsNoPlanet(socket);
      await answerSystem(socket, sliceBodies());
      expect(
        wholeText("OPEN BODY REJECTED: body is no planet: the viewer reads a planet's inputs"),
      ).toHaveLength(1);
    });

    it("notes that its inputs hold one host of a system of more stars", async () => {
      const { socket, user } = await renderViewer();
      await openUniverse(socket, user);
      await openTyped(user, FIXTURE_EARTH);
      await answerOpen(socket, true);
      expect(wholeText("MULTIPLE STARS: the inputs hold its host alone")).toHaveLength(1);
    });

    it("is opened as OPEN IN PLANET VIEWER hands it over, its system already read", async () => {
      const { socket, handOver } = await renderViewer();
      handOver({ inspection: sliceEarthInspection(), sequence: 1 });
      await act(async () => {
        socket.serverAnswers("viewer_inputs", () => readAnswer());
        await vi.advanceTimersByTimeAsync(0);
      });
      await settle();
      expect([
        screen.queryByText(/NO PLANET/u),
        screen.getByRole("textbox", { name: "BODY" }).getAttribute("value"),
        socket.requestsOfKind("resolve_system").length,
      ]).toEqual([null, FIXTURE_EARTH.toUpperCase(), 0]);
    });

    it("is opened again when the display is shown, where hiding it cut the hand-over short", async () => {
      const { socket, handOver, show } = await renderViewer();
      handOver({ inspection: sliceEarthInspection(), sequence: 1 });
      show(false);
      show(true);
      await settle();
      expect(socket.requestsOfKind("viewer_inputs")).toHaveLength(2);
    });

    it("is not opened again where a newer command cut the hand-over short", async () => {
      const { socket, user, handOver, show } = await renderViewer();
      await openUniverse(socket, user);
      handOver({ inspection: sliceEarthInspection(), sequence: 1 });
      await user.click(screen.getByRole("button", { name: "G GENERATE" }));
      show(false);
      show(true);
      await settle();
      expect(socket.requestsOfKind("viewer_inputs")).toHaveLength(1);
    });

    it("is not asked again when the display is shown again once its planet is shown", async () => {
      const { socket, handOver, show } = await renderViewer();
      handOver({ inspection: sliceEarthInspection(), sequence: 1 });
      await act(async () => {
        socket.serverAnswers("viewer_inputs", () => readAnswer());
        await vi.advanceTimersByTimeAsync(0);
      });
      await settle();
      show(false);
      show(true);
      await settle();
      expect(socket.requestsOfKind("viewer_inputs")).toHaveLength(1);
    });
  });

  describe("a moon of the galaxy (R15.T5's follow-up)", () => {
    /** Hands the populated answer's moon over, as `OPEN IN PLANET VIEWER` does. */
    async function handOverMoon() {
      const rendered = await renderViewer();
      await openUniverse(rendered.socket, rendered.user);
      rendered.handOver({ inspection: populatedMoonInspection(), sequence: 1 });
      await settle();
      return rendered;
    }

    it("is opened as OPEN IN PLANET VIEWER hands it over, asking no inputs of it", async () => {
      const { socket, user } = await handOverMoon();
      await openShown(user);
      expect([
        screen.queryByText(/NO PLANET/u),
        screen.getByRole("textbox", { name: "BODY" }).getAttribute("value"),
        socket.requestsOfKind("viewer_inputs").length,
      ]).toEqual([null, FIXTURE_MOON.toUpperCase(), 0]);
    });

    it("says, folded, that the moon is shown as a body of the galaxy", async () => {
      const { user } = await handOverMoon();
      await user.click(screen.getByRole("button", { name: "SHOWN" }));
      expect(wholeText(`BODY ${FIXTURE_MOON.toUpperCase()}`)).toHaveLength(1);
    });

    it("is opened by the ID typed in BODY, the server's refusal of its inputs read as none", async () => {
      const { socket, user } = await renderViewer();
      await openUniverse(socket, user);
      await openTyped(user, FIXTURE_MOON);
      await refuseAsNoPlanet(socket);
      await answerSystem(socket, populatedBodies());
      expect([
        screen.queryByText(/NO PLANET/u),
        screen.queryByText(/OPEN BODY REJECTED/u),
        wholeText("NO INPUTS: a moon has none, enter one to GENERATE or SHARE").length,
      ]).toEqual([null, null, 1]);
    });

    it("keeps the panel's last inputs", async () => {
      await handOverMoon();
      expect(screen.getByRole("textbox", { name: "SMA" })).toHaveProperty("value", "1");
    });

    it("describes each muted field by its NO INPUTS line, and none once an input is entered", async () => {
      const { user } = await handOverMoon();
      const sma = screen.getByRole("textbox", { name: "SMA" });
      expect(sma).toHaveAccessibleDescription(/^NO INPUTS: ?a moon has none/u);
      await user.clear(sma);
      await user.type(sma, "2{Enter}");
      expect(sma).not.toHaveAccessibleDescription(/NO INPUTS/u);
    });

    it("holds GENERATE back, saying why", async () => {
      const { socket, user } = await handOverMoon();
      const generate = screen.getByRole("button", { name: "G GENERATE" });
      await user.click(generate);
      await user.keyboard("g");
      expect([
        generate.getAttribute("aria-disabled"),
        socket.requestsOfKind("viewer_planet").length,
      ]).toEqual(["true", 0]);
      expect(generate).toHaveAccessibleDescription(NO_INPUTS_DESCRIPTION);
    });

    it("holds SHARE's string back, saying so under it and on a refused copy or cut, and offers LOAD", async () => {
      const { user } = await handOverMoon();
      const share = screen.getByRole("textbox", { name: "SHARE" });
      expect([
        screen.getByRole("button", { name: "LOAD" }).getAttribute("aria-disabled"),
        wholeText(SHARE_HELD_NOTE).length,
      ]).toEqual([null, 1]);
      expect(share).toHaveAccessibleDescription(
        /this string is another planet's, paste one to LOAD$/u,
      );
      const shown = share.getAttribute("value");
      await navigator.clipboard.writeText("before");
      await user.click(share);
      await user.copy();
      await user.cut();
      expect([
        await navigator.clipboard.readText(),
        share.getAttribute("value"),
        wholeText(SHARE_NOT_COPIED).length,
      ]).toEqual(["before", shown, 1]);
    });

    it("still offers RANDOMIZE", async () => {
      const { socket, user } = await handOverMoon();
      const randomize = screen.getByRole("button", { name: "N RANDOMIZE" });
      await user.click(randomize);
      expect([
        randomize.getAttribute("aria-disabled"),
        socket.requestsOfKind("viewer_randomize").length,
      ]).toEqual([null, 1]);
    });

    it("offers GENERATE again once an input is entered", async () => {
      const { socket, user } = await handOverMoon();
      const sma = screen.getByRole("textbox", { name: "SMA" });
      await user.clear(sma);
      await user.type(sma, "2{Enter}");
      await user.click(screen.getByRole("button", { name: "G GENERATE" }));
      expect([
        screen.queryByText(/NO INPUTS/u),
        socket.requestsOfKind("viewer_planet").length,
      ]).toEqual([null, 1]);
    });

    it("gives a planet opened after it its inputs again", async () => {
      const { socket, user, handOver } = await handOverMoon();
      handOver({ inspection: sliceEarthInspection(), sequence: 2 });
      await act(async () => {
        socket.serverAnswers("viewer_inputs", () => readAnswer());
        await vi.advanceTimersByTimeAsync(0);
      });
      await settle();
      await user.click(screen.getByRole("button", { name: "G GENERATE" }));
      expect([
        screen.queryByText(/NO INPUTS/u),
        socket.requestsOfKind("viewer_planet").length,
      ]).toEqual([null, 1]);
    });

    it("keeps the panel's inputs and its hold on a LOAD with nothing pasted", async () => {
      const { user } = await handOverMoon();
      await user.click(screen.getByRole("button", { name: "LOAD" }));
      expect([
        screen.getByRole("textbox", { name: "SMA" }),
        wholeText("NO INPUTS: a moon has none, enter one to GENERATE or SHARE").length,
      ]).toEqual([expect.objectContaining({ value: "1" }), 1]);
    });

    it("loads a pasted string's inputs, which give the panel its inputs again (the guide's Controls)", async () => {
      const { socket, user } = await handOverMoon();
      const share = screen.getByRole("textbox", { name: "SHARE" });
      const pasted = formatInputsString({
        ...DEFAULT_INPUTS,
        values: { ...DEFAULT_INPUTS.values, tiltDeg: 60 },
      });
      await user.click(share);
      await user.paste(pasted);
      expect(wholeText(SHARE_HELD_NOTE)).toHaveLength(0);
      await user.click(screen.getByRole("button", { name: "LOAD" }));
      expect([
        screen.getByRole("textbox", { name: "OBLIQUITY" }),
        screen.queryByText(/NO INPUTS/u),
      ]).toEqual([expect.objectContaining({ value: "60" }), null]);
      await user.click(screen.getByRole("button", { name: "G GENERATE" }));
      expect(socket.requestsOfKind("viewer_planet")).toHaveLength(1);
    });

    it("offers SHARE again once an input is entered", async () => {
      const { user } = await handOverMoon();
      const sma = screen.getByRole("textbox", { name: "SMA" });
      await user.clear(sma);
      await user.type(sma, "2{Enter}");
      const share = screen.getByRole("textbox", { name: "SHARE" });
      await user.click(share);
      await user.copy();
      expect([await navigator.clipboard.readText(), wholeText(SHARE_HELD_NOTE).length]).toEqual([
        share.getAttribute("value"),
        0,
      ]);
      expect(share).not.toHaveAccessibleDescription(/NO INPUTS/u);
    });

    it("gives the panel the drawn inputs of a RANDOMIZE whose planet is refused", async () => {
      const { socket, user } = await handOverMoon();
      await user.click(screen.getByRole("button", { name: "N RANDOMIZE" }));
      await act(async () => {
        socket.serverAnswers("viewer_randomize", () => ({
          kind: "viewer_randomize",
          universe: aUniverse().id,
          draw_seed: "0000000000000001",
          inputs: toInputsDto(DEFAULT_INPUTS),
          source: FIXTURE_EARTH,
        }));
        await vi.advanceTimersByTimeAsync(0);
      });
      const [asked] = socket.requestsOfKind("viewer_planet");
      await act(async () => {
        if (asked !== undefined) {
          socket.serverRejects(asked.id, { code: "internal", message: "failed", field: null });
        }
        await vi.advanceTimersByTimeAsync(0);
      });
      expect([
        screen.queryByText(/NO INPUTS/u),
        screen.getByRole("button", { name: "G GENERATE" }).getAttribute("aria-disabled"),
      ]).toEqual([null, null]);
    });

    it("keeps an input entered while it opens, offering GENERATE once it is shown", async () => {
      const { socket, user } = await renderViewer();
      await openUniverse(socket, user);
      await openTyped(user, FIXTURE_MOON);
      await refuseAsNoPlanet(socket);
      const sma = screen.getByRole("textbox", { name: "SMA" });
      await user.clear(sma);
      await user.type(sma, "2{Enter}");
      await answerSystem(socket, populatedBodies());
      expect([
        screen.queryByText(/OPEN BODY PENDING/u),
        screen.queryByText(/NO INPUTS/u),
        screen.getByRole("button", { name: "G GENERATE" }).getAttribute("aria-disabled"),
      ]).toEqual([null, null, null]);
    });

    it("opens in VIEW's ORBIT camera about the moon", async () => {
      await handOverMoon();
      expect(screen.getByRole("button", { name: "0 ORBIT" })).toHaveAttribute(
        "aria-pressed",
        "true",
      );
    });
  });

  describe("BODY's field", () => {
    it("states the form it takes while it is empty", async () => {
      const { user } = await renderViewer();
      await openShown(user);
      expect(screen.getByRole("textbox", { name: "BODY" })).toHaveAccessibleDescription(
        "a body's ID: 16 hexadecimal digits, a full stop and 4 more",
      );
    });

    it("shows the planet on show again under Escape, selected", async () => {
      const { socket, user } = await renderViewer();
      await openUniverse(socket, user);
      await openTyped(user, FIXTURE_EARTH);
      await answerOpen(socket);
      const field = screen.getByRole("textbox", { name: "BODY" });
      await user.clear(field);
      await user.type(field, "02{Escape}");
      expect(field).toHaveProperty("value", FIXTURE_EARTH.toUpperCase());
    });

    it("shows the planet on show again when it is left emptied", async () => {
      const { socket, user } = await renderViewer();
      await openUniverse(socket, user);
      await openTyped(user, FIXTURE_EARTH);
      await answerOpen(socket);
      const field = screen.getByRole("textbox", { name: "BODY" });
      await user.clear(field);
      await user.tab();
      expect(field).toHaveProperty("value", FIXTURE_EARTH.toUpperCase());
    });
  });

  describe("SHOWN", () => {
    it("is folded as the display mounts, saying no planet is shown", async () => {
      await renderViewer();
      expect([
        screen.getByRole("button", { name: "SHOWN" }).getAttribute("aria-expanded"),
        screen.queryByRole("textbox", { name: "BODY" }),
      ]).toEqual(["false", null]);
      expect(screen.getByText("NONE")).toBeInTheDocument();
    });

    it("says a generated planet is shown", async () => {
      const { socket, user } = await renderViewer();
      await openUniverse(socket, user);
      await user.click(screen.getByRole("button", { name: "G GENERATE" }));
      await showPlanet(socket, toInputsDto(DEFAULT_INPUTS));
      expect(screen.getByText("GENERATED")).toBeInTheDocument();
    });

    it("opens as a planet is handed over, so that OPEN BODY's PENDING stands under its field", async () => {
      const { handOver } = await renderViewer();
      handOver({ inspection: sliceEarthInspection(), sequence: 1 });
      await settle();
      expect(wholeText("OPEN BODY PENDING")).toHaveLength(1);
    });

    it("folds under its press while OPEN BODY is pending", async () => {
      const { user, handOver } = await renderViewer();
      handOver({ inspection: sliceEarthInspection(), sequence: 1 });
      await settle();
      await user.click(screen.getByRole("button", { name: "SHOWN" }));
      expect([
        screen.getByRole("button", { name: "SHOWN" }).getAttribute("aria-expanded"),
        screen.queryByRole("textbox", { name: "BODY" }),
      ]).toEqual(["false", null]);
    });

    it("says, folded, which planet of the galaxy is shown", async () => {
      const { socket, user, handOver } = await renderViewer();
      handOver({ inspection: sliceEarthInspection(), sequence: 1 });
      await act(async () => {
        socket.serverAnswers("viewer_inputs", () => readAnswer());
        await vi.advanceTimersByTimeAsync(0);
      });
      await settle();
      await user.click(screen.getByRole("button", { name: "SHOWN" }));
      expect(wholeText(`BODY ${FIXTURE_EARTH.toUpperCase()}`)).toHaveLength(1);
    });
  });

  it("opens the volatile ranks once a refusal names one of them", async () => {
    const { socket, user } = await renderViewer();
    await openUniverse(socket, user);
    await user.click(screen.getByRole("button", { name: "G GENERATE" }));
    const [asked] = socket.requestsOfKind("viewer_planet");
    await act(async () => {
      if (asked !== undefined) {
        socket.serverRejects(asked.id, {
          code: "bad_request",
          message: "the water rank lies outside 0.001 to 0.999",
          field: "inputs.water_rank",
        });
      }
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(screen.getByRole("textbox", { name: "H2O" })).toHaveAttribute("aria-invalid", "true");
  });
});

/** Generates the panel's inputs and has the server refuse them, naming SMA. */
async function rejectSma(
  socket: FakeWebSocket,
  user: ReturnType<typeof userEvent.setup>,
): Promise<void> {
  await user.click(screen.getByRole("button", { name: "G GENERATE" }));
  const [asked] = socket.requestsOfKind("viewer_planet");
  await act(async () => {
    if (asked !== undefined) {
      socket.serverRejects(asked.id, {
        code: "bad_request",
        message: "the orbit's periapsis lies inside its star",
        field: "inputs.semi_major_axis_au",
      });
    }
    await vi.advanceTimersByTimeAsync(0);
  });
}

/** The column of a 1280 × 720 window, the floor's. */
function atFloor(): number {
  return COLUMN_HEIGHT_PX.window1280x720;
}

/** Whether the `INPUTS` part `name`'s disclosure says it is open. */
function expanded(name: string): string | null {
  return screen.getByRole("button", { name }).getAttribute("aria-expanded");
}

/** The `INPUTS` parts that fold, in the panel's order. */
const PARTS = ["SHOWN", "STAR", "ORBIT", "PLANET", "VOLATILES · RANK"];

describe("the INPUTS column's fold (plan R15, T6)", () => {
  it("opens STAR alone at 1280 × 720 as the display mounts, every other part folded", async () => {
    await renderViewer(atFloor);
    expect(PARTS.map(expanded)).toEqual(["false", "true", "false", "false", "false"]);
    expect([
      screen.queryByRole("textbox", { name: "INIT MASS" }) === null,
      screen.queryByRole("textbox", { name: "SMA" }) === null,
    ]).toEqual([false, true]);
  });

  it("says, folded at 1280 × 720, what a group's rows show, a drawn input's dash marked DRAWN", async () => {
    await renderViewer(atFloor);
    expect(wholeText("1 AU · ECC 0.0167 · FORMED AT — AU DRAWN")).toHaveLength(1);
  });

  it("says, folded, the value drawn for the planet on show", async () => {
    const { socket, user } = await renderViewer(atFloor);
    await openUniverse(socket, user);
    await user.click(screen.getByRole("button", { name: "G GENERATE" }));
    await showPlanet(socket, toInputsDto(DEFAULT_INPUTS));
    expect(wholeText("1 AU · ECC 0.0167 · FORMED AT 1 AU DRAWN")).toHaveLength(1);
  });

  it("folds the part open as another opens at 1280 × 720, and opens STAR again as that folds", async () => {
    const { user } = await renderViewer(atFloor);
    await user.click(screen.getByRole("button", { name: "ORBIT" }));
    const opened = PARTS.map(expanded);
    await user.click(screen.getByRole("button", { name: "ORBIT" }));
    expect([opened, PARTS.map(expanded)]).toEqual([
      ["false", "false", "true", "false", "false"],
      ["false", "true", "false", "false", "false"],
    ]);
  });

  it("counts SHOWN among the parts of which one is open at 1280 × 720", async () => {
    const { user } = await renderViewer(atFloor);
    await user.click(screen.getByRole("button", { name: "SHOWN" }));
    expect(PARTS.map(expanded)).toEqual(["true", "false", "false", "false", "false"]);
    expect(screen.getByRole("textbox", { name: "BODY" })).toBeInTheDocument();
  });

  it("opens SHOWN at 1280 × 720 as a planet is handed over, so that OPEN BODY's PENDING stands", async () => {
    const { handOver } = await renderViewer(atFloor);
    handOver({ inspection: sliceEarthInspection(), sequence: 1 });
    await settle();
    expect([expanded("SHOWN"), expanded("STAR"), wholeText("OPEN BODY PENDING").length]).toEqual([
      "true",
      "false",
      1,
    ]);
  });

  it("opens a folded group at 1280 × 720 once the server refuses one of its inputs", async () => {
    const { socket, user } = await renderViewer(atFloor);
    await openUniverse(socket, user);
    await user.click(screen.getByRole("button", { name: "G GENERATE" }));
    const [asked] = socket.requestsOfKind("viewer_planet");
    await act(async () => {
      if (asked !== undefined) {
        socket.serverRejects(asked.id, {
          code: "bad_request",
          message: "the orbit's periapsis lies inside its star",
          field: "inputs.semi_major_axis_au",
        });
      }
      await vi.advanceTimersByTimeAsync(0);
    });
    expect([
      expanded("ORBIT"),
      screen.getByRole("textbox", { name: "SMA" }).getAttribute("aria-invalid"),
    ]).toEqual(["true", "true"]);
  });

  it("names the refused input in GENERATE's outcome, its reason standing under its field alone", async () => {
    const { socket, user } = await renderViewer();
    await openUniverse(socket, user);
    await user.click(screen.getByRole("button", { name: "G GENERATE" }));
    const [asked] = socket.requestsOfKind("viewer_planet");
    await act(async () => {
      if (asked !== undefined) {
        socket.serverRejects(asked.id, {
          code: "bad_request",
          message: "the orbit's periapsis lies inside its star",
          field: "inputs.semi_major_axis_au",
        });
      }
      await vi.advanceTimersByTimeAsync(0);
    });
    expect([
      wholeText("GENERATE REJECTED: see SMA").length,
      screen.getAllByText(/the orbit's periapsis lies inside its star/u).length,
    ]).toEqual([1, 1]);
  });

  it("heads the server's reason under its field with the input's REJECTED", async () => {
    const { socket, user } = await renderViewer();
    await openUniverse(socket, user);
    await rejectSma(socket, user);
    expect(wholeText("SMA REJECTED: the orbit's periapsis lies inside its star")).toHaveLength(1);
  });

  it("states the reason in GENERATE's outcome once the refused input's group is folded", async () => {
    const { socket, user } = await renderViewer(atFloor);
    await openUniverse(socket, user);
    await rejectSma(socket, user);
    await user.click(screen.getByRole("button", { name: "ORBIT" }));
    expect(wholeText("GENERATE REJECTED: the orbit's periapsis lies inside its star")).toHaveLength(
      1,
    );
  });

  it("sets a folded group's missing value as the Missing dash", async () => {
    await renderViewer(atFloor);
    const summary = wholeText("1 AU · ECC 0.0167 · FORMED AT — AU DRAWN")[0];
    expect(summary === undefined ? null : within(summary).getByText("—")).toHaveClass(
      "readout__missing",
    );
  });

  it("gives the focus in a group that folds to that group's disclosure", async () => {
    const { user, handOver } = await renderViewer(atFloor);
    await user.click(screen.getByRole("textbox", { name: "AGE" }));
    handOver({ inspection: sliceEarthInspection(), sequence: 1 });
    await settle();
    expect(screen.getByRole("button", { name: "STAR" })).toHaveFocus();
  });

  it("opens the part that holds the focus as the column becomes too short for every part", async () => {
    let heightPx: number = COLUMN_HEIGHT_PX.window1920x1080;
    const { user } = await renderViewer(() => heightPx);
    await user.click(screen.getByRole("textbox", { name: "SMA" }));
    heightPx = COLUMN_HEIGHT_PX.window1280x720;
    act(() => {
      FakeResizeObserver.resizeAll();
    });
    expect([expanded("STAR"), expanded("ORBIT")]).toEqual(["false", "true"]);
  });

  it("keeps the focus in its field as the column becomes too short for every part", async () => {
    let heightPx: number = COLUMN_HEIGHT_PX.window1920x1080;
    const { user } = await renderViewer(() => heightPx);
    await user.click(screen.getByRole("textbox", { name: "SMA" }));
    heightPx = COLUMN_HEIGHT_PX.window1280x720;
    act(() => {
      FakeResizeObserver.resizeAll();
    });
    expect(screen.getByRole("textbox", { name: "SMA" })).toHaveFocus();
  });

  it("keeps the part open in the compact layout open as the column grows to the full one", async () => {
    let heightPx: number = COLUMN_HEIGHT_PX.window1280x720;
    const { user } = await renderViewer(() => heightPx);
    await user.click(screen.getByRole("button", { name: "VOLATILES · RANK" }));
    heightPx = COLUMN_HEIGHT_PX.window1920x1080;
    act(() => {
      FakeResizeObserver.resizeAll();
    });
    expect(expanded("VOLATILES · RANK")).toBe("true");
  });

  it("gives the focus on a disclosure the full layout does without to its group's first field", async () => {
    let heightPx: number = COLUMN_HEIGHT_PX.window1280x720;
    const { user } = await renderViewer(() => heightPx);
    await user.click(screen.getByRole("button", { name: "ORBIT" }));
    heightPx = COLUMN_HEIGHT_PX.window1920x1080;
    act(() => {
      FakeResizeObserver.resizeAll();
    });
    expect(screen.getByRole("textbox", { name: "SMA" })).toHaveFocus();
  });

  it("stands STAR, ORBIT and PLANET open at 1920 × 1080, VOLATILES and SHOWN folding on their own", async () => {
    const { user } = await renderViewer();
    await user.click(screen.getByRole("button", { name: "VOLATILES · RANK" }));
    await user.click(screen.getByRole("button", { name: "SHOWN" }));
    expect([
      ["STAR", "ORBIT", "PLANET"].map((name) => screen.queryByRole("button", { name })),
      ["SHOWN", "VOLATILES · RANK"].map(expanded),
      ["INIT MASS", "SMA", "MASS", "H2O", "BODY"].map(
        (name) => screen.queryByRole("textbox", { name }) !== null,
      ),
    ]).toEqual([
      [null, null, null],
      ["true", "true"],
      [true, true, true, true, true],
    ]);
  });

  it("keeps one entry refusal on the panel, a newer one dropping the older and what was typed", async () => {
    const { user } = await renderViewer();
    const mass = screen.getByRole("textbox", { name: "INIT MASS" });
    await user.clear(mass);
    await user.type(mass, "200{Enter}");
    const age = screen.getByRole("textbox", { name: "AGE" });
    await user.clear(age);
    await user.type(age, "99{Enter}");
    expect([
      wholeText("INIT MASS INVALID: enter 0.08 to 150").length,
      wholeText("AGE INVALID: enter 0.001 to 13.787").length,
    ]).toEqual([0, 1]);
    expect(mass).toHaveProperty("value", "1");
  });

  it("drops a refusal under SUN's field once an input's entry is refused", async () => {
    const { user } = await renderViewer();
    await user.click(screen.getByRole("button", { name: "HELD" }));
    const latitude = screen.getByRole("textbox", { name: "LATITUDE" });
    await user.clear(latitude);
    await user.type(latitude, "95{Enter}");
    const mass = screen.getByRole("textbox", { name: "INIT MASS" });
    await user.clear(mass);
    await user.type(mass, "200{Enter}");
    expect([
      wholeText("LATITUDE INVALID: enter -90.0 to 90.0").length,
      wholeText("INIT MASS INVALID: enter 0.08 to 150").length,
    ]).toEqual([0, 1]);
  });

  it("drops what was typed in BODY, and its refusal, as SHOWN folds", async () => {
    const { socket, user } = await renderViewer();
    await openUniverse(socket, user);
    const shownButton = screen.getByRole("button", { name: "SHOWN" });
    await user.click(shownButton);
    await user.type(screen.getByRole("textbox", { name: "BODY" }), "xyz{Enter}");
    const refused = wholeText(
      "BODY INVALID: enter 16 hexadecimal digits, a full stop and 4 more (0-9, A-F)",
    ).length;
    await user.click(shownButton);
    await user.click(shownButton);
    expect([
      refused,
      wholeText("BODY INVALID: enter 16 hexadecimal digits, a full stop and 4 more (0-9, A-F)")
        .length,
    ]).toEqual([1, 0]);
    expect(screen.getByRole("textbox", { name: "BODY" })).toHaveProperty("value", "");
  });

  it("drops what was typed in a group, and its refusal, as the group folds", async () => {
    const { user } = await renderViewer();
    const volatiles = screen.getByRole("button", { name: "VOLATILES · RANK" });
    await user.click(volatiles);
    const water = screen.getByRole("textbox", { name: "H2O" });
    await user.type(water, "5{Enter}");
    const refused = wholeText("H2O INVALID: enter 0.001 to 0.999").length;
    await user.click(volatiles);
    await user.click(volatiles);
    expect([refused, wholeText("H2O INVALID: enter 0.001 to 0.999").length]).toEqual([1, 0]);
    expect(water).toHaveProperty("value", "—");
  });
});
