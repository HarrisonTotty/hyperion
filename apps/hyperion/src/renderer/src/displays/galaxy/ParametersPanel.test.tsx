import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { UniverseProvider } from "../../components/UniverseProvider";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import {
  anOpenedUniverse,
  aUniverse,
  aUniverseList,
  someGalaxyParameters,
} from "../../test/galaxyFixtures";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { ParametersPanel } from "./ParametersPanel";
import { UniversePanel } from "./UniversePanel";

/** A layout unit of the list, 1.5rem, at the 16px root font size jsdom reports. */
const UNIT_PX = 24;

const SURVEY_1 = aUniverse();
const SURVEY_2 = aUniverse({ id: "00000000000000b2", name: "SURVEY 2", seed: "000000000000beef" });

/** Plays the server's side, letting the outcomes it settles reach React. */
async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

/** Renders the panel beside the universe panel, which opens universes for it. */
async function renderPanel() {
  const user = userEvent.setup();
  render(
    <ServerLinkHarness>
      <UniverseProvider>
        <UniversePanel expanded onToggle={() => undefined} />
        <div role="tabpanel" aria-label="PARAMETERS">
          <ParametersPanel />
        </div>
      </UniverseProvider>
    </ServerLinkHarness>,
  );
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  await server(() => {
    socket.serverAnswers("list_universes", () => aUniverseList([SURVEY_1, SURVEY_2]));
  });
  return { user, socket };
}

/** Opens a listed universe from the universe panel, as the operator does. */
async function openFromList(
  user: ReturnType<typeof userEvent.setup>,
  socket: FakeWebSocket,
  universe: typeof SURVEY_1,
): Promise<void> {
  await user.click(screen.getByRole("button", { name: `Open universe ${universe.name}` }));
  await server(() => {
    socket.serverAnswers("open_universe", () => anOpenedUniverse(universe));
  });
}

/** The page, as the pages panel labels it. */
function panel(): HTMLElement {
  return screen.getByRole("tabpanel", { name: "PARAMETERS" });
}

