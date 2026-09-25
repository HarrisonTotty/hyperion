/**
 * The `GALAXY` display's readout of the selected system's stars, from `system_summary` at the
 * chart's time (plan 06, P06.T36), and its star list (plan 11, P11.T14), through `App`.
 */
import { type SystemSummaryDto, universeTimeFromYears } from "@hyperion/protocol";
import { act, render, screen, within } from "@testing-library/react";
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
import {
  aBlackHole,
  aNeutronStar,
  aNotYetBornSummary,
  anUnknownSystemError,
  aSingleStarSummary,
  aSummaryResponse,
  aSunlikeStar,
  aSystemSummary,
  aTripleSummary,
  aWhiteDwarf,
} from "../../test/systemFixtures";

/** The chart's time, at which the stars are asked about. */
const CHART_TIME_YR = 12.5;

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
        centreLy: [26_000, 0, 0],
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

/** Selects the chart's first system and answers its stars with `summary`, at the time asked. */
async function selectAnswered(summary: SystemSummaryDto) {
  const rendered = await renderCharted();
  await rendered.user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-1,/ }));
  await server(() => {
    rendered.socket.serverAnswers("system_summary", (body) =>
      aSummaryResponse({
        ...summary,
        universe: body.universe,
        system: body.system,
        time: body.time,
      }),
    );
  });
  return rendered;
}

function systemsPanel(): HTMLElement {
  return screen.getByRole("region", { name: "Systems" });
}

function readout(): HTMLElement {
  return screen.getByRole("status", { name: "Selected system" });
}

/** Every value under `label` in the readout, in order: the system's rows, then the primary's. */
function valuesOf(label: string): ReadonlyArray<string> {
  return within(readout())
    .queryAllByText(label, { selector: "dt" })
    .map((term) => term.nextElementSibling?.textContent ?? "");
}

function valueOf(label: string): string {
  const [value, ...others] = valuesOf(label);
  if (value === undefined || others.length > 0) {
    throw new Error(`${label} reads ${String(valuesOf(label).length)} times`);
  }
  return value;
}

beforeEach(() => {
  FakeWebSocket.instances = [];
  vi.stubGlobal("WebSocket", FakeWebSocket);
});

