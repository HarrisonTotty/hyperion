/**
 * The `SYSTEM` display's planets over a fake socket, against the shared wire fixture (plan 14,
 * P14.T41.b, T42 and T43 for bodies).
 */
import type { ResponseFor } from "@hyperion/protocol";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { FakeResizeObserver } from "../../test/FakeResizeObserver";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import {
  earthDetail,
  earthMassAndOrbit,
  FIXTURE_EARTH,
  FIXTURE_JUPITER,
  jupiterDetail,
  populatedBodies,
  sliceBodies,
} from "../../test/planetaryFixture";
import { stubCanvas } from "../../test/RecordingContext2D";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import {
  A_CENTURY,
  anUnsupportedBodiesError,
  aSingleStarSummary,
  aSummaryResponse,
  aSystemTarget,
  PIN_SYSTEM,
} from "../../test/systemFixtures";
import { SystemView } from "./SystemView";

const WIDTH_PX = 400;
const HEIGHT_PX = 300;

/** Plays the server's side, letting the outcomes it settles reach React. */
async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

function renderView() {
  const user = userEvent.setup();
  stubCanvas();
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: WIDTH_PX, height: HEIGHT_PX }),
  );
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(HEIGHT_PX);
  render(
    <ServerLinkHarness>
      <SystemView target={aSystemTarget()} />
    </ServerLinkHarness>,
  );
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  return { user, socket };
}

/** Answers the hosts, then the bodies with `bodies`, or with `unsupported` for `null`. */
async function answer(
  socket: FakeWebSocket,
  bodies: ResponseFor<"system_bodies"> | null = sliceBodies(),
): Promise<void> {
  await server(() => {
    socket.serverAnswers("system_summary", () => aSummaryResponse(aSingleStarSummary()));
  });
  await server(() => {
    const [request] = socket.requestsOfKind("system_bodies").toReversed();
    if (request === undefined) {
      throw new Error("the display asks for its bodies");
    }
    if (bodies === null) {
      socket.serverRejects(request.id, anUnsupportedBodiesError());
    } else {
      socket.serverResponds(request.id, bodies);
    }
  });
}

function mapPanel(): HTMLElement {
  return screen.getByRole("region", { name: /^Orbit map/ });
}

function bodiesPanel(): HTMLElement {
  return screen.getByRole("region", { name: /^Bodies/ });
}

function readout(): HTMLElement {
  return screen.getByRole("status", { name: "Selected body" });
}

/** The terms of the readout that read `label`. */
function terms(label: string): HTMLElement[] {
  return within(readout())
    .queryAllByRole("term")
    .filter((term) => term.textContent === label);
}

/** The value a readout label reads, as its `dd` holds it. */
function reading(label: string): string {
  return terms(label)[0]?.nextElementSibling?.textContent ?? "";
}

function rowNames(): Array<string | null> {
  return within(screen.getByRole("tree", { name: "Bodies" }))
    .getAllByRole("treeitem")
    .map((item) => item.getAttribute("aria-label"));
}

async function selectRow(user: ReturnType<typeof userEvent.setup>, name: RegExp): Promise<void> {
  const row = within(screen.getByRole("tree", { name: "Bodies" })).getByRole("treeitem", { name });
  await user.click(row);
}

async function answerDetail(socket: FakeWebSocket, detail: ResponseFor<"body_detail">) {
  await server(() => {
    socket.serverAnswers("body_detail", () => detail);
  });
}

beforeEach(() => {
  FakeWebSocket.instances = [];
  vi.stubGlobal("WebSocket", FakeWebSocket);
});

describe("SystemView's bodies request", () => {
  it("asks once for its system's bodies at the time it was opened at, at every level", () => {
    const { socket } = renderView();

    expect(socket.requestsOfKind("system_bodies").map((request) => request.body)).toEqual([
      {
        kind: "system_bodies",
        universe: aSystemTarget().universe,
        system: PIN_SYSTEM,
        time: A_CENTURY,
        detail: "full",
      },
    ]);
  });

  it("shows the hosts alone, with no fault, while the server does not serve the bodies", async () => {
    const { socket } = renderView();
    await answer(socket, null);

    expect(screen.getByRole("application", { name: "Orbit map" })).toBeInTheDocument();
    expect(within(mapPanel()).queryByText(/REJECTED/)).not.toBeInTheDocument();
    expect(
      within(mapPanel()).getByText(
        "PLANETS, MOONS, RINGS, BELTS AND COMETARY HALO: NOT YET MODELLED",
      ),
    ).toBeInTheDocument();
    expect(rowNames()).toEqual(["H7K 4C0RFZ D-7 /0, DWARF"]);
  });

  it("says PENDING while the bodies are asked for, the hosts drawn", async () => {
    const { socket } = renderView();
    await server(() => {
      socket.serverAnswers("system_summary", () => aSummaryResponse(aSingleStarSummary()));
    });

    expect(screen.getByRole("application", { name: "Orbit map" })).toBeInTheDocument();
    expect(within(mapPanel()).getByText("PENDING")).toBeInTheDocument();
  });
});

