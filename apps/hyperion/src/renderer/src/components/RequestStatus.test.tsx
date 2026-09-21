import type { RequestKind } from "@hyperion/protocol";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { RequestState } from "../lib/useServerRequest";
import { aUniverseList } from "../test/galaxyFixtures";
import { RequestStatus } from "./RequestStatus";

describe("RequestStatus", () => {
  it.each<[string, RequestState<RequestKind>, string, boolean]>([
    ["pending", { kind: "pending" }, "PENDING", false],
    [
      "rejected by an overloaded server",
      { kind: "rejected", code: "queue_full", reason: "the interactive queue is full" },
      "REJECTED: the interactive queue is full",
      true,
    ],
    [
      "rejected by a failed server",
      { kind: "rejected", code: "storage_failed", reason: "the save could not be written" },
      "REJECTED: the save could not be written",
      true,
    ],
    [
      "refused for a name taken",
      { kind: "rejected", code: "name_taken", reason: "a universe named SURVEY 1 exists" },
      "REJECTED: a universe named SURVEY 1 exists",
      false,
    ],
    [
      "refused for an unknown universe",
      { kind: "rejected", code: "unknown_universe", reason: "no universe 00000000000000a1" },
      "REJECTED: no universe 00000000000000a1",
      false,
    ],
    ["timed out", { kind: "timed_out" }, "TIMED OUT", true],
    ["with the link down", { kind: "link_down", reason: "NO CARRIER" }, "NO CARRIER", false],
  ])(
    "reads a request %s in words, in caution only for a failed system",
    (_, state, text, fault) => {
      render(<RequestStatus state={state} />);

      const status = screen.getByRole("status");
      expect(status).toHaveTextContent(text);
      // The caution colour is set by this class; the words carry the state on their own.
      expect(status.classList.contains("request-status__text--fault")).toBe(fault);
    },
  );

  it.each<[string, RequestState<RequestKind>]>([
    ["idle", { kind: "idle" }],
    ["ok", { kind: "ok", response: aUniverseList() }],
  ])("shows nothing for a request that is %s", (_, state) => {
    const { container } = render(<RequestStatus state={state} />);

    expect(container).toBeEmptyDOMElement();
  });

  it.each<[string, RequestState<RequestKind>]>([
    ["rejected", { kind: "rejected", code: "queue_full", reason: "busy" }],
    ["refused", { kind: "rejected", code: "unknown_universe", reason: "no such universe" }],
    ["timed out", { kind: "timed_out" }],
  ])("offers RETRY for a request %s, and sends it again", async (_, state) => {
    const user = userEvent.setup();
    const onRetry = vi.fn<() => void>();
    render(<RequestStatus state={state} onRetry={onRetry} />);

    await user.click(screen.getByRole("button", { name: "RETRY" }));

    expect(onRetry).toHaveBeenCalledOnce();
  });

  it.each<[string, RequestState<RequestKind>]>([
    ["pending", { kind: "pending" }],
    ["with the link down", { kind: "link_down", reason: "NO CARRIER" }],
  ])("offers no RETRY for a request %s", (_, state) => {
    render(<RequestStatus state={state} onRetry={() => {}} />);

    expect(screen.queryByRole("button", { name: "RETRY" })).not.toBeInTheDocument();
  });

  it("offers no RETRY when the caller cannot send again", () => {
    render(<RequestStatus state={{ kind: "timed_out" }} />);

    expect(screen.queryByRole("button", { name: "RETRY" })).not.toBeInTheDocument();
  });
});