describe("the GALAXY readout's stars", () => {
  it("asks for the selected system's stars at the chart's time", async () => {
    const { user, socket } = await renderCharted();

    await user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-1,/ }));

    expect(socket.requestsOfKind("system_summary").map(({ body }) => body)).toEqual([
      {
        kind: "system_summary",
        universe: aUniverse().id,
        system: "0000000000000001",
        time: universeTimeFromYears(CHART_TIME_YR),
      },
    ]);
  });

  it("says PENDING beside the readout, not in it, and fills no row until the answer", async () => {
    const { user } = await renderCharted();

    await user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-1,/ }));

    expect(within(systemsPanel()).getByText("PENDING")).toBeInTheDocument();
    expect(within(readout()).queryByText("PENDING")).not.toBeInTheDocument();
    expect(valuesOf("CLASS")).toEqual([]);
    // Its row stands ready with the em dash, so that nothing moves when the answer comes.
    expect(valuesOf("MASS")).toEqual(["—"]);
  });

  it("reads a living star: its kind, class, mass now, light, size, temperature and magnitude", async () => {
    await selectAnswered(aSingleStarSummary(aSunlikeStar()));

    expect(valueOf("KIND")).toBe("DWARF");
    expect(valueOf("PHASE")).toBe("MAIN SEQUENCE");
    expect(valueOf("CLASS")).toBe("G2V");
    expect(valueOf("MASS")).toBe("1.00 M");
    expect(within(readout()).getAllByRole("img", { name: "solar masses" }).length).toBeGreaterThan(
      0,
    );
    expect(valueOf("LUM")).toBe("1.00 L");
    // The system's galactic radius, then the primary's own.
    expect(valuesOf("RADIUS")[0]?.startsWith("26,000")).toBe(true);
    expect(valuesOf("RADIUS")[1]).toBe("1.00 R");
    expect(valueOf("T EFF")).toBe("5772 K");
    expect(valueOf("M V")).toBe("+4.83 mag");
    expect(valueOf("[Fe/H]")).toBe("-0.13 dex");
    expect(within(readout()).getByRole("heading", { name: "STAR A" })).toBeInTheDocument();
  });

  it("reads what this generator version does not model as the em dash, never a zero", async () => {
    await selectAnswered(aSingleStarSummary(aSunlikeStar()));

    for (const label of ["ROTATION", "VARIABILITY", "NEBULA", "EVENTS"]) {
      expect(valueOf(label)).toBe("—");
    }
    expect(valuesOf("DIES IN")).toEqual([]);
  });

  it("reads a death inside the clock window as DIES IN, from the answer's time", async () => {
    const at = universeTimeFromYears(CHART_TIME_YR);
    await selectAnswered(
      aSingleStarSummary(
        aSunlikeStar({ death_time: { seconds: at.seconds + 312 * 31_557_600, nanos: at.nanos } }),
      ),
    );

    expect(valueOf("DIES IN")).toBe("312 yr");
  });

  it("reads a white dwarf's remnant and cooling age", async () => {
    await selectAnswered(aSingleStarSummary(aWhiteDwarf()));

    expect(valueOf("KIND")).toBe("WHITE DWARF");
    expect(valueOf("REMNANT")).toBe("WHITE DWARF");
    expect(valueOf("COOLING AGE")).toBe("3.95 Gyr");
    expect(valueOf("M V")).toBe("—");
    expect(valueOf("KICK")).toBe("—");
  });

  it("reads a neutron star's radius in km and its pulsar as not modelled", async () => {
    await selectAnswered(aSingleStarSummary(aNeutronStar()));

    expect(valueOf("KIND")).toBe("NEUTRON STAR");
    expect(valuesOf("RADIUS")[1]).toBe("12.2 km");
    expect(valueOf("PULSAR PERIOD")).toBe("—");
  });

  it("reads a black hole's light as missing, with the reason", async () => {
    await selectAnswered(aSingleStarSummary(aBlackHole()));

    expect(valueOf("KIND")).toBe("BLACK HOLE");
    expect(valueOf("LUM")).toBe("—NO LIGHT");
    expect(valueOf("T EFF")).toBe("—NO LIGHT");
    expect(valueOf("SPIN")).toBe("—");
  });

  it("reads a star that left no remnant with no light or size, and its mass as the em dash", async () => {
    await selectAnswered(
      aSingleStarSummary(
        aBlackHole({
          kind: "no_remnant",
          phase: "no_remnant",
          class: "NONE",
          mass_msun: 0,
          core_mass_msun: 0,
          radius_rsun: 0,
          remnant: { type: "no_remnant" },
        }),
      ),
    );

    expect(valueOf("KIND")).toBe("NO REMNANT");
    expect(valueOf("REMNANT")).toBe("NONE");
    expect(valueOf("MASS")).toBe("—");
    for (const label of ["LUM", "T EFF", "M V", "ROTATION", "EVENTS"]) {
      expect(valuesOf(label)).toEqual([]);
    }
    const stars = within(readout()).getByRole("table", { name: "STARS" });
    expect(within(stars).getAllByRole("row")[1]?.textContent).toBe("ANONE—NO REMNANT");
  });

  it("reads NOT YET FORMED for a system not yet born", async () => {
    await selectAnswered(aNotYetBornSummary());

    expect(valueOf("STARS")).toBe("NOT YET FORMED");
    expect(within(readout()).queryByRole("heading", { name: "STAR A" })).not.toBeInTheDocument();
  });

  it("says why the server refused, outside the readout", async () => {
    const { user, socket } = await renderCharted();
    await user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-1,/ }));

    await server(() => {
      const [request] = socket.requestsOfKind("system_summary");
      socket.serverRejects(request?.id ?? -1, anUnknownSystemError());
    });

    expect(
      screen.getByText("REJECTED: no system 0200080020000000 in this universe"),
    ).toBeInTheDocument();
    expect(within(readout()).queryByText(/REJECTED/)).not.toBeInTheDocument();
    expect(valuesOf("CLASS")).toEqual([]);
  });

  it("offers RETRY after the server's queue refuses it, which asks again", async () => {
    const { user, socket } = await renderCharted();
    await user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-1,/ }));
    await server(() => {
      const [request] = socket.requestsOfKind("system_summary");
      socket.serverRejects(request?.id ?? -1, {
        code: "queue_full",
        message: "the interactive queue is full",
        field: null,
      });
    });

    await user.click(within(systemsPanel()).getByRole("button", { name: "RETRY" }));

    expect(socket.requestsOfKind("system_summary")).toHaveLength(2);
  });

  it("says SYSTEM DATA INVALID for an answer it cannot read, with RETRY", async () => {
    await selectAnswered(aSystemSummary({ stars: [], existence: "exists" }));

    expect(
      within(systemsPanel()).getByText("SYSTEM DATA INVALID: star list empty"),
    ).toBeInTheDocument();
    expect(within(systemsPanel()).getByRole("button", { name: "RETRY" })).toBeInTheDocument();
    expect(valuesOf("CLASS")).toEqual([]);
  });

  it("reads the answer for a time the chart has left as stale, until the new one comes", async () => {
    const { user, socket } = await selectAnswered(aSingleStarSummary(aSunlikeStar()));
    expect(within(readout()).queryByText("stale")).not.toBeInTheDocument();

    const time = screen.getByLabelText("CHART TIME");
    await user.clear(time);
    await user.type(time, "20{Enter}");
    await server(() => {
      socket.serverAnswers("systems_in_range", () =>
        aSystemsInRange({
          centreLy: [26_000, 0, 0],
          timeYr: 20,
          systems: [
            { relLy: [0, 0, 10], layer: "c" },
            { relLy: [0, 0, 20], layer: "c" },
          ],
        }),
      );
    });

    expect(valueOf("CLASS")).toBe("G2V Sstale");
    expect(within(readout()).getByRole("heading", { name: "STAR A stale" })).toBeInTheDocument();
    expect(within(readout()).getByRole("table", { name: "STARS stale" })).toBeInTheDocument();

    await server(() => {
      socket.serverAnswers("system_summary", (body) =>
        aSummaryResponse({
          ...aSingleStarSummary(aSunlikeStar()),
          universe: body.universe,
          system: body.system,
          time: body.time,
        }),
      );
    });

    expect(valueOf("CLASS")).toBe("G2V");
  });

  it("drops the answer to a selection since superseded", async () => {
    const { user, socket } = await renderCharted();
    await user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-1,/ }));
    const [first] = socket.requestsOfKind("system_summary");
    await user.click(screen.getByRole("option", { name: /^H7K 4C0RFZ C-2,/ }));

    await server(() => {
      socket.serverResponds(
        first?.id ?? -1,
        aSummaryResponse(aSingleStarSummary(aWhiteDwarf({ class: "DA4.0" }))),
      );
    });

    expect(valuesOf("CLASS")).toEqual([]);
    expect(valueOf("DESIG")).toBe("H7K 4C0RFZ C-2");
  });

  it("lists a single star alone, with no orbits", async () => {
    await selectAnswered(aSingleStarSummary(aSunlikeStar()));

    const stars = within(readout()).getByRole("table", { name: "STARS" });
    expect(within(stars).getAllByRole("row")).toHaveLength(2);
    expect(within(stars).getByRole("rowheader", { name: "A" })).toBeInTheDocument();
    expect(within(readout()).queryByRole("table", { name: "ORBITS" })).not.toBeInTheDocument();
  });

  it("scrolls its readings in a region the keyboard reaches, with their position shown", async () => {
    await selectAnswered(aTripleSummary());

    const region = within(systemsPanel()).getByRole("region", { name: "Readings" });
    expect(region).toHaveAttribute("tabindex", "0");
    expect(within(region).getByRole("table", { name: "STARS" })).toBeInTheDocument();
    expect(within(systemsPanel()).getByText(/^1-\d+ of \d+$/)).toBeInTheDocument();
  });

  it("lists every star of a triple and each orbit that holds them", async () => {
    await selectAnswered(aTripleSummary());

    const stars = within(readout()).getByRole("table", { name: "STARS" });
    expect(
      within(stars)
        .getAllByRole("rowheader")
        .map((cell) => cell.textContent),
    ).toEqual(["A", "B", "C"]);
    const orbits = within(readout()).getByRole("table", { name: "ORBITS" });
    expect(
      within(orbits)
        .getAllByRole("rowheader")
        .map((cell) => cell.textContent),
    ).toEqual(["AB–C", "A–B"]);
  });

  it("reads the stars of the pinned binary with its white dwarf first", async () => {
    await selectAnswered(aSystemSummary());

    const stars = within(readout()).getByRole("table", { name: "STARS" });
    const rows = within(stars).getAllByRole("row").slice(1);
    expect(rows.map((row) => row.textContent)).toEqual(["ADA9.20.69WHITE DWARF", "BG2V1.00DWARF"]);
  });
});
