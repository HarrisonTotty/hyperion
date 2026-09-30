import type { RequestOf, ResponseFor } from "@hyperion/protocol";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { UniverseProvider } from "../../components/UniverseProvider";
import type { CentreLy } from "../../lib/galaxy/model";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { aDensityMap, anOpenedUniverse, aUniverseList } from "../../test/galaxyFixtures";
import { stubCanvas } from "../../test/RecordingContext2D";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { GalaxyMapPanel } from "./GalaxyMapPanel";
import { DUST_OVERLAY_LABEL } from "./GalaxyMapView";
import { UniversePanel } from "./UniversePanel";

/** Plays the server's side, letting the outcomes it settles reach React. */
async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

/**
 * A map for the request: 8 × 8 face-on and 8 × 4 edge-on, spanning the M1 extents, with the floor
 * 5 dex under the ceiling face-on and 7 dex edge-on, as plan 04 places it.
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

/** Holds the cursor for the page, as the display does. */
function MapPage() {
  const [cursorLy, setCursorLy] = useState<CentreLy>([0, 0, 0]);
  return (
    <div role="tabpanel" aria-label="GALAXY MAP">
      <GalaxyMapPanel cursorLy={cursorLy} onCursor={setCursorLy} centreLy={null} />
    </div>
  );
}

/**
 * Lays the page out at `widthPx` by `heightPx`. 792 × 488 is 30.5 rem tall, so compact: the words
 * and axes take 32.5 rem of the width, leaving 272 px, and the controls' row 8 rem of the height,
 * leaving the pictures 360 px together, face-on 240 and edge-on 120.
 */
function stubPageSize(widthPx = 792, heightPx = 488): void {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: widthPx, height: heightPx }),
  );
}

/** Renders the page beside the universe panel, which opens universes for it. */
async function renderPanel() {
  const user = userEvent.setup();
  stubCanvas();
  render(
    <ServerLinkHarness>
      <UniverseProvider>
        <UniversePanel expanded onToggle={() => undefined} />
        <MapPage />
      </UniverseProvider>
    </ServerLinkHarness>,
  );
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  await server(() => {
    socket.serverAnswers("list_universes", () => aUniverseList());
  });
  return { user, socket };
}

async function openUniverse(
  user: ReturnType<typeof userEvent.setup>,
  socket: FakeWebSocket,
): Promise<void> {
  await user.click(screen.getByRole("button", { name: "Open universe SURVEY 1" }));
  await server(() => {
    socket.serverAnswers("open_universe", () => anOpenedUniverse());
  });
}

async function answerBothMaps(socket: FakeWebSocket): Promise<void> {
  await server(() => {
    socket.serverAnswers("density_map", mapFor);
    socket.serverAnswers("density_map", mapFor);
  });
}

function panel(): HTMLElement {
  return screen.getByRole("tabpanel", { name: "GALAXY MAP" });
}

/** The pictures' width the page gives the stylesheet. */
function pictureWidth(): string {
  const grid = panel().querySelector<HTMLElement>(".galaxy-map__grid");
  return grid?.style.getPropertyValue("--map-picture") ?? "";
}

