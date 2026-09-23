import type { RequestOf, ResponseFor } from "@hyperion/protocol";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { UniverseProvider } from "../../components/UniverseProvider";
import { formatNumber } from "../../lib/format";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import {
  aDensityMap,
  anOpenedUniverse,
  aUniverse,
  aUniverseList,
  someGalaxyParameters,
} from "../../test/galaxyFixtures";
import { announcements } from "../../test/liveRegions";
import { type RecordingContext2D, stubCanvas } from "../../test/RecordingContext2D";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { GalaxyDisplay } from "./GalaxyDisplay";

const RECONNECT_DELAY_MS = 2_000;

const SURVEY_2 = aUniverse({ id: "00000000000000b2", name: "SURVEY 2", seed: "000000000000beef" });

/** A face-on picture 320 px square and an edge-on one 320 × 160, 8 map pixels across: 16,384 ly. */
const PIXEL_LY = 16_384;

/** Plays the server's side, letting the outcomes it settles reach React. */
async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

/**
 * A map for the request: 8 × 8 face-on and 8 × 4 edge-on, each pixel's code its index, spanning
 * the M1 extents, with the floor 5 dex under the ceiling face-on and 7 dex edge-on.
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

interface Rendered {
  readonly user: ReturnType<typeof userEvent.setup>;
  readonly socket: FakeWebSocket;
  readonly recorder: RecordingContext2D;
}

/**
 * Renders the display under a welcomed link, laid out so that the map page is 792 × 488 px: its
 * words take 29.5rem of the width and leave the pictures 320 px, face-on 320 and edge-on 160 tall.
 */
function renderDisplay(): Rendered {
  const user = userEvent.setup();
  const recorder = stubCanvas();
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: 792, height: 488 }),
  );
  render(
    <ServerLinkHarness>
      <UniverseProvider>
        <GalaxyDisplay />
      </UniverseProvider>
    </ServerLinkHarness>,
  );
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  return { user, socket, recorder };
}

async function openFromList({ user, socket }: Rendered, universe = aUniverse()): Promise<void> {
  await user.click(screen.getByRole("button", { name: `Open universe ${universe.name}` }));
  await server(() => {
    socket.serverAnswers("open_universe", () => anOpenedUniverse(universe));
  });
}

/** Renders the display, opens SURVEY 1 and answers both maps. */
async function renderWithMaps(): Promise<Rendered> {
  const rendered = renderDisplay();
  await server(() => {
    rendered.socket.serverAnswers("list_universes", () => aUniverseList([aUniverse(), SURVEY_2]));
  });
  await openFromList(rendered);
  await server(() => {
    rendered.socket.serverAnswers("density_map", mapFor);
    rendered.socket.serverAnswers("density_map", mapFor);
  });
  return rendered;
}

function universeToggle(): HTMLElement {
  return screen.getByRole("button", { name: "Universe" });
}

/** The element in the UNIVERSE panel showing `text` that is on screen, not in its folded part. */
function shownInUniverse(text: string): HTMLElement {
  const universe = screen.getByRole("region", { name: "Universe" });
  const shown = within(universe)
    .getAllByText(text)
    .filter((element) => element.closest("[hidden]") === null);
  if (shown.length !== 1) {
    throw new Error(`${shown.length} elements show ${text} in UNIVERSE`);
  }
  const [element] = shown;
  if (element === undefined) {
    throw new Error(`nothing shows ${text} in UNIVERSE`);
  }
  return element;
}

function mapPage(): HTMLElement {
  return screen.getByRole("tabpanel", { name: "GALAXY MAP" });
}

function mapCanvas(view: "face-on" | "edge-on"): HTMLElement {
  return within(mapPage()).getByRole("application", { name: `Galaxy map, ${view}` });
}

/** Places a canvas on the screen, as layout would, at the origin of the viewport. */
function placeCanvas(view: "face-on" | "edge-on"): HTMLElement {
  const canvas = mapCanvas(view);
  const heightPx = view === "face-on" ? 320 : 160;
  vi.spyOn(canvas, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: 320, height: heightPx }),
  );
  return canvas;
}

function cursorPanel(): HTMLElement {
  return screen.getByRole("region", { name: "Cursor" });
}

/** The text of the cursor readout's value for `label`. */
function reading(label: string): string | null | undefined {
  return within(cursorPanel()).getByText(label, { exact: true }).nextElementSibling?.textContent;
}

