/**
 * The `SYSTEM` display's `RUN` and `HOLD` (plan 14, D24 and P14.T44.b): the display opens held,
 * runs only when told, leaves nothing running once held, hidden or unmounted, drops to `HOLD` at
 * `+H` and on losing the link, changes its readouts at most four times a second while the map
 * follows every frame, and under reduced motion steps four times a second with no frame between.
 */
import { SECONDS_PER_JULIAN_YEAR, type UniverseTime } from "@hyperion/protocol";
import { Activity, type ReactNode } from "react";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { fakeFramesAndTimeouts } from "../../test/fakeFramesAndTimeouts";
import { FakeWebSocket } from "../../test/FakeWebSocket";
import { type RecordingContext2D, stubCanvas } from "../../test/RecordingContext2D";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { stubMatchMedia } from "../../test/stubMatchMedia";
import {
  A_CENTURY,
  anUnsupportedBodiesError,
  aSummaryResponse,
  aSystemSummary,
  aSystemTarget,
} from "../../test/systemFixtures";
import { CLOCK_WINDOW_S } from "./displayTime";
import { SystemView } from "./SystemView";

const DAY_S = 86_400;

/** The frame period the fake clock steps by, as `advanceTimersToNextFrame` does. */
const FRAME_MS = 16;

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

function advance(ms: number): void {
  act(() => {
    vi.advanceTimersByTime(ms);
  });
}

function paints(recorder: RecordingContext2D): number {
  return recorder.calls("fillRect").length;
}

interface Rendered {
  readonly user: ReturnType<typeof userEvent.setup>;
  readonly socket: FakeWebSocket;
  readonly recorder: RecordingContext2D;
  readonly rerender: (activity: "visible" | "hidden") => void;
  readonly unmount: () => void;
}

async function renderAnswered(time: UniverseTime = A_CENTURY): Promise<Rendered> {
  const advanceClock = fakeFramesAndTimeouts();
  const user = userEvent.setup({ advanceTimers: advanceClock });
  const recorder = stubCanvas();
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: 400, height: 300 }),
  );
  const target = aSystemTarget({ time });
  const tree = (activity: "visible" | "hidden"): ReactNode => (
    <ServerLinkHarness>
      <Activity mode={activity}>
        <SystemView target={target} />
      </Activity>
    </ServerLinkHarness>
  );
  const rendered = render(tree("visible"));
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  await server(() => {
    socket.serverAnswers("system_summary", () => aSummaryResponse(aSystemSummary()));
    const [request] = socket.requestsOfKind("system_bodies").toReversed();
    if (request !== undefined) {
      socket.serverRejects(request.id, anUnsupportedBodiesError());
    }
  });
  // The first paint of the answer, so that what follows counts only what running asks for.
  nextFrame();
  return {
    user,
    socket,
    recorder,
    rerender: (activity) => {
      rendered.rerender(tree(activity));
    },
    unmount: rendered.unmount,
  };
}

function timePanel(): HTMLElement {
  return screen.getByRole("region", { name: "Display time" });
}

function shownTime(): string {
  return within(timePanel()).getByRole("status", { name: "DISPLAY TIME UT" }).textContent;
}

/** The display time read, as seconds after {@link A_CENTURY}, which every test opens near. */
function shownOffsetS(): number {
  const match = /^([+-])(\d+) yr (\d{3})\/(\d{2}):(\d{2}):(\d{2})$/.exec(shownTime());
  if (match === null) {
    throw new Error(`not a display time: ${shownTime()}`);
  }
  const [, , years, days, hours, minutes, seconds] = match.map(Number);
  return (
    ((years ?? 0) - 100) * SECONDS_PER_JULIAN_YEAR +
    (days ?? 0) * DAY_S +
    (hours ?? 0) * 3_600 +
    (minutes ?? 0) * 60 +
    (seconds ?? 0)
  );
}

function mode(): string {
  return within(timePanel()).getByRole("status", { name: "MODE" }).textContent;
}