describe("SystemView with the slice's bodies", () => {
  it("lists the planets under their star by semi-major axis", async () => {
    const { socket } = renderView();
    await answer(socket);

    expect(rowNames()).toEqual([
      "H7K 4C0RFZ D-7 /0, DWARF",
      "H7K 4C0RFZ D-7 /768, PLANET, SMA 1.00 AU",
      "H7K 4C0RFZ D-7 /1280, PLANET, SMA 5.20 AU",
    ]);
    expect(screen.getByText("1-3 of 3")).toBeInTheDocument();
  });

  it("composes the note from the tags, without the planets", async () => {
    const { socket } = renderView();
    await answer(socket);

    expect(
      within(mapPanel()).getByText("MOONS, RINGS, BELTS AND COMETARY HALO: NOT YET MODELLED"),
    ).toBeInTheDocument();
    expect(within(mapPanel()).queryByText(/^PLANETS/)).not.toBeInTheDocument();
  });

  it("shows the granted level and the system's architecture class", async () => {
    const { socket } = renderView();
    await answer(socket);
    // The system's own readings come first in the panel, before the selected star's.
    const [detail] = within(bodiesPanel()).getAllByText("DETAIL");
    const [arch] = within(bodiesPanel()).getAllByText("ARCH");

    expect(detail?.nextElementSibling).toHaveTextContent("FULL");
    expect(arch?.nextElementSibling).toHaveTextContent("SOLAR-LIKE");
  });

  it("draws the map on the system plane, a giant planet apart from a smaller one", async () => {
    const { socket } = renderView();
    await answer(socket);
    const legend = screen.getByRole("group", { name: "Orbit map legend" });

    expect(within(legend).getByText("FILLED ABOVE SYSTEM PLANE")).toBeInTheDocument();
    expect(within(legend).getByText("PLANET")).toBeInTheDocument();
    expect(within(legend).getByText("GIANT PLANET")).toBeInTheDocument();
  });

  it("reads the zones in the star's readout", async () => {
    const { socket } = renderView();
    await answer(socket);

    expect(reading("ZONE")).toBe("H7K 4C0RFZ D-7 /0");
    expect(reading("ARCH")).toBe("SOLAR-LIKE");
    expect(reading("SNOW LINE")).toBe("2.26 AU");
    expect(reading("HABITABLE ZONE")).toBe("0.989 AU – 1.69 AU");
    expect(reading("OPTIMISTIC")).toBe("0.749 AU – 1.77 AU");
    expect(terms("STABLE ZONE")).toHaveLength(0);
  });

  it("switches the optimistic habitable zone's annulus off and on, apart from the conservative", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    const toggle = within(mapPanel()).getByRole("button", { name: "OPTIMISTIC" });
    const label = (text: string) =>
      within(mapPanel()).queryByText(text, { selector: ".spatial-label" });

    expect(toggle).toHaveAttribute("aria-pressed", "true");
    expect(label("OPTIMISTIC")).toBeInTheDocument();

    await user.click(toggle);

    expect(toggle).toHaveAttribute("aria-pressed", "false");
    expect(label("OPTIMISTIC")).not.toBeInTheDocument();
    expect(label("HABITABLE ZONE")).toBeInTheDocument();
    // The readout reads the zone whatever the map draws.
    expect(reading("OPTIMISTIC")).toBe("0.749 AU – 1.77 AU");

    await user.click(toggle);

    expect(label("OPTIMISTIC")).toBeInTheDocument();
  });

  it("switches the habitable zone's annulus off and on", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    const toggle = within(mapPanel()).getByRole("button", { name: "HABITABLE ZONE" });
    const annulusLabel = () =>
      within(mapPanel()).queryByText("HABITABLE ZONE", { selector: ".spatial-label" });

    expect(toggle).toHaveAttribute("aria-pressed", "true");
    expect(annulusLabel()).toBeInTheDocument();

    await user.click(toggle);

    expect(toggle).toHaveAttribute("aria-pressed", "false");
    expect(annulusLabel()).not.toBeInTheDocument();
    expect(
      within(mapPanel()).queryByText("SNOW LINE", { selector: ".spatial-label" }),
    ).toBeVisible();
  });
});

