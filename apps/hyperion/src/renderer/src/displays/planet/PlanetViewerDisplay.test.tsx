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
import { FakeWebSocket } from "../../test/FakeWebSocket";
import {
  anOpenedUniverse,
  aSystemRecord,
  aUniverse,
  aUniverseList,
} from "../../test/galaxyFixtures";
import { sliceEarthInspection } from "../../test/inspectionFixture";
import { FIXTURE_EARTH, FIXTURE_SYSTEM, sliceBodies } from "../../test/planetaryFixture";
import { REQUEST_TIMEOUT_MS } from "../../lib/useServerRequest";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { stubViewLayout } from "../../test/viewDisplayHarness";
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
 * Renders the display in its providers, a universe's panel beside it to open one, and a way to hand
 * it a planet over, as `App` does.
 */
async function renderViewer(): Promise<{
  readonly socket: FakeWebSocket;
  readonly user: ReturnType<typeof userEvent.setup>;
  readonly handOver: (opening: PlanetViewerOpening) => void;
  /** Hides the display, as `App`'s `Activity` does when another is chosen, or shows it again. */
  readonly show: (shown: boolean) => void;
}> {
  const advanceTimers = fakeFramesAndTimeouts();
  stubViewLayout({ widthPx: 1280, heightPx: 720 }, () => ({ widthPx: 900, heightPx: 600 }));
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

/** Opens the panel's `SOURCE`, folded as the display mounts, unless it is open. */
async function openSource(user: ReturnType<typeof userEvent.setup>): Promise<void> {
  const source = screen.getByRole("button", { name: "SOURCE" });
  if (source.getAttribute("aria-expanded") !== "true") {
    await user.click(source);
  }
}

/** Types `id` into `BODY`, under `SOURCE`, and presses `OPEN BODY`. */
async function openTyped(user: ReturnType<typeof userEvent.setup>, id: string): Promise<void> {
  await openSource(user);
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

    it("states the server's refusal of a body that is no planet", async () => {
      const { socket, user } = await renderViewer();
      await openUniverse(socket, user);
      await openTyped(user, FIXTURE_EARTH);
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
        screen.queryByText(`BODY ${FIXTURE_EARTH.toUpperCase()}`) !== null,
        socket.requestsOfKind("resolve_system").length,
      ]).toEqual([null, true, 0]);
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

  describe("BODY's field", () => {
    it("states the form it takes while it is empty", async () => {
      const { user } = await renderViewer();
      await openSource(user);
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

  describe("SOURCE", () => {
    it("is folded as the display mounts, saying no planet is shown", async () => {
      await renderViewer();
      expect([
        screen.getByRole("button", { name: "SOURCE" }).getAttribute("aria-expanded"),
        screen.queryByRole("textbox", { name: "BODY" }),
        screen.queryByText("NONE") !== null,
      ]).toEqual(["false", null, true]);
    });

    it("says a generated planet is shown", async () => {
      const { socket, user } = await renderViewer();
      await openUniverse(socket, user);
      await user.click(screen.getByRole("button", { name: "G GENERATE" }));
      await showPlanet(socket, toInputsDto(DEFAULT_INPUTS));
      expect(screen.queryByText("GENERATED")).not.toBeNull();
    });
  });
});
