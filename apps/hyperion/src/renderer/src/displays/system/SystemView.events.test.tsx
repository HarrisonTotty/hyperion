/**
 * The `SYSTEM` display's events over a fake socket, against the shared wire fixture (plan 14,
 * P14.T43.c): the century asked about, each event's time and countdown, the keys, the selection of
 * an event's body, and the panel's states.
 */
import {
  type BodyEventDto,
  type ResponseFor,
  SECONDS_PER_JULIAN_YEAR,
  type UniverseTime,
} from "@hyperion/protocol";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { fakeFramesAndTimeouts } from "../../test/fakeFramesAndTimeouts";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import {
  FIXTURE_JUPITER,
  jupiterDetail,
  noEvents,
  populatedEvents,
  sliceBodies,
} from "../../test/planetaryFixture";
import { stubCanvas } from "../../test/RecordingContext2D";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import {
  anUnsupportedEventsError,
  aNotYetBornSummary,
  aSingleStarSummary,
  aSummaryResponse,
  aSystemTarget,
  PIN_SYSTEM,
} from "../../test/systemFixtures";
import { SystemView } from "./SystemView";

const WIDTH_PX = 1214;
const HEIGHT_PX = 625;

/** The epoch, the fixture's bodies' time and the middle of its events' century. */
const EPOCH: UniverseTime = { seconds: 0, nanos: 0 };

/** Seconds in a day. */
const DAY_S = 86_400;

/** The time `years` Julian years from the epoch. */
function years(count: number): UniverseTime {
  return { seconds: count * SECONDS_PER_JULIAN_YEAR, nanos: 0 };
}

/** The `from` of the century about the epoch, which the display asks for first. */
const CENTURY_FROM = years(-50);

/** Plays the server's side, letting the outcomes it settles reach React. */
async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

/** Runs the frames the display steps its time on. */
function nextFrame(): void {
  act(() => {
    vi.advanceTimersToNextFrame();
  });
}

/**
 * The display opened at the epoch on the fixture's system, its link up; with `frames`, its frames
 * and timeouts faked, so that a step of its time can be run.
 */
function renderView(frames = false) {
  const user = frames
    ? userEvent.setup({ advanceTimers: fakeFramesAndTimeouts() })
    : userEvent.setup();
  stubCanvas();
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: WIDTH_PX, height: HEIGHT_PX }),
  );
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(HEIGHT_PX);
  render(
    <ServerLinkHarness>
      <SystemView target={aSystemTarget({ time: EPOCH })} />
    </ServerLinkHarness>,
  );
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  return { user, socket };
}

/** Answers the hosts and the slice's bodies. */
async function answerSystem(socket: FakeWebSocket): Promise<void> {
  await server(() => {
    socket.serverAnswers("system_summary", () => aSummaryResponse(aSingleStarSummary()));
  });
  await server(() => {
    socket.serverAnswers("system_bodies", () => sliceBodies());
  });
}

/** A busy system's answer: two days either side of the epoch, of the century asked about. */
function cutAnswer(): ResponseFor<"body_events"> {
  return {
    ...populatedEvents(),
    listed_from: { seconds: -2 * DAY_S, nanos: 0 },
    listed_to: { seconds: 2 * DAY_S, nanos: 0 },
    events: [],
  };
}

/** Answers the latest events request with `events`, or `unsupported` for `null`. */
async function answerEvents(
  socket: FakeWebSocket,
  events: ResponseFor<"body_events"> | null = populatedEvents(),
): Promise<void> {
  await server(() => {
    const [request] = socket.requestsOfKind("body_events").toReversed();
    if (request === undefined) {
      throw new Error("the display asks for its events");
    }
    if (events === null) {
      socket.serverRejects(request.id, anUnsupportedEventsError());
    } else {
      socket.serverResponds(request.id, events);
    }
  });
}

/** Answers the hosts and the slice's bodies, then the events with `events`, or `unsupported`. */
async function answer(
  socket: FakeWebSocket,
  events: ResponseFor<"body_events"> | null = populatedEvents(),
): Promise<void> {
  await answerSystem(socket);
  await answerEvents(socket, events);
}

function eventsPanel(): HTMLElement {
  return screen.getByRole("region", { name: /^Events/u });
}

