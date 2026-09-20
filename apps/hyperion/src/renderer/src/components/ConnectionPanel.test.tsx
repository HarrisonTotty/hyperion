import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { ConnectionState } from "../lib/connection";
import { ConnectionPanel } from "./ConnectionPanel";

const CONNECTED: ConnectionState = { status: "connected", serverVersion: "9.9.9", latencyMs: 12.4 };

function renderPanel(connection: ConnectionState): void {
  render(<ConnectionPanel url="ws://ship/ws" clientVersion="1.2.3" connection={connection} />);
}

/** The value shown beside the field labelled `label`. */
function reading(label: string): HTMLElement {
  const value = screen.getByText(label).nextElementSibling;
  if (!(value instanceof HTMLElement)) {
    throw new Error(`no value beside ${label}`);
  }
  return value;
}

describe("ConnectionPanel", () => {
  it("labels the endpoint and both versions", () => {
    renderPanel(CONNECTED);

    expect(reading("Endpoint")).toHaveTextContent("ws://ship/ws");
    expect(reading("Client Ver")).toHaveTextContent("1.2.3");
    expect(reading("Server Ver")).toHaveTextContent("9.9.9");
  });

  it.each([
    [0.4, "<1 ms"],
    [12.4, "12 ms"],
    [1234.5, "1235 ms"],
    [12345, "12,345 ms"],
  ])("shows a latency of %f ms as %s", (latencyMs, expected) => {
    renderPanel({ ...CONNECTED, latencyMs });

    expect(reading("Latency")).toHaveTextContent(expected);
  });

  it("shows a dash for values it does not have", () => {
    renderPanel({ status: "disconnected", serverVersion: null, latencyMs: null });

    expect(reading("Server Ver")).toHaveTextContent("—");
    expect(reading("Latency")).toHaveTextContent("—");
  });
});
