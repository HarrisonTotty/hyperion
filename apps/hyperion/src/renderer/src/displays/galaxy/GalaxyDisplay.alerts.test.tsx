/**
 * The `GALAXY` display's alert list and the chart's marks of it (plan 12, P12.T12): the list in the
 * `SYSTEMS` panel, the subscription from the chart's observer, and a resolved row's host selected.
 */
import { galacticPositionFromLy } from "@hyperion/protocol";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { UniverseProvider } from "../../components/UniverseProvider";
import { aBearingContact, aResolvedContact } from "../../test/alertFixtures";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import {
  anOpenedUniverse,
  aSystemsInRange,
  aUniverse,
  aUniverseList,
} from "../../test/galaxyFixtures";
import { stubCanvas } from "../../test/RecordingContext2D";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { GalaxyDisplay } from "./GalaxyDisplay";
import { NO_OBSERVER } from "./SystemsPanel";

const SURVEY = aUniverse();
const SURVEY_2 = aUniverse({ id: "00000000000000b2", name: "SURVEY 2", seed: "000000000000beef" });

/** The chart's one system, 1 ly from the galactic centre, as the server answers it. */
const ANSWER = aSystemsInRange({
  centreLy: [0, 0, 0],
  systems: [{ relLy: [1, 0, 0], layer: "c" }],
});
const HOST = ANSWER.systems[0];

async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

/** Renders the display under a welcomed link, opens the universe and charts its centre. */
async function renderCharted() {
  const user = userEvent.setup();
  stubCanvas();
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: 792, height: 488 }),
  );
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(480);
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
  await server(() => {
    socket.serverAnswers("list_universes", () => aUniverseList([SURVEY, SURVEY_2]));
  });
  await user.click(screen.getByRole("button", { name: `Open universe ${SURVEY.name}` }));
  await server(() => {
    socket.serverAnswers("open_universe", () => anOpenedUniverse(SURVEY));
  });
  await user.keyboard("c");
  await server(() => {
    socket.serverAnswers("systems_in_range", (body) => ({ ...ANSWER, time: body.time }));
  });
  return { user, socket };
}

/** The chart's legend, under its picture on the `LOCAL CHART` page. */
function chartLegend(): HTMLElement {
  return within(screen.getByRole("tabpanel", { name: "LOCAL CHART" })).getByRole("group", {
    name: "Chart legend",
  });
}

function systemsPanel(): HTMLElement {
  return screen.getByRole("region", { name: "Systems" });
}

