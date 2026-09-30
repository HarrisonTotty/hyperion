import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { GpuProcessGoneReport, GraphicsApi } from "../../../../preload/api";
import { FakeAdapter, FakeGpu, INTEL_UHD_620_INFO } from "../../test/fakeGpu";
import { GraphicsStatusProvider } from "./GraphicsStatusProvider";
import { useGraphicsStatus } from "./status";

function Condition() {
  const status = useGraphicsStatus();
  return <p>{`${status.condition.kind} ${status.launchMode}`}</p>;
}

function graphicsApi(listeners: Set<(report: GpuProcessGoneReport) => void>): GraphicsApi {
  return {
    launchMode: "vulkan",
    gpuTiming: false,
    onGpuProcessGone: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
  };
}

describe("GraphicsStatusProvider", () => {
  it("feeds its store from the adapter's answer", async () => {
    const gpu = new FakeGpu([new FakeAdapter({ info: INTEL_UHD_620_INFO, features: [] })]);
    render(
      <GraphicsStatusProvider graphics={graphicsApi(new Set())} gpu={gpu}>
        <Condition />
      </GraphicsStatusProvider>,
    );
    expect(await screen.findByText("nominal vulkan")).toBeInTheDocument();
  });

  it("stops listening for crash reports when unmounted", () => {
    const listeners = new Set<(report: GpuProcessGoneReport) => void>();
    const { unmount } = render(
      <GraphicsStatusProvider graphics={graphicsApi(listeners)} gpu={undefined}>
        <Condition />
      </GraphicsStatusProvider>,
    );
    expect(listeners.size).toBe(1);
    unmount();
    expect(listeners.size).toBe(0);
  });
});
