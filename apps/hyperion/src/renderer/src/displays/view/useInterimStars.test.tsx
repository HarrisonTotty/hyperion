import type { ResponseFor, SystemsInRange } from "@hyperion/protocol";
import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { FakeWebSocket } from "../../test/FakeWebSocket";
import { aStellarBrief, aSystemsInRange, UNIVERSE_ID } from "../../test/galaxyFixtures";
import { ServerLinkHarness } from "../../test/ServerLinkHarness";
import { galacticPositionFromLy } from "@hyperion/protocol";
import { type InterimStarsInput, useInterimStars } from "./useInterimStars";

const CENTRE_LY = [0, 26_000, 0] as const;
const INPUT: InterimStarsInput = {
  universe: UNIVERSE_ID,
  system: "0200080020000000",
  centre: galacticPositionFromLy(CENTRE_LY),
  time: { seconds: 0, nanos: 0 },
};

function renderStars(input: InterimStarsInput = INPUT) {
  const hook = renderHook((current: InterimStarsInput) => useInterimStars(current), {
    initialProps: input,
    wrapper: ServerLinkHarness,
  });
  const socket = FakeWebSocket.latest();
  act(() => {
    socket.serverWelcomes();
  });
  return { ...hook, socket };
}

async function server(play: () => void): Promise<void> {
  await act(async () => {
    play();
    await Promise.resolve();
  });
}

/** An answer to `body` with one star of the floor's layer, its V given or not. */
function oneStar(
  body: {
    readonly min_layer: SystemsInRange["census"]["layers"][number]["layer"];
    readonly radius_ly: number;
  },
  absoluteV: number | null,
  overLimit = false,
): ResponseFor<"systems_in_range"> {
  const brief = aStellarBrief("a");
  const answer = aSystemsInRange({
    centreLy: CENTRE_LY,
    radiusLy: body.radius_ly,
    minLayer: body.min_layer,
    limit: 20_000,
    overLimit: overLimit ? [body.min_layer] : [],
    systems: [
      {
        relLy: [10, 0, 0],
        layer: "a",
        stellar:
          absoluteV === null
            ? { ...brief, kind: "white_dwarf" }
            : { ...brief, absolute_v_mag: absoluteV },
      },
    ],
  });
  return answer;
}

describe("useInterimStars", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  it("asks the four queries with the briefs at the server's limit of 20,000", () => {
    const { socket } = renderStars();
    expect(
      socket
        .requestsOfKind("systems_in_range")
        .map(({ body }) => [body.min_layer, body.radius_ly, body.limit, body.include_stellar]),
    ).toEqual([
      ["e", 620, 20_000, true],
      ["d", 360, 20_000, true],
      ["c", 210, 20_000, true],
      ["a", 60, 20_000, true],
    ]);
  });

  it("asks nothing while the system's position is not known", () => {
    const { socket, result } = renderStars({ ...INPUT, centre: null });
    expect([socket.requestsOfKind("systems_in_range").length, result.current]).toEqual([
      0,
      { field: null, countLine: null },
    ]);
  });

  it("asks the four queries once when the system's position is learnt after its arrival", () => {
    const { socket, rerender } = renderStars({ ...INPUT, centre: null });
    rerender(INPUT);
    rerender(INPUT);
    expect(socket.requestsOfKind("systems_in_range").map(({ body }) => body.min_layer)).toEqual([
      "e",
      "d",
      "c",
      "a",
    ]);
  });

  it("asks once more at the shrunk radius where the floor was over the limit, and no second time", async () => {
    const { socket } = renderStars();
    await server(() => {
      socket.serverAnswers("systems_in_range", (body) => oneStar(body, 4.8, true));
    });
    await server(() => {
      socket.serverAnswers("systems_in_range", (body) => oneStar(body, 4.8, true));
    });
    const floorA = socket
      .requestsOfKind("systems_in_range")
      .filter(({ body }) => body.min_layer === "a")
      .map(({ body }) => body.radius_ly);
    expect([floorA.length, (floorA[1] ?? 0) < 60]).toEqual([2, true]);
  });

  it("merges the answers to one star per system", async () => {
    const { socket, result } = renderStars();
    for (let i = 0; i < 4; i += 1) {
      // Each answer goes to the latest request not yet answered, so they are played in turn.
      // oxlint-disable-next-line no-await-in-loop
      await server(() => {
        socket.serverAnswers("systems_in_range", (body) => oneStar(body, 4.8));
      });
    }
    expect(result.current.countLine).toBe("STARS 1 DRAWN · 0 WITHOUT V · RADII 620/360/210/60 ly");
  });

  it("counts a row without V and draws it not", async () => {
    const { socket, result } = renderStars();
    await server(() => {
      socket.serverAnswers("systems_in_range", (body) => {
        const answer = oneStar(body, 4.8);
        const withoutV = oneStar(body, null).systems.map((row) =>
          Object.assign({}, row, { id: "00000000000000ff" }),
        );
        return { ...answer, systems: [...answer.systems, ...withoutV] };
      });
    });
    expect(result.current.countLine).toBe("STARS 1 DRAWN · 1 WITHOUT V · RADII 620/360/210/60 ly");
  });

  it("asks again only on arrival in another system", () => {
    const { socket, rerender } = renderStars();
    rerender({ ...INPUT, time: { seconds: 100, nanos: 0 } });
    const before = socket.requestsOfKind("systems_in_range").length;
    rerender({
      ...INPUT,
      system: "0200080020000001",
      centre: galacticPositionFromLy([5, 26_000, 0]),
    });
    expect([before, socket.requestsOfKind("systems_in_range").length]).toEqual([4, 8]);
  });
});
