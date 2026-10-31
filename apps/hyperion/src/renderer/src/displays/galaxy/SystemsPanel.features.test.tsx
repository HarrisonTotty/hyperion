/**
 * The `SYSTEMS` panel's second list, the chart's features, and the readout of the feature selected
 * in it (plan 09, P09.T41.c), with the chart's own hook behind it over a fake socket.
 */
import { type FeatureSummary, universeTimeFromYears } from "@hyperion/protocol";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { UniverseProvider } from "../../components/UniverseProvider";
import {
  aBlackHoleState,
  aCentreDetail,
  aClusterDetail,
  aFeaturesInRange,
  ASSOCIATION,
  GALACTIC_CENTRE,
  OPEN_CLUSTER,
  STAR_FORMING_REGION,
} from "../../test/featureFixtures";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { anOpenedUniverse, aSystemsInRange, aUniverseList } from "../../test/galaxyFixtures";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { statusLine } from "../../test/statusLine";
import { featureMarkId } from "./chartModel";
import { SystemsPanel } from "./SystemsPanel";
import { UniversePanel } from "./UniversePanel";
import { useLocalChart } from "./useLocalChart";

/** The golden's Sun-like point, the chart's centre. */
const CENTRE = [0, 26_000, 0] as const;

/** Plays the server's side, letting the outcomes it settles reach React. */
async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

/**
 * The chart, centred, and the panel beside it, as the display holds them, with a stand-in for a
 * pick on the chart's canvas of the golden's open cluster's mark.
 */
function Chart() {
  const chart = useLocalChart(CENTRE);
  return (
    <>
      <button
        type="button"
        onClick={() => {
          chart.selectMark(featureMarkId(OPEN_CLUSTER.id));
        }}
      >
        PICK ON CHART
      </button>
      <button
        type="button"
        onClick={() => {
          chart.setTime(100);
        }}
      >
        CHART AT +100 yr
      </button>
      <SystemsPanel chart={chart} onOpenSystem={() => undefined} />
    </>
  );
}

/** Opens a universe, which centres the chart, and answers its systems: two, 1 and 2 ly out. */
async function renderChart() {
  const user = userEvent.setup();
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(640);
  render(
    <ServerLinkHarness>
      <UniverseProvider>
        <UniversePanel expanded onToggle={() => undefined} />
        <Chart />
      </UniverseProvider>
    </ServerLinkHarness>,
  );
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  await server(() => {
    socket.serverAnswers("list_universes", () => aUniverseList());
  });
  await user.click(screen.getByRole("button", { name: "Open universe SURVEY 1" }));
  await server(() => {
    socket.serverAnswers("open_universe", () => anOpenedUniverse());
  });
  await server(() => {
    socket.serverAnswers("systems_in_range", (body) =>
      aSystemsInRange({
        centreLy: CENTRE,
        radiusLy: body.radius_ly,
        systems: [
          { relLy: [1, 0, 0], layer: "c" },
          { relLy: [2, 0, 0], layer: "c" },
        ],
      }),
    );
  });
  return { user, socket };
}

/** Answers the chart's features with `features`, for the chart's own centre, radius and time. */
async function answerFeatures(
  socket: FakeWebSocket,
  features: ReadonlyArray<FeatureSummary> = [OPEN_CLUSTER, STAR_FORMING_REGION, ASSOCIATION],
): Promise<void> {
  await server(() => {
    socket.serverAnswers("features_in_range", (body) =>
      aFeaturesInRange({ centreLy: CENTRE, radiusLy: body.radius_ly, features }),
    );
  });
}

function panel(): HTMLElement {
  return screen.getByRole("region", { name: "Systems" });
}

function featureList(): HTMLElement {
  return within(panel()).getByRole("listbox", { name: "Features by distance" });
}

/** The value beside a label in the feature readout. */
function featureReading(label: string): string | null | undefined {
  const readout = screen.getByRole("status", { name: "Selected feature" });
  const term = within(readout).queryAllByText(label, { selector: "dt" })[0];
  return term === undefined ? undefined : (term.nextElementSibling?.textContent ?? null);
}

