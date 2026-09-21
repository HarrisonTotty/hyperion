import { PROTOCOL_VERSION } from "@hyperion/protocol";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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
    expect(within(navigation).getByRole("button", { name: "F1 Link" })).toHaveAttribute(
      "aria-current",
      "page",
    );
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
      socket.serverSends({
        type: "welcome",
        server_version: "9.9.9",
        protocol_version: PROTOCOL_VERSION,
        generator_version: 2,
      });
    });
    expect(screen.getByRole("status")).toHaveTextContent("LINK NOMINAL");
    expect(screen.getByText("9.9.9")).toBeInTheDocument();
  });

  it("reports an incompatible link when the server speaks another protocol version", () => {
    render(<App />);
    const socket = FakeWebSocket.latest();

    act(() => {
      socket.serverOpens();
      socket.serverSends({
        type: "welcome",
        server_version: "0.0.9",
        protocol_version: 1,
        generator_version: 1,
      });
    });

    expect(screen.getByRole("status")).toHaveTextContent("LINK INCOMPATIBLE");
  });

  it("reports loss of carrier when the socket closes", () => {
    render(<App />);
    const socket = FakeWebSocket.latest();

    act(() => {
      socket.close();
    });

    expect(screen.getByRole("status")).toHaveTextContent("NO CARRIER");
  });

  it("switches to the galaxy display when its tab is clicked", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "F2 Galaxy" }));

    expect(screen.getByRole("heading", { level: 1, name: "Galaxy" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "F2 Galaxy" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(screen.getByRole("button", { name: "F1 Link" })).not.toHaveAttribute("aria-current");
  });

  it("switches display from the keyboard alone", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.tab();
    await user.tab();
    expect(screen.getByRole("button", { name: "F2 Galaxy" })).toHaveFocus();
    await user.keyboard("{Enter}");

    expect(screen.getByRole("heading", { level: 1, name: "Galaxy" })).toBeInTheDocument();
  });

  it("switches display with a function key", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.keyboard("{F2}");
    expect(screen.getByRole("heading", { level: 1, name: "Galaxy" })).toBeInTheDocument();
    await user.keyboard("{F1}");
    expect(screen.getByRole("heading", { level: 1, name: "Link" })).toBeInTheDocument();
  });

  it("hides the inactive display from the accessibility tree", async () => {
    const user = userEvent.setup();
    render(<App />);

    expect(screen.queryByRole("heading", { level: 2, name: "Galaxy" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "F2 Galaxy" }));

    expect(screen.getByRole("heading", { level: 2, name: "Galaxy" })).toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { level: 2, name: "Server Link" }),
    ).not.toBeInTheDocument();
  });

  it("keeps a display it leaves mounted, out of sight, so that its state survives", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "F2 Galaxy" }));

    const linkTitle = screen.getByRole("heading", { level: 2, name: "Server Link", hidden: true });
    expect(linkTitle).not.toBeVisible();
  });

  it("keeps the link display and the link itself across a visit to another display", async () => {
    const user = userEvent.setup();
    render(<App />);
    const socket = FakeWebSocket.latest();
    act(() => {
      socket.serverOpens();
      socket.serverSends({
        type: "welcome",
        server_version: "9.9.9",
        protocol_version: PROTOCOL_VERSION,
        generator_version: 2,
      });
    });

    await user.click(screen.getByRole("button", { name: "F2 Galaxy" }));
    await user.click(screen.getByRole("button", { name: "F1 Link" }));

    expect(screen.getByRole("heading", { level: 2, name: "Server Link" })).toBeInTheDocument();
    expect(screen.getByText("9.9.9")).toBeInTheDocument();
    expect(FakeWebSocket.instances).toHaveLength(1);
  });
});
