import { act, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { App } from "./App";
import { FakeWebSocket } from "./test/FakeWebSocket";

describe("App", () => {
  beforeEach(() => {
    FakeWebSocket.instances = [];
    vi.stubGlobal("WebSocket", FakeWebSocket);
  });

  it("opens on the link display inside the console frame", () => {
    render(<App />);

    expect(screen.getByRole("heading", { level: 1, name: "Link" })).toBeInTheDocument();
    const navigation = screen.getByRole("navigation", { name: "Displays" });
    expect(within(navigation).getByText("Link")).toHaveAttribute("aria-current", "page");
  });

  it("shows the link being established on launch", () => {
    render(<App />);

    expect(screen.getByRole("status")).toHaveTextContent("ESTABLISHING LINK");
  });

  it("greets the server and reports a nominal link once welcomed", () => {
    render(<App />);
    const socket = FakeWebSocket.latest();

    act(() => {
      socket.serverOpens();
    });
    expect(socket.sent).toEqual([{ type: "hello", client_version: __APP_VERSION__ }]);

    act(() => {
      socket.serverSends({ type: "welcome", server_version: "9.9.9", protocol_version: 1 });
    });
    expect(screen.getByRole("status")).toHaveTextContent("LINK NOMINAL");
    expect(screen.getByText("9.9.9")).toBeInTheDocument();
  });

  it("reports loss of carrier when the socket closes", () => {
    render(<App />);
    const socket = FakeWebSocket.latest();

    act(() => {
      socket.close();
    });

    expect(screen.getByRole("status")).toHaveTextContent("NO CARRIER");
  });
});