function runButton(): HTMLElement {
  return within(timePanel()).getByRole("button", { name: "G RUN" });
}

beforeEach(() => {
  FakeWebSocket.instances = [];
  vi.stubGlobal("WebSocket", FakeWebSocket);
});

describe("TimeControl's RUN and HOLD", () => {
  it("opens in HOLD, a congruent pair with RUN before HOLD, and a day a second chosen", async () => {
    await renderAnswered();

    expect(mode()).toBe("HOLD");
    const pair = within(within(timePanel()).getByRole("group", { name: "Run or hold" }));
    expect(pair.getAllByRole("button").map((button) => button.textContent)).toEqual([
      "G RUN",
      "H HOLD",
    ]);
    expect(pair.getByRole("button", { name: "H HOLD" })).toHaveAttribute("aria-pressed", "true");
    expect(runButton()).toHaveAttribute("aria-pressed", "false");
    expect(
      within(within(timePanel()).getByRole("group", { name: "Rate" })).getByRole("button", {
        name: "8 1 d/s",
      }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(vi.getTimerCount()).toBe(0);
  });

  it("runs at the chosen rate and says so, then HOLD keeps the time and leaves nothing running", async () => {
    const { user } = await renderAnswered();

    await user.keyboard("g");

    expect(mode()).toBe("RUN 1 d/s");
    expect(runButton()).toHaveAttribute("aria-pressed", "true");

    // The first frame starts the run's clock; a second's worth of frames after it is a day, which
    // the readout, changing each quarter second, shows to within its last quarter.
    nextFrame();
    advance(1_000);

    expect(shownOffsetS()).toBeGreaterThanOrEqual(0.75 * DAY_S);
    expect(shownOffsetS()).toBeLessThanOrEqual(DAY_S);

    await user.click(within(timePanel()).getByRole("button", { name: "H HOLD" }));

    // Held, the time is where the map last drew it: a day, to within a frame.
    expect(mode()).toBe("HOLD");
    expect(shownOffsetS()).toBeGreaterThanOrEqual(DAY_S - (FRAME_MS / 1_000) * DAY_S);
    expect(shownOffsetS()).toBeLessThanOrEqual(DAY_S + (FRAME_MS / 1_000) * DAY_S);
    expect(vi.getTimerCount()).toBe(0);
    const held = shownTime();
    advance(5_000);
    expect(shownTime()).toBe(held);
  });

  it("changes rate while running, and reads the new one", async () => {
    const { user } = await renderAnswered();

    await user.keyboard("0g");

    expect(mode()).toBe("RUN 1 yr/s");

    nextFrame();
    advance(1_000);
    await user.keyboard("7");

    expect(mode()).toBe("RUN 1 h/s");
    const rerated = shownOffsetS();
    expect(rerated).toBeGreaterThanOrEqual(0.75 * SECONDS_PER_JULIAN_YEAR);

    nextFrame();
    advance(1_000);

    // An hour a second from where the year a second had reached, which the readout showed to
    // within a quarter of a second's worth: never back, and at most that quarter and an hour on.
    expect(shownOffsetS()).toBeGreaterThanOrEqual(rerated);
    expect(shownOffsetS()).toBeLessThanOrEqual(rerated + 0.25 * SECONDS_PER_JULIAN_YEAR + 3_600);
  });

  it("leaves no timer or frame alive when unmounted while running", async () => {
    const { user, unmount } = await renderAnswered();
    await user.keyboard("g");
    nextFrame();

    unmount();

    expect(vi.getTimerCount()).toBe(0);
  });

  it("drops to HOLD on leaving the display, and does not run again on return", async () => {
    const { user, rerender } = await renderAnswered();
    await user.keyboard("g");
    nextFrame();
    advance(500);

    act(() => {
      rerender("hidden");
    });
    act(() => {
      rerender("visible");
    });
    const back = shownTime();
    advance(2_000);

    expect(mode()).toBe("HOLD");
    expect(shownTime()).toBe(back);
  });

  it("drops to HOLD at +H, and holds RUN back there", async () => {
    const { user } = await renderAnswered({ seconds: CLOCK_WINDOW_S - DAY_S / 2, nanos: 0 });

    await user.keyboard("g");
    nextFrame();
    advance(2_000);

    expect(shownTime()).toBe("+1000 yr 000/00:00:00");
    expect(mode()).toBe("HOLD");
    expect(vi.getTimerCount()).toBe(0);
    expect(runButton()).toHaveAttribute("aria-disabled", "true");
    expect(runButton()).toHaveAccessibleDescription("CLOCK WINDOW LIMIT");

    await user.keyboard("g");

    expect(mode()).toBe("HOLD");
  });

  it("drops to HOLD on losing the link, and holds RUN back while it is down", async () => {
    const { user, socket } = await renderAnswered();
    await user.keyboard("g");
    nextFrame();
    advance(500);

    act(() => {
      socket.close();
    });

    expect(mode()).toBe("HOLD");
    expect(runButton()).toHaveAttribute("aria-disabled", "true");
    expect(runButton()).toHaveAccessibleDescription(/NO CARRIER|CONNECTING/);
  });

  it("holds a running display before a step, which lands where the run reached", async () => {
    const { user } = await renderAnswered();
    await user.keyboard("g");
    nextFrame();
    advance(1_000);

    await user.keyboard("]");
    const reached = shownOffsetS();
    nextFrame();

    expect(mode()).toBe("HOLD");
    expect(shownOffsetS()).toBe(reached + DAY_S);
    expect(reached).toBeGreaterThan(0.9 * DAY_S);
  });

  it("changes its readouts at most four times a second while the map follows every frame", async () => {
    const { user, recorder } = await renderAnswered();
    await user.keyboard("g");
    nextFrame();
    const before = paints(recorder);
    const shown = new Set<string>([shownTime()]);
    let changes = 0;
    let last = shownTime();

    for (let elapsed = 0; elapsed < 1_000; elapsed += FRAME_MS) {
      nextFrame();
      if (shownTime() !== last) {
        changes += 1;
        last = shownTime();
        shown.add(last);
      }
    }

    expect(changes).toBeLessThanOrEqual(4);
    expect(changes).toBeGreaterThanOrEqual(3);
    expect(paints(recorder) - before).toBeGreaterThan(20);
  });

  it("under reduced motion steps four times a second, a paint each, with no frame between", async () => {
    stubMatchMedia(true);
    const { user, recorder } = await renderAnswered();
    await user.keyboard("g");
    const before = paints(recorder);

    for (let tick = 1; tick <= 4; tick += 1) {
      advance(250);
      nextFrame();

      // Only the run's own interval is left: no frame is waiting between the steps.
      expect(vi.getTimerCount()).toBe(1);
    }

    expect(paints(recorder) - before).toBe(4);
    expect(shownTime()).toBe("+100 yr 001/00:00:00");
  });

  it("goes on from where it reached when the motion setting changes mid-run", async () => {
    const motion = stubMatchMedia(false);
    const { user } = await renderAnswered();
    await user.keyboard("g");
    nextFrame();
    advance(1_000);
    const reached = shownOffsetS();

    act(() => {
      motion.set(true);
    });
    advance(250);
    nextFrame();

    expect(mode()).toBe("RUN 1 d/s");
    // A quarter of a day on from where the frames had reached, not from where the run began.
    expect(shownOffsetS()).toBeGreaterThan(reached);
    expect(shownOffsetS()).toBeLessThanOrEqual(reached + 0.25 * DAY_S + 0.25 * DAY_S);
    expect(shownOffsetS()).toBeGreaterThanOrEqual(DAY_S);
  });

  it("never starts on its own", async () => {
    const { recorder } = await renderAnswered({
      seconds: A_CENTURY.seconds + SECONDS_PER_JULIAN_YEAR,
      nanos: 0,
    });
    const before = paints(recorder);

    advance(10_000);

    expect(mode()).toBe("HOLD");
    expect(paints(recorder)).toBe(before);
  });
});
