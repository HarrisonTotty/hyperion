/**
 * The `GALAXY MAP` page's feature overlay (plan 09, P09.T41.b): its pending state, its toggles and
 * what the page says of the kinds the server does not generate.
 */
import type { RequestOf, ResponseFor } from "@hyperion/protocol";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { UniverseProvider } from "../../components/UniverseProvider";
import type { CentreLy } from "../../lib/galaxy/model";
import { aGalaxyFeatures, BRIGHT_SHELL } from "../../test/featureFixtures";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { aDensityMap, anOpenedUniverse, aUniverseList } from "../../test/galaxyFixtures";
import { stubCanvas } from "../../test/RecordingContext2D";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { statusLine } from "../../test/statusLine";
import { GalaxyMapPanel } from "./GalaxyMapPanel";
import { UniversePanel } from "./UniversePanel";

/** Plays the server's side, letting the outcomes it settles reach React. */
async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

/** A map for the request: 8 × 8 face-on and 8 × 4 edge-on, spanning the M1 extents. */
function mapFor(body: RequestOf<"density_map">): ResponseFor<"density_map"> {
  const heightPx = body.view === "face_on" ? 8 : 4;
  return aDensityMap({
    codes: Array.from({ length: 8 * heightPx }, (_, pixel) => pixel),
    widthPx: 8,
    heightPx,
    view: body.view,
    population: body.population,
    floorLog10PerLy2: body.view === "face_on" ? -4 : -6,
    ceilingLog10PerLy2: 1,
  });
}

/** Holds the cursor for the page, as the display does. */
function MapPage() {
  const [cursorLy, setCursorLy] = useState<CentreLy>([0, 0, 0]);
  return <GalaxyMapPanel cursorLy={cursorLy} onCursor={setCursorLy} centreLy={null} shown />;
}

/** Renders the page at 1920 × 1080, stacked, beside the panel that opens universes for it. */
async function renderPage() {
  const user = userEvent.setup();
  stubCanvas();
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: 1_036, height: 880 }),
  );
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
  await user.click(screen.getByRole("button", { name: "Open universe SURVEY 1" }));
  await server(() => {
    socket.serverAnswers("open_universe", () => anOpenedUniverse());
  });
  await server(() => {
    socket.serverAnswers("density_map", mapFor);
    socket.serverAnswers("density_map", mapFor);
  });
  return { user, socket };
}

/** Each view's overlay, face-on then edge-on, by its accessible name. */
function overlays(): HTMLElement[] {
  return screen.getAllByRole("img", { name: /^Features: / });
}

describe("GalaxyMapPanel's feature overlay (plan 09, P09.T41.b)", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  it("asks once for both views' features, at the epoch, and says FEATURES PENDING meanwhile", async () => {
    const { socket } = await renderPage();

    expect(socket.requestsOfKind("galaxy_features").map(({ body }) => body)).toEqual([
      {
        kind: "galaxy_features",
        universe: anOpenedUniverse().id,
        kinds: ["globular", "galactic_centre", "bright_shell"],
        time: { seconds: 0, nanos: 0 },
      },
    ]);
    expect(screen.getByText(statusLine("FEATURES PENDING"))).toBeInTheDocument();
    expect(screen.queryByRole("img", { name: /^Features: / })).not.toBeInTheDocument();
  });

  it("draws the features on both views once they arrive, and says which kind is not modelled", async () => {
    const { socket } = await renderPage();

    await server(() => {
      socket.serverAnswers("galaxy_features", () => aGalaxyFeatures());
    });

    expect(overlays().map((overlay) => overlay.getAttribute("aria-label"))).toEqual([
      "Features: 1 globular cluster, the galactic centre",
      "Features: 1 globular cluster, the galactic centre",
    ]);
    expect(screen.queryByText(statusLine("FEATURES PENDING"))).not.toBeInTheDocument();
    expect(screen.getByText(statusLine("SNR: NOT YET MODELLED"))).toBeInTheDocument();
  });

  it("takes a kind off both views with its toggle, and puts it back", async () => {
    const { user, socket } = await renderPage();
    await server(() => {
      socket.serverAnswers("galaxy_features", () => aGalaxyFeatures({ shells: [BRIGHT_SHELL] }));
    });
    const globulars = screen.getByRole("checkbox", { name: "GC" });

    expect(globulars).toBeChecked();
    await user.click(globulars);

    expect(globulars).not.toBeChecked();
    for (const overlay of overlays()) {
      expect(overlay).toHaveAccessibleName(
        "Features: the galactic centre, 1 bright supernova remnant shell",
      );
    }
    await user.click(globulars);
    for (const overlay of overlays()) {
      expect(overlay).toHaveAccessibleName(
        /^Features: 1 globular cluster, the galactic centre, 1 bright/u,
      );
    }
  });

  it("stands the toggles once, in the face-on view's words, every kind on", async () => {
    await renderPage();

    const toggles = screen.getAllByRole("group", { name: "FEATURES" });
    expect(toggles).toHaveLength(1);
    expect(
      within(toggles[0] ?? document.body)
        .getAllByRole("checkbox")
        .map((box) => [box.getAttribute("checked") !== null, box.closest("label")?.textContent]),
    ).toEqual([
      [true, "GC"],
      [true, "CENTRE"],
      [true, "SNR"],
    ]);
  });

  it("says nothing of a kind not modelled while it is not shown", async () => {
    const { user, socket } = await renderPage();
    await server(() => {
      socket.serverAnswers("galaxy_features", () => aGalaxyFeatures());
    });

    await user.click(screen.getByRole("checkbox", { name: "SNR" }));

    expect(screen.queryByText(statusLine("SNR: NOT YET MODELLED"))).not.toBeInTheDocument();
  });

  it("reports a failure with RETRY, which asks again from FEATURES PENDING", async () => {
    const { user, socket } = await renderPage();
    const [request] = socket.requestsOfKind("galaxy_features");
    await server(() => {
      socket.serverRejects(request?.id ?? 0, {
        code: "internal",
        message: "the features failed",
        field: null,
      });
    });

    expect(
      screen.getByText(statusLine("FEATURES REJECTED: the features failed")),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "RETRY" }));

    expect(socket.requestsOfKind("galaxy_features")).toHaveLength(2);
    expect(screen.getByText(statusLine("FEATURES PENDING"))).toBeInTheDocument();
  });

  it("reads an answer it cannot draw as FEATURE DATA INVALID, and draws none of it", async () => {
    const { socket } = await renderPage();

    await server(() => {
      socket.serverAnswers("galaxy_features", () =>
        aGalaxyFeatures({
          shells: [{ ...BRIGHT_SHELL, shell: { ...BRIGHT_SHELL.shell, radius_ly: -1 } }],
        }),
      );
    });

    expect(
      screen.getByText(statusLine("FEATURE DATA INVALID: feature unusable")),
    ).toBeInTheDocument();
    expect(screen.queryByRole("img", { name: /^Features: / })).not.toBeInTheDocument();
  });
});
