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

/** A chart centre off the galactic axis, typed into `CURSOR`: the observer `OBSERVED` takes. */
const OFF_AXIS_LY = [8_000, 6_000, -12] as const;

/** Types `centreLy` into `CURSOR`'s fields and centres the chart there with `CENTRE CHART`. */
async function centreAt(
  { user }: Rendered,
  [xLy, yLy, zLy]: readonly [number, number, number],
): Promise<void> {
  const cursor = screen.getByRole("region", { name: "Cursor" });
  const enter = async (label: "X" | "Y" | "Z", valueLy: number): Promise<void> => {
    const field = within(cursor).getByRole("textbox", { name: label });
    await user.clear(field);
    await user.type(field, `${valueLy}{Enter}`);
  };
  await enter("X", xLy);
  await enter("Y", yLy);
  await enter("Z", zLy);
  await user.click(within(cursor).getByRole("button", { name: "C CENTRE CHART" }));
}

/**
 * Answers the latest range query with one system 1 ly from `centreLy`, the centre asked, carrying
 * what the observer at the centre receives where the query was asked observed.
 */
async function answerChartAt(
  { socket }: Rendered,
  centreLy: readonly [number, number, number],
): Promise<void> {
  await server(() => {
    socket.serverAnswers("systems_in_range", (body) => ({
      ...aSystemsInRange({
        universe: body.universe,
        centreLy,
        systems: [
          {
            relLy: [1, 0, 0],
            layer: "c",
            ...(body.mode?.type === "observed"
              ? { observed: { lightAgeYr: 1, apparentRelLy: [1, 0, 0] } }
              : {}),
          },
        ],
      }),
      time: body.time,
    }));
  });
}

function hrPage(): HTMLElement {
  return screen.getByRole("tabpanel", { name: "HR DIAGRAM" });
}

/** The value `label` reads on a page: the chart's furniture, or the HR page's mode line. */
function valueOn(page: HTMLElement, label: string): string {
  return within(page).getByText(label, { selector: "dt" }).nextElementSibling?.textContent ?? "";
}

/** The `MODE` reading's value on a page: its name, then any observer. */
function modeOn(page: HTMLElement): Element | null {
  return within(page).getByText("MODE", { selector: "dt" }).nextElementSibling;
}

async function showPage({ user }: Rendered, name: "LOCAL CHART" | "HR DIAGRAM"): Promise<void> {
  await user.click(screen.getByRole("tab", { name }));
}

/**
 * Opens `SURVEY 1`, centres the chart off the galactic axis, and observes from that centre: the
 * chart is answered in `now`, then `OBSERVED` is chosen and answered so, the `LOCAL CHART` on show.
 */
async function observeOffAxis(rendered: Rendered): Promise<void> {
  await server(() => {
    rendered.socket.serverAnswers("list_universes", () => aUniverseList([SURVEY_1]));
  });
  await open(rendered);
  await centreAt(rendered, OFF_AXIS_LY);
  await answerChartAt(rendered, OFF_AXIS_LY);
  await rendered.user.click(within(chartPage()).getByRole("button", { name: "OBSERVED" }));
  await answerChartAt(rendered, OFF_AXIS_LY);
}

describe("the HR DIAGRAM page's mode (plan 12, P12.T11.b)", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  it("reads OBSERVED FROM the chart's observer once the chart is answered so", async () => {
    const rendered = renderDisplay();
    await observeOffAxis(rendered);

    await showPage(rendered, "HR DIAGRAM");

    expect(modeOn(hrPage())).toHaveTextContent(/^OBSERVED FROM/u);
    expect(valueOn(hrPage(), "FRAME")).toBe("GALACTIC");
    expect(valueOn(hrPage(), "RADIUS")).toBe("10,000.0 ly");
    expect(valueOn(hrPage(), "ANGLE")).toBe("036.9°");
    expect(valueOn(hrPage(), "HEIGHT")).toBe("-12.0 ly");
  });

  it("reads the answer's mode, observer and time as the chart's furniture does", async () => {
    const rendered = renderDisplay();
    await observeOffAxis(rendered);
    const chart = { mode: valueOn(chartPage(), "MODE"), time: valueOn(chartPage(), "UT") };

    await showPage(rendered, "HR DIAGRAM");

    expect({ mode: valueOn(hrPage(), "MODE"), time: valueOn(hrPage(), "UT") }).toEqual(chart);
  });

  it("keeps the answer's OBSERVED FROM while NOW is chosen and not yet answered", async () => {
    const rendered = renderDisplay();
    await observeOffAxis(rendered);
    await rendered.user.click(within(chartPage()).getByRole("button", { name: "NOW" }));

    await showPage(rendered, "HR DIAGRAM");

    expect(modeOn(hrPage())).toHaveTextContent(/^OBSERVED FROM/u);
  });

  it("reads MODE NOW once NOW is answered", async () => {
    const rendered = renderDisplay();
    await observeOffAxis(rendered);
    await rendered.user.click(within(chartPage()).getByRole("button", { name: "NOW" }));
    await answerChartAt(rendered, OFF_AXIS_LY);

    await showPage(rendered, "HR DIAGRAM");

    expect(valueOn(hrPage(), "MODE")).toBe("NOW");
    expect(within(hrPage()).queryByText("OBSERVED FROM")).not.toBeInTheDocument();
  });

  it("reads NOW for an answer asked observed whose rows all came without their observation", async () => {
    const rendered = renderDisplay();
    const { user } = rendered;
    await server(() => {
      rendered.socket.serverAnswers("list_universes", () => aUniverseList([SURVEY_1]));
    });
    await open(rendered);
    await user.keyboard("c");
    await answerChart(rendered);
    await user.click(within(chartPage()).getByRole("button", { name: "OBSERVED" }));
    expect(rendered.socket.requestsOfKind("systems_in_range").at(-1)?.body.mode?.type).toBe(
      "observed",
    );
    // As a server that ignores the mode answers: every row without its observation.
    await answerChart(rendered);

    await showPage(rendered, "HR DIAGRAM");

    expect(valueOn(hrPage(), "MODE")).toBe("NOW");
    expect(within(hrPage()).queryByText("OBSERVED FROM")).not.toBeInTheDocument();
  });
});