describe("the GALAXY display's alert list (plan 12, P12.T12)", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  it("stands as the SYSTEMS panel's third list, saying it has no observer until OBSERVED", async () => {
    const { user, socket } = await renderCharted();

    await user.click(within(systemsPanel()).getByRole("button", { name: "ALERTS" }));

    expect(within(systemsPanel()).getByText("NO OBSERVER:")).toBeInTheDocument();
    expect(systemsPanel()).toHaveTextContent(NO_OBSERVER);
    expect(socket.requestsOfKind("subscribe")).toEqual([]);
  });

  it("subscribes from the chart's observer once OBSERVED takes one", async () => {
    const { user, socket } = await renderCharted();

    await user.click(
      within(screen.getByRole("tabpanel", { name: "LOCAL CHART" })).getByRole("button", {
        name: "OBSERVED",
      }),
    );

    const [subscribe] = socket.requestsOfKind("subscribe");
    expect(subscribe?.body.topic).toMatchObject({
      topic: "alerts",
      observer: galacticPositionFromLy([0, 0, 0]),
    });
  });

  it("selects a resolved alert's host, which the readout reads, and keeps the alert list shown", async () => {
    const { user, socket } = await renderCharted();
    await user.click(
      within(screen.getByRole("tabpanel", { name: "LOCAL CHART" })).getByRole("button", {
        name: "OBSERVED",
      }),
    );
    await server(() => {
      socket.serverAnswers("subscribe", () => ({
        kind: "subscribe",
        subscription: 1,
        state: {
          topic: "alerts",
          census: [],
          contacts: [
            aBearingContact(),
            aResolvedContact([1, 0, 0], {
              host: HOST?.id ?? "",
              designation: HOST?.designation ?? "",
            }),
          ],
        },
      }));
    });
    await user.click(within(systemsPanel()).getByRole("button", { name: "ALERTS" }));

    const resolved = within(systemsPanel()).getByRole("option", { name: /: resolved/u });
    await user.click(resolved);

    expect(resolved).toHaveAttribute("aria-selected", "true");
    expect(within(systemsPanel()).getByRole("button", { name: "ALERTS" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(within(systemsPanel()).getByRole("button", { name: "SYSTEMS" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
    await user.click(within(systemsPanel()).getByRole("button", { name: "SYSTEMS" }));
    expect(
      within(systemsPanel()).getByRole("option", {
        name: new RegExp(HOST?.designation ?? "-", "u"),
      }),
    ).toHaveAttribute("aria-selected", "true");
  });

  it("selects nothing for an unresolved alert, whose host is not known", async () => {
    const { user, socket } = await renderCharted();
    await user.click(
      within(screen.getByRole("tabpanel", { name: "LOCAL CHART" })).getByRole("button", {
        name: "OBSERVED",
      }),
    );
    await server(() => {
      socket.serverAnswers("subscribe", () => ({
        kind: "subscribe",
        subscription: 1,
        state: { topic: "alerts", census: [], contacts: [aBearingContact()] },
      }));
    });
    await user.click(within(systemsPanel()).getByRole("button", { name: "ALERTS" }));
    await user.click(within(systemsPanel()).getByRole("option", { name: /unresolved/u }));
    await user.click(within(systemsPanel()).getByRole("button", { name: "SYSTEMS" }));

    for (const row of within(systemsPanel()).getAllByRole("option")) {
      expect(row).toHaveAttribute("aria-selected", "false");
    }
  });

  it("names the transient in the chart's legend once a resolved host on the chart is marked", async () => {
    const { user, socket } = await renderCharted();
    await user.click(
      within(screen.getByRole("tabpanel", { name: "LOCAL CHART" })).getByRole("button", {
        name: "OBSERVED",
      }),
    );
    expect(chartLegend()).not.toHaveTextContent("TRANSIENT");

    await server(() => {
      socket.serverAnswers("subscribe", () => ({
        kind: "subscribe",
        subscription: 1,
        state: {
          topic: "alerts",
          census: [],
          contacts: [aResolvedContact([1, 0, 0], { host: HOST?.id ?? "" })],
        },
      }));
    });

    expect(chartLegend()).toHaveTextContent("TRANSIENT");
  });

  it("drops the active alert with its universe, whose contact numbers are its own", async () => {
    const { user, socket } = await renderCharted();
    const observe = async (): Promise<void> => {
      await user.click(
        within(screen.getByRole("tabpanel", { name: "LOCAL CHART" })).getByRole("button", {
          name: "OBSERVED",
        }),
      );
      await server(() => {
        socket.serverAnswers("subscribe", () => ({
          kind: "subscribe",
          subscription: socket.requestsOfKind("subscribe").length,
          state: { topic: "alerts", census: [], contacts: [aBearingContact()] },
        }));
      });
    };
    await observe();
    await user.click(within(systemsPanel()).getByRole("button", { name: "ALERTS" }));
    await user.click(within(systemsPanel()).getByRole("option", { name: /unresolved/u }));
    expect(within(systemsPanel()).getByRole("option")).toHaveAttribute("aria-selected", "true");

    await user.click(screen.getByRole("button", { name: "Universe" }));
    await user.click(screen.getByRole("button", { name: `Open universe ${SURVEY_2.name}` }));
    await server(() => {
      socket.serverAnswers("open_universe", () => anOpenedUniverse(SURVEY_2));
    });
    await server(() => {
      socket.serverAnswers("systems_in_range", (body) => ({ ...ANSWER, time: body.time }));
    });
    await user.click(screen.getByRole("tab", { name: "LOCAL CHART" }));
    await observe();
    await user.click(within(systemsPanel()).getByRole("button", { name: "ALERTS" }));

    expect(within(systemsPanel()).getByRole("option")).toHaveAttribute("aria-selected", "false");
  });
});