describe("SystemsPanel's features (plan 09, P09.T41.c)", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  it("asks for the features beside the systems, for the same centre, radius and time", async () => {
    const { socket } = await renderChart();

    const [systems] = socket.requestsOfKind("systems_in_range");
    const [features] = socket.requestsOfKind("features_in_range");
    expect(features?.body.centre).toEqual(systems?.body.centre);
    expect(features?.body.radius_ly).toBe(systems?.body.radius_ly);
    expect(features?.body.time).toEqual(systems?.body.time);
  });

  it("shows the systems' list first, and the features' on FEATURES, saying FEATURES PENDING meanwhile", async () => {
    const { user } = await renderChart();

    expect(within(panel()).getByRole("button", { name: "SYSTEMS", pressed: true })).toBeVisible();
    expect(within(panel()).getByRole("listbox", { name: "Systems by distance" })).toBeVisible();
    await user.click(within(panel()).getByRole("button", { name: "FEATURES" }));

    expect(within(panel()).getByText(statusLine("FEATURES PENDING"))).toBeInTheDocument();
    expect(within(panel()).queryByRole("listbox", { name: "Systems by distance" })).toBeNull();
  });

  it("lists the features nearest first once they arrive, and what the census leaves out", async () => {
    const { user, socket } = await renderChart();
    await answerFeatures(socket);

    await user.click(within(panel()).getByRole("button", { name: "FEATURES" }));

    expect(
      within(featureList())
        .getAllByRole("option")
        .map((row) => row.querySelector(".system-list__designation")?.textContent),
    ).toEqual(["GPF F207", "GPG F336", "GPG F530"]);
    expect(
      within(panel()).getByText(statusLine("STREAM AND DWARF CORE: NOT YET MODELLED")),
    ).toBeVisible();
  });

  it("moves between the two lists with the arrow keys, one stop in the tab order", async () => {
    const { user, socket } = await renderChart();
    await answerFeatures(socket);
    const systems = within(panel()).getByRole("button", { name: "SYSTEMS" });

    systems.focus();
    await user.keyboard("{ArrowRight}");

    const features = within(panel()).getByRole("button", { name: "FEATURES", pressed: true });
    expect(features).toHaveFocus();
    expect(systems).toHaveAttribute("tabindex", "-1");
    expect(featureList()).toBeVisible();
  });

  it("selects a feature with the list's keys, reads it, and asks for its structure", async () => {
    const { user, socket } = await renderChart();
    await answerFeatures(socket);
    await user.click(within(panel()).getByRole("button", { name: "FEATURES" }));

    featureList().focus();
    await user.keyboard("{ArrowDown}");

    expect(within(featureList()).getByRole("option", { selected: true })).toHaveAccessibleName(
      /^GPG F336,/u,
    );
    expect(featureReading("DESIG")).toBe("GPG F336");
    expect(socket.requestsOfKind("feature_detail").at(-1)?.body.feature).toBe(
      STAR_FORMING_REGION.id,
    );
    expect(within(panel()).getByText(statusLine("STRUCTURE PENDING"))).toBeInTheDocument();
    // A feature is no system to open.
    expect(within(panel()).getByText("NO SYSTEM SELECTED")).toBeInTheDocument();
    expect(socket.requestsOfKind("black_hole_state")).toHaveLength(0);
  });

  it("reads a cluster's structure once its detail answers", async () => {
    const { user, socket } = await renderChart();
    await answerFeatures(socket);
    await user.click(within(panel()).getByRole("button", { name: "FEATURES" }));
    featureList().focus();
    await user.keyboard("{Home}");

    await server(() => {
      socket.serverAnswers("feature_detail", () => aClusterDetail());
    });

    expect(screen.getByRole("heading", { name: "STRUCTURE" })).toBeInTheDocument();
    // An answer for +100 yr, the chart being at the epoch: a snapshot of its own.
    expect(featureReading("TIDAL RADIUS")).toMatch(/^26\.9 ly S/u);
  });

  it("asks for the black hole with the centre selected, and reads it under BLACK HOLE", async () => {
    const { user, socket } = await renderChart();
    await answerFeatures(socket, [GALACTIC_CENTRE]);
    await user.click(within(panel()).getByRole("button", { name: "FEATURES" }));
    featureList().focus();
    await user.keyboard("{Home}");

    await server(() => {
      socket.serverAnswers("feature_detail", () => aCentreDetail());
      socket.serverAnswers("black_hole_state", () => aBlackHoleState());
    });

    expect(screen.getByRole("heading", { name: "BLACK HOLE" })).toBeInTheDocument();
    expect(featureReading("X-RAY LUMINOSITY")).toBe("1.69E28 W");
  });

  it("goes back to the systems' list and readout when a system is selected there", async () => {
    const { user, socket } = await renderChart();
    await answerFeatures(socket);
    await user.click(within(panel()).getByRole("button", { name: "FEATURES" }));
    featureList().focus();
    await user.keyboard("{Home}");

    await user.click(within(panel()).getByRole("button", { name: "SYSTEMS" }));
    const systems = within(panel()).getByRole("listbox", { name: "Systems by distance" });
    systems.focus();
    await user.keyboard("{Home}");

    expect(screen.getByRole("status", { name: "Selected system" })).toBeInTheDocument();
    expect(screen.queryByRole("status", { name: "Selected feature" })).toBeNull();
  });

  it("shows the features' list when a feature is picked on the chart, so the pick shows in both", async () => {
    const { user, socket } = await renderChart();
    await answerFeatures(socket);

    await user.click(screen.getByRole("button", { name: "PICK ON CHART" }));

    expect(within(panel()).getByRole("button", { name: "FEATURES", pressed: true })).toBeVisible();
    expect(within(featureList()).getByRole("option", { selected: true })).toHaveAccessibleName(
      /^GPF F207,/u,
    );
    expect(featureReading("KIND")).toBe("OPEN CLUSTER");
  });

  it("draws and lists no features from an answer for another chart than the one on show", async () => {
    const { user, socket } = await renderChart();
    await server(() => {
      socket.serverAnswers("features_in_range", (body) =>
        aFeaturesInRange({ centreLy: CENTRE, radiusLy: body.radius_ly * 2 }),
      );
    });

    await user.click(within(panel()).getByRole("button", { name: "FEATURES" }));

    expect(within(panel()).getByText(statusLine("FEATURES PENDING"))).toBeInTheDocument();
    expect(within(panel()).queryByRole("listbox", { name: "Features by distance" })).toBeNull();
  });

  it("keeps the selected feature, stale, while a new chart's features are awaited", async () => {
    const { user, socket } = await renderChart();
    await answerFeatures(socket);
    await user.click(screen.getByRole("button", { name: "PICK ON CHART" }));
    await server(() => {
      socket.serverAnswers("feature_detail", () => ({
        ...aClusterDetail(),
        time: { seconds: 0, nanos: 0 },
      }));
    });
    expect(featureReading("TIDAL RADIUS")).toBe("26.9 ly");

    await user.click(screen.getByRole("button", { name: "CHART AT +100 yr" }));
    await server(() => {
      socket.serverAnswers("systems_in_range", (body) =>
        aSystemsInRange({
          centreLy: CENTRE,
          radiusLy: body.radius_ly,
          timeYr: 100,
          systems: [{ relLy: [1, 0, 0], layer: "c" }],
        }),
      );
    });

    // The systems are the new chart's and its features are awaited: the feature stays selected,
    // read from the answer before as a snapshot, its structure too, for a time the chart has left.
    expect(within(panel()).getByRole("button", { name: "FEATURES", pressed: true })).toBeVisible();
    expect(featureReading("DESIG")).toMatch(/^GPF F207 S/u);
    expect(featureReading("TIDAL RADIUS")).toMatch(/^26\.9 ly S/u);
    expect(socket.requestsOfKind("feature_detail").at(-1)?.body.time).toEqual({
      seconds: 3_155_760_000,
      nanos: 0,
    });

    await server(() => {
      socket.serverAnswers("features_in_range", (body) =>
        aFeaturesInRange({ centreLy: CENTRE, radiusLy: body.radius_ly, timeYr: 100 }),
      );
    });
    expect(featureReading("DESIG")).toBe("GPF F207");
  });

  it("keeps the structure, stale, when its request at the chart's new time fails, with RETRY", async () => {
    const { user, socket } = await renderChart();
    await answerFeatures(socket);
    await user.click(screen.getByRole("button", { name: "PICK ON CHART" }));
    await server(() => {
      socket.serverAnswers("feature_detail", () => ({
        ...aClusterDetail(),
        time: { seconds: 0, nanos: 0 },
      }));
    });
    await user.click(screen.getByRole("button", { name: "CHART AT +100 yr" }));
    await server(() => {
      socket.serverAnswers("systems_in_range", (body) =>
        aSystemsInRange({ centreLy: CENTRE, radiusLy: body.radius_ly, timeYr: 100 }),
      );
      socket.serverAnswers("features_in_range", (body) =>
        aFeaturesInRange({ centreLy: CENTRE, radiusLy: body.radius_ly, timeYr: 100 }),
      );
    });
    // The structure is asked again at the chart's new time, and that request fails.
    const request = socket.requestsOfKind("feature_detail").at(-1);
    expect(request?.body.time).toEqual({ seconds: 3_155_760_000, nanos: 0 });
    await server(() => {
      socket.serverRejects(request?.id ?? 0, { code: "internal", message: "failed", field: null });
    });

    expect(featureReading("TIDAL RADIUS")).toMatch(/^26\.9 ly S/u);
    expect(within(panel()).getByText(statusLine("STRUCTURE REJECTED: failed"))).toBeInTheDocument();
  });

  it("reads the black hole stale once the chart's time has left the one it was answered for", async () => {
    const { user, socket } = await renderChart();
    await answerFeatures(socket, [GALACTIC_CENTRE]);
    await user.click(within(panel()).getByRole("button", { name: "FEATURES" }));
    featureList().focus();
    await user.keyboard("{Home}");
    await server(() => {
      socket.serverAnswers("black_hole_state", () => aBlackHoleState());
    });
    expect(featureReading("X-RAY LUMINOSITY")).toBe("1.69E28 W");

    await user.click(screen.getByRole("button", { name: "CHART AT +100 yr" }));
    await server(() => {
      socket.serverAnswers("systems_in_range", (body) =>
        aSystemsInRange({ centreLy: CENTRE, radiusLy: body.radius_ly, timeYr: 100 }),
      );
      socket.serverAnswers("features_in_range", (body) =>
        aFeaturesInRange({
          centreLy: CENTRE,
          radiusLy: body.radius_ly,
          timeYr: 100,
          features: [GALACTIC_CENTRE],
        }),
      );
    });

    expect(featureReading("X-RAY LUMINOSITY")).toMatch(/^1\.69E28 W S/u);
    // Asked again at the chart's new time, which its answer is read at (plan 09, P09.T42).
    expect(socket.requestsOfKind("black_hole_state").at(-1)?.body.time).toEqual(
      universeTimeFromYears(100),
    );
  });
});
