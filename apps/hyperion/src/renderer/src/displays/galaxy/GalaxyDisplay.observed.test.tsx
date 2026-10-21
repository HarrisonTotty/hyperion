import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { UniverseProvider } from "../../components/UniverseProvider";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import {
  anOpenedUniverse,
  aRangeRequest,
  aSystemsInRange,
  aUniverse,
  aUniverseList,
} from "../../test/galaxyFixtures";
import { stubCanvas } from "../../test/RecordingContext2D";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { describedText } from "../../test/statusLine";
import { GalaxyDisplay } from "./GalaxyDisplay";

const SURVEY_1 = aUniverse();
const SURVEY_2 = aUniverse({ id: "00000000000000b2", name: "SURVEY 2", seed: "000000000000beef" });

async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

/** Renders the display under a welcomed link, the page laid out 792 × 488 px. */
function renderDisplay() {
  const user = userEvent.setup();
  stubCanvas();
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: 792, height: 488 }),
  );
  render(
    <ServerLinkHarness>
      <UniverseProvider>
        <GalaxyDisplay onOpenSystem={() => undefined} />
      </UniverseProvider>
    </ServerLinkHarness>,
  );
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  return { user, socket };
}

type Rendered = ReturnType<typeof renderDisplay>;

async function open({ user, socket }: Rendered, universe = SURVEY_1): Promise<void> {
  await user.click(screen.getByRole("button", { name: `Open universe ${universe.name}` }));
  await server(() => {
    socket.serverAnswers("open_universe", () => anOpenedUniverse(universe));
  });
}

/** Answers the latest range query with one system, as asked. */
async function answerChart({ socket }: Rendered): Promise<void> {
  await server(() => {
    socket.serverAnswers("systems_in_range", (body) => ({
      ...aSystemsInRange({
        universe: body.universe,
        centreLy: [0, 0, 0],
        systems: [{ relLy: [1, 0, 0], layer: "c" }],
      }),
      time: body.time,
    }));
  });
}

function chartPage(): HTMLElement {
  return screen.getByRole("tabpanel", { name: "LOCAL CHART" });
}

describe("the chart's mode across universes (plan 12, P12.T11)", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  it("holds OBSERVED back with its reason until a chart is on show", async () => {
    const rendered = renderDisplay();
    await server(() => {
      rendered.socket.serverAnswers("list_universes", () => aUniverseList([SURVEY_1]));
    });
    await open(rendered);
    await rendered.user.keyboard("c");

    const observed = within(chartPage()).getByRole("button", { name: "OBSERVED" });
    expect(observed).toHaveAttribute("aria-disabled", "true");
    expect(describedText(observed)).toBe("NO CHART: centre one to observe from it");

    await answerChart(rendered);

    expect(observed).not.toHaveAttribute("aria-disabled");
  });

  it("drops the mode and the observer with the universe, asking the next one in NOW", async () => {
    const rendered = renderDisplay();
    const { user, socket } = rendered;
    await server(() => {
      socket.serverAnswers("list_universes", () => aUniverseList([SURVEY_1, SURVEY_2]));
    });
    await open(rendered);
    await user.keyboard("c");
    await answerChart(rendered);
    await user.click(within(chartPage()).getByRole("button", { name: "OBSERVED" }));
    expect(socket.requestsOfKind("systems_in_range").at(-1)?.body).toEqual(
      aRangeRequest({ centreLy: [0, 0, 0], universe: SURVEY_1.id, observerLy: [0, 0, 0] }),
    );

    await user.click(screen.getByRole("button", { name: "Universe" }));
    await open(rendered, SURVEY_2);

    expect(socket.requestsOfKind("systems_in_range").at(-1)?.body).toEqual(
      aRangeRequest({ centreLy: [0, 0, 0], universe: SURVEY_2.id }),
    );
    await user.click(screen.getByRole("tab", { name: "LOCAL CHART" }));
    expect(
      within(chartPage()).getByRole("button", { name: "NOW", pressed: true }),
    ).toBeInTheDocument();

    // Dropped for good: going back to the first universe does not bring them back.
    await user.click(screen.getByRole("button", { name: "Universe" }));
    await open(rendered, SURVEY_1);

    expect(socket.requestsOfKind("systems_in_range").at(-1)?.body).toEqual(
      aRangeRequest({ centreLy: [0, 0, 0], universe: SURVEY_1.id }),
    );
  });
});