describe("GalaxyMapPanel", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
    stubPageSize();
  });

  it("reads NO UNIVERSE OPEN and asks for no map before a universe is open", async () => {
    const { socket } = await renderPanel();

    expect(within(panel()).getByText("NO UNIVERSE OPEN")).toBeInTheDocument();
    expect(socket.requestsOfKind("density_map")).toEqual([]);
  });

  it("shows the galaxy face-on above edge-on, each on a canvas of its own", async () => {
    const { user, socket } = await renderPanel();
    await openUniverse(user, socket);

    await answerBothMaps(socket);

    const faceOn = within(panel()).getByRole("application", { name: "Galaxy map, face-on" });
    const edgeOn = within(panel()).getByRole("application", { name: "Galaxy map, edge-on" });
    expect(faceOn.compareDocumentPosition(edgeOn)).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
  });

  it("offers the population as a radio group, every system first", async () => {
    const { user, socket } = await renderPanel();

    await openUniverse(user, socket);

    const group = within(panel()).getByRole("group", { name: "POPULATION" });
    expect(within(group).getByRole("radio", { name: "ALL" })).toBeChecked();
    expect(within(group).getByRole("radio", { name: "YOUNG THIN DISC" })).not.toBeChecked();
  });

  it("asks for both views of every system at first, at the resolution the pictures need", async () => {
    const { user, socket } = await renderPanel();

    await openUniverse(user, socket);

    // Pictures 240 px wide at a device pixel ratio of 1 take the 256-pixel map, drawn smaller.
    expect(
      socket
        .requestsOfKind("density_map")
        .map(({ body }) => [body.view, body.population, body.resolution]),
    ).toEqual([
      ["face_on", "all", 256],
      ["edge_on", "all", 256],
    ]);
  });

  it("asks for both views again of the young thin disc when it is chosen", async () => {
    const { user, socket } = await renderPanel();
    await openUniverse(user, socket);
    await answerBothMaps(socket);

    await user.click(within(panel()).getByRole("radio", { name: "YOUNG THIN DISC" }));

    const requests = socket.requestsOfKind("density_map").map(({ body }) => body);
    expect(requests.slice(2).map((body) => [body.view, body.population])).toEqual([
      ["face_on", "young"],
      ["edge_on", "young"],
    ]);
  });

  it("holds the population back while the link is down, and keeps the maps on show", async () => {
    const { user, socket } = await renderPanel();
    await openUniverse(user, socket);
    await answerBothMaps(socket);

    act(() => {
      socket.close();
    });
    const young = within(panel()).getByRole("radio", { name: "YOUNG THIN DISC" });
    await user.click(young);

    expect(young).not.toBeChecked();
    expect(young).toHaveAttribute("aria-disabled", "true");
    expect(young).toHaveAccessibleDescription("NO CARRIER");
    expect(socket.requestsOfKind("density_map")).toHaveLength(2);
    expect(
      within(panel()).getByRole("application", { name: "Galaxy map, face-on" }),
    ).toBeInTheDocument();
    expect(
      within(panel()).getByRole("application", { name: "Galaxy map, edge-on" }),
    ).toBeInTheDocument();
  });

  it("offers the quantity with its key, systems first, and the dust overlay with its own", async () => {
    const { user, socket } = await renderPanel();

    await openUniverse(user, socket);

    const group = within(panel()).getByRole("group", { name: "Q QUANTITY" });
    expect(group).toHaveAttribute("aria-keyshortcuts", "Q");
    expect(within(group).getByRole("radio", { name: "SYSTEMS" })).toBeChecked();
    expect(within(group).getByRole("radio", { name: "EXTINCTION" })).not.toBeChecked();
    const overlay = within(panel()).getByRole("button", { name: "D DUST OVERLAY" });
    expect(overlay).toHaveAttribute("aria-pressed", "false");
    expect(overlay).toHaveAttribute("aria-keyshortcuts", "D");
  });

  it("asks for both views' extinction maps under EXTINCTION, and holds the overlay back there", async () => {
    const { user, socket } = await renderPanel();
    await openUniverse(user, socket);
    await answerBothMaps(socket);

    await user.click(within(panel()).getByRole("radio", { name: "EXTINCTION" }));

    expect(
      socket.requestsOfKind("extinction_map").map(({ body }) => [body.view, body.resolution]),
    ).toEqual([
      ["face_on", 256],
      ["edge_on", 256],
    ]);
    const overlay = within(panel()).getByRole("button", { name: "D DUST OVERLAY" });
    expect(overlay).toHaveAttribute("aria-disabled", "true");
    expect(overlay).toHaveAccessibleDescription("SYSTEMS ONLY");
  });

  it("steps the quantity with Q and puts the overlay on with D", async () => {
    const { user, socket } = await renderPanel();
    await openUniverse(user, socket);
    await answerBothMaps(socket);

    await user.keyboard("d");

    expect(within(panel()).getByRole("button", { name: "D DUST OVERLAY" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    // The overlay needs the extinction maps beside the density maps it dims.
    expect(socket.requestsOfKind("extinction_map")).toHaveLength(2);

    await user.keyboard("q");

    expect(within(panel()).getByRole("radio", { name: "EXTINCTION" })).toBeChecked();
    await user.keyboard("q");
    expect(within(panel()).getByRole("radio", { name: "SYSTEMS" })).toBeChecked();
  });

  it("holds the quantity back while the link is down", async () => {
    const { user, socket } = await renderPanel();
    await openUniverse(user, socket);
    await answerBothMaps(socket);

    act(() => {
      socket.close();
    });
    await user.keyboard("q");
    const extinction = within(panel()).getByRole("radio", { name: "EXTINCTION" });
    await user.click(extinction);

    expect(extinction).not.toBeChecked();
    expect(extinction).toHaveAccessibleDescription("NO CARRIER");
    expect(socket.requestsOfKind("extinction_map")).toHaveLength(0);
  });

  it("gives each view a legend of its own, with its own floor", async () => {
    const { user, socket } = await renderPanel();
    await openUniverse(user, socket);

    await answerBothMaps(socket);

    const faceOn = within(panel()).getByRole("region", { name: "FACE-ON FROM NORTH" });
    const edgeOn = within(panel()).getByRole("region", { name: "EDGE-ON ALONG +Y" });
    expect(
      within(faceOn).getByText("FLOOR 1.00E-4: AT OR BELOW SHOWN AS BACKGROUND"),
    ).toBeInTheDocument();
    expect(
      within(edgeOn).getByText("FLOOR 1.00E-6: AT OR BELOW SHOWN AS BACKGROUND"),
    ).toBeInTheDocument();
    expect(within(faceOn).getAllByText("COLUMN DENSITY")).toHaveLength(1);
    expect(within(edgeOn).getAllByText("COLUMN DENSITY")).toHaveLength(1);
  });

  it("states the frame and the scale once, since both pictures are at one scale", async () => {
    const { user, socket } = await renderPanel();
    await openUniverse(user, socket);

    await answerBothMaps(socket);

    expect(within(panel()).getAllByText("FRAME")).toHaveLength(1);
    expect(within(panel()).getByText("FRAME").parentElement).toHaveTextContent("FRAME GALACTIC");
    // 240 px for 131,072 ly: the longest 1-2-5 bar within a quarter of the width.
    expect(within(panel()).getAllByText("20,000 ly")).toHaveLength(1);
    expect(within(panel()).getByRole("img", { name: "Scale bar, 20,000 ly" })).toBeInTheDocument();
  });

  it("gives both pictures one width, as large as the page's width allows", async () => {
    // Stacked, 792 px wide leaves 320 px beside the 24 rem words and the axes.
    stubPageSize(792, 800);
    const { user, socket } = await renderPanel();

    await openUniverse(user, socket);

    expect(pictureWidth()).toBe("320px");
  });

  it("gives both pictures one width, as large as the page's height allows", async () => {
    // 1,000 px wide leaves 528 px beside the words, but 720 px of height, less the tabs' 2.5 rem
    // and the gap, holds face-on 448 px and edge-on 224 px.
    stubPageSize(1_000, 720);
    const { user, socket } = await renderPanel();

    await openUniverse(user, socket);

    expect(pictureWidth()).toBe("448px");
  });

  it("lays a tall page out stacked and a short one compact, as the local chart is", async () => {
    stubPageSize(1_036, 880);
    const { user, socket } = await renderPanel();
    await openUniverse(user, socket);
    const grid = (): Element | null => panel().querySelector(".galaxy-map__grid");

    expect(grid()).toHaveClass("galaxy-map__grid--stacked");

    // 1280 × 720's page.
    stubPageSize(798, 520);
    act(() => {
      window.dispatchEvent(new Event("resize"));
    });

    expect(grid()).toHaveClass("galaxy-map__grid--compact");
    expect(pictureWidth()).toBe("260px");
  });

  it.each([
    ["stacked", 1_036, 880],
    ["compact", 798, 520],
  ])(
    "keeps one tab order laid out %s: the choices, the toggle, then face-on and edge-on",
    async (_layout, widthPx, heightPx) => {
      stubPageSize(widthPx, heightPx);
      const { user, socket } = await renderPanel();
      await openUniverse(user, socket);
      await answerBothMaps(socket);

      within(panel()).getByRole("radio", { name: "ALL" }).focus();
      await user.tab();
      expect(within(panel()).getByRole("radio", { name: "SYSTEMS" })).toHaveFocus();
      await user.tab();
      expect(within(panel()).getByRole("button", { name: "D DUST OVERLAY" })).toHaveFocus();
      await user.tab();
      expect(
        within(panel()).getByRole("application", { name: "Galaxy map, face-on" }),
      ).toHaveFocus();
      await user.tab();
      expect(
        within(panel()).getByRole("application", { name: "Galaxy map, edge-on" }),
      ).toHaveFocus();
    },
  );

  it("names the overlay once for the page while it is on, and not under EXTINCTION", async () => {
    const { user, socket } = await renderPanel();
    await openUniverse(user, socket);
    await answerBothMaps(socket);

    expect(within(panel()).queryByText(DUST_OVERLAY_LABEL)).not.toBeInTheDocument();

    await user.keyboard("d");

    expect(within(panel()).getAllByText(DUST_OVERLAY_LABEL)).toHaveLength(1);

    await user.keyboard("q");

    expect(within(panel()).queryByText(DUST_OVERLAY_LABEL)).not.toBeInTheDocument();
  });

  it("keeps its maps while the page is hidden and not laid out", async () => {
    const { user, socket } = await renderPanel();
    await openUniverse(user, socket);
    await answerBothMaps(socket);

    // A hidden page measures 0 × 0; it keeps the size, and so the maps, it had.
    stubPageSize(0, 0);
    act(() => {
      window.dispatchEvent(new Event("resize"));
    });

    expect(pictureWidth()).toBe("240px");
    expect(socket.requestsOfKind("density_map")).toHaveLength(2);
    expect(socket.cancelledIds()).toEqual([]);
  });
});
