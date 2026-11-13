/**
 * The `GALAXY MAP` page's stream overlay (plan 10, P10.T11): its pending state, the tracks it asks
 * for, its refresh of the global list while tracks are being computed, and its toggles.
 */
import type { RequestOf, ResponseFor } from "@hyperion/protocol";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { UniverseProvider } from "../../components/UniverseProvider";
import type { CentreLy } from "../../lib/galaxy/model";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { aDensityMap, anOpenedUniverse, aUniverse, aUniverseList } from "../../test/galaxyFixtures";
import {
  aCoreEntry,
  aGlobalFeatures,
  aStreamEntry,
  aStreamTrack,
} from "../../test/globalListFixtures";
import { stubCanvas } from "../../test/RecordingContext2D";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { statusLine } from "../../test/statusLine";
import { GalaxyMapPanel } from "./GalaxyMapPanel";
import { LIST_REFRESH_MS } from "./useGlobalList";
import { UniversePanel } from "./UniversePanel";

/** A second universe to open, of another seed. */
const SURVEY_2 = aUniverse({ id: "00000000000000b2", name: "SURVEY 2", seed: "000000000000beef" });

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

/** Renders the page with its maps drawn, a universe open, and its clock faked. */
async function renderPage() {
  const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime.bind(vi) });
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
    socket.serverAnswers("list_universes", () => aUniverseList([aUniverse(), SURVEY_2]));
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

/** A list of two streams, the heavier's track held, and a core. */
function twoStreams(secondReady = false): ResponseFor<"global_features"> {
  return aGlobalFeatures({
    entries: [
      aStreamEntry({ number: 0, mass_msun: 8_000, track: secondReady ? "ready" : "pending" }),
      aStreamEntry({ number: 1, mass_msun: 20_000, track: "ready" }),
      aCoreEntry(),
    ],
  });
}

/** Each view's stream overlay, face-on then edge-on, by its accessible name. */
function overlayNames(): Array<string | null> {
  return screen
    .getAllByRole("img", { name: /^Streams: / })
    .map((overlay) => overlay.getAttribute("aria-label"));
}

/** A list of `count` streams, every track held. */
function warmed(count: number): ResponseFor<"global_features"> {
  return aGlobalFeatures({
    entries: Array.from({ length: count }, (_, number) =>
      aStreamEntry({ number, mass_msun: 1_000 * (count - number), track: "ready" }),
    ),
  });
}

/** Stream `stream`'s track, a short run inside both views. */
function aTrackOf(stream: number): ResponseFor<"stream_track"> {
  return aStreamTrack(stream, [[1_000 * stream, 0, 0]], [[1_000 * stream + 500, 1_000, 0]]);
}