function eventList(): HTMLElement {
  return within(eventsPanel()).getByRole("listbox", { name: "Events" });
}

function options(): HTMLElement[] {
  return within(eventList()).getAllByRole("option");
}

function optionNames(): Array<string | null> {
  return options().map((option) => option.getAttribute("aria-label"));
}

/** The name of the row the keys have reached, the list's active descendant. */
function activeName(): string | null {
  const id = eventList().getAttribute("aria-activedescendant");
  const active = options().find((option) => option.id === id);
  return active?.getAttribute("aria-label") ?? null;
}

/** The kind of the row the keys have reached, as its name reads it. */
function activeKind(): string | null {
  return activeName()?.split(", ")[2] ?? null;
}

/** The name of the event chosen, whose body the display selected; `null` for none. */
function chosenName(): string | null {
  return (
    within(eventList()).queryByRole("option", { selected: true })?.getAttribute("aria-label") ??
    null
  );
}

/**
 * `count` impacts on the Jupiter a day apart, centred on `centre`, the first `count / 2` before it;
 * `word` sets their IDs apart from another list's.
 */
function impacts(centre: UniverseTime, count: number, word: number): BodyEventDto[] {
  const impact = populatedEvents().events.find((event) => event.type === "impact");
  if (impact?.type !== "impact") {
    throw new Error("the fixture holds an impact");
  }
  return Array.from({ length: count }, (_, index) => ({
    ...impact,
    id: `${impact.body}:0400${(word + index).toString(16).padStart(12, "0")}`,
    time: { seconds: centre.seconds + (index - count / 2) * DAY_S, nanos: 0 },
  }));
}

/** Steps the display time three times by 10 yr, to +30 yr, past the middle half of the century. */
async function stepThirtyYears(user: ReturnType<typeof userEvent.setup>): Promise<void> {
  await user.keyboard("5]");
  nextFrame();
  await user.keyboard("]");
  nextFrame();
  await user.keyboard("]");
  nextFrame();
}

/** The answer about +30 yr, listing `events`. */
function answerAboutThirtyYears(events: BodyEventDto[]): ResponseFor<"body_events"> {
  return {
    ...populatedEvents(),
    from: years(-20),
    to: years(80),
    listed_from: years(-20),
    listed_to: years(80),
    events,
  };
}

/** The reading a label of the panel's span readout holds. */
function reading(label: string): string | null {
  const term = within(eventsPanel())
    .queryAllByRole("term")
    .find((candidate) => candidate.textContent === label);
  return term?.nextElementSibling?.textContent ?? null;
}

beforeEach(() => {
  FakeWebSocket.instances = [];
  vi.stubGlobal("WebSocket", FakeWebSocket);
});

afterEach(() => {
  vi.useRealTimers();
});

