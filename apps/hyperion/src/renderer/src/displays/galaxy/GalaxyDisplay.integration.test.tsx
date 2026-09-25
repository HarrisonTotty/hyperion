/**
 * The `GALAXY` display end to end, over a fake socket (plan 05, P05.T12.a).
 *
 * @remarks
 * One script, played through `App` from the welcome to a chart at another time: what an operator
 * does, in order, with nothing faked but the socket, the canvas, the layout and the clock. Each step
 * of the script is a function, and each step has its own `it`, which plays the script up to that
 * step and then reads what that step changed, so that a failure names the step that broke. Frames and timeouts are faked throughout, since a preset's turn takes several
 * frames and the camera's angle readouts hold for 250 ms: the script waits that out rather than
 * racing it.
 */
import {
  type MassLayer,
  type RequestOf,
  type ResponseFor,
  universeTimeFromYears,
} from "@hyperion/protocol";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "../../App";
import { layerIndex } from "../../lib/galaxy/wire";
import { fakeFramesAndTimeouts } from "../../test/fakeFramesAndTimeouts";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import {
  aCreatedUniverse,
  aDensityMap,
  aRangeRequest,
  aSystemsInRange,
  aUniverse,
  aUniverseList,
  type RelativeSystemSpec,
  someGalaxyParameters,
} from "../../test/galaxyFixtures";
import { announcements } from "../../test/liveRegions";
import { type RecordingContext2D, stubCanvas } from "../../test/RecordingContext2D";

/** The universe the operator creates: `SURVEY 1` from the seed typed below. */
const UNIVERSE = aUniverse();

/** The seed as it is typed, as it goes on the wire, and as the console reads it back. */
const SEED_TYPED = "4D2";
const SEED_WIRE = "00000000000004d2";
const SEED_SHOWN = "00000000000004D2";

/**
 * The page's size, which leaves both map pictures 320 px wide: the words take 29.5rem of the width,
 * and the face-on picture is square above an edge-on one half as tall.
 */
const PAGE_WIDTH_PX = 792;
const PAGE_HEIGHT_PX = 488;
const PICTURE_PX = 320;

/** The narrowest map offered that covers a 320 px picture (plan 05, T8.d's resolution ruling). */
const MAP_RESOLUTION_PX = 256;

/** The chart centre: picked on the face-on map, off the galactic axis, with its height typed. */
const CENTRE_LY = [32_768, 0, 12] as const;

/** The chart time the operator enters, in years from the epoch. */
const CHART_TIME_YR = 12.5;

/**
 * Faked time to let pass after a preset is chosen: the turn takes 120 ms, and the angle readouts
 * hold for 250 ms after the change they show.
 */
const READOUT_SETTLE_MS = 500;

/** The systems of the query's answer that lie above the lightest mass layer. */
const SYSTEMS_ABOVE_A = 24;

/** The systems of the query's answer that lie in the lightest mass layer, the farthest out. */
const SYSTEMS_IN_A = 16;

/** The six axis directions the fixture's systems are spread along. */
const DIRECTIONS: ReadonlyArray<readonly [number, number, number]> = [
  [1, 0, 0],
  [-1, 0, 0],
  [0, 1, 0],
  [0, -1, 0],
  [0, 0, 1],
  [0, 0, -1],
];

/** The offset `distanceLy` from the chart centre along one axis direction, chosen by `index`. */
function alongAxis(index: number, distanceLy: number): readonly [number, number, number] {
  const direction = DIRECTIONS[index % DIRECTIONS.length];
  if (direction === undefined) {
    throw new Error(`no direction for system ${index}`);
  }
  return [direction[0] * distanceLy, direction[1] * distanceLy, direction[2] * distanceLy];
}

/** The mass layer of the `index`th system above the lightest layer, cycling B to E. */
function layerAboveA(index: number): MassLayer {
  const layers = ["b", "c", "d", "e"] as const;
  const layer = layers[index % layers.length];
  if (layer === undefined) {
    throw new Error(`no layer for system ${index}`);
  }
  return layer;
}

/**
 * The forty systems the server answers the range query with: twenty-four above the lightest layer,
 * 1 to 24 ly out, then sixteen of layer A from 30 ly.
 *
 * @remarks
 * The layer-A systems come last, so that raising the mass floor drops a suffix of the list and every
 * system kept keeps the designation and ID it was answered with, as a real server's would.
 */
