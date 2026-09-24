/**
 * Opening the `SYSTEM` display from the `GALAXY` chart, through `App` (plan 14, P14.T41.a).
 */
import { universeTimeFromYears } from "@hyperion/protocol";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "../../App";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import {
  anOpenedUniverse,
  aSystemsInRange,
  aUniverse,
  aUniverseList,
} from "../../test/galaxyFixtures";
import { stubCanvas } from "../../test/RecordingContext2D";
import { aSummaryResponse, aSystemSummary } from "../../test/systemFixtures";

/** A second universe, which the operator can open instead of the first. */
const SURVEY_2 = aUniverse({ id: "00000000000000b2", name: "SURVEY 2", seed: "000000000000beef" });

/** The chart's time, which the answer echoes and `OPEN SYSTEM` carries. */
const CHART_TIME_YR = 12.5;

/** Plays the server's side, letting the outcomes it settles reach React. */
async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

function navigation(): HTMLElement {
  return screen.getByRole("navigation");
}

/** The display time, as the `DISPLAY TIME` panel reads it. */
function displayTime(): HTMLElement {
  return screen.getByRole("status", { name: "DISPLAY TIME UT" });
}

/** Renders the console, opens SURVEY 1 and charts one system 10 ly from the centre. */
async function renderCharted() {
  const user = userEvent.setup();
  stubCanvas();
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: 800, height: 500 }),
  );
  render(<App />);
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  await user.keyboard("{F2}");
  await server(() => {
    socket.serverAnswers("list_universes", () => aUniverseList([aUniverse(), SURVEY_2]));
  });
  await user.click(screen.getByRole("button", { name: "Open universe SURVEY 1" }));
  await server(() => {
    socket.serverAnswers("open_universe", () => anOpenedUniverse(aUniverse()));
  });
  await user.clear(screen.getByRole("textbox", { name: "X" }));
  await user.type(screen.getByRole("textbox", { name: "X" }), "26000");
  await user.click(screen.getByRole("button", { name: "C CENTRE CHART" }));
  await server(() => {
    socket.serverAnswers("systems_in_range", () =>
      aSystemsInRange({
        centreLy: [26_000, 0, 0],
        timeYr: CHART_TIME_YR,
        systems: [{ relLy: [0, 0, 10], layer: "c" }],
      }),
    );
  });
  return { user, socket };
}

beforeEach(() => {
  FakeWebSocket.instances = [];
  vi.stubGlobal("WebSocket", FakeWebSocket);
});

describe("the SYSTEM display", () => {
  it("is reached by F3 and says NO SYSTEM SELECTED before one is opened", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.keyboard("{F3}");

    expect(within(navigation()).getByRole("button", { name: "F3 System" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(
      screen.getByText("NO SYSTEM SELECTED: select one on GALAXY and press OPEN SYSTEM"),
    ).toBeInTheDocument();
  });

  it("holds OPEN SYSTEM back, and says why, while no system is selected", async () => {
    await renderCharted();

    const open = screen.getByRole("button", { name: "OPEN SYSTEM" });
    expect(open).toHaveAttribute("aria-disabled", "true");
    expect(open).toHaveAccessibleDescription("NO SYSTEM SELECTED");
  });

  it("keeps OPEN SYSTEM out of the selected system's live region", async () => {
    await renderCharted();

    const live = screen.getByRole("status", { name: "Selected system" });
    expect(within(live).queryByRole("button")).not.toBeInTheDocument();
  });

  it("opens the selected system at the chart's time with one request", async () => {
    const { user, socket } = await renderCharted();
    await user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-1,/ }));

    await user.click(screen.getByRole("button", { name: "OPEN SYSTEM" }));

    expect(within(navigation()).getByRole("button", { name: "F3 System" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(socket.requestsOfKind("system_summary").map((request) => request.body)).toEqual([
      {
        kind: "system_summary",
        universe: aUniverse().id,
        system: "0000000000000001",
        time: universeTimeFromYears(CHART_TIME_YR),
      },
    ]);
    expect(
      within(screen.getByRole("region", { name: /^Orbit map/ })).getByText("H7K 4C0RFZ C-1"),
    ).toBeInTheDocument();
  });

  it("starts afresh at the chart's time when the system is opened again", async () => {
    const { user, socket } = await renderCharted();
    await user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-1,/ }));
    await user.click(screen.getByRole("button", { name: "OPEN SYSTEM" }));
    const opened = "+12 yr 182/15:00:00";
    expect(displayTime()).toHaveTextContent(opened);

    await user.keyboard("]");
    await waitFor(() => {
      expect(displayTime()).toHaveTextContent("+12 yr 183/15:00:00");
    });
    await user.keyboard("{F2}");
    await user.click(screen.getByRole("button", { name: "OPEN SYSTEM" }));

    expect(displayTime()).toHaveTextContent(opened);
    expect(socket.requestsOfKind("system_summary")).toHaveLength(2);
  });

  it("says NO SYSTEM SELECTED again once another universe is opened", async () => {
    const { user, socket } = await renderCharted();
    await user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-1,/ }));
    await user.click(screen.getByRole("button", { name: "OPEN SYSTEM" }));

    await user.keyboard("{F2}");
    await user.click(screen.getByRole("button", { name: "Universe" }));
    await user.click(screen.getByRole("button", { name: "Open universe SURVEY 2" }));
    await server(() => {
      socket.serverAnswers("open_universe", () => anOpenedUniverse(SURVEY_2));
    });
    await user.keyboard("{F3}");

    expect(
      screen.getByText("NO SYSTEM SELECTED: select one on GALAXY and press OPEN SYSTEM"),
    ).toBeInTheDocument();
  });

  it("asks nothing again for its answer when the display is left and shown again", async () => {
    const { user, socket } = await renderCharted();
    await user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-1,/ }));
    await user.click(screen.getByRole("button", { name: "OPEN SYSTEM" }));
    await server(() => {
      socket.serverAnswers("system_summary", (body) =>
        aSummaryResponse(aSystemSummary({ universe: body.universe, system: body.system })),
      );
    });

    await user.keyboard("{F2}");
    await user.keyboard("{F3}");

    expect(socket.requestsOfKind("system_summary")).toHaveLength(1);
  });
});