describe("SystemView's events", () => {
  it("asks for the century around the display time once the system is known", async () => {
    const { socket } = renderView();
    expect(socket.requestsOfKind("body_events")).toEqual([]);

    await answerSystem(socket);

    expect(socket.requestsOfKind("body_events").map((request) => request.body)).toEqual([
      {
        kind: "body_events",
        universe: aSystemTarget().universe,
        system: PIN_SYSTEM,
        from: CENTURY_FROM,
        to: years(50),
      },
    ]);
  });

  it("reads the century listed whole as LISTED, with no ASKED", async () => {
    const { socket } = renderView();
    await answer(socket);

    expect([reading("LISTED"), reading("ASKED")]).toEqual([
      "UT -50 yr 000/00:00:00 – +50 yr 000/00:00:00",
      null,
    ]);
  });

  it("counts T+ for an event before the display time and T- for one after it, each with its time system", async () => {
    const { socket } = renderView();
    await answer(socket);

    expect(optionNames()).toEqual([
      "UT -49 yr 360/17:36:12, T+49 yr 360/17:36:12, COMET, body not listed",
      "UT -48 yr 350/03:18:35, T+48 yr 350/03:18:35, ERUPTION, H7K 4C0RFZ D-7 /768",
      "UT -1 yr 343/06:14:10, T+1 yr 343/06:14:10, IMPACT, H7K 4C0RFZ D-7 /1280",
      "UT +0 yr 290/16:48:00, T-0 yr 290/16:48:00, DUST STORM, body not listed",
      "UT +6 yr 102/22:22:51, T-6 yr 102/22:22:51, GREAT STORM, H7K 4C0RFZ D-7 /1280",
    ]);
  });

  it("shows each event's time and countdown in its row", async () => {
    const { socket } = renderView();
    await answer(socket);

    const storm = within(eventList()).getByRole("option", { name: /GREAT STORM/u });
    expect(
      within(storm)
        .getAllByText(/^(?:UT \+6 yr 102\/22:22:51|T-6 yr 102\/22:22:51)$/u)
        .map((cell) => cell.textContent),
    ).toEqual(["UT +6 yr 102/22:22:51", "T-6 yr 102/22:22:51"]);
  });

  it("opens on the next event", async () => {
    const { socket } = renderView();
    await answer(socket);

    expect(activeName()).toMatch(/^UT \+0 yr 290\/16:48:00, .*DUST STORM/u);
  });

  it("shows the position and total", async () => {
    const { socket } = renderView();
    await answer(socket);

    expect(within(eventsPanel()).getByText("1-5 of 5")).toBeInTheDocument();
  });

  it("moves the active row with the arrow, page and end keys", async () => {
    const { user, socket } = renderView();
    await answer(socket);

    await user.click(eventList());
    const reached: Array<string | null> = [];
    await user.keyboard("{ArrowDown}");
    reached.push(activeKind());
    await user.keyboard("{Home}");
    reached.push(activeKind());
    await user.keyboard("{End}");
    reached.push(activeKind());
    await user.keyboard("{ArrowUp}");
    reached.push(activeKind());
    await user.keyboard("{PageUp}");
    reached.push(activeKind());
    await user.keyboard("{PageDown}");
    reached.push(activeKind());

    expect(reached).toEqual([
      "GREAT STORM",
      "COMET",
      "GREAT STORM",
      "DUST STORM",
      "COMET",
      "GREAT STORM",
    ]);
  });

  it("chooses nothing with the keys that move the active row", async () => {
    const { user, socket } = renderView();
    await answer(socket);

    await user.click(eventList());
    await user.keyboard("{ArrowDown}{Home}{End}{PageUp}{PageDown}");

    expect([chosenName(), socket.requestsOfKind("body_detail")]).toEqual([null, []]);
  });

  it("selects an event's body with Enter", async () => {
    const { user, socket } = renderView();
    await answer(socket);

    await user.click(eventList());
    await user.keyboard("{ArrowDown}{Enter}");

    expect(socket.requestsOfKind("body_detail").map((request) => request.body.body)).toEqual([
      FIXTURE_JUPITER,
    ]);
  });

  it("shows the body an event selects in the body list and the readout", async () => {
    const { user, socket } = renderView();
    await answer(socket);

    await user.click(eventList());
    await user.keyboard("{ArrowDown}{Enter}");
    await server(() => {
      socket.serverAnswers("body_detail", () => jupiterDetail());
    });

    expect([
      within(screen.getByRole("tree", { name: "Bodies" }))
        .getByRole("treeitem", { selected: true })
        .getAttribute("aria-label"),
      screen.getByRole("status", { name: "Selected body" }).textContent.includes("/1280"),
      chosenName()?.includes("GREAT STORM"),
    ]).toEqual(["H7K 4C0RFZ D-7 /1280, PLANET, SMA 5.20 AU", true, true]);
  });

  it("selects an event's body with a click", async () => {
    const { user, socket } = renderView();
    await answer(socket);

    await user.click(within(eventList()).getByRole("option", { name: /IMPACT/u }));

    expect(socket.requestsOfKind("body_detail").map((request) => request.body.body)).toEqual([
      FIXTURE_JUPITER,
    ]);
  });

  it("drops the chosen event's selected look once the display selects another body", async () => {
    const { user, socket } = renderView();
    await answer(socket);
    await user.click(within(eventList()).getByRole("option", { name: /IMPACT/u }));
    expect(chosenName()).toMatch(/IMPACT/u);

    await user.click(
      within(screen.getByRole("tree", { name: "Bodies" })).getByRole("treeitem", {
        name: /^H7K 4C0RFZ D-7 \/768,/u,
      }),
    );

    expect(chosenName()).toBeNull();
  });

  it("selects nothing with Space for an event whose body is not listed", async () => {
    const { user, socket } = renderView();
    await answer(socket);

    // The list opens on the dust storm, whose body is not among the bodies on show: selecting it
    // would select the display's fallback, the primary, in its place.
    await user.click(eventList());
    await user.keyboard(" ");

    expect([chosenName(), socket.requestsOfKind("body_detail")]).toEqual([null, []]);
  });

  it("disables a row whose body is not listed, and says why while it is the active row", async () => {
    const { socket } = renderView();
    await answer(socket);

    const dust = within(eventList()).getByRole("option", { name: /DUST STORM/u });
    expect([
      dust.getAttribute("aria-disabled"),
      within(eventsPanel()).queryByText("BODY NOT LISTED") !== null,
    ]).toEqual(["true", true]);
  });

  it("says nothing of a body not listed while the active row's body is", async () => {
    const { user, socket } = renderView();
    await answer(socket);

    await user.click(eventList());
    await user.keyboard("{ArrowDown}");

    expect(within(eventsPanel()).queryByText("BODY NOT LISTED")).toBeNull();
  });

  it("renders the last row once End reaches it, far below the window", async () => {
    const { user, socket } = renderView();
    await answer(socket, { ...populatedEvents(), events: impacts(EPOCH, 100, 0) });
    expect(optionNames().length).toBeLessThan(100);

    await user.click(eventList());
    await user.keyboard("{End}");

    expect(activeName()).toMatch(/^UT \+0 yr 049\/00:00:00, /u);
  });

  it("keeps the active row rendered when the list is scrolled away from it", async () => {
    const { user, socket } = renderView();
    await answer(socket, { ...populatedEvents(), events: impacts(EPOCH, 100, 0) });
    await user.click(eventList());
    await user.keyboard("{End}");

    fireEvent.scroll(eventList(), { target: { scrollTop: 0 } });

    expect([options()[0]?.getAttribute("aria-posinset"), activeName()]).toEqual([
      "1",
      expect.stringMatching(/^UT \+0 yr 049\/00:00:00, /u),
    ]);
  });

  it("counts down from the display time as it steps", async () => {
    const { user, socket } = renderView(true);
    await answer(socket);

    await user.keyboard("]");
    nextFrame();

    expect(optionNames()[3]).toMatch(/T-0 yr 289\/16:48:00/u);
  });

  it("asks nothing while the display time stays in the middle half of the span listed", async () => {
    const { user, socket } = renderView(true);
    await answer(socket);

    await user.keyboard("]");
    nextFrame();

    expect(socket.requestsOfKind("body_events")).toHaveLength(1);
  });

  it("reads LISTED and ASKED where a busy system's list holds less than the century", async () => {
    const { socket } = renderView();
    await answer(socket, cutAnswer());

    expect([reading("LISTED"), reading("ASKED")]).toEqual([
      "UT -0 yr 002/00:00:00 – +0 yr 002/00:00:00",
      "UT -50 yr 000/00:00:00 – +50 yr 000/00:00:00",
    ]);
  });

  it("asks about the display time again once it leaves the middle half of the span listed", async () => {
    const { user, socket } = renderView(true);
    await answer(socket, cutAnswer());

    await user.keyboard("]");
    nextFrame();
    expect(socket.requestsOfKind("body_events")).toHaveLength(1);
    await user.keyboard("]");
    nextFrame();

    expect(socket.requestsOfKind("body_events").map((request) => request.body.from)).toEqual([
      CENTURY_FROM,
      { seconds: CENTURY_FROM.seconds + 2 * DAY_S, nanos: 0 },
    ]);
  });

  it("keeps the event the keys reached through a newer answer that still lists it", async () => {
    const { user, socket } = renderView(true);
    await answer(socket);
    await user.click(eventList());
    await user.keyboard("{Home}{ArrowDown}{ArrowDown}");
    expect(activeName()).toMatch(/IMPACT/u);

    // The impact and the two storms, all passed by +30 yr.
    await stepThirtyYears(user);
    await answerEvents(
      socket,
      answerAboutThirtyYears(
        populatedEvents().events.filter(
          (event) =>
            event.type === "impact" || event.type === "dust_storm" || event.type === "storm",
        ),
      ),
    );

    expect(activeName()).toMatch(/IMPACT/u);
  });

  it("opens a newer answer that no longer lists the event the keys reached on the next event", async () => {
    const { user, socket } = renderView(true);
    await answer(socket);
    await user.click(eventList());
    await user.keyboard("{Home}");

    await stepThirtyYears(user);
    await answerEvents(socket, answerAboutThirtyYears(impacts(years(30), 100, 0x1000)));

    expect(activeName()).toMatch(/^UT \+30 yr 000\/00:00:00, /u);
  });

  it("asks about the display time once a system not yet formed is, not about the time it opened at", async () => {
    // Formed 36 hours after the opening time: an age of -36 h in megayears.
    const ageMyr = -(36 * 3_600) / (1e6 * SECONDS_PER_JULIAN_YEAR);
    const { user, socket } = renderView(true);
    await server(() => {
      socket.serverAnswers("system_summary", () =>
        aSummaryResponse(aNotYetBornSummary({ age_myr: ageMyr, time: EPOCH })),
      );
    });
    expect(within(eventsPanel()).getByText("NOT YET FORMED")).toBeInTheDocument();

    await user.keyboard("]");
    nextFrame();
    await user.keyboard("]");
    nextFrame();
    await answerSystem(socket);

    expect(socket.requestsOfKind("body_events").map((request) => request.body.from)).toEqual([
      { seconds: CENTURY_FROM.seconds + 2 * DAY_S, nanos: 0 },
    ]);
  });

  it("names the panel stale when the link is lost", async () => {
    const { socket } = renderView();
    await answer(socket);

    await server(() => {
      socket.close();
    });

    expect(screen.getByRole("region", { name: "Events stale" })).toBeInTheDocument();
  });

  it("names each row stale when the link is lost", async () => {
    const { socket } = renderView();
    await answer(socket);

    await server(() => {
      socket.close();
    });

    expect(optionNames().filter((name) => name?.endsWith(", stale") !== true)).toEqual([]);
  });

  it("marks the span listed stale when the link is lost", async () => {
    const { socket } = renderView();
    await answer(socket);

    await server(() => {
      socket.close();
    });

    expect(reading("LISTED")).toBe("UT -50 yr 000/00:00:00 – +50 yr 000/00:00:00 Sstale");
  });

  it("offers RETRY when the first request fails, which asks again", async () => {
    const { user, socket } = renderView();
    await answerSystem(socket);
    await server(() => {
      const [request] = socket.requestsOfKind("body_events");
      if (request === undefined) {
        throw new Error("the display asks for its events");
      }
      socket.serverRejects(request.id, { code: "internal", message: "pool gone", field: null });
    });

    await user.click(within(eventsPanel()).getByRole("button", { name: "RETRY" }));

    expect(socket.requestsOfKind("body_events")).toHaveLength(2);
  });

  it("says NO EVENTS for a century with none", async () => {
    const { socket } = renderView();
    await answer(socket, noEvents());

    expect([
      within(eventsPanel()).queryByText("NO EVENTS") !== null,
      within(eventsPanel()).queryByRole("listbox"),
    ]).toEqual([true, null]);
  });

  it("says NOT YET MODELLED for a server that does not serve the events", async () => {
    const { socket } = renderView();
    await answer(socket, null);

    expect(within(eventsPanel()).getByText("NOT YET MODELLED")).toBeInTheDocument();
  });

  it("names an answer for another system a fault", async () => {
    const { socket } = renderView();
    await answer(socket, { ...populatedEvents(), system: "0200080020000001" });

    expect(within(eventsPanel()).getByRole("status")).toHaveTextContent(
      "EVENTS DATA INVALID: answer for system 0200080020000001",
    );
  });

  it("offers RETRY for a fault, which asks again", async () => {
    const { user, socket } = renderView();
    await answer(socket, { ...populatedEvents(), system: "0200080020000001" });

    await user.click(within(eventsPanel()).getByRole("button", { name: "RETRY" }));

    expect(socket.requestsOfKind("body_events")).toHaveLength(2);
  });
});