describe("GalaxyMapPanel's stream overlay (plan 10, P10.T11)", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("asks once for the global list, and says TRACKS PENDING until it arrives", async () => {
    const { socket } = await renderPage();

    expect(socket.requestsOfKind("global_features").map(({ body }) => body)).toEqual([
      { kind: "global_features", universe: anOpenedUniverse().id },
    ]);
    expect(screen.getByText(statusLine("TRACKS PENDING"))).toBeInTheDocument();
    expect(screen.queryByRole("img", { name: /^Streams: / })).not.toBeInTheDocument();
  });

  it("asks only for the tracks the server holds, and counts the streams not yet drawn", async () => {
    const { socket } = await renderPage();

    await server(() => {
      socket.serverAnswers("global_features", () => twoStreams());
    });

    expect(socket.requestsOfKind("stream_track").map(({ body }) => body.stream)).toEqual([1]);
    expect(screen.getByText(statusLine("TRACKS PENDING 2 OF 2"))).toBeInTheDocument();
    expect(overlayNames()).toEqual(["Streams: 1 dwarf core", "Streams: 1 dwarf core"]);
    await server(() => {
      socket.serverAnswers("stream_track", () =>
        aStreamTrack(1, [[20_000, 0, 0]], [[30_000, 1_000, 0]]),
      );
    });

    expect(screen.getByText(statusLine("TRACKS PENDING 1 OF 2"))).toBeInTheDocument();
    expect(overlayNames()).toEqual([
      "Streams: 1 stream track, 1 dwarf core",
      "Streams: 1 stream track, 1 dwarf core",
    ]);
  });

  it("asks for the list again while a track is pending, then for the track once it is held", async () => {
    const { socket } = await renderPage();
    await server(() => {
      socket.serverAnswers("global_features", () => twoStreams());
    });
    await server(() => {
      socket.serverAnswers("stream_track", () =>
        aStreamTrack(1, [[20_000, 0, 0]], [[30_000, 1_000, 0]]),
      );
    });

    await act(async () => {
      await vi.advanceTimersByTimeAsync(LIST_REFRESH_MS);
    });

    expect(socket.requestsOfKind("global_features")).toHaveLength(2);
    await server(() => {
      socket.serverAnswers("global_features", () => twoStreams(true));
    });
    expect(socket.requestsOfKind("stream_track").map(({ body }) => body.stream)).toEqual([1, 0]);
    await server(() => {
      socket.serverAnswers("stream_track", () =>
        aStreamTrack(0, [[-20_000, 0, 0]], [[-30_000, 1_000, 0]]),
      );
    });

    expect(screen.queryByText(statusLine(/^TRACKS PENDING/))).not.toBeInTheDocument();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(LIST_REFRESH_MS);
    });
    expect(socket.requestsOfKind("global_features")).toHaveLength(2);
  });

  it("takes the tracks off with STREAM and the cores with DWARF CORE, and their count with them", async () => {
    const { user, socket } = await renderPage();
    await server(() => {
      socket.serverAnswers("global_features", () => twoStreams());
    });
    await server(() => {
      socket.serverAnswers("stream_track", () =>
        aStreamTrack(1, [[20_000, 0, 0]], [[30_000, 1_000, 0]]),
      );
    });

    await user.click(screen.getByRole("checkbox", { name: "STREAM" }));

    expect(overlayNames()).toEqual(["Streams: 1 dwarf core", "Streams: 1 dwarf core"]);
    expect(screen.queryByText(statusLine(/^TRACKS PENDING/))).not.toBeInTheDocument();
    await user.click(screen.getByRole("checkbox", { name: "DWARF CORE" }));
    expect(overlayNames()).toEqual(["Streams: none drawn", "Streams: none drawn"]);
  });

  it("says why the list was refused, and asks again on RETRY", async () => {
    const { user, socket } = await renderPage();
    const [request] = socket.requestsOfKind("global_features");
    if (request === undefined) {
      throw new Error("the page asked for no list");
    }

    await server(() => {
      socket.serverRejects(request.id, {
        code: "internal",
        message: "the list could not be built",
        field: null,
      });
    });

    expect(
      screen.getByText(statusLine("TRACKS REJECTED: the list could not be built")),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "RETRY" }));
    expect(socket.requestsOfKind("global_features")).toHaveLength(2);
  });

  it("asks for at most four tracks at once, and for the next as each is answered", async () => {
    const { socket } = await renderPage();
    await server(() => {
      socket.serverAnswers("global_features", () => warmed(6));
    });

    expect(socket.requestsOfKind("stream_track").map(({ body }) => body.stream)).toEqual([
      0, 1, 2, 3,
    ]);
    await server(() => {
      socket.serverAnswers("stream_track", aTrackOf.bind(null, 3));
    });
    expect(socket.requestsOfKind("stream_track").map(({ body }) => body.stream)).toEqual([
      0, 1, 2, 3, 4,
    ]);
  });

  it("says a track that cannot be drawn is invalid, and clears it when RETRY brings a good one", async () => {
    const { user, socket } = await renderPage();
    await server(() => {
      socket.serverAnswers("global_features", () => warmed(1));
    });
    const bad = aTrackOf(0);
    if (bad.track.status !== "ready") {
      throw new Error("the fixture's track is ready");
    }
    const [first] = bad.track.leading;
    if (first === undefined) {
      throw new Error("the fixture's track has a leading point");
    }
    await server(() => {
      socket.serverAnswers("stream_track", () => ({
        ...bad,
        track: { ...bad.track, leading: [{ ...first, speed_km_s: Number.NaN }] },
      }));
    });

    expect(screen.getByText(statusLine("TRACK DATA INVALID: track unusable"))).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "RETRY" }));
    await server(() => {
      socket.serverAnswers("global_features", () => warmed(1));
    });
    await server(() => {
      socket.serverAnswers("stream_track", () => aTrackOf(0));
    });
    expect(screen.queryByText(statusLine(/^TRACK DATA INVALID/))).not.toBeInTheDocument();
    expect(overlayNames()).toEqual(["Streams: 1 stream track", "Streams: 1 stream track"]);
  });

  it("draws no core that cannot be drawn, and says so as the cores' fault", async () => {
    const { socket } = await renderPage();

    await server(() => {
      socket.serverAnswers("global_features", () =>
        aGlobalFeatures({
          entries: [aCoreEntry(), aCoreEntry({ number: 1, extent_ly: -1 })],
        }),
      );
    });

    expect(
      screen.getByText(statusLine("DWARF CORE DATA INVALID: core unusable")),
    ).toBeInTheDocument();
    expect(overlayNames()).toEqual(["Streams: 1 dwarf core", "Streams: 1 dwarf core"]);
  });

  it("says how the list stands for the cores alone while STREAM is off", async () => {
    const { user } = await renderPage();

    await user.click(screen.getByRole("checkbox", { name: "STREAM" }));

    expect(screen.getByText(statusLine("DWARF CORES PENDING"))).toBeInTheDocument();
  });

  it("drops the tracks and the cores of a universe no longer open", async () => {
    const { user, socket } = await renderPage();
    await server(() => {
      socket.serverAnswers("global_features", () => twoStreams());
    });
    expect(overlayNames()).toEqual(["Streams: 1 dwarf core", "Streams: 1 dwarf core"]);

    await user.click(screen.getByRole("button", { name: "Open universe SURVEY 2" }));
    await server(() => {
      socket.serverAnswers("open_universe", () => anOpenedUniverse(SURVEY_2));
    });

    expect(screen.queryByRole("img", { name: /^Streams: / })).not.toBeInTheDocument();
    expect(socket.requestsOfKind("global_features").at(-1)?.body.universe).toBe(SURVEY_2.id);
  });
});
