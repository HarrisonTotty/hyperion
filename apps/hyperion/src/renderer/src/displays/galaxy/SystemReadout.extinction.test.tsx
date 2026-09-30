/**
 * The `GALAXY` display's readout of the extinction from the chart's centre to the selected system,
 * from `extinction` at the chart's time (plan 07, P07.T11.c), through `App`.
 */
import { galacticPositionFromLy, universeTimeFromYears } from "@hyperion/protocol";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "../../App";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import {
  anExtinctionResult,
  anOpenedUniverse,
  aSystemsInRange,
  aUniverse,
  aUniverseList,
} from "../../test/galaxyFixtures";
import { stubCanvas } from "../../test/RecordingContext2D";

/** The chart's time, at which the line is asked about. */
const CHART_TIME_YR = 12.5;

const CENTRE_LY = [26_000, 0, 0] as const;

async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

/** Renders the console, opens SURVEY 1 and charts two systems, 10 and 20 ly from the centre. */
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
    socket.serverAnswers("list_universes", () => aUniverseList([aUniverse()]));
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
        centreLy: CENTRE_LY,
        timeYr: CHART_TIME_YR,
        systems: [
          { relLy: [0, 0, 10], layer: "c" },
          { relLy: [0, 0, 20], layer: "c" },
        ],
      }),
    );
  });
  return { user, socket };
}

/** Charts, selects the first system and answers its line with the fixture's figures. */
async function selectAnswered() {
  const rendered = await renderCharted();
  await rendered.user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-1,/ }));
  await server(() => {
    rendered.socket.serverAnswers("extinction", (body) => anExtinctionResult(body));
  });
  return rendered;
}

function systemsPanel(): HTMLElement {
  return screen.getByRole("region", { name: "Systems" });
}

function readout(): HTMLElement {
  return screen.getByRole("status", { name: "Selected system" });
}

/** The value under `label` in the readout. */
function valueOf(label: string): string {
  const terms = within(readout()).queryAllByText(label, { selector: "dt" });
  const [term] = terms;
  if (term === undefined || terms.length > 1) {
    throw new Error(`${label} reads ${String(terms.length)} times`);
  }
  return term.nextElementSibling?.textContent ?? "";
}

const ROWS = ["A(V)", "E(B-V)", "A(K)", "N(H)"] as const;

beforeEach(() => {
  FakeWebSocket.instances = [];
  vi.stubGlobal("WebSocket", FakeWebSocket);
});

describe("the GALAXY readout's extinction", () => {
  it("asks for the line from the chart's centre to the selected system at the chart's time", async () => {
    const { user, socket } = await renderCharted();

    await user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-1,/ }));

    expect(socket.requestsOfKind("extinction").map(({ body }) => body)).toEqual([
      {
        kind: "extinction",
        universe: aUniverse().id,
        origin: galacticPositionFromLy(CENTRE_LY),
        time: universeTimeFromYears(CHART_TIME_YR),
        targets: [{ type: "system", id: "0000000000000001" }],
      },
    ]);
  });

  it("reads the missing state in every row while pending, and says so beside the readout", async () => {
    const { user } = await renderCharted();

    await user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-1,/ }));

    for (const label of ROWS) {
      expect(valueOf(label)).toBe("—");
    }
    expect(within(systemsPanel()).getByText("EXTINCTION: PENDING")).toBeInTheDocument();
    expect(within(readout()).queryByText("EXTINCTION: PENDING")).not.toBeInTheDocument();
  });

  it("reads A(V), E(B-V), A(K) and N(H) once the server answers", async () => {
    const { user, socket } = await renderCharted();
    await user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-1,/ }));

    await server(() => {
      socket.serverAnswers("extinction", (body) => anExtinctionResult(body));
    });

    expect(valueOf("A(V)")).toBe("0.84 mag");
    expect(valueOf("E(B-V)")).toBe("0.27 mag");
    expect(valueOf("A(K)")).toBe("0.09 mag");
    expect(valueOf("N(H)")).toBe("1.57E21 /cm²");
    expect(within(systemsPanel()).queryByText("EXTINCTION: PENDING")).not.toBeInTheDocument();
  });

  it("reads the missing state for a system the server does not know", async () => {
    const { user, socket } = await renderCharted();
    await user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-1,/ }));

    await server(() => {
      socket.serverAnswers("extinction", (body) => anExtinctionResult(body, null));
    });

    for (const label of ROWS) {
      expect(valueOf(label)).toBe("—");
    }
  });

  it("says the server knows no such system beside the readout", async () => {
    const { user, socket } = await renderCharted();
    await user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-1,/ }));

    await server(() => {
      socket.serverAnswers("extinction", (body) => anExtinctionResult(body, null));
    });

    expect(within(systemsPanel()).getByText("EXTINCTION: NO SUCH SYSTEM")).toBeInTheDocument();
  });

  it("asks for the line to a new selection", async () => {
    const { user } = await selectAnswered();

    await user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-2,/ }));

    expect(
      FakeWebSocket.latest()
        .requestsOfKind("extinction")
        .map(({ body }) => body.targets),
    ).toEqual([
      [{ type: "system", id: "0000000000000001" }],
      [{ type: "system", id: "0000000000000002" }],
    ]);
  });

  it("drops the last selection's figures until the new selection's answer", async () => {
    const { user } = await selectAnswered();

    await user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-2,/ }));

    expect(valueOf("A(V)")).toBe("—");
  });

  it("reads the new selection's figures once they arrive", async () => {
    const { user, socket } = await selectAnswered();
    await user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-2,/ }));

    await server(() => {
      socket.serverAnswers("extinction", (body) =>
        anExtinctionResult(body, {
          aVMag: 1.5,
          eBVMag: 0.48,
          aKMag: 0.15,
          hydrogenColumnPerCm2: 2.8e21,
        }),
      );
    });
    expect(valueOf("A(V)")).toBe("1.50 mag");
  });

  it("says why the line failed beside the readout, and asks again on RETRY", async () => {
    const { user, socket } = await renderCharted();
    await user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-1,/ }));
    const [first] = socket.requestsOfKind("extinction");
    await server(() => {
      socket.serverRejects(first?.id ?? -1, {
        code: "queue_full",
        message: "the server is busy; try again shortly",
        field: null,
      });
    });

    expect(
      within(systemsPanel()).getByText(
        "EXTINCTION: REJECTED: the server is busy; try again shortly",
      ),
    ).toBeInTheDocument();
    await user.click(within(systemsPanel()).getByRole("button", { name: "RETRY" }));

    const requests = socket.requestsOfKind("extinction");
    expect(requests).toHaveLength(2);
    expect(requests[1]?.body).toEqual(first?.body);
  });

  it("cancels the line in flight when the selection changes", async () => {
    const { user, socket } = await renderCharted();
    await user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-1,/ }));
    const [first] = socket.requestsOfKind("extinction");

    await user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-2,/ }));

    expect(socket.cancelledIds()).toContain(first?.id);
  });
});