describe("SystemView's body readout", () => {
  it("asks for the record of the planet selected, at the time the system was asked for", async () => {
    const { user, socket } = renderView();
    await answer(socket);

    await selectRow(user, /\/768/);

    expect(socket.requestsOfKind("body_detail").map((request) => request.body)).toEqual([
      {
        kind: "body_detail",
        universe: aSystemTarget().universe,
        body: FIXTURE_EARTH,
        time: A_CENTURY,
        detail: "full",
      },
    ]);
    expect(within(bodiesPanel()).getByText("PENDING")).toBeInTheDocument();
    // The list's entry reads until the record comes.
    expect(reading("MASS")).toBe("1.00 M");
  });

  it("reads the whole record with a unit on every value", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthDetail());

    expect(reading("KIND")).toBe("PLANET");
    expect(reading("STATE")).toBe("PRESENT");
    expect(reading("PARENT")).toBe("H7K 4C0RFZ D-7 /0");
    expect(reading("DETAIL")).toBe("FULL");
    expect(reading("MASS")).toBe("1.00 M");
    expect(within(readout()).getByRole("img", { name: "Earth masses" })).toBeInTheDocument();
    expect(reading("SMA")).toBe("1.00 AU");
    expect(reading("PERIOD")).toBe("365 d");
    expect(reading("ECC")).toBe("0.0167");
    expect(reading("INC")).toBe("0.0°");
    expect(reading("DIST")).toMatch(/^0\.9\d{2} AU$|^1\.0\d AU$/);
    expect(reading("RADIUS")).toBe("6370 km");
    expect(reading("DENSITY")).toBe("5510 kg/m³");
    expect(reading("GRAVITY")).toBe("9.82 m/s²");
    expect(reading("T EQ")).toBe("255 K");
    expect(reading("CLASS")).toBe("ROCKY");
    expect(reading("IRON")).toBe("32.3 %");
    expect(within(bodiesPanel()).queryByText("PENDING")).not.toBeInTheDocument();
  });

  it("reads NOT YET MODELLED once for each section not modelled", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthDetail());

    for (const section of ["LABEL", "MOONS", "RINGS", "SURFACE", "HOOKS"]) {
      expect(terms(section)).toHaveLength(1);
      expect(reading(section)).toBe("NOT YET MODELLED");
    }
    expect(within(readout()).getAllByText("NOT YET MODELLED")).toHaveLength(5);
  });

  it("reads NOT RESOLVED for what a mass-and-orbit record withholds", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthMassAndOrbit());

    expect(reading("DETAIL")).toBe("MASS AND ORBIT ONLY");
    expect(reading("BULK")).toBe("NOT RESOLVED");
    expect(reading("SURFACE")).toBe("NOT RESOLVED");
    expect(reading("HOOKS")).toBe("NOT RESOLVED");
    expect(terms("RADIUS")).toHaveLength(0);
    expect(reading("SMA")).toBe("1.00 AU");
  });

  it("shows no surface row at all for a gas giant, whose surface does not apply", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/1280/);
    await answerDetail(socket, jupiterDetail());

    expect(reading("DESIG")).toBe("H7K 4C0RFZ D-7 /1280");
    expect(reading("CLASS")).toBe("GAS GIANT");
    expect(terms("SURFACE")).toHaveLength(0);
    expect(reading("HOOKS")).toBe("NOT YET MODELLED");
    expect(reading("MASS")).toBe("318 M");
  });

  it("selects a planet from the map's list by the keyboard, and the map with it", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    const tree = screen.getByRole("tree", { name: "Bodies" });

    act(() => {
      tree.focus();
    });
    await user.keyboard("{ArrowDown}{ArrowDown}{Enter}");

    expect(reading("DESIG")).toBe("H7K 4C0RFZ D-7 /1280");
    expect(socket.requestsOfKind("body_detail").at(-1)?.body.body).toBe(FIXTURE_JUPITER);
  });
});

