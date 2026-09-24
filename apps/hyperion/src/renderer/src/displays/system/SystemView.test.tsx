/**
 * The `SYSTEM` display's shell, orbit map, bodies and data states over a fake socket (plan 14,
 * P14.T41–T43, hosts only).
 */
import type { SystemSummaryDto } from "@hyperion/protocol";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { layoutHierarchy } from "../../lib/system/hierarchy";
import { toSystemModel } from "../../lib/system/wire";
import { fitPxPerUnit, PRESETS, project, viewBasis } from "../../spatial/camera";
import { buildDrawList } from "../../spatial/drawList";
import type { Vec3 } from "../../spatial/vec3";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { stubCanvas } from "../../test/RecordingContext2D";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import {
  A_CENTURY,
  aBlackHole,
  aNeutronStar,
  aNotYetBornSummary,
  anUnknownSystemError,
  anUnsupportedError,
  aSingleStarSummary,
  aSummaryResponse,
  aSystemSummary,
  aSystemTarget,
  BANDS,
  PIN_SYSTEM,
} from "../../test/systemFixtures";
import { fitRadiiAu, orbitPlane, orbitScene } from "./orbitMap";
import { SystemView } from "./SystemView";

/** Every element laid out 400 × 300, so that the map's stage and canvas are that size. */
const WIDTH_PX = 400;
const HEIGHT_PX = 300;
const REM_PX = 16;

const STAR_0 = `${PIN_SYSTEM}.0000`;
const STAR_1 = `${PIN_SYSTEM}.0001`;

/** Plays the server's side, letting the outcomes it settles reach React. */
async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

function renderView() {
  const user = userEvent.setup();
  const recorder = stubCanvas();
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: WIDTH_PX, height: HEIGHT_PX }),
  );
  // The body list's visible height, which its position readout reads.
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
  return { user, socket, recorder };
}

async function answer(socket: FakeWebSocket, summary: SystemSummaryDto = aSystemSummary()) {
  await server(() => {
    socket.serverAnswers("system_summary", () => aSummaryResponse(summary));
  });
}

function mapPanel(): HTMLElement {
  return screen.getByRole("region", { name: /^Orbit map/ });
}

function readout(): HTMLElement {
  return screen.getByRole("status", { name: "Selected body" });
}

/** The value a readout label reads, as its `dd` holds it. */
function reading(label: string): string {
  const term = within(readout())
    .getAllByRole("term")
    .find((candidate) => candidate.textContent === label);
  return term?.nextElementSibling?.textContent ?? "";
}

/** Where each of the pinned binary's stars is drawn on the 400 × 300 canvas at the opening camera. */
function anchorsOfPinnedBinary() {
  const result = toSystemModel(aSystemSummary(), aSystemTarget().designation);
  if (result.kind !== "ok") {
    throw new Error(result.fault);
  }
  const { hosts, hierarchy } = result.model;
  const layout = layoutHierarchy(hierarchy, hosts);
  const plane = orbitPlane(layout, aSystemTarget().positionLy);
  const fitRadiusAu = fitRadiiAu(layout, hosts).all;
  const viewport = { widthPx: WIDTH_PX, heightPx: HEIGHT_PX, remPx: REM_PX };
  const scene = orbitScene({
    hosts,
    layout,
    plane,
    time: A_CENTURY,
    selectedId: STAR_0,
    bands: BANDS,
    fitRadiusAu,
  });
  const camera = {
    ...PRESETS.oblique,
    pxPerUnit: fitPxPerUnit(fitRadiusAu, viewport, 2 * REM_PX),
  };
  const basis = viewBasis(scene.frame, camera);
  return {
    drawList: buildDrawList(scene, camera, viewport),
    scene,
    toScreen: (point: Vec3) => project(point, basis, camera, viewport),
  };
}

beforeEach(() => {
  FakeWebSocket.instances = [];
  vi.stubGlobal("WebSocket", FakeWebSocket);
});