const FORTY_SYSTEMS: ReadonlyArray<RelativeSystemSpec> = [
  ...Array.from({ length: SYSTEMS_ABOVE_A }, (_, index) => ({
    relLy: alongAxis(index, index + 1),
    layer: layerAboveA(index),
  })),
  ...Array.from({ length: SYSTEMS_IN_A }, (_, index) => ({
    relLy: alongAxis(index, 30 + index),
    layer: "a" as const,
  })),
];

/**
 * A map for the request: 8 × 8 face-on and 8 × 4 edge-on, each pixel's code its index, spanning the
 * M1 extents, with the floor 5 dex under the ceiling face-on and 7 dex edge-on.
 */
function mapFor(body: RequestOf<"density_map">): ResponseFor<"density_map"> {
  const faceOn = body.view === "face_on";
  const heightPx = faceOn ? 8 : 4;
  return aDensityMap({
    codes: Array.from({ length: 8 * heightPx }, (_, pixel) => pixel),
    widthPx: 8,
    heightPx,
    view: body.view,
    population: body.population,
    floorLog10PerLy2: faceOn ? -4 : -6,
    ceilingLog10PerLy2: 1,
  });
}

/** Plays the server's side, letting the outcomes it settles reach React. */
async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

/** Lets `ms` of faked time pass, so that frames run and a readout's hold ends. */
function wait(ms: number): void {
  act(() => {
    vi.advanceTimersByTime(ms);
  });
}

/** The console under test: the operator, the socket the server plays, and what was painted. */
interface Session {
  readonly user: ReturnType<typeof userEvent.setup>;
  readonly socket: FakeWebSocket;
  readonly recorder: RecordingContext2D;
}

/** The `GALAXY` page on show, which is the only one in the accessibility tree. */
function chartPage(): HTMLElement {
  return screen.getByRole("tabpanel", { name: "LOCAL CHART" });
}

function mapPage(): HTMLElement {
  return screen.getByRole("tabpanel", { name: "GALAXY MAP" });
}

function cursorPanel(): HTMLElement {
  return screen.getByRole("region", { name: "Cursor" });
}

/** The one element showing `text` in `UNIVERSE` that is on screen, not in its folded-away part. */
function shownInUniverse(text: string): HTMLElement {
  const shown = within(screen.getByRole("region", { name: "Universe" }))
    .getAllByText(text)
    .filter((element) => element.closest("[hidden]") === null);
  const [element] = shown;
  if (shown.length !== 1 || element === undefined) {
    throw new Error(`${shown.length} elements show ${text} in UNIVERSE`);
  }
  return element;
}

/** Places a map canvas on the screen, as layout would: the picture is 320 px wide. */
function placeCanvas(view: "face-on" | "edge-on"): HTMLElement {
  const canvas = within(mapPage()).getByRole("application", { name: `Galaxy map, ${view}` });
  vi.spyOn(canvas, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({
      x: 0,
      y: 0,
      width: PICTURE_PX,
      height: view === "face-on" ? PICTURE_PX : PICTURE_PX / 2,
    }),
  );
  return canvas;
}

/** The chart's list of systems, which takes the focus itself. */
function systemList(): HTMLElement {
  return screen.getByRole("listbox", { name: "Systems by distance" });
}

function options(): HTMLElement[] {
  return within(systemList()).getAllByRole("option");
}

/** The text of the selected system readout's value for `label`. */
function reading(label: string): string {
  const readout = screen.getByRole("status", { name: "Selected system" });
  return within(readout).getByText(label, { exact: true }).nextElementSibling?.textContent ?? "";
}

/** The text of a reading shown with the chart, such as the camera's `AZM` or the chart's time. */
function chartReading(label: string): string {
  return (
    within(chartPage()).getByText(label, { selector: "dt" }).nextElementSibling?.textContent ?? ""
  );
}

/** How many requests of each kind the client has sent, so that a step can be shown to send none. */
function requestCounts(socket: FakeWebSocket): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const kind of [
    "list_universes",
    "create_universe",
    "open_universe",
    "galaxy_parameters",
    "density_map",
    "systems_in_range",
  ] as const) {
    counts[kind] = socket.requestsOfKind(kind).length;
  }
  return counts;
}

/** 1. The client is launched, and the server opens the link and welcomes it. */
function launch(): Session {
  const advanceTimers = fakeFramesAndTimeouts();
  const user = userEvent.setup({ advanceTimers });
  const recorder = stubCanvas();
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: PAGE_WIDTH_PX, height: PAGE_HEIGHT_PX }),
  );
  render(<App />);
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  return { user, socket, recorder };
}

