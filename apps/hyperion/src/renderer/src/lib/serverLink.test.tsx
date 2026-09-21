import { act, render, renderHook, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useEffect, useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "../App";
import { FakeWebSocket } from "../test/FakeWebSocket";
import { aUniverseList } from "../test/galaxyFixtures";
import { ServerLinkHarness } from "../test/ServerLinkHarness";
import type { ConnectionStatus } from "./connection";
import { linkDownReason, useServerLink } from "./serverLink";

const PING_INTERVAL_MS = 2_000;

/** A display in miniature: one button that lists the universes, and what came back. */
function ListingConsumer() {
  const { status, requests } = useServerLink();
  const [answer, setAnswer] = useState("—");
  return (
    <>
      <output aria-label="Link">{status}</output>
      <button
        type="button"
        onClick={() => {
          void requests
            .request({ kind: "list_universes" })
            .outcome.then((outcome) => {
              setAnswer(outcome.ok ? `UNIVERSES ${outcome.response.universes.length}` : "FAILED");
              return undefined;
            })
            .catch((error: unknown) => {
              console.error(error);
            });
        }}
      >
        LIST
      </button>
      <output aria-label="Answer">{answer}</output>
    </>
  );
}

interface CommitCounterProps {
  /** Called after every commit of the counter, which re-renders only through the context. */
  readonly onCommit: () => void;
}

/** A consumer of the link that reports each of its commits. */
function CommitCounter({ onCommit }: CommitCounterProps) {
  const { status } = useServerLink();
  useEffect(() => {
    onCommit();
  });
  return <output aria-label="Link">{status}</output>;
}

describe("ServerLinkContext", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("lets a consumer make a request over the link and receive the response", async () => {
    const user = userEvent.setup();
    render(
      <ServerLinkHarness>
        <ListingConsumer />
      </ServerLinkHarness>,
    );
    const socket = FakeWebSocket.latest();
    act(() => {
      socket.serverWelcomes();
    });

    await user.click(screen.getByRole("button", { name: "LIST" }));
    expect(socket.requestsOfKind("list_universes")).toHaveLength(1);
    act(() => {
      socket.serverAnswers("list_universes", () => aUniverseList());
    });

    expect(await screen.findByText("UNIVERSES 1")).toBeInTheDocument();
  });

  it("carries a request from a consumer under App to the socket, and its response back", async () => {
    const user = userEvent.setup();
    render(<App />);
    const socket = FakeWebSocket.latest();
    expect(socket.requestsOfKind("list_universes")).toEqual([]);

    act(() => {
      socket.serverWelcomes();
    });
    expect(socket.requestsOfKind("list_universes")).toHaveLength(1);
    await act(async () => {
      socket.serverAnswers("list_universes", () => aUniverseList());
      await Promise.resolve();
    });
    await user.keyboard("{F2}");

    expect(screen.getByRole("button", { name: "Open universe SURVEY 1" })).toBeInTheDocument();
  });

  it("does not re-render a consumer when a pong updates the latency", () => {
    vi.useFakeTimers();
    const onCommit = vi.fn<() => void>();
    render(
      <ServerLinkHarness>
        <CommitCounter onCommit={onCommit} />
      </ServerLinkHarness>,
    );
    const socket = FakeWebSocket.latest();
    act(() => {
      socket.serverWelcomes();
    });
    // Mounted while connecting, then re-rendered once for the welcome.
    expect(onCommit).toHaveBeenCalledTimes(2);

    act(() => {
      vi.advanceTimersByTime(PING_INTERVAL_MS);
    });
    expect(socket.sent).toContainEqual({ type: "ping", nonce: 0 });
    act(() => {
      socket.serverSends({ type: "pong", nonce: 0 });
    });
    expect(onCommit).toHaveBeenCalledTimes(2);

    act(() => {
      socket.close();
    });
    expect(onCommit).toHaveBeenCalledTimes(3);
    expect(screen.getByRole("status", { name: "Link" })).toHaveTextContent("disconnected");
  });

  it("names the missing provider when used outside one", () => {
    vi.spyOn(console, "error").mockImplementation(() => {});

    expect(() => renderHook(() => useServerLink())).toThrow(
      "useServerLink needs a ServerLinkContext provider",
    );
  });
});

describe("linkDownReason", () => {
  it.each<[ConnectionStatus, string | null]>([
    ["connected", null],
    ["connecting", "ESTABLISHING LINK"],
    ["disconnected", "NO CARRIER"],
    ["incompatible", "LINK INCOMPATIBLE"],
  ])("gives %s as %s", (status, reason) => {
    expect(linkDownReason(status)).toBe(reason);
  });
});