describe("SystemView's requests", () => {
  it("asks once for its system at the time it was opened at", () => {
    const { socket } = renderView();

    expect(socket.requestsOfKind("system_summary").map((request) => request.body)).toEqual([
      {
        kind: "system_summary",
        universe: aSystemTarget().universe,
        system: PIN_SYSTEM,
        time: A_CENTURY,
      },
    ]);
  });

  it("says PENDING in words, and draws nothing, before the answer", () => {
    renderView();

    expect(within(mapPanel()).getByText("PENDING")).toBeInTheDocument();
    expect(screen.queryByRole("application", { name: "Orbit map" })).not.toBeInTheDocument();
  });

  it("reads a server that does not serve the kind yet as a fault, with RETRY", async () => {
    const { user, socket } = renderView();
    const [request] = socket.requestsOfKind("system_summary");
    await server(() => {
      socket.serverRejects(request?.id ?? -1, anUnsupportedError());
    });

    const status = within(mapPanel()).getByText("REJECTED: system_summary is not served yet");
    expect(status).toHaveClass("request-status__text--fault");

    await user.click(within(mapPanel()).getByRole("button", { name: "RETRY" }));

    expect(socket.requestsOfKind("system_summary")).toHaveLength(2);
  });

  it("reads an unknown system as a refusal, not a fault", async () => {
    const { socket } = renderView();
    const [request] = socket.requestsOfKind("system_summary");
    await server(() => {
      socket.serverRejects(request?.id ?? -1, anUnknownSystemError());
    });

    const status = within(mapPanel()).getByText(
      "REJECTED: no system 0200080020000000 in this universe",
    );
    expect(status).not.toHaveClass("request-status__text--fault");
  });

  it("says NO CARRIER while the link is down before any answer", async () => {
    const { socket } = renderView();
    await server(() => {
      socket.close();
    });

    expect(within(mapPanel()).getByText("NO CARRIER")).toBeInTheDocument();
  });

  it("keeps the answer on show when the link is lost, marked stale", async () => {
    const { socket } = renderView();
    await answer(socket);
    await server(() => {
      socket.close();
    });

    expect(screen.getByRole("application", { name: "Orbit map, stale" })).toBeInTheDocument();
    expect(within(mapPanel()).getByRole("heading", { name: "Orbit map stale" })).toBeVisible();
    expect(reading("CLASS")).toBe("DA9.2");
  });

  it("reads a system not yet born as NOT YET FORMED, drawing nothing", async () => {
    const { socket } = renderView();
    await answer(socket, aNotYetBornSummary());

    expect(within(mapPanel()).getByText("NOT YET FORMED")).toBeInTheDocument();
    expect(screen.queryByRole("application", { name: "Orbit map" })).not.toBeInTheDocument();
    expect(screen.queryByRole("tree", { name: "Bodies" })).not.toBeInTheDocument();
  });

  it("reads an answer it cannot place as SYSTEM DATA INVALID, with RETRY", async () => {
    const { socket } = renderView();
    await answer(socket, aSystemSummary({ hierarchy: { nodes: [] } }));

    expect(
      within(mapPanel()).getByText("SYSTEM DATA INVALID: hierarchy malformed"),
    ).toBeInTheDocument();
    expect(within(mapPanel()).getByRole("button", { name: "RETRY" })).toBeInTheDocument();
  });

  it("says NO BODIES when nothing the system holds can be drawn", async () => {
    const { socket } = renderView();
    await answer(
      socket,
      aSingleStarSummary(aBlackHole({ kind: "no_remnant", remnant: { type: "no_remnant" } })),
    );

    expect(within(mapPanel()).getByText("NO BODIES")).toBeInTheDocument();
  });
});