/** 2. `F2` shows `GALAXY`, and the server answers with no universes at all. */
async function showGalaxy({ user, socket }: Session): Promise<void> {
  await user.keyboard("{F2}");
  await server(() => {
    socket.serverAnswers("list_universes", () => aUniverseList([]));
  });
}

/** 3. A name is typed, a seed entered, and `CREATE` given. */
async function giveCreate({ user }: Session): Promise<void> {
  await user.type(screen.getByRole("textbox", { name: "NAME" }), UNIVERSE.name);
  await user.click(screen.getByRole("radio", { name: "ENTERED" }));
  await user.type(screen.getByRole("textbox", { name: "SEED VALUE" }), SEED_TYPED);
  await user.click(screen.getByRole("button", { name: "CREATE" }));
}

/** 4. The server answers the create with the new universe, then the list refresh that follows. */
async function answerCreate({ socket }: Session): Promise<void> {
  await server(() => {
    socket.serverAnswers("create_universe", () => aCreatedUniverse(UNIVERSE));
  });
  await server(() => {
    socket.serverAnswers("list_universes", () => aUniverseList([UNIVERSE]));
  });
}

/** 5. The parameters and both galaxy maps of the open universe are answered. */
async function answerGalaxyData({ socket }: Session): Promise<void> {
  await server(() => {
    socket.serverAnswers("galaxy_parameters", (body) => someGalaxyParameters(body.universe));
    socket.serverAnswers("density_map", mapFor);
    socket.serverAnswers("density_map", mapFor);
  });
}

/** 6. A point is picked on the face-on map, its height typed, and `C` centres the chart on it. */
async function centreChart({ user }: Session): Promise<void> {
  // Three quarters of the way down the picture and half way across: x = +32,768 ly, y = 0.
  await user.pointer({
    keys: "[MouseLeft]",
    target: placeCanvas("face-on"),
    coords: { clientX: PICTURE_PX / 2, clientY: (PICTURE_PX * 3) / 4 },
  });
  const height = within(cursorPanel()).getByRole("textbox", { name: "Z" });
  await user.clear(height);
  await user.type(height, "12");
  // A coordinate is entered as its field is left; `C` then publishes the cursor as the centre.
  await user.tab();
  await user.keyboard("c");
}

/** 7. The server answers the range query with forty systems, and the census of them. */
async function answerQuery({ socket }: Session): Promise<void> {
  await server(() => {
    socket.serverAnswers("systems_in_range", (body) => {
      const kept = FORTY_SYSTEMS.filter(
        (system) => layerIndex(system.layer) >= layerIndex(body.min_layer),
      );
      const answer = aSystemsInRange({
        centreLy: CENTRE_LY,
        radiusLy: body.radius_ly,
        minLayer: body.min_layer,
        systems: kept,
      });
      // The server echoes the instant it was asked about, to the nanosecond, and answers in the
      // order it walked its cells, which is not by distance: what puts the list nearest first is
      // the client's own sort, so the answer is sent farthest first to make it say so.
      return { ...answer, time: body.time, systems: answer.systems.toReversed() };
    });
  });
}

/** 8. The list of systems is reached with `Tab`, and the next system selected. */
async function selectSystem({ user }: Session): Promise<void> {
  // The list is the next stop after the `CURSOR` fields, so one `Tab` from the last of them
  // reaches it; the step asserts the focus landed there.
  await user.click(within(cursorPanel()).getByRole("textbox", { name: "Z" }));
  await user.tab();
  await user.keyboard("{ArrowDown}");
}

/** 9. The chart is turned to the `TOP` view with its key. */
async function turnToTop({ user }: Session): Promise<void> {
  await user.keyboard("t");
  wait(READOUT_SETTLE_MS);
}

/** 10. The mass floor is raised to 0.5 M☉, and the query that follows is answered. */
async function raiseFloor(session: Session): Promise<void> {
  await session.user.click(screen.getByRole("radio", { name: "0.5" }));
  await answerQuery(session);
}

/** 11. A chart time of +12.50 yr is entered, and the query that follows is answered. */
async function enterChartTime(session: Session): Promise<void> {
  const time = within(chartPage()).getByLabelText("CHART TIME");
  await session.user.clear(time);
  await session.user.type(time, `${CHART_TIME_YR}{Enter}`);
  await answerQuery(session);
}

