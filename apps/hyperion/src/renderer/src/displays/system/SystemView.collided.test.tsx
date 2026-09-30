/**
 * A body lost to the scattering after a supernova (the orchestrator's rulings 71 and 80): listed as
 * `DESTROYED`, not drawn, and read with its cause, `COLLIDED`, in the body readout (plan 14,
 * P14.T28.c and T43.a–b).
 */
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { FakeWebSocket } from "../../test/FakeWebSocket";
import { FIXTURE_EARTH, sliceBodiesWith } from "../../test/planetaryFixture";
import { stubCanvas } from "../../test/RecordingContext2D";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import {
  A_CENTURY,
  aSingleStarSummary,
  aSummaryResponse,
  aSystemTarget,
} from "../../test/systemFixtures";
import { SystemView } from "./SystemView";

/** When the Earth of the fixture collided: 400 years before the display's time. */
const COLLIDED_AT = { seconds: A_CENTURY.seconds - 400 * 31_557_600, nanos: 0 };

async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

async function renderCollided() {
  const user = userEvent.setup();
  stubCanvas();
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
    DOMRect.fromRect({ x: 0, y: 0, width: 400, height: 300 }),
  );
  render(
    <ServerLinkHarness>
      <SystemView target={aSystemTarget()} />
    </ServerLinkHarness>,
  );
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  const bodies = sliceBodiesWith((body) =>
    body.id === FIXTURE_EARTH
      ? {
          ...body,
          state: { type: "destroyed", cause: "collided", at: COLLIDED_AT },
          position_m: null,
        }
      : body,
  );
  await server(() => {
    socket.serverAnswers("system_summary", () => aSummaryResponse(aSingleStarSummary()));
  });
  await server(() => {
    socket.serverAnswers("system_bodies", () => bodies);
  });
  return { user };
}

function readingOf(label: string): string {
  const readout = screen.getByRole("status", { name: "Selected body" });
  const term = within(readout)
    .queryAllByRole("term")
    .find((candidate) => candidate.textContent === label);
  return term?.nextElementSibling?.textContent ?? "";
}

beforeEach(() => {
  FakeWebSocket.instances = [];
  vi.stubGlobal("WebSocket", FakeWebSocket);
});

describe("a body that collided", () => {
  it("is listed as DESTROYED and not drawn", async () => {
    await renderCollided();

    const tree = screen.getByRole("tree", { name: "Bodies" });
    expect(
      within(tree).getByRole("treeitem", { name: /\/768, PLANET, DESTROYED$/ }),
    ).toBeInTheDocument();
    const map = screen.getByRole("region", { name: /^Orbit map/ });
    expect(
      within(map).queryByText(/\/768$/, { selector: ".spatial-label" }),
    ).not.toBeInTheDocument();
  });

  it("reads COLLIDED as its cause, and since when", async () => {
    const { user } = await renderCollided();

    await user.click(
      within(screen.getByRole("tree", { name: "Bodies" })).getByRole("treeitem", {
        name: /\/768, PLANET, DESTROYED$/,
      }),
    );

    expect(readingOf("STATE")).toBe("DESTROYED");
    expect(readingOf("CAUSE")).toBe("COLLIDED");
    expect(readingOf("SINCE")).toBe("UT -300 yr 000/00:00:00");
  });
});
