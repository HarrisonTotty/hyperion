import { PROTOCOL_VERSION } from "@hyperion/protocol";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, type Mock, vi } from "vitest";

import { App } from "./App";
import { FakeWebSocket } from "./test/FakeWebSocket";
import { aDensityMap, anOpenedUniverse, aUniverseList } from "./test/galaxyFixtures";
import {
  SCENE_TIDAL_RADIUS_M,
  sceneClock,
  shipInSpace,
  shipInSystem,
  sliceSceneSystem,
} from "./test/sceneFixture";
import { stubCanvas } from "./test/RecordingContext2D";
import { stubHyperionApi, TEST_GRAPHICS, TEST_SERVER_URL } from "./test/stubHyperionApi";
import type { ViewsCheckApi } from "../../preload/api";

/** Plays the server's side, letting the outcomes it settles reach React. */
async function answer(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

function chartCentreMarks(): HTMLElement[] {
  return screen.queryAllByRole("img", { name: "Chart centre" });
}

/** The header strip's mode banner, or `null` while none stands. */
function modeBanner(): HTMLElement | null {
  return screen.queryByRole("status", { name: "Mode" });
}

/** A views-check launch's functions, each a mock. */
function viewsCheckApi(): ViewsCheckApi & { readonly end: Mock<ViewsCheckApi["end"]> } {
  return {
    launch: { setting: "high", smoke: true, out: null },
    startPhase: () => Promise.resolve(),
    endPhase: () => Promise.resolve(),
    askRightWayUp: () => Promise.resolve(),
    writeResults: () => Promise.resolve({ json: "a", markdown: "b" }),
    end: vi.fn<ViewsCheckApi["end"]>(() => Promise.resolve()),
  };
}

describe("App", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("opens on the link display inside the console frame", () => {
    render(<App />);

    expect(screen.getByRole("heading", { level: 1, name: "Link" })).toBeInTheDocument();
    const navigation = screen.getByRole("navigation", { name: "Displays" });
    expect(within(navigation).getByRole("button", { name: "F1 Link" })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("links to the server the client was launched to link to", () => {
    stubHyperionApi("ws://10.0.0.5:9100/ws");

    render(<App />);

    expect(FakeWebSocket.latest().url).toBe("ws://10.0.0.5:9100/ws");
    expect(screen.getByText("ws://10.0.0.5:9100/ws")).toBeInTheDocument();
  });

  it("shows the graphics panel on the link display, without WebGPU in jsdom", async () => {
    render(<App />);

    const panel = screen.getByRole("region", { name: "Graphics" });
    expect(within(panel).getByText("VULKAN")).toBeInTheDocument();
    expect(await within(panel).findByText("GRAPHICS NOT AVAILABLE: no WebGPU")).toBeInTheDocument();
  });

  it("opens a views-check launch on VIEW", () => {
    stubHyperionApi(TEST_SERVER_URL, TEST_GRAPHICS, undefined, viewsCheckApi());
    render(<App />);

    const navigation = screen.getByRole("navigation", { name: "Displays" });
    expect(within(navigation).getByRole("button", { name: "F4 View" })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("fails a views-check run whose VIEW cannot be drawn", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const api = viewsCheckApi();
    stubHyperionApi(TEST_SERVER_URL, TEST_GRAPHICS, undefined, api);
    render(<App />);

    // jsdom has no WebGPU, so the script's first wait runs out.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(61_000);
    });
    expect(api.end).toHaveBeenCalledWith({
      status: "fail",
      reason: expect.stringContaining("VIEW did not show its primary view"),
    });
  });

  it("shows a safe launch in the graphics panel and the header strip", () => {
    stubHyperionApi(TEST_SERVER_URL, { ...TEST_GRAPHICS, launchMode: "safe" });
    render(<App />);

    const panel = screen.getByRole("region", { name: "Graphics" });
    expect(within(panel).getByText("SAFE MODE")).toBeInTheDocument();
    expect(
      within(screen.getByRole("banner")).getByRole("status", { name: "Graphics mode" }),
    ).toHaveTextContent("GRAPHICS SAFE MODE");
  });

  it.each([
    { setting: "high", words: "HIGH" },
    { setting: "low", words: "LOW" },
  ] as const)(
    "shows the launch's quality setting $setting in the graphics panel as $words (R07.T17)",
    ({ setting, words }) => {
      stubHyperionApi(TEST_SERVER_URL, TEST_GRAPHICS, undefined, undefined, setting);
      render(<App />);

      const panel = screen.getByRole("region", { name: "Graphics" });
      const term = within(panel).getByText("Quality", { selector: "dt" });
      expect(term.nextElementSibling).toHaveTextContent(words);
    },
  );

  it("shows the link being established on launch", () => {
    render(<App />);

    expect(screen.getByRole("status", { name: "Server link" })).toHaveTextContent(
      "ESTABLISHING LINK",
    );
  });

  it("greets the server and reports a nominal link once welcomed", () => {
    render(<App />);
    const socket = FakeWebSocket.latest();

    act(() => {
      socket.serverOpens();
    });
    expect(socket.sent).toEqual([{ type: "hello", client_version: __APP_VERSION__ }]);

    act(() => {
      socket.serverSends({
        type: "welcome",
        server_version: "9.9.9",
        protocol_version: PROTOCOL_VERSION,
        generator_version: 2,
      });
    });
    expect(screen.getByRole("status", { name: "Server link" })).toHaveTextContent("LINK NOMINAL");
    expect(screen.getByText("9.9.9")).toBeInTheDocument();
  });

  it("reports an incompatible link when the server speaks another protocol version", () => {
    render(<App />);
    const socket = FakeWebSocket.latest();

    act(() => {
      socket.serverOpens();
      socket.serverSends({
        type: "welcome",
        server_version: "0.0.9",
        protocol_version: 1,
        generator_version: 1,
      });
    });

    expect(screen.getByRole("status", { name: "Server link" })).toHaveTextContent(
      "LINK INCOMPATIBLE",
    );
  });

  it("reports loss of carrier when the socket closes", () => {
    render(<App />);
    const socket = FakeWebSocket.latest();

    act(() => {
      socket.close();
    });

    expect(screen.getByRole("status", { name: "Server link" })).toHaveTextContent("NO CARRIER");
  });

  it("switches to the galaxy display when its tab is clicked", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "F2 Galaxy" }));

    expect(screen.getByRole("heading", { level: 1, name: "Galaxy" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "F2 Galaxy" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(screen.getByRole("button", { name: "F1 Link" })).not.toHaveAttribute("aria-current");
  });

  it("switches display from the keyboard alone", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.tab();
    await user.tab();
    expect(screen.getByRole("button", { name: "F2 Galaxy" })).toHaveFocus();
    await user.keyboard("{Enter}");

    expect(screen.getByRole("heading", { level: 1, name: "Galaxy" })).toBeInTheDocument();
  });

  it("switches display with a function key", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.keyboard("{F2}");
    expect(screen.getByRole("heading", { level: 1, name: "Galaxy" })).toBeInTheDocument();
    await user.keyboard("{F1}");
    expect(screen.getByRole("heading", { level: 1, name: "Link" })).toBeInTheDocument();
  });

  it("switches to the view display on F4", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.keyboard("{F4}");
    expect(screen.getByRole("heading", { level: 1, name: "View" })).toBeInTheDocument();
  });

  describe("the TRAINING banner, which follows what VIEW draws", () => {
    /** Opens a universe on GALAXY, then shows VIEW, whose scene subscription goes out. */
    async function viewOfAnOpenUniverse(
      user: ReturnType<typeof userEvent.setup>,
    ): Promise<FakeWebSocket> {
      const socket = FakeWebSocket.latest();
      act(() => {
        socket.serverWelcomes();
      });
      await user.keyboard("{F2}");
      await answer(() => {
        socket.serverAnswers("list_universes", () => aUniverseList());
      });
      await user.click(screen.getByRole("button", { name: "Open universe SURVEY 1" }));
      await answer(() => {
        socket.serverAnswers("open_universe", () => anOpenedUniverse());
      });
      await user.keyboard("{F4}");
      return socket;
    }

    /** Answers the scene subscription with the ship in the slice's system. */
    async function sceneInSystem(socket: FakeWebSocket): Promise<void> {
      await answer(() => {
        socket.serverAnswers("subscribe", () => ({
          kind: "subscribe",
          subscription: 7,
          state: {
            topic: "scene",
            sequence: 0,
            clock: sceneClock(3_000),
            ship: shipInSystem(3_000),
            system: sliceSceneSystem(),
            tidal_radius_m: SCENE_TIDAL_RADIUS_M,
            craft: [],
          },
        }));
      });
    }

    it("shows TRAINING while the view draws a kept scene", async () => {
      const user = userEvent.setup();
      render(<App />);

      await user.keyboard("{F4}");
      expect(modeBanner()).toHaveTextContent("TRAINING");
    });

    it("clears TRAINING once the server's scene is selected and drawn", async () => {
      const user = userEvent.setup();
      render(<App />);
      const socket = await viewOfAnOpenUniverse(user);
      await user.click(screen.getByRole("button", { name: "PRECISION TEST" }));
      expect(modeBanner()).toHaveTextContent("TRAINING");

      await user.click(screen.getByRole("button", { name: "SERVER" }));
      await sceneInSystem(socket);

      expect(modeBanner()).not.toBeInTheDocument();
    });

    it("restores TRAINING when the server's scene is lost and a kept scene stands in", async () => {
      const user = userEvent.setup();
      render(<App />);
      const socket = await viewOfAnOpenUniverse(user);
      await sceneInSystem(socket);
      expect(modeBanner()).not.toBeInTheDocument();

      // The ship leaves the system: the server's scene has nothing to draw.
      await answer(() => {
        socket.serverNotifies(7, {
          topic: "scene",
          sequence: 1,
          clock: sceneClock(3_100),
          ship: shipInSpace(3_100),
          arrival: { type: "no_system" },
          bodies: [],
        });
      });

      expect(modeBanner()).toHaveTextContent("TRAINING");
      expect(screen.getByText("SCENE NOT AVAILABLE: the ship is in no system")).toBeInTheDocument();
    });
  });

  it("hands the focus to the shown display's tab when a function key hides the one that had it", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.keyboard("{F2}");
    act(() => {
      screen.getByRole("button", { name: "Universe" }).focus();
    });

    await user.keyboard("{F1}");

    // A hidden element keeps the focus in jsdom, and Chromium takes it away to the page's body.
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "F1 Link" }));
  });

  it("hides the inactive display from the accessibility tree", async () => {
    const user = userEvent.setup();
    render(<App />);

    expect(screen.queryByRole("heading", { level: 2, name: "Universe" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "F2 Galaxy" }));

    expect(screen.getByRole("heading", { level: 2, name: "Universe" })).toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { level: 2, name: "Server Link" }),
    ).not.toBeInTheDocument();
  });

  it("keeps a display it leaves mounted, out of sight, so that its state survives", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "F2 Galaxy" }));

    const linkTitle = screen.getByRole("heading", { level: 2, name: "Server Link", hidden: true });
    expect(linkTitle).not.toBeVisible();
  });

  it("keeps the link display and the link itself across a visit to another display", async () => {
    const user = userEvent.setup();
    render(<App />);
    const socket = FakeWebSocket.latest();
    act(() => {
      socket.serverOpens();
      socket.serverSends({
        type: "welcome",
        server_version: "9.9.9",
        protocol_version: PROTOCOL_VERSION,
        generator_version: 2,
      });
    });

    await user.click(screen.getByRole("button", { name: "F2 Galaxy" }));
    await user.click(screen.getByRole("button", { name: "F1 Link" }));

    expect(screen.getByRole("heading", { level: 2, name: "Server Link" })).toBeInTheDocument();
    expect(screen.getByText("9.9.9")).toBeInTheDocument();
    expect(FakeWebSocket.instances).toHaveLength(1);
  });

  describe("the key C, which centres the chart", () => {
    /** Opens a universe on GALAXY with both maps drawn, the map page laid out 792 × 488 px. */
    async function galaxyWithMaps(user: ReturnType<typeof userEvent.setup>): Promise<void> {
      stubCanvas();
      vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
        DOMRect.fromRect({ x: 0, y: 0, width: 792, height: 488 }),
      );
      const socket = FakeWebSocket.latest();
      act(() => {
        socket.serverWelcomes();
      });
      await user.keyboard("{F2}");
      await answer(() => {
        socket.serverAnswers("list_universes", () => aUniverseList());
      });
      await user.click(screen.getByRole("button", { name: "Open universe SURVEY 1" }));
      await answer(() => {
        socket.serverAnswers("open_universe", () => anOpenedUniverse());
      });
      await answer(() => {
        for (let answered = 0; answered < 2; answered += 1) {
          socket.serverAnswers("density_map", ({ view }) => {
            const heightPx = view === "face_on" ? 8 : 4;
            return aDensityMap({
              codes: Array<number>(8 * heightPx).fill(1),
              widthPx: 8,
              heightPx,
              view,
            });
          });
        }
      });
    }

    it("centres the chart while GALAXY is shown", async () => {
      const user = userEvent.setup();
      render(<App />);
      await galaxyWithMaps(user);
      await user.click(screen.getByRole("tab", { name: "GALAXY MAP" }));

      await user.keyboard("c");

      // C shows the chart it centred, so the map is chosen again to read its marks.
      await user.click(screen.getByRole("tab", { name: "GALAXY MAP" }));
      expect(chartCentreMarks()).toHaveLength(2);
    });

    it("does nothing while LINK is shown", async () => {
      const user = userEvent.setup();
      render(<App />);
      await galaxyWithMaps(user);
      await user.click(screen.getByRole("tab", { name: "GALAXY MAP" }));
      await user.keyboard("{F1}");

      await user.keyboard("c");
      await user.keyboard("{F2}");

      expect(screen.getByRole("heading", { level: 1, name: "Galaxy" })).toBeInTheDocument();
      expect(chartCentreMarks()).toEqual([]);
    });
  });
});