describe("ParametersPanel", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  it("reads NO UNIVERSE OPEN and requests nothing before a universe is open", async () => {
    const { socket } = await renderPanel();

    expect(within(panel()).getByText("NO UNIVERSE OPEN")).toBeInTheDocument();
    expect(socket.requestsOfKind("galaxy_parameters")).toEqual([]);
  });

  it("requests the open universe's parameters and shows PENDING", async () => {
    const { user, socket } = await renderPanel();

    await openFromList(user, socket, SURVEY_1);

    expect(socket.requestsOfKind("galaxy_parameters").map(({ body }) => body)).toEqual([
      { kind: "galaxy_parameters", universe: SURVEY_1.id },
    ]);
    expect(within(panel()).getByRole("status")).toHaveTextContent("PENDING");
    expect(within(panel()).queryByText("NO UNIVERSE OPEN")).not.toBeInTheDocument();
  });

  it("reads each parameter with its value, unit and origin", async () => {
    const { user, socket } = await renderPanel();
    await openFromList(user, socket, SURVEY_1);

    await server(() => {
      socket.serverAnswers("galaxy_parameters", (body) => someGalaxyParameters(body.universe));
    });

    const stellarMass = within(panel()).getByRole("row", { name: /STELLAR MASS/ });
    expect(stellarMass).toHaveTextContent("5.20E10");
    expect(within(stellarMass).getByRole("img", { name: "solar masses" })).toBeInTheDocument();
    expect(stellarMass).toHaveTextContent("DRAWN");
    const patternSpeed = within(panel()).getByRole("row", { name: /PATTERN SPEED/ });
    expect(patternSpeed).toHaveTextContent("2.23°/MyrDERIVED");
    expect(within(panel()).getByRole("heading", { level: 3, name: "BULGE AND BAR" })).toBeVisible();
    expect(within(panel()).queryByRole("status")).not.toBeInTheDocument();
  });

  it("shows the seed and generator version first, and not again in their group", async () => {
    const { user, socket } = await renderPanel();
    await openFromList(user, socket, SURVEY_1);

    await server(() => {
      socket.serverAnswers("galaxy_parameters", (body) => someGalaxyParameters(body.universe));
    });

    const seed = within(panel()).getByText("SEED");
    expect(seed.nextElementSibling).toHaveTextContent("00000000000004D2");
    expect(within(panel()).getByText("GEN VER").nextElementSibling).toHaveTextContent("2");
    expect(within(panel()).queryByRole("row", { name: /^SEED/ })).not.toBeInTheDocument();
    expect(within(panel()).getByRole("row", { name: /MASS FUNCTION/ })).toHaveTextContent("KROUPA");
  });

  it("shows the server's reason when the request is rejected", async () => {
    const { user, socket } = await renderPanel();
    await openFromList(user, socket, SURVEY_1);
    const [request] = socket.requestsOfKind("galaxy_parameters");

    await server(() => {
      socket.serverRejects(request?.id ?? -1, {
        code: "queue_full",
        message: "the interactive queue is full",
        field: null,
      });
    });

    expect(within(panel()).getByRole("status")).toHaveTextContent(
      "REJECTED: the interactive queue is full",
    );
  });

  it("offers RETRY after a failure, and requests again", async () => {
    const { user, socket } = await renderPanel();
    await openFromList(user, socket, SURVEY_1);
    const [request] = socket.requestsOfKind("galaxy_parameters");
    await server(() => {
      socket.serverRejects(request?.id ?? -1, { code: "internal", message: "failed", field: null });
    });

    await user.click(within(panel()).getByRole("button", { name: "RETRY" }));

    expect(socket.requestsOfKind("galaxy_parameters")).toHaveLength(2);
    expect(within(panel()).getByRole("status")).toHaveTextContent("PENDING");
  });

  it("lets the list be reached from the keyboard, so that it can be scrolled", async () => {
    const { user, socket } = await renderPanel();
    await openFromList(user, socket, SURVEY_1);
    await server(() => {
      socket.serverAnswers("galaxy_parameters", (body) => someGalaxyParameters(body.universe));
    });

    const list = within(panel()).getByRole("region", { name: "Galaxy parameters" });

    // In the tab order, with nothing focusable inside it to take focus instead.
    expect(list).toHaveAttribute("tabindex", "0");
    expect(within(list).queryAllByRole("button")).toEqual([]);
    list.focus();
    expect(list).toHaveFocus();
  });

  it("requests again when another universe is opened", async () => {
    const { user, socket } = await renderPanel();
    await openFromList(user, socket, SURVEY_1);
    await server(() => {
      socket.serverAnswers("galaxy_parameters", (body) => someGalaxyParameters(body.universe));
    });

    await openFromList(user, socket, SURVEY_2);

    expect(socket.requestsOfKind("galaxy_parameters").map(({ body }) => body.universe)).toEqual([
      SURVEY_1.id,
      SURVEY_2.id,
    ]);
    expect(within(panel()).getByRole("status")).toHaveTextContent("PENDING");
    await server(() => {
      socket.serverAnswers("galaxy_parameters", (body) =>
        someGalaxyParameters(body.universe, SURVEY_2.seed),
      );
    });
    expect(within(panel()).getByText("SEED").nextElementSibling).toHaveTextContent(
      "000000000000BEEF",
    );
  });

  it("shows which parameters are in view and how many there are", async () => {
    // Six and a half layout units: a heading, a parameter, a heading, a parameter, half the next.
    vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(UNIT_PX * 6.5);
    const { user, socket } = await renderPanel();
    await openFromList(user, socket, SURVEY_1);

    await server(() => {
      socket.serverAnswers("galaxy_parameters", (body) => someGalaxyParameters(body.universe));
    });

    // The fixture has 18 parameters, of which the seed and generator version are shown above.
    expect(within(panel()).getByText("1-3 of 16")).toBeInTheDocument();
  });
});
