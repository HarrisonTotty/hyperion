/**
 * The `SYSTEM` display's time: stepping, `RESET`, the clock window's edge, one redraw per step and
 * the requests that stepping makes (plan 14, D18, D24 and P14.T44.a), with frames and timeouts
 * faked.
 */
import { type ResponseFor, SECONDS_PER_JULIAN_YEAR, type UniverseTime } from "@hyperion/protocol";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { fakeFramesAndTimeouts } from "../../test/fakeFramesAndTimeouts";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { type RecordingContext2D, stubCanvas } from "../../test/RecordingContext2D";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import {
  A_CENTURY,
  aSummaryResponse,
  anUnsupportedBodiesError,
  aNotYetBornSummary,
  aSingleStarSummary,
  aSunlikeStar,
  aSystemSummary,
  aSystemTarget,
  aWhiteDwarf,
} from "../../test/systemFixtures";
import { CLOCK_WINDOW_S } from "./displayTime";
import { sliceBodiesWith } from "../../test/planetaryFixture";
import { SystemView } from "./SystemView";

const DAY_S = 86_400;

/** Plays the server's side, letting the outcomes it settles reach React. */
async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

function nextFrame(): void {
  act(() => {
    vi.advanceTimersToNextFrame();
  });
}

/** How many times the map's canvas was painted: each paint begins by clearing it. */
function paints(recorder: RecordingContext2D): number {
  return recorder.calls("fillRect").length;
}

interface Rendered {
  readonly user: ReturnType<typeof userEvent.setup>;
  readonly socket: FakeWebSocket;
  readonly recorder: RecordingContext2D;
}

async function renderAnswered(
  time: UniverseTime = A_CENTURY,
  summary = aSystemSummary(),
  bodies: ResponseFor<"system_bodies"> | null = null,
): Promise<Rendered> {
  const advance = fakeFramesAndTimeouts();
  const user = userEvent.setup({ advanceTimers: advance });
  const recorder = stubCanvas();
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: 400, height: 300 }),
  );
  render(
    <ServerLinkHarness>
      <SystemView target={aSystemTarget({ time })} />
    </ServerLinkHarness>,
  );
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  await server(() => {
    socket.serverAnswers("system_summary", () => aSummaryResponse(summary));
    // Without bodies, as the owner's server answers until P14.T36: the hosts alone (the
    // orchestrator's ruling 59.1).
    const [request] = socket.requestsOfKind("system_bodies").toReversed();
    if (request !== undefined && bodies === null) {
      socket.serverRejects(request.id, anUnsupportedBodiesError());
    } else if (request !== undefined && bodies !== null) {
      socket.serverResponds(request.id, bodies);
    }
  });
  return { user, socket, recorder };
}

function timePanel(): HTMLElement {
  return screen.getByRole("region", { name: "Display time" });
}

/** The display time as the time panel reads it, without its label. */
function shownTime(): string {
  return within(timePanel()).getByRole("status", { name: "DISPLAY TIME UT" }).textContent;
}

/** Where the clock window's limit is said. */
function limitStatus(): HTMLElement {
  return within(timePanel()).getByRole("status", { name: "Clock window" });
}

function requestTimes(socket: FakeWebSocket): ReadonlyArray<UniverseTime> {
  return socket.requestsOfKind("system_summary").map((request) => request.body.time);
}

beforeEach(() => {
  FakeWebSocket.instances = [];
  vi.stubGlobal("WebSocket", FakeWebSocket);
});

