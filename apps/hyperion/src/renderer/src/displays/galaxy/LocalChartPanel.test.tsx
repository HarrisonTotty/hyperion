import type { MassLayer } from "@hyperion/protocol";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { UniverseProvider } from "../../components/UniverseProvider";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import {
  anOpenedUniverse,
  aSystemsInRange,
  aUniverse,
  aUniverseList,
} from "../../test/galaxyFixtures";
import { type RecordingContext2D, stubCanvas } from "../../test/RecordingContext2D";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { stubMatchMedia } from "../../test/stubMatchMedia";
import { GalaxyDisplay } from "./GalaxyDisplay";

/** The chart centre every test uses, off the galactic axis so that the directions are defined. */
const CENTRE = [26_000, 0, 0] as const;

/** The stage, and every other element, laid out 400 × 300: the centre of the canvas is 200, 150. */
const WIDTH_PX = 400;
const HEIGHT_PX = 300;

/** Three systems: one at the chart centre, one in range and one beyond the drive range. */
const SYSTEMS: ReadonlyArray<{
  readonly relLy: readonly [number, number, number];
  readonly layer: MassLayer;
}> = [
  { relLy: [0, 0, 0], layer: "c" },
  { relLy: [0, 0, 10], layer: "a" },
  { relLy: [0, 0, -60], layer: "e" },
];

/** Plays the server's side, letting the outcomes it settles reach React. */
async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

/** The colours of the reticles in the latest paint: paths of four corner brackets. */
function reticleColours(recorder: RecordingContext2D): string[] {
  const colours: string[] = [];
  const records = recorder.records;
  const lastClear = records.findLastIndex(
    (record) => record.type === "call" && record.name === "fillRect",
  );
  let moves = 0;
  let stroke = "";
  for (const record of records.slice(lastClear)) {
    if (record.type === "set" && record.name === "strokeStyle") {
      stroke = String(record.value);
    } else if (record.type === "call" && record.name === "beginPath") {
      moves = 0;
    } else if (record.type === "call" && record.name === "moveTo") {
      moves += 1;
    } else if (record.type === "call" && record.name === "stroke" && moves === 4) {
      colours.push(stroke);
    }
  }
  return colours;
}

interface Rendered {
  readonly user: ReturnType<typeof userEvent.setup>;
  readonly socket: FakeWebSocket;
  readonly recorder: RecordingContext2D;
}

/** Renders the display with a universe open, its maps left pending: the chart needs neither. */
async function renderChart({ radiusLy = 80 } = {}): Promise<Rendered> {
  const user = userEvent.setup();
  const recorder = stubCanvas();
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: WIDTH_PX, height: HEIGHT_PX }),
  );
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
  await server(() => {
    socket.serverAnswers("list_universes", () => aUniverseList([aUniverse()]));
  });
  await user.click(screen.getByRole("button", { name: "Open universe SURVEY 1" }));
  await server(() => {
    socket.serverAnswers("open_universe", () => anOpenedUniverse(aUniverse()));
  });
  // The centre is typed, entered as the field is left, and published by CENTRE CHART.
  await user.clear(screen.getByRole("textbox", { name: "X" }));
  await user.type(screen.getByRole("textbox", { name: "X" }), "26000");
  await user.click(screen.getByRole("button", { name: "C CENTRE CHART" }));
  await answerQuery(socket, radiusLy);
  return { user, socket, recorder };
}

/** Answers the query in flight with the three fixture systems. */
async function answerQuery(socket: FakeWebSocket, radiusLy: number): Promise<void> {
  await server(() => {
    socket.serverAnswers("systems_in_range", (body) =>
      aSystemsInRange({
        centreLy: CENTRE,
        radiusLy,
        minLayer: body.min_layer,
        systems: SYSTEMS,
      }),
    );
  });
}

function chartPage(): HTMLElement {
  return screen.getByRole("tabpanel", { name: "LOCAL CHART" });
}

function canvas(): HTMLElement {
  return within(chartPage()).getByRole("application", { name: "Local chart" });
}

function options(): HTMLElement[] {
  return within(screen.getByRole("region", { name: /^Systems/u })).getAllByRole("option");
}

/** The text of the selected system readout's value for `label`. */
function reading(label: string): string {
  const readout = screen.getByRole("status", { name: "Selected system" });
  return within(readout).getByText(label, { exact: true }).nextElementSibling?.textContent ?? "";
}