describe("SystemView's orbit map", () => {
  it("draws the system in the SYSTEM BARYCENTRIC frame at the display time", async () => {
    const { socket } = renderView();
    await answer(socket);

    const map = mapPanel();
    expect(within(map).getByRole("application", { name: "Orbit map" })).toBeInTheDocument();
    expect(within(map).getByText("SYSTEM BARYCENTRIC")).toBeInTheDocument();
    expect(within(map).getAllByText("+100 yr 000/00:00:00").length).toBeGreaterThan(0);
    expect(within(map).getByText("DISPLAY TIME UT")).toBeInTheDocument();
  });

  it("says BODIES NOT TO SCALE once, and no second not-to-scale label", async () => {
    const { socket } = renderView();
    await answer(socket);

    const legend = screen.getByRole("group", { name: "Orbit map legend" });
    expect(within(legend).getByText("BODIES NOT TO SCALE")).toBeInTheDocument();
    expect(screen.queryByText("SYMBOLS NOT TO SCALE")).not.toBeInTheDocument();
    expect(within(legend).getByText("FILLED ABOVE SYSTEM PLANE")).toBeInTheDocument();
    expect(within(legend).getByText("WHITE DWARF")).toBeInTheDocument();
  });

  it("names what this generator version does not model, so the space is not read as empty", async () => {
    const { socket } = renderView();
    await answer(socket);

    expect(
      within(mapPanel()).getByText(
        "PLANETS, MOONS, RINGS, BELTS AND COMETARY HALO: NOT YET MODELLED",
      ),
    ).toBeInTheDocument();
  });

  it("reads its scale bar in AU at ALL and fits the primary's orbit at INNER", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    const map = mapPanel();
    const all = within(map).getByRole("button", { name: "A ALL" });
    const inner = within(map).getByRole("button", { name: "I INNER" });

    expect(all).toHaveAttribute("aria-pressed", "true");
    const allBar = within(map)
      .getByRole("img", { name: /^Scale bar/ })
      .getAttribute("aria-label");

    await user.keyboard("i");

    expect(inner).toHaveAttribute("aria-pressed", "true");
    expect(all).toHaveAttribute("aria-pressed", "false");
    expect(within(map).getByRole("img", { name: /^Scale bar/ })).not.toHaveAttribute(
      "aria-label",
      allBar ?? "",
    );
    expect(allBar).toMatch(/ AU$/);
  });

  it("selects the star a click lands on, and nothing for a click on an orbit", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    const canvas = screen.getByRole("application", { name: "Orbit map" });
    const { drawList, scene, toScreen } = anchorsOfPinnedBinary();
    const companion = drawList.anchors.find((anchor) => anchor.id === STAR_1);
    const path = scene.paths?.find((candidate) => candidate.id === STAR_1);
    if (companion === undefined || path === undefined) {
      throw new Error("the pinned binary draws its companion and its path");
    }
    // The point of the companion's orbit farthest from both stars on the screen, where no star is.
    const missing = (point: Vec3) =>
      Math.min(
        ...drawList.anchors.map((anchor) => {
          const at = toScreen(point);
          return Math.hypot(at.xPx - anchor.xPx, at.yPx - anchor.yPx);
        }),
      );
    const onOrbit = path.points.reduce((best, point) =>
      missing(point) > missing(best) ? point : best,
    );
    expect(missing(onOrbit)).toBeGreaterThan(2 * REM_PX);

    await user.pointer({
      keys: "[MouseLeft]",
      target: canvas,
      coords: { clientX: companion.xPx + 3, clientY: companion.yPx - 2 },
    });

    expect(reading("DESIG")).toBe("H7K 4C0RFZ D-7 /1");
    // The selection made on the map is the tree's active row too.
    const tree = screen.getByRole("tree", { name: "Bodies" });
    expect(tree).toHaveAttribute(
      "aria-activedescendant",
      within(tree).getAllByRole("treeitem")[1]?.id ?? "",
    );

    const farPoint = toScreen(onOrbit);
    await user.pointer({
      keys: "[MouseLeft]",
      target: canvas,
      coords: { clientX: farPoint.xPx, clientY: farPoint.yPx },
    });

    expect(reading("DESIG")).toBe("H7K 4C0RFZ D-7 /1");
  });
});