/** The script, in order, each step named for the state it leaves the console in. */
const SCRIPT = [
  { stage: "galaxy", play: showGalaxy },
  { stage: "commanded", play: giveCreate },
  { stage: "created", play: answerCreate },
  { stage: "loaded", play: answerGalaxyData },
  { stage: "centred", play: centreChart },
  { stage: "charted", play: answerQuery },
  { stage: "selected", play: selectSystem },
  { stage: "top", play: turnToTop },
  { stage: "floored", play: raiseFloor },
  { stage: "timed", play: enterChartTime },
] as const satisfies ReadonlyArray<{
  readonly stage: string;
  readonly play: (session: Session) => Promise<void>;
}>;

/** How far through the script a test plays. */
type Stage = "linked" | (typeof SCRIPT)[number]["stage"];

/**
 * Plays the script from launch up to and including `stage`.
 *
 * @throws Error when no step of the script leaves the console in that state.
 */
async function playTo(stage: Stage): Promise<Session> {
  const end = SCRIPT.findIndex((step) => step.stage === stage) + 1;
  if (end === 0 && stage !== "linked") {
    throw new Error(`no step of the script leaves the console ${stage}`);
  }
  const session = launch();
  for (const step of SCRIPT.slice(0, end)) {
    // Each step must land before the next, as an operator's actions do.
    // oxlint-disable-next-line no-await-in-loop
    await step.play(session);
  }
  return session;
}