/** The cursor's coordinate as its entry field shows it, in light-years. */
function coordinate(label: "X" | "Y" | "Z"): string {
  const field = within(cursorPanel()).getByRole("textbox", { name: label });
  return field instanceof HTMLInputElement ? field.value : "";
}

function mapView(title: "FACE-ON FROM NORTH" | "EDGE-ON ALONG +Y"): HTMLElement {
  return within(mapPage()).getByRole("region", { name: title });
}

async function click(
  user: ReturnType<typeof userEvent.setup>,
  target: HTMLElement,
  clientX: number,
  clientY: number,
): Promise<void> {
  await user.pointer({ keys: "[MouseLeft]", target, coords: { clientX, clientY } });
}

describe("GalaxyDisplay", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("shows UNIVERSE whole and nothing else before a universe is open", () => {
    renderDisplay();

    const universe = screen.getByRole("region", { name: "Universe" });
    expect(
      within(universe).getByRole("heading", { level: 2, name: "Universe" }),
    ).toBeInTheDocument();
    expect(universeToggle()).toHaveAttribute("aria-expanded", "true");
    expect(within(universe).getByText("NO UNIVERSE OPEN")).toBeInTheDocument();
    // Nothing to page through, to centre on, or to list yet.
    expect(screen.queryByRole("tablist")).not.toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Cursor" })).not.toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Systems" })).not.toBeInTheDocument();
  });

  it("drops the empty state once a universe opens", async () => {
    const rendered = renderDisplay();
    await server(() => {
      rendered.socket.serverAnswers("list_universes", () => aUniverseList());
    });

    await openFromList(rendered);

    expect(screen.queryByText("NO UNIVERSE OPEN")).not.toBeInTheDocument();
  });

  describe("UNIVERSE", () => {
    it("folds to one line naming the universe once one is open", async () => {
      await renderWithMaps();

      expect(universeToggle()).toHaveAttribute("aria-expanded", "false");
      expect(shownInUniverse("NAME").nextElementSibling).toHaveTextContent("SURVEY 1");
      expect(shownInUniverse("SEED").nextElementSibling).toHaveTextContent("00000000000004D2");
      expect(shownInUniverse("GEN VER").nextElementSibling).toHaveTextContent("2");
      expect(screen.queryByRole("table", { name: "Universes" })).not.toBeInTheDocument();
    });

    it("moves the focus to its control when opening a universe folds it", async () => {
      const rendered = renderDisplay();
      await server(() => {
        rendered.socket.serverAnswers("list_universes", () => aUniverseList());
      });

      await openFromList(rendered);

      expect(universeToggle()).toHaveFocus();
    });

    it("takes the column when shown whole, and gives it back to the pages when folded", async () => {
      const { user } = await renderWithMaps();

      await user.click(universeToggle());

      expect(universeToggle()).toHaveAttribute("aria-expanded", "true");
      expect(screen.getByRole("table", { name: "Universes" })).toBeInTheDocument();
      expect(screen.queryByRole("tablist")).not.toBeInTheDocument();

      await user.click(universeToggle());

      expect(screen.getByRole("tablist", { name: "Galaxy pages" })).toBeInTheDocument();
    });

    it("stays whole, once shown, until another universe is opened", async () => {
      const rendered = await renderWithMaps();
      await rendered.user.click(universeToggle());

      await openFromList(rendered, SURVEY_2);

      expect(universeToggle()).toHaveAttribute("aria-expanded", "false");
      expect(shownInUniverse("SURVEY 2")).toBeInTheDocument();
    });

    it("takes the focus to its control when a create left unconfirmed shows it over the map", async () => {
      const rendered = await renderWithMaps();
      await rendered.user.click(universeToggle());
      await rendered.user.click(screen.getByRole("button", { name: "NEW UNIVERSE" }));
      await rendered.user.type(screen.getByRole("textbox", { name: "NAME" }), "SURVEY 3{Enter}");
      // Folded again while the create is pending, and the map taken up meanwhile.
      await rendered.user.click(universeToggle());
      mapCanvas("face-on").focus();
      vi.useFakeTimers();
      await server(() => {
        rendered.socket.close();
      });
      act(() => {
        vi.advanceTimersByTime(RECONNECT_DELAY_MS);
      });
      vi.useRealTimers();

      act(() => {
        FakeWebSocket.latest().serverWelcomes();
      });

      // The report of the lost create shows the panel whole, which hides the map that had it.
      expect(document.activeElement).toBe(universeToggle());
    });

    it("stays whole while a create's result is unconfirmed, so that the report is seen", async () => {
      const rendered = await renderWithMaps();
      await rendered.user.click(universeToggle());
      await rendered.user.click(screen.getByRole("button", { name: "NEW UNIVERSE" }));
      await rendered.user.type(screen.getByRole("textbox", { name: "NAME" }), "SURVEY 3{Enter}");
      vi.useFakeTimers();
      await server(() => {
        rendered.socket.close();
      });
      act(() => {
        vi.advanceTimersByTime(RECONNECT_DELAY_MS);
      });
      vi.useRealTimers();
      act(() => {
        FakeWebSocket.latest().serverWelcomes();
      });

      await rendered.user.click(universeToggle());

      expect(universeToggle()).toHaveAttribute("aria-expanded", "true");
      expect(universeToggle()).toHaveAttribute("aria-disabled", "true");
      expect(screen.getByText(/^CREATE UNCONFIRMED/)).toBeVisible();
      expect(screen.queryByRole("tablist")).not.toBeInTheDocument();
    });
  });

  describe("pages", () => {
    it("offers the parameters, the map and the chart, and shows the map by default", async () => {
      await renderWithMaps();

      const tabs = within(screen.getByRole("tablist", { name: "Galaxy pages" })).getAllByRole(
        "tab",
      );
      expect(tabs.map((tab) => tab.textContent)).toEqual([
        "PARAMETERS",
        "GALAXY MAP",
        "LOCAL CHART",
      ]);
      expect(screen.getByRole("tab", { name: "GALAXY MAP" })).toHaveAttribute(
        "aria-selected",
        "true",
      );
      expect(mapPage()).toBeVisible();
      expect(screen.queryByRole("tabpanel", { name: "PARAMETERS" })).not.toBeInTheDocument();
    });

    it("shows the parameters, fetched with the universe, when PARAMETERS is chosen", async () => {
      const { user, socket } = await renderWithMaps();
      await server(() => {
        socket.serverAnswers("galaxy_parameters", (body) => someGalaxyParameters(body.universe));
      });

      await user.click(screen.getByRole("tab", { name: "PARAMETERS" }));

      const page = screen.getByRole("tabpanel", { name: "PARAMETERS" });
      expect(within(page).getByText("STELLAR MASS")).toBeInTheDocument();
      expect(screen.queryByRole("tabpanel", { name: "GALAXY MAP" })).not.toBeInTheDocument();
      // The map page is only hidden, so its maps are not asked for again on its return.
      await user.click(screen.getByRole("tab", { name: "GALAXY MAP" }));
      expect(socket.requestsOfKind("density_map")).toHaveLength(2);
    });

    it("hands the focus to the PARAMETERS tab when its RETRY is pressed", async () => {
      const { user, socket } = await renderWithMaps();
      await user.click(screen.getByRole("tab", { name: "PARAMETERS" }));
      const [parameters] = socket.requestsOfKind("galaxy_parameters");
      await server(() => {
        socket.serverRejects(parameters?.id ?? -1, {
          code: "queue_full",
          message: "the interactive queue is full",
          field: null,
        });
      });
      act(() => {
        within(screen.getByRole("tabpanel", { name: "PARAMETERS" }))
          .getByRole("button", { name: "RETRY" })
          .focus();
      });

      await user.keyboard("{Enter}");

      // RETRY goes as the request goes pending, and nothing on the page can take the focus until
      // the answer: its tab can (the orchestrator's ruling 18).
      expect(document.activeElement).toBe(screen.getByRole("tab", { name: "PARAMETERS" }));
    });

    it("hands the focus to the GALAXY MAP tab when a map's RETRY is pressed", async () => {
      const rendered = renderDisplay();
      const { user, socket } = rendered;
      await server(() => {
        socket.serverAnswers("list_universes", () => aUniverseList([aUniverse()]));
      });
      await openFromList(rendered);
      const faceOnMap = socket
        .requestsOfKind("density_map")
        .find(({ body }) => body.view === "face_on");
      await server(() => {
        socket.serverRejects(faceOnMap?.id ?? -1, {
          code: "queue_full",
          message: "the interactive queue is full",
          field: null,
        });
      });
      act(() => {
        within(mapView("FACE-ON FROM NORTH")).getByRole("button", { name: "RETRY" }).focus();
      });

      await user.keyboard("{Enter}");

      // The view is mounted afresh from PENDING, RETRY and all; the page's tab stays.
      expect(document.activeElement).toBe(screen.getByRole("tab", { name: "GALAXY MAP" }));
    });

    it("is one stop in the tab order, its pages chosen with the arrow keys", async () => {
      const { user } = await renderWithMaps();
      const parameters = screen.getByRole("tab", { name: "PARAMETERS" });
      const map = screen.getByRole("tab", { name: "GALAXY MAP" });
      expect(parameters).toHaveAttribute("tabindex", "-1");
      expect(map).toHaveAttribute("tabindex", "0");
      map.focus();

      await user.keyboard("{ArrowLeft}");

      expect(parameters).toHaveFocus();
      expect(parameters).toHaveAttribute("aria-selected", "true");
      expect(screen.getByRole("tabpanel", { name: "PARAMETERS" })).toBeVisible();

      await user.keyboard("{End}");

      expect(screen.getByRole("tab", { name: "LOCAL CHART" })).toHaveFocus();
      expect(screen.getByRole("tabpanel", { name: "LOCAL CHART" })).toBeVisible();

      await user.keyboard("{Home}");

      expect(parameters).toHaveFocus();
      expect(parameters).toHaveAttribute("aria-selected", "true");
    });

    it("wraps round from the last page to the first, and back", async () => {
      const { user } = await renderWithMaps();
      const parameters = screen.getByRole("tab", { name: "PARAMETERS" });
      const chart = screen.getByRole("tab", { name: "LOCAL CHART" });
      await user.click(chart);

      await user.keyboard("{ArrowRight}");

      expect(parameters).toHaveFocus();
      expect(parameters).toHaveAttribute("aria-selected", "true");

      await user.keyboard("{ArrowLeft}");

      expect(chart).toHaveFocus();
      expect(chart).toHaveAttribute("aria-selected", "true");
    });

    it("shows the map again once another universe is opened", async () => {
      const rendered = await renderWithMaps();
      await rendered.user.click(screen.getByRole("tab", { name: "PARAMETERS" }));
      await rendered.user.click(universeToggle());

      await openFromList(rendered, SURVEY_2);

      expect(screen.getByRole("tab", { name: "GALAXY MAP" })).toHaveAttribute(
        "aria-selected",
        "true",
      );
    });
  });

  describe("the map cursor", () => {
    it("starts at the galactic centre, read in light-years", async () => {
      await renderWithMaps();

      expect(coordinate("X")).toBe("0.0");
      expect(coordinate("Y")).toBe("0.0");
      expect(coordinate("Z")).toBe("0.0");
      expect(reading("RADIUS")).toBe("0.0 ly");
      expect(reading("HEIGHT")).toBe("+0.0 ly");
    });

    it("reads no angle on the galactic axis, where it is undefined", async () => {
      await renderWithMaps();

      expect(reading("ANGLE")).toBe("—");
    });

    it("stands beside the map, after it in reading order", async () => {
      await renderWithMaps();

      expect(mapPage().compareDocumentPosition(cursorPanel())).toBe(
        Node.DOCUMENT_POSITION_FOLLOWING,
      );
      expect(
        cursorPanel().compareDocumentPosition(screen.getByRole("region", { name: "Systems" })),
      ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    });

    it("announces the cursor as an arrow key moves it", async () => {
      const { user } = await renderWithMaps();
      mapCanvas("face-on").focus();

      await user.keyboard("{ArrowDown}");

      expect(within(cursorPanel()).getByText(/^CURSOR X/)).toHaveTextContent(
        "CURSOR X 16,384.0 ly, Y 0.0 ly, Z 0.0 ly",
      );
    });

    it("raises X by one map pixel on the down arrow, +x being down face-on", async () => {
      const { user } = await renderWithMaps();
      mapCanvas("face-on").focus();

      await user.keyboard("{ArrowDown}");

      expect(coordinate("X")).toBe("16,384.0");
      expect(coordinate("Y")).toBe("0.0");
    });

    it("raises Y by one map pixel on the right arrow, +y being right face-on", async () => {
      const { user } = await renderWithMaps();
      mapCanvas("face-on").focus();

      await user.keyboard("{ArrowRight}");

      expect(coordinate("X")).toBe("0.0");
      expect(coordinate("Y")).toBe("16,384.0");
    });

    it("replaces a refused entry with the cursor's value when a map moves it", async () => {
      const { user } = await renderWithMaps();
      const x = within(cursorPanel()).getByRole("textbox", { name: "X" });
      await user.clear(x);
      await user.type(x, "70000{Enter}");
      mapCanvas("face-on").focus();

      await user.keyboard("{ArrowDown}{ArrowUp}");

      expect(coordinate("X")).toBe("0.0");
      expect(screen.queryByText(/INVALID/)).not.toBeInTheDocument();
    });

    it("centres the chart with C while a population option has focus", async () => {
      const { user } = await renderWithMaps();
      await user.click(within(mapPage()).getByRole("radio", { name: "ALL" }));

      await user.keyboard("c");

      // C shows the chart it centred, so the map is chosen again to read its mark.
      await user.click(screen.getByRole("tab", { name: "GALAXY MAP" }));
      expect(
        within(mapView("FACE-ON FROM NORTH")).getByRole("img", { name: "Chart centre" }),
      ).toHaveStyle({ left: "50%", top: "50%" });
    });

    it("hands the focus to the LOCAL CHART tab when C hides the map that had it", async () => {
      const { user } = await renderWithMaps();
      mapCanvas("face-on").focus();

      await user.keyboard("c");

      // The map's page is hidden to show the chart's, and a hidden element keeps the focus in
      // jsdom, where Chromium takes it away; either way it must reach the page now shown.
      expect(document.activeElement).toBe(screen.getByRole("tab", { name: "LOCAL CHART" }));
    });

    it("hands the focus to the LOCAL CHART tab when C is pressed on the map's tab", async () => {
      const { user } = await renderWithMaps();
      act(() => {
        screen.getByRole("tab", { name: "GALAXY MAP" }).focus();
      });

      await user.keyboard("c");

      // The tab list's one stop follows the page shown, and so does the focus that was on it.
      expect(document.activeElement).toBe(screen.getByRole("tab", { name: "LOCAL CHART" }));
    });

    it("leaves the focus on CENTRE CHART, which stays on show, when it shows the chart", async () => {
      const { user } = await renderWithMaps();
      const centre = within(cursorPanel()).getByRole("button", { name: "C CENTRE CHART" });

      await user.click(centre);

      expect(document.activeElement).toBe(centre);
    });

    it("makes each map a focusable picture with its key hint", async () => {
      await renderWithMaps();

      expect(mapCanvas("face-on")).toHaveAttribute("tabindex", "0");
      expect(mapCanvas("face-on")).toHaveAccessibleDescription("ARROWS MOVE CURSOR");
      expect(mapCanvas("edge-on")).toHaveAccessibleDescription("↑ ↓ MOVE CURSOR");
    });

    it("stops at the centre of the map's edge pixel, inside the root cube", async () => {
      const { user } = await renderWithMaps();
      mapCanvas("face-on").focus();

      await user.keyboard("{Shift>}{ArrowDown}{/Shift}");

      // Half of the 16,384 ly pixel inside the face at 65,536 ly, which lies outside the cube.
      expect(coordinate("X")).toBe("57,344.0");
    });

    it("sets X and Y to the extent's centre on a click at the face-on centre, leaving Z", async () => {
      const { user } = await renderWithMaps();
      await click(user, placeCanvas("edge-on"), 40, 40);
      mapCanvas("face-on").focus();
      await user.keyboard("{ArrowDown}{ArrowRight}");

      await click(user, placeCanvas("face-on"), 160, 160);

      expect(coordinate("X")).toBe("0.0");
      expect(coordinate("Y")).toBe("0.0");
      expect(coordinate("Z")).toBe(formatNumber(PIXEL_LY, 1));
    });

    it("centres the chart on the plane after a face-on click and face-on steps", async () => {
      const { user, socket } = await renderWithMaps();
      // A pointer inside a pixel, off its centre, then steps of one and ten pixels.
      await click(user, placeCanvas("face-on"), 123.4, 77.7);
      await user.keyboard("{ArrowDown}{ArrowLeft}{Shift>}{ArrowUp}{ArrowRight}{/Shift}");

      await user.keyboard("c");

      // The edge-on map has an even number of rows, as at 128 pixels, so that no row is centred on
      // the plane; a face-on pick or step keeps the cursor's z, which starts at 0, and nothing
      // takes it from the edge-on row nearest the plane (plan 04's note for this plan).
      const centre = socket.requestsOfKind("systems_in_range")[0]?.body.centre;
      expect(centre?.cell_ly[2]).toBe(0);
      expect(centre?.offset_m[2]).toBe(0);
    });

    it("sets X from the height of a face-on click and Y from its place across", async () => {
      const { user } = await renderWithMaps();

      // Three quarters of the way down (x = +32,768 ly) and a quarter across (y = -32,768 ly).
      await click(user, placeCanvas("face-on"), 80, 240);

      expect(coordinate("X")).toBe("32,768.0");
      expect(coordinate("Y")).toBe("-32,768.0");
    });

    it("changes only Z on a click edge-on", async () => {
      const { user } = await renderWithMaps();
      mapCanvas("face-on").focus();
      await user.keyboard("{ArrowDown}{ArrowRight}");

      // A quarter of the way down the edge-on picture, and an eighth of the way across.
      await click(user, placeCanvas("edge-on"), 40, 40);

      expect(coordinate("X")).toBe("16,384.0");
      expect(coordinate("Y")).toBe("16,384.0");
      expect(coordinate("Z")).toBe("16,384.0");
    });

    it("leaves the cursor alone when an arrow key comes with Control", async () => {
      const { user } = await renderWithMaps();
      mapCanvas("face-on").focus();

      await user.keyboard("{Control>}{ArrowDown}{/Control}");

      expect(coordinate("X")).toBe("0.0");
    });

    it("moves only Z with the arrow keys edge-on", async () => {
      const { user } = await renderWithMaps();
      mapCanvas("edge-on").focus();

      await user.keyboard("{ArrowLeft}{ArrowUp}");

      expect(coordinate("X")).toBe("0.0");
      expect(coordinate("Z")).toBe("16,384.0");
      expect(reading("HEIGHT")).toBe("+16,384.0 ly");
    });

    it("reads the cursor's radius and angle in the GALACTIC frame", async () => {
      const { user } = await renderWithMaps();
      mapCanvas("face-on").focus();

      await user.keyboard("{ArrowDown}{ArrowRight}");

      expect(reading("RADIUS")).toBe("23,170.5 ly");
      expect(reading("ANGLE")).toBe("045.0°");
    });

    it("marks the cursor on both maps where it is", async () => {
      const { user } = await renderWithMaps();
      mapCanvas("face-on").focus();

      await user.keyboard("{ArrowDown}");

      // x = 16,384 ly is 62.5% of the way down face-on and across edge-on; y and z are 0.
      expect(
        within(mapView("FACE-ON FROM NORTH")).getByRole("img", { name: "Cursor" }),
      ).toHaveStyle({
        left: "50%",
        top: "62.5%",
      });
      expect(within(mapView("EDGE-ON ALONG +Y")).getByRole("img", { name: "Cursor" })).toHaveStyle({
        left: "62.5%",
        top: "50%",
      });
    });

    it("reads the column density under the cursor on each map", async () => {
      await renderWithMaps();

      // At the centre: face-on code 36 of 255 over 5 dex from 1E-4, edge-on code 20 over 7 from 1E-6.
      expect(
        within(mapView("FACE-ON FROM NORTH")).getByText("CURSOR DENSITY").parentElement,
      ).toHaveTextContent("CURSOR DENSITY 4.89E-4 SYSTEMS/ly²");
      expect(
        within(mapView("EDGE-ON ALONG +Y")).getByText("CURSOR DENSITY").parentElement,
      ).toHaveTextContent("CURSOR DENSITY 3.34E-6 SYSTEMS/ly²");
    });

    it("announces the density once for one arrow key press, from the map being moved over", async () => {
      const { user } = await renderWithMaps();
      mapCanvas("face-on").focus();

      const announced = await announcements(() => user.keyboard("{ArrowDown}"));

      // Both readings follow the shared cursor's x, so both change on this key press and both
      // announced it before the orchestrator's ruling 16. The CURSOR panel's position summary is a
      // region of its own and says something else, so it is not counted here.
      const densities = announced.filter((region) =>
        region.textContent.startsWith("CURSOR DENSITY"),
      );
      expect(densities).toEqual([
        within(mapView("FACE-ON FROM NORTH")).getByText("CURSOR DENSITY").parentElement,
      ]);
    });

    it("announces the position once for one arrow key press, and nothing but it and the density", async () => {
      const { user } = await renderWithMaps();
      mapCanvas("face-on").focus();

      const announced = await announcements(() => user.keyboard("{ArrowDown}"));

      // What P05.T12.b listens for by ear, held here by count: beside the map's reading, which the
      // test above counts, the CURSOR panel's position summary once, and no other region.
      const others = announced.filter((region) => !region.textContent.startsWith("CURSOR DENSITY"));
      expect(others).toEqual([within(cursorPanel()).getByText(/^CURSOR X /u)]);
    });

    it("reads BELOW FLOOR where the map's code is 0", async () => {
      const { user } = await renderWithMaps();
      mapCanvas("face-on").focus();

      // The raster's first pixel, code 0 in the fixture: the most -x and the most +y, which the
      // turned picture shows at its top right.
      await user.keyboard("{Shift>}{ArrowUp}{ArrowRight}{/Shift}");

      expect(
        within(mapView("FACE-ON FROM NORTH")).getByText("CURSOR DENSITY").parentElement,
      ).toHaveTextContent("CURSOR DENSITY BELOW FLOOR");
    });

    it("paints nothing when the cursor moves", async () => {
      const { user, recorder } = await renderWithMaps();
      recorder.clear();
      mapCanvas("face-on").focus();

      await user.keyboard("{ArrowDown}");

      expect(recorder.calls("drawImage")).toEqual([]);
      expect(recorder.calls("putImageData")).toEqual([]);
    });

    it("moves the crosshair when a coordinate is typed and entered", async () => {
      const { user } = await renderWithMaps();
      const x = within(cursorPanel()).getByRole("textbox", { name: "X" });

      await user.clear(x);
      await user.type(x, "26000");
      await user.tab();

      expect(
        within(mapView("FACE-ON FROM NORTH")).getByRole("img", { name: "Cursor" }),
      ).toHaveStyle({
        left: "50%",
        top: `${((26_000 + 65_536) / 131_072) * 100}%`,
      });
    });

    it("pegs the edge-on cursor to the map's top for a typed Z above it", async () => {
      const { user } = await renderWithMaps();
      const z = within(cursorPanel()).getByRole("textbox", { name: "Z" });

      await user.clear(z);
      await user.type(z, "50000");
      await user.tab();

      const edgeOn = mapView("EDGE-ON ALONG +Y");
      expect(within(edgeOn).getByRole("img", { name: "Cursor, off the map above" })).toHaveStyle({
        top: "0%",
      });
      expect(within(edgeOn).getByText("↑")).toBeInTheDocument();
      expect(within(edgeOn).getByText("CURSOR DENSITY").parentElement).toHaveTextContent(
        "CURSOR DENSITY —",
      );
    });

    it("holds CENTRE CHART back while the link is down, saying why", async () => {
      const { user, socket } = await renderWithMaps();

      act(() => {
        socket.close();
      });
      const centre = within(cursorPanel()).getByRole("button", { name: "C CENTRE CHART" });
      await user.click(centre);

      expect(centre).toHaveAttribute("aria-disabled", "true");
      expect(centre).toHaveAccessibleDescription("NO CARRIER");
      expect(
        within(mapView("FACE-ON FROM NORTH")).queryByRole("img", { name: "Chart centre" }),
      ).not.toBeInTheDocument();
    });
  });

  it("holds the chart centre that CENTRE CHART sets, and marks it on both maps", async () => {
    const { user } = await renderWithMaps();
    expect(
      within(mapView("FACE-ON FROM NORTH")).queryByRole("img", { name: "Chart centre" }),
    ).not.toBeInTheDocument();
    mapCanvas("face-on").focus();

    // One map pixel down the face-on map, x = 16,384 ly, then the chart is centred on the cursor.
    await user.keyboard("{ArrowDown}c");

    // C shows the chart it centred, so the map is chosen again to read its marks.
    await user.click(screen.getByRole("tab", { name: "GALAXY MAP" }));
    expect(
      within(mapView("FACE-ON FROM NORTH")).getByRole("img", { name: "Chart centre" }),
    ).toHaveStyle({ left: "50%", top: "62.5%" });
    expect(
      within(mapView("EDGE-ON ALONG +Y")).getByRole("img", { name: "Chart centre" }),
    ).toHaveStyle({
      left: "62.5%",
      top: "50%",
    });
  });
});
