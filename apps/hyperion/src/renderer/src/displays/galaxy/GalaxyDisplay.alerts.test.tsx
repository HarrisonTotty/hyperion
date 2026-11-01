/**
 * The `GALAXY` display's alert list and the chart's marks of it (plan 12, P12.T12): the list in the
 * `SYSTEMS` panel, the subscription from the chart's observer, and a resolved row's host selected.
 */
import { type ContactDto, galacticPositionFromLy } from "@hyperion/protocol";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { UniverseProvider } from "../../components/UniverseProvider";
import { aBearingContact, alertsNotification, aResolvedContact } from "../../test/alertFixtures";
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

/** The panel's `ALERTS` choice, its name carrying the count of unacknowledged alerts. */
function alertsChoice(): HTMLElement {
  return within(systemsPanel()).getByRole("button", { name: /^ALERTS\b/u });
}

/** Chooses `OBSERVED` on the chart and answers the alerts subscription with `contacts`. */
async function observeWith(
  user: ReturnType<typeof userEvent.setup>,
  socket: FakeWebSocket,
  contacts: ReadonlyArray<ContactDto>,
): Promise<void> {
  await user.click(
    within(screen.getByRole("tabpanel", { name: "LOCAL CHART" })).getByRole("button", {
      name: "OBSERVED",
    }),
  );
  await server(() => {
    socket.serverAnswers("subscribe", () => ({
      kind: "subscribe",
      subscription: 1,
      state: { topic: "alerts", census: [], contacts: [...contacts] },
    }));
  });
}

describe("the GALAXY display's alert list (plan 12, P12.T12)", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  it("stands as the SYSTEMS panel's third list, saying it has no observer until OBSERVED", async () => {
    const { user, socket } = await renderCharted();

    await user.click(alertsChoice());

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
    await user.click(alertsChoice());

    const resolved = within(systemsPanel()).getByRole("option", { name: /: resolved/u });
    await user.click(resolved);

    expect(resolved).toHaveAttribute("aria-selected", "true");
    expect(alertsChoice()).toHaveAttribute("aria-pressed", "true");
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
    await user.click(alertsChoice());
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
    await user.click(alertsChoice());
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
    await user.click(alertsChoice());

    expect(within(systemsPanel()).getByRole("option")).toHaveAttribute("aria-selected", "false");
  });
});

describe("the ALERTS choice's count of unacknowledged alerts (ux-calls-data's ruling 5)", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  it("shows no count with no alert unacknowledged", async () => {
    const { user, socket } = await renderCharted();
    await observeWith(user, socket, [aBearingContact({ acknowledged: true })]);

    expect(alertsChoice()).toHaveAccessibleName("ALERTS");
    expect(alertsChoice()).toHaveTextContent(/^ALERTS$/u);
  });

  it("raises its count when a push brings an alert while SYSTEMS shows", async () => {
    const { user, socket } = await renderCharted();
    await observeWith(user, socket, [aBearingContact()]);
    expect(alertsChoice()).toHaveAccessibleName("ALERTS, 1 unacknowledged");

    await server(() => {
      socket.serverNotifies(1, alertsNotification(1, [aResolvedContact([1, 0, 0])]));
    });

    expect(alertsChoice()).toHaveAccessibleName("ALERTS, 2 unacknowledged");
    expect(alertsChoice()).toHaveTextContent(/^ALERTS2$/u);
    expect(within(systemsPanel()).getByRole("button", { name: "SYSTEMS" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("lowers its count once the server accepts an acknowledgement", async () => {
    const { user, socket } = await renderCharted();
    await observeWith(user, socket, [aBearingContact(), aResolvedContact([1, 0, 0])]);
    await user.click(alertsChoice());
    await user.click(within(systemsPanel()).getByRole("option", { name: /unresolved/u }));
    await user.click(within(systemsPanel()).getByRole("button", { name: /ACKNOWLEDGE$/u }));
    expect(alertsChoice()).toHaveAccessibleName("ALERTS, 2 unacknowledged");

    await server(() => {
      socket.serverAnswers("alerts_acknowledge", () => ({ kind: "alerts_acknowledge" }));
    });

    expect(alertsChoice()).toHaveAccessibleName("ALERTS, 1 unacknowledged");
  });

  it("writes a count past 999 as 999+ in the same place, its name keeping the count", async () => {
    const { user, socket } = await renderCharted();
    await observeWith(
      user,
      socket,
      Array.from({ length: 1000 }, (_, index) => aBearingContact({ contact: index + 1 })),
    );

    expect(alertsChoice()).toHaveAccessibleName("ALERTS, 1000 unacknowledged");
    expect(alertsChoice()).toHaveTextContent(/^ALERTS999\+$/u);
  });

  it("goes stale with the list's rows when the link is lost", async () => {
    const { user, socket } = await renderCharted();
    await observeWith(user, socket, [aBearingContact()]);

    await server(() => {
      socket.close();
    });

    expect(alertsChoice()).toHaveAccessibleName("ALERTS, 1 unacknowledged, stale");
    expect(alertsChoice()).toHaveTextContent(/^ALERTS1 Sstale$/u);
  });
});
