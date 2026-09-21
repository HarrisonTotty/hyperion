import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { UniverseProvider } from "../../components/UniverseProvider";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { anOpenedUniverse, aUniverseList } from "../../test/galaxyFixtures";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { GalaxyDisplay } from "./GalaxyDisplay";

const DATA_PANELS = ["Parameters", "Galaxy Map", "Local Chart"];

/** Plays the server's side, letting the outcomes it settles reach React. */
async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

function renderDisplay() {
  const user = userEvent.setup();
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
  return { user, socket };
}

describe("GalaxyDisplay", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  it("lays out the universe, parameters, galaxy map and local chart as named regions", () => {
    renderDisplay();

    for (const name of ["Universe", ...DATA_PANELS]) {
      const region = screen.getByRole("region", { name });
      expect(within(region).getByRole("heading", { level: 2, name })).toBeInTheDocument();
    }
  });

  it("says NO UNIVERSE OPEN in every data panel until a universe is open", () => {
    renderDisplay();

    for (const name of DATA_PANELS) {
      expect(
        within(screen.getByRole("region", { name })).getByText("NO UNIVERSE OPEN"),
      ).toBeInTheDocument();
    }
  });

  it("drops the empty state once a universe opens", async () => {
    const { user, socket } = renderDisplay();
    await server(() => {
      socket.serverAnswers("list_universes", () => aUniverseList());
    });

    await user.click(screen.getByRole("button", { name: "Open universe SURVEY 1" }));
    await server(() => {
      socket.serverAnswers("open_universe", () => anOpenedUniverse());
    });

    expect(screen.queryByText("NO UNIVERSE OPEN")).not.toBeInTheDocument();
  });
});