describe("SystemView's bodies", () => {
  it("lists the hosts in body-index order with the list's position and total", async () => {
    const { socket } = renderView();
    await answer(socket);

    const tree = screen.getByRole("tree", { name: "Bodies" });
    expect(
      within(tree)
        .getAllByRole("treeitem")
        .map((item) => item.getAttribute("aria-label")),
    ).toEqual(["H7K 4C0RFZ D-7 /0, WHITE DWARF", "H7K 4C0RFZ D-7 /1, DWARF, SMA 23.5 AU"]);
    expect(screen.getByText("1-2 of 2")).toBeInTheDocument();
  });

  it("opens with the primary selected, and moves with the arrows and selects with Enter", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    const tree = screen.getByRole("tree", { name: "Bodies" });

    expect(reading("DESIG")).toBe("H7K 4C0RFZ D-7 /0");

    act(() => {
      tree.focus();
    });
    await user.keyboard("{ArrowDown}");

    expect(reading("DESIG")).toBe("H7K 4C0RFZ D-7 /0");

    await user.keyboard("{Enter}");

    expect(reading("DESIG")).toBe("H7K 4C0RFZ D-7 /1");
    expect(within(tree).getAllByRole("treeitem")[1]).toHaveAttribute("aria-selected", "true");
  });

  it("reads a white dwarf's star in its units, its remnant and its cooling age", async () => {
    const { socket } = renderView();
    await answer(socket);

    expect(reading("KIND")).toBe("WHITE DWARF");
    expect(reading("PHASE")).toBe("CARBON-OXYGEN WHITE DWARF");
    expect(reading("INIT MASS")).toBe("2.50 M");
    expect(reading("MASS")).toBe("0.69 M");
    expect(reading("LUM")).toBe("1.07E-4 L");
    expect(reading("RADIUS")).toBe("0.0115 R");
    expect(reading("T EFF")).toBe("5480 K");
    expect(reading("REMNANT")).toBe("WHITE DWARF");
    expect(reading("COOLING AGE")).toBe("3.95 Gyr");
    expect(within(readout()).getAllByRole("img", { name: "solar masses" })).toHaveLength(2);
    expect(within(readout()).getByRole("img", { name: "solar luminosities" })).toBeInTheDocument();
    expect(within(readout()).getByRole("img", { name: "solar radii" })).toBeInTheDocument();
  });

  it("shows what this generator version does not model as the em dash", async () => {
    const { socket } = renderView();
    await answer(socket);

    for (const label of ["ROTATION", "ACTIVITY", "VARIABILITY", "KICK"]) {
      expect(reading(label)).toBe("—");
    }
  });

  it("reads a neutron star's radius in km", async () => {
    const { socket } = renderView();
    await answer(socket, aSingleStarSummary(aNeutronStar()));

    expect(reading("RADIUS")).toBe("12.2 km");
    expect(reading("PULSAR PERIOD")).toBe("—");
  });

  it("reads a black hole's light as the em dash with the reason", async () => {
    const { socket } = renderView();
    await answer(socket, aSingleStarSummary(aBlackHole()));

    expect(reading("LUM")).toBe("—NO LIGHT");
    expect(reading("T EFF")).toBe("—NO LIGHT");
    expect(reading("RADIUS")).toBe("36.9 km");
    expect(reading("SPIN")).toBe("—");
  });

  it("names the system with its age and metallicity", async () => {
    const { socket } = renderView();
    await answer(socket);
    const panel = screen.getByRole("region", { name: /^Bodies/ });

    expect(within(panel).getByText("H7K 4C0RFZ D-7")).toBeInTheDocument();
    expect(within(panel).getByText("0200080020000000")).toBeInTheDocument();
    expect(within(panel).getByText("4.60")).toBeInTheDocument();
    expect(within(panel).getByText("-0.13")).toBeInTheDocument();
  });
});