describe("the GALAXY display, end to end", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("greets the server and reports a nominal link once welcomed", async () => {
    const { socket } = await playTo("linked");

    // The client greets the server first; the universe list follows, as soon as the link is up.
    expect(socket.sent[0]).toEqual({ type: "hello", client_version: __APP_VERSION__ });
    expect(screen.getByRole("status", { name: "Server link" })).toHaveTextContent("LINK NOMINAL");
    expect(screen.getByRole("heading", { level: 1, name: "Link" })).toBeInTheDocument();
  });

  it("shows GALAXY on F2, where the server holds no universe to open", async () => {
    const { socket } = await playTo("galaxy");

    expect(screen.getByRole("heading", { level: 1, name: "Galaxy" })).toBeInTheDocument();
    expect(socket.requestsOfKind("list_universes")).toHaveLength(1);
    expect(screen.getByText("NO UNIVERSES: create one below")).toBeInTheDocument();
  });

  it("sends the name and the seed typed when CREATE is given, and waits for the answer", async () => {
    const { socket } = await playTo("commanded");

    expect(socket.requestsOfKind("create_universe").map(({ body }) => body)).toEqual([
      { kind: "create_universe", name: UNIVERSE.name, seed: SEED_WIRE },
    ]);
    expect(screen.getByText("PENDING")).toBeInTheDocument();
  });

  it("opens the universe the create answered with, reading its seed back in upper case", async () => {
    await playTo("created");

    expect(shownInUniverse("NAME").nextElementSibling).toHaveTextContent(UNIVERSE.name);
    expect(shownInUniverse("SEED").nextElementSibling).toHaveTextContent(SEED_SHOWN);
    expect(screen.getByRole("tablist", { name: "Galaxy pages" })).toBeInTheDocument();
  });

  it("draws both maps of the open universe and fills its parameters", async () => {
    const { user, socket, recorder } = await playTo("loaded");

    expect(socket.requestsOfKind("density_map").map(({ body }) => body)).toEqual([
      {
        kind: "density_map",
        universe: UNIVERSE.id,
        view: "face_on",
        population: "all",
        resolution: MAP_RESOLUTION_PX,
        bits: 8,
      },
      {
        kind: "density_map",
        universe: UNIVERSE.id,
        view: "edge_on",
        population: "all",
        resolution: MAP_RESOLUTION_PX,
        bits: 8,
      },
    ]);
    expect(recorder.calls("drawImage")).toHaveLength(2);

    await user.click(screen.getByRole("tab", { name: "PARAMETERS" }));

    const parameters = screen.getByRole("tabpanel", { name: "PARAMETERS" });
    expect(within(parameters).getByText("STELLAR MASS")).toBeInTheDocument();
    expect(within(parameters).getByText("5.20E10")).toBeInTheDocument();
  });

  it("sends a range query for the point picked on the map and published with C", async () => {
    const { socket } = await playTo("centred");

    expect(socket.requestsOfKind("systems_in_range").map(({ body }) => body)).toEqual([
      aRangeRequest({ centreLy: CENTRE_LY, universe: UNIVERSE.id }),
    ]);
    // The chart is what CENTRE CHART acts on, so it is the page that shows the query's state.
    expect(within(chartPage()).getByText("PENDING")).toBeInTheDocument();
  });

  it("charts the forty systems the query returned, nearest first", async () => {
    await playTo("charted");

    expect(chartReading("RADIUS")).toBe("32,768.0 ly");
    expect(within(chartPage()).getByText(/^COMPLETE ABOVE/u)).toHaveTextContent(
      "COMPLETE ABOVE 0.08",
    );
    expect(chartReading("SYSTEMS")).toBe("40");
    expect(options()[0]).toHaveAttribute("aria-setsize", "40");
    expect(options()[0]).toHaveAccessibleName(
      "H7K 4C0RFZ B-1, DWARF K5V, 1.00 ly, 0.63 solar masses, IN RANGE",
    );
    expect(options()[1]).toHaveAccessibleName(
      "H7K 4C0RFZ C-2, DWARF G2V, 2.00 ly, 1.63 solar masses, IN RANGE",
    );
  });

  it("fills the readout for the system the list's arrow key selects", async () => {
    await playTo("selected");

    // Reached from the keyboard alone, as every control on the console must be.
    expect(systemList()).toHaveFocus();
    expect(options()[1]).toHaveAttribute("aria-selected", "true");
    expect(reading("DESIG")).toBe("H7K 4C0RFZ C-2");
    expect(reading("DIST")).toBe("2.00 ly");
    expect(reading("COREWARD")).toBe("+2.00 ly");
    expect(reading("POPULATION")).toBe("OLD THIN DISC");
  });

  it("announces a selection once, from the selected system's readout alone", async () => {
    const { user } = await playTo("charted");
    await user.click(within(cursorPanel()).getByRole("textbox", { name: "Z" }));
    await user.tab();

    const announced = await announcements(() => user.keyboard("{ArrowDown}"));

    // P05.T12.b's "the `Selected system` output is announced on selection", held by count: the
    // list's rows and its position line and the chart's mark labels change too, and none of them
    // announces.
    expect(announced).toEqual([screen.getByRole("status", { name: "Selected system" })]);
  });

  it("turns the chart to TOP on t, where it reads AZM 000° and ELV +90°", async () => {
    await playTo("top");

    expect(chartReading("AZM")).toBe("000°");
    expect(chartReading("ELV")).toBe("+90°");
    expect(screen.getByRole("button", { name: "T TOP" })).toHaveAttribute("aria-pressed", "true");
  });

  it("asks again above a mass floor of 0.5 M☉, and the census line follows", async () => {
    const { socket } = await playTo("floored");

    expect(socket.requestsOfKind("systems_in_range").at(-1)?.body.min_layer).toBe("b");
    expect(within(chartPage()).getByText(/^COMPLETE ABOVE/u)).toHaveTextContent(
      "COMPLETE ABOVE 0.50",
    );
    expect(chartReading("SYSTEMS")).toBe(String(SYSTEMS_ABOVE_A));
  });

  it("asks at the chart time entered, and reads it on the chart and the readout", async () => {
    const { socket } = await playTo("timed");

    expect(socket.requestsOfKind("systems_in_range").at(-1)?.body.time).toEqual(
      universeTimeFromYears(CHART_TIME_YR),
    );
    expect(chartReading("UT")).toBe("+12.50 yr");
    expect(
      within(screen.getByRole("status", { name: "Selected system" })).getByText("AT UT +12.50 yr"),
    ).toBeInTheDocument();
  });

  it("keeps the chart, its camera and its selection across a visit to LINK", async () => {
    const { user, socket } = await playTo("timed");
    const asked = requestCounts(socket);

    await user.keyboard("{F1}");
    await user.keyboard("{F2}");

    expect(chartReading("AZM")).toBe("000°");
    expect(chartReading("UT")).toBe("+12.50 yr");
    expect(reading("DESIG")).toBe("H7K 4C0RFZ C-2");
    // Requests are stateless and their answers are kept, so a visit to LINK asks for nothing again
    // and the link itself is never re-opened.
    expect(requestCounts(socket)).toEqual(asked);
    expect(FakeWebSocket.instances).toHaveLength(1);
  });
});