describe("TimeControl", () => {
  it("opens held at the time it was opened with, and steps a day at a time", async () => {
    const { user } = await renderAnswered();

    expect(shownTime()).toBe("+100 yr 000/00:00:00");
    expect(within(timePanel()).getByRole("button", { name: "2 1 d" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    await user.keyboard("]");
    nextFrame();

    expect(shownTime()).toBe("+100 yr 001/00:00:00");

    await user.click(within(timePanel()).getByRole("button", { name: "[ -1 d" }));
    nextFrame();

    expect(shownTime()).toBe("+100 yr 000/00:00:00");
  });

  it("steps by the amount chosen, and RESET returns to the opening time", async () => {
    const { user } = await renderAnswered();

    await user.keyboard("1");
    await user.keyboard("]");
    nextFrame();

    expect(shownTime()).toBe("+100 yr 000/01:00:00");

    await user.keyboard("r");
    nextFrame();

    expect(shownTime()).toBe("+100 yr 000/00:00:00");
  });

  it("gathers the steps asked for before a frame into one redraw, and asks for no frame after it", async () => {
    const { user, recorder } = await renderAnswered();
    nextFrame();
    const before = paints(recorder);

    await user.keyboard("]]]");
    nextFrame();

    expect(shownTime()).toBe("+100 yr 003/00:00:00");
    expect(paints(recorder)).toBe(before + 1);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("paints nothing while the time is held", async () => {
    const { recorder } = await renderAnswered();
    nextFrame();
    const before = paints(recorder);

    act(() => {
      vi.advanceTimersByTime(10_000);
    });

    expect(paints(recorder)).toBe(before);
  });

  it("stops at the clock window's edge and says CLOCK WINDOW LIMIT", async () => {
    const { user } = await renderAnswered({ seconds: CLOCK_WINDOW_S - 3 * DAY_S, nanos: 0 });
    const limit = limitStatus();

    expect(limit).toHaveTextContent("");

    await user.keyboard("]]]]]");
    nextFrame();

    expect(shownTime()).toBe("+1000 yr 000/00:00:00");
    expect(limit).toHaveTextContent("CLOCK WINDOW LIMIT");
    const ahead = within(timePanel()).getByRole("button", { name: "] +1 d" });
    expect(ahead).toHaveAttribute("aria-disabled", "true");
    expect(ahead).toHaveAccessibleDescription("CLOCK WINDOW LIMIT");

    await user.keyboard("6]");
    nextFrame();

    expect(shownTime()).toBe("+1000 yr 000/00:00:00");
  });

  it("stops at -H too", async () => {
    const { user } = await renderAnswered({ seconds: 10 - CLOCK_WINDOW_S, nanos: 0 });

    // `[[` is user-event's escape for the `[` key.
    await user.keyboard("[[");
    nextFrame();

    expect(shownTime()).toBe("-1000 yr 000/00:00:00");
    expect(limitStatus()).toHaveTextContent("CLOCK WINDOW LIMIT");
  });

  it("asks the server again exactly once when the time has moved more than a year", async () => {
    const { user, socket } = await renderAnswered();

    await user.keyboard("4]");
    nextFrame();

    // A year exactly is not more than a year.
    expect(requestTimes(socket)).toEqual([A_CENTURY]);

    await user.keyboard("1]");
    nextFrame();
    await user.keyboard("]");
    nextFrame();

    const later = { seconds: A_CENTURY.seconds + SECONDS_PER_JULIAN_YEAR + 3_600, nanos: 0 };
    expect(requestTimes(socket)).toEqual([A_CENTURY, later]);
  });

  it("keeps the map on show while the new answer is pending", async () => {
    const { user } = await renderAnswered();

    await user.keyboard("5]");
    nextFrame();

    expect(screen.getByRole("application", { name: "Orbit map" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: /^Orbit map/ })).toHaveTextContent("PENDING");
  });

  it("marks the answer on show stale when the newer request fails", async () => {
    const { user, socket } = await renderAnswered();

    await user.keyboard("5]");
    nextFrame();
    const request = socket.requestsOfKind("system_summary").at(-1);
    await server(() => {
      socket.serverRejects(request?.id ?? -1, {
        code: "queue_full",
        message: "the interactive queue is full",
        field: null,
      });
    });

    expect(screen.getByRole("heading", { name: "Orbit map stale" })).toBeInTheDocument();
    expect(screen.getByRole("application", { name: "Orbit map, stale" })).toBeInTheDocument();
  });

  it("asks for the bodies again exactly once when the time moves past an orbit's valid_until", async () => {
    const validUntil = { seconds: A_CENTURY.seconds + 2 * DAY_S, nanos: 0 };
    const bodies = sliceBodiesWith((body) =>
      body.orbit.state === "ok"
        ? {
            ...body,
            orbit: { state: "ok", value: { ...body.orbit.value, valid_until: validUntil } },
          }
        : body,
    );
    const { user, socket } = await renderAnswered(A_CENTURY, aSingleStarSummary(), bodies);
    const bodyTimes = () => socket.requestsOfKind("system_bodies").map(({ body }) => body.time);

    await user.keyboard("]]");
    nextFrame();

    // The last instant at which the elements hold is not past it.
    expect(bodyTimes()).toEqual([A_CENTURY]);

    await user.keyboard("]");
    nextFrame();
    await user.keyboard("]");
    nextFrame();

    const past = { seconds: A_CENTURY.seconds + 3 * DAY_S, nanos: 0 };
    expect(bodyTimes()).toEqual([A_CENTURY, past]);
    expect(requestTimes(socket)).toEqual([A_CENTURY, past]);
  });

  it("asks again when the time moves past the birth of a system not yet formed", async () => {
    // Formed 36 hours after the answer's time: an age of -36 h in megayears.
    const ageMyr = -(36 * 3_600) / (1e6 * SECONDS_PER_JULIAN_YEAR);
    const { user, socket } = await renderAnswered(
      A_CENTURY,
      aNotYetBornSummary({ age_myr: ageMyr }),
    );

    await user.keyboard("]");
    nextFrame();

    expect(requestTimes(socket)).toHaveLength(1);

    await user.keyboard("]");
    nextFrame();

    expect(requestTimes(socket)).toEqual([
      A_CENTURY,
      { seconds: A_CENTURY.seconds + 2 * DAY_S, nanos: 0 },
    ]);
  });

  it("asks again when the time moves past a star's death", async () => {
    const death = { seconds: A_CENTURY.seconds + 2 * DAY_S, nanos: 0 };
    const { user, socket } = await renderAnswered(
      A_CENTURY,
      aSystemSummary({
        stars: [aWhiteDwarf(), aSunlikeStar({ body_index: 1, death_time: death })],
      }),
    );

    await user.keyboard("]");
    nextFrame();

    expect(requestTimes(socket)).toHaveLength(1);

    await user.keyboard("]");
    nextFrame();

    expect(requestTimes(socket)).toEqual([A_CENTURY, death]);
  });
});