describe("SystemView with every kind of body", () => {
  it("lists a destroyed planet with its state in words, and does not draw it", async () => {
    const { user, socket } = renderView();
    await answer(socket, populatedBodies());

    expect(rowNames()).toContain("H7K 4C0RFZ D-7 /512, PLANET, DESTROYED");
    expect(rowNames()).toContain("H7K 4C0RFZ D-7 /768, PLANET, NOT YET FORMED");
    expect(rowNames()).toContain("H7K 4C0RFZ D-7 /1024, PLANET, UNBOUND");
    expect(
      within(mapPanel()).queryByText("H7K 4C0RFZ D-7 /512", { selector: ".spatial-label" }),
    ).not.toBeInTheDocument();

    await selectRow(user, /\/512,/);

    expect(reading("STATE")).toBe("DESTROYED");
    expect(reading("CAUSE")).toBe("ENGULFED");
    expect(reading("SINCE")).toBe("UT -500 yr 000/00:00:00");
    expect(terms("ORBIT")).toHaveLength(0);
  });

  it("names only the moons and rings the tags leave unmodelled", async () => {
    const { socket } = renderView();
    await answer(socket, populatedBodies());

    expect(within(mapPanel()).getByText("MOONS AND RINGS: NOT YET MODELLED")).toBeInTheDocument();
    expect(within(bodiesPanel()).getAllByText("DETAIL")[0]?.nextElementSibling).toHaveTextContent(
      "TO BULK",
    );
  });
});

/**
 * The bodies panel's two scrolling regions as the built client laid them out in Electron, with a
 * planet selected in a system of twelve bodies (round 8, measured over the DevTools protocol): at
 * 1920 × 1080 the list's rows had 366 px and the readout 395 px, at 1280 × 720 170 px and 214 px.
 * The readout's 26 readings then ran to 431 px at 1280 px, about 16.6 px apiece as they stand two
 * to a line with some a line of their own; jsdom lays nothing out, so the readings are placed so.
 */
const WINDOWS = [
  { name: "1920 × 1080", listPx: 366, readoutPx: 395 },
  { name: "1280 × 720", listPx: 170, readoutPx: 214 },
] as const;
const READING_PX = 431 / 26;
const ROW_PX = 32;

/** Lays the panel out: each region's visible height, and each reading a line of its own. */
function layOut(listPx: number, readoutPx: number): void {
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockImplementation(function height(
    this: HTMLElement,
  ) {
    if (this.getAttribute("role") === "tree") {
      return listPx;
    }
    return this.classList.contains("body-readout__scroll") ? readoutPx : HEIGHT_PX;
  });
  vi.spyOn(HTMLElement.prototype, "offsetTop", "get").mockImplementation(function top(
    this: HTMLElement,
  ) {
    const readings = this.closest("dl")?.querySelectorAll("dt");
    const index = readings === undefined ? -1 : [...readings].indexOf(this);
    return index < 0 ? 0 : index * READING_PX;
  });
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockImplementation(function tall(
    this: HTMLElement,
  ) {
    return this.tagName === "DT" ? READING_PX - 1 : 0;
  });
  FakeResizeObserver.resizeAll();
}

function readoutRegion(): HTMLElement {
  return screen.getByRole("region", { name: "Readings" });
}

/** The position readout under the readings: `1-13 of 26`. */
function readingsPosition(): string {
  return readoutRegion().nextElementSibling?.textContent ?? "";
}

describe.each(WINDOWS)("SystemView's bodies panel at $name", ({ listPx, readoutPx }) => {
  it("scrolls the planet's readings, with those in view and their total under them", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthDetail());
    act(() => {
      layOut(listPx, readoutPx);
    });
    const total = within(readout()).getAllByRole("term").length;
    const inView = Math.min(total, Math.ceil(readoutPx / READING_PX));

    expect(readingsPosition()).toBe(`1-${inView} of ${total}`);

    const region = readoutRegion();
    act(() => {
      region.scrollTop = total * READING_PX - readoutPx;
      fireEvent.scroll(region);
    });

    expect(readingsPosition()).toMatch(new RegExp(`^\\d+-${total} of ${total}$`));
  });

  it("keeps the focus on the list and the selected row in view as the keys select", async () => {
    const { user, socket } = renderView();
    await answer(socket, populatedBodies());
    act(() => {
      layOut(listPx, readoutPx);
    });
    const tree = screen.getByRole("tree", { name: "Bodies" });
    const rows = within(tree).getAllByRole("treeitem");
    act(() => {
      tree.focus();
    });

    await user.keyboard("{End}{Enter}");

    expect(tree).toHaveFocus();
    expect(rows.at(-1)).toHaveAttribute("aria-selected", "true");
    // The last row's bottom edge meets the list's, or the list holds every row.
    expect(tree.scrollTop).toBe(Math.max(0, rows.length * ROW_PX - listPx));
  });

  it("returns the readings to their top when another body is selected", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await selectRow(user, /\/768/);
    await answerDetail(socket, earthDetail());
    act(() => {
      layOut(listPx, readoutPx);
    });
    const region = readoutRegion();
    act(() => {
      region.scrollTop = 120;
      fireEvent.scroll(region);
    });

    await selectRow(user, /\/1280/);

    expect(region.scrollTop).toBe(0);
    expect(readingsPosition()).toMatch(/^1-/);
  });
});