describe("LocalChartPanel", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  it("says how to choose a centre before one is chosen", async () => {
    const user = userEvent.setup();
    stubCanvas();
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
    await server(() => {
      socket.serverAnswers("list_universes", () => aUniverseList([aUniverse()]));
    });
    await user.click(screen.getByRole("button", { name: "Open universe SURVEY 1" }));
    await server(() => {
      socket.serverAnswers("open_universe", () => anOpenedUniverse(aUniverse()));
    });

    await user.click(screen.getByRole("tab", { name: "LOCAL CHART" }));

    expect(
      within(chartPage()).getByText("NO CENTRE: pick on the map and press C"),
    ).toBeInTheDocument();
    expect(socket.requestsOfKind("systems_in_range")).toHaveLength(0);
  });

  it("shows the chart it centred, with its frame, centre and time", async () => {
    await renderChart();

    const page = chartPage();
    expect(page).toBeVisible();
    expect(within(page).getByText("FRAME").nextElementSibling).toHaveTextContent("GALACTIC");
    expect(within(page).getByText("RADIUS").nextElementSibling).toHaveTextContent("26,000.0 ly");
    expect(within(page).getByText("UT", { selector: "dt" }).nextElementSibling).toHaveTextContent(
      "+0.00 yr",
    );
  });

  it("lists every system the query returned, nearest first", async () => {
    await renderChart();

    expect(options().map((option) => option.textContent)).toEqual([
      "H7K 4C0RFZ C-10.001.63IN RANGE",
      "H7K 4C0RFZ A-210.000.29IN RANGE",
      "H7K 4C0RFZ E-360.0079.0OUT",
    ]);
  });

  it("fills the readout and draws the reticle when a system is selected in the list", async () => {
    const { user, recorder } = await renderChart();
    expect(reticleColours(recorder)).toEqual([]);

    await user.click(options()[1] ?? canvas());

    expect(reading("DESIG")).toBe("H7K 4C0RFZ A-2");
    expect(reading("NORTH")).toBe("+10.00 ly");
    expect(reticleColours(recorder)).toEqual(["#5cc8e6"]);
  });

  it("selects the system's row in the list when its symbol is clicked on the chart", async () => {
    const { user } = await renderChart();

    // The system at the chart centre projects at the centre of the canvas at every camera angle.
    await user.pointer({
      keys: "[MouseLeft]",
      target: canvas(),
      coords: { clientX: WIDTH_PX / 2, clientY: HEIGHT_PX / 2 },
    });

    expect(options()[0]).toHaveAttribute("aria-selected", "true");
    expect(reading("DESIG")).toBe("H7K 4C0RFZ C-1");
  });

  it("draws the drive range and the query edge apart, the range labelled as a setting", async () => {
    await renderChart();

    // Each curve label is one span whose figures are set apart, so the text is read as a whole.
    const labels = within(chartPage())
      .getAllByText(/ly/u)
      .map((element) => element.textContent);
    expect(labels).toContain("RANGE 50 ly SET");
    expect(labels).toContain("QUERY EDGE 80 ly");
    // Both curves of the range say so: the value is entered, not measured (ruling 10).
    expect(labels).toContain("PLANE 50 ly SET");
  });

  it("keeps all three curve labels at TOP, where the range sphere and its plane ring meet", async () => {
    // Reduced motion, so that the turn to the preset is instant.
    stubMatchMedia(true);
    const { user } = await renderChart();

    await user.keyboard("t");

    // Looking down the north axis, the plane ring projects onto the range sphere's outline, so the
    // two labels that grew a `SET` are drawn at nearly one place: `placeCurveLabels` must still
    // place both, and the query edge's, rather than dropping one where they collide. This is the
    // one part of T12.b's by-eye point (d) that a test can hold.
    const labels = within(chartPage())
      .getAllByText(/ly/u)
      .map((element) => element.textContent);
    expect(within(chartPage()).getByText("ELV").nextElementSibling).toHaveTextContent("+90°");
    expect(labels).toContain("RANGE 50 ly SET");
    expect(labels).toContain("PLANE 50 ly SET");
    expect(labels).toContain("QUERY EDGE 80 ly");
  });

  it("says what its symbols mean, and that they are not to scale", async () => {
    await renderChart();

    const legend = screen.getByRole("group", { name: "Chart legend" });
    for (const words of [
      "SYMBOLS NOT TO SCALE",
      "FILLED NORTH OF PLANE",
      "OPEN SOUTH OF PLANE",
      "ACCENT WITHIN DRIVE RANGE",
      "BRACKET SELECTED",
    ]) {
      expect(within(legend).getByText(words)).toBeInTheDocument();
    }
    // Size is shown as the five marks between the lightest and the heaviest initial mass.
    expect(legend.textContent).toContain("INIT MASS M");
    expect(legend.textContent).toContain("0.08");
    expect(legend.textContent).toContain("150");
  });

  it("asks again for a new centre, keeping every setting but the selection", async () => {
    const { user, socket } = await renderChart();
    await user.click(screen.getByRole("radio", { name: "0.5" }));
    await answerQuery(socket, 80);
    await user.click(options()[1] ?? canvas());
    expect(reading("DESIG")).toBe("H7K 4C0RFZ A-2");

    await user.click(screen.getByRole("tab", { name: "GALAXY MAP" }));
    await user.clear(screen.getByRole("textbox", { name: "Z" }));
    await user.type(screen.getByRole("textbox", { name: "Z" }), "12");
    await user.click(screen.getByRole("button", { name: "C CENTRE CHART" }));
    const sent = socket.requestsOfKind("systems_in_range").at(-1)?.body;
    // The old answer, its rows and its selection, stay on show until the new one arrives.
    expect(reading("DESIG")).toBe("H7K 4C0RFZ A-2");
    expect(options()).toHaveLength(3);
    await server(() => {
      socket.serverAnswers("systems_in_range", () =>
        aSystemsInRange({ centreLy: [26_000, 0, 12], radiusLy: 80, minLayer: "b", systems: [] }),
      );
    });

    expect(sent?.min_layer).toBe("b");
    expect(sent?.centre.cell_ly).toEqual([26_000, 0, 12]);
    // The system selected is not in the new answer, so the selection is gone.
    expect(reading("DESIG")).toBe("—");
    expect(screen.getByRole("radio", { name: "0.5" })).toBeChecked();
  });

  it("keeps the chart's own keys to itself while another page is shown", async () => {
    // The pages stay mounted so that none asks the server again, so the view's document keys (D3)
    // must act only while the chart is the page on show.
    stubMatchMedia(true);
    const { user } = await renderChart();
    expect(within(chartPage()).getByText("AZM").nextElementSibling).toHaveTextContent("030°");

    await user.click(screen.getByRole("tab", { name: "GALAXY MAP" }));
    await user.keyboard("t");
    await user.click(screen.getByRole("tab", { name: "LOCAL CHART" }));

    expect(within(chartPage()).getByText("AZM").nextElementSibling).toHaveTextContent("030°");
    expect(within(chartPage()).getByText("ELV").nextElementSibling).toHaveTextContent("+30°");
  });

  it("holds the chart's controls back with the link's reason and keeps the chart", async () => {
    const { socket } = await renderChart();

    await act(async () => {
      socket.close();
      await Promise.resolve();
    });

    const page = chartPage();
    expect(within(page).getByText("NO CARRIER")).toBeInTheDocument();
    expect(within(page).getByLabelText("QUERY RADIUS")).toHaveAccessibleDescription("NO CARRIER");
    expect(options()).toHaveLength(3);
  });

  it("marks the chart stale when the link no longer backs it", async () => {
    const { socket } = await renderChart();
    expect(
      within(chartPage()).getByRole("application", { name: "Local chart" }),
    ).toBeInTheDocument();

    await act(async () => {
      socket.close();
      await Promise.resolve();
    });

    // The guide's stale state: named so, muted, and marked with a trailing S in each block.
    expect(
      within(chartPage()).getByRole("application", { name: "Local chart, stale" }),
    ).toBeInTheDocument();
    expect(within(chartPage()).getAllByText("stale").length).toBeGreaterThan(0);
    expect(screen.getByRole("region", { name: "Systems stale" })).toBeInTheDocument();
  });

  it("reports a census it cannot read as a fault and asks again on RETRY", async () => {
    const user = userEvent.setup();
    const { socket } = await renderChart();

    await user.selectOptions(within(chartPage()).getByLabelText("QUERY RADIUS"), "100");
    await server(() => {
      socket.serverAnswers("systems_in_range", (body) => {
        const answer = aSystemsInRange({
          centreLy: CENTRE,
          radiusLy: 100,
          minLayer: body.min_layer,
        });
        return { ...answer, census: { ...answer.census, layers: answer.census.layers.slice(1) } };
      });
    });

    expect(
      within(chartPage()).getByText("CHART DATA INVALID: census incomplete"),
    ).toBeInTheDocument();
    expect(
      within(chartPage()).queryByRole("application", { name: "Local chart" }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByText("CHART DATA INVALID: retry on the LOCAL CHART page"),
    ).toBeInTheDocument();

    const sentBefore = socket.requestsOfKind("systems_in_range").length;
    await user.click(within(chartPage()).getByRole("button", { name: "RETRY" }));

    expect(socket.requestsOfKind("systems_in_range")).toHaveLength(sentBefore + 1);
  });
});
