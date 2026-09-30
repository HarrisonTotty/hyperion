import { act, render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { GraphicsLaunchMode } from "../../../preload/api";
import { FakeAdapter, FakeGpu, INTEL_UHD_620_INFO, SWIFTSHADER_INFO } from "../test/fakeGpu";
import { type AdapterOutcome, requestAdapterOutcome } from "../view/engine/platform";
import {
  type GraphicsEvent,
  GraphicsStatusContext,
  GraphicsStatusStore,
  initialGraphicsStatus,
} from "../view/engine/status";
import { GraphicsPanel } from "./GraphicsPanel";

async function adapter(
  info = INTEL_UHD_620_INFO,
  features: ReadonlyArray<GPUFeatureName> = ["subgroups", "shader-f16", "timestamp-query"],
): Promise<AdapterOutcome> {
  return requestAdapterOutcome(new FakeGpu([new FakeAdapter({ info, features })]));
}

function renderPanel(
  mode: GraphicsLaunchMode,
  ...events: ReadonlyArray<GraphicsEvent>
): GraphicsStatusStore {
  const store = new GraphicsStatusStore(initialGraphicsStatus(mode, false));
  for (const event of events) {
    store.dispatch(event);
  }
  render(
    <GraphicsStatusContext value={store}>
      <GraphicsPanel />
    </GraphicsStatusContext>,
  );
  return store;
}

/** The reading under the term `label`. */
function reading(label: string): HTMLElement {
  const panel = screen.getByRole("region", { name: "Graphics" });
  const term = within(panel).getByText(label, { selector: "dt" });
  const value = term.nextElementSibling;
  if (!(value instanceof HTMLElement)) {
    throw new Error(`no reading follows ${label}`);
  }
  return value;
}

const LOST: GraphicsEvent = { kind: "device-lost", reason: "unknown", message: "gpu gone" };

describe("GraphicsPanel", () => {
  it("shows every reading missing while the adapter is asked for", () => {
    renderPanel("vulkan");
    for (const label of ["Adapter", "Software Adapter", "Features", "Styles", "GPU Timer"]) {
      expect(reading(label)).toHaveTextContent("—");
      expect(reading(label)).toHaveClass("readout__missing");
    }
    expect(reading("Mode")).toHaveTextContent("VULKAN");
    expect(reading("Device Losses")).toHaveTextContent("0");
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("reads a nominal hardware adapter with no status colour", async () => {
    renderPanel("vulkan", { kind: "adapter-outcome", outcome: await adapter() });
    expect(reading("Adapter")).toHaveTextContent("intel · gen-9");
    expect(reading("Software Adapter")).toHaveTextContent("NO");
    expect(reading("Features")).toHaveTextContent("subgroups, shader-f16, timestamp-query");
    expect(reading("Styles")).toHaveTextContent("WIREFRAME, PHOTOREALISTIC");
    expect(reading("GPU Timer")).toHaveTextContent("QUANTIZED");
    const panel = screen.getByRole("region", { name: "Graphics" });
    expect(panel.querySelector("output")).toBeNull();
    expect(panel.querySelector(".request-status__text--fault")).toBeNull();
  });

  it("states a software adapter in plain text", async () => {
    renderPanel("vulkan", {
      kind: "adapter-outcome",
      outcome: await adapter(SWIFTSHADER_INFO, ["subgroups"]),
    });
    expect(reading("Software Adapter")).toHaveTextContent("YES");
    expect(reading("Styles")).toHaveTextContent("WIREFRAME");
    expect(reading("GPU Timer")).toHaveTextContent("ABSENT");
    const statement = screen.getByText(
      "GRAPHICS SOFTWARE ADAPTER: PHOTOREALISTIC STYLE UNAVAILABLE",
    );
    expect(statement).not.toHaveClass("request-status__text--fault");
  });

  it("states no WebGPU and no adapter", () => {
    renderPanel("vulkan", { kind: "adapter-outcome", outcome: { kind: "no-webgpu" } });
    expect(screen.getByText("GRAPHICS NOT AVAILABLE: no WebGPU")).toBeInTheDocument();
    expect(reading("Styles")).toHaveTextContent("NONE");
  });

  it("states no adapter", () => {
    renderPanel("vulkan", { kind: "adapter-outcome", outcome: { kind: "no-adapter" } });
    expect(screen.getByText("GRAPHICS NO ADAPTER: views unavailable")).toBeInTheDocument();
  });

  it("reports a lost device as a fault, with its count", async () => {
    renderPanel("vulkan", { kind: "adapter-outcome", outcome: await adapter() }, LOST);
    expect(screen.getByText("GRAPHICS DEVICE LOST: re-creating")).toHaveClass(
      "request-status__text--fault",
    );
    expect(reading("Device Losses")).toHaveTextContent("1");
  });

  it("reports a restarted GPU process as a fault, with its count", async () => {
    const store = renderPanel("vulkan", { kind: "adapter-outcome", outcome: await adapter() });
    act(() => {
      store.dispatch({ kind: "gpu-process-gone", count: 2 });
    });
    expect(screen.getByText("GRAPHICS PROCESS RESTARTED")).toHaveClass(
      "request-status__text--fault",
    );
    expect(reading("Process Restarts")).toHaveTextContent("2");
  });

  it("states the safe mode", () => {
    renderPanel("safe");
    expect(reading("Mode")).toHaveTextContent("SAFE");
    expect(reading("Styles")).toHaveTextContent("NONE");
    expect(
      screen.getByText("GRAPHICS SAFE MODE: views unavailable, relaunch to retry"),
    ).not.toHaveClass("request-status__text--fault");
  });

  it("states the disabled state after three losses", async () => {
    renderPanel("vulkan", { kind: "adapter-outcome", outcome: await adapter() }, LOST, LOST, LOST);
    expect(
      screen.getByText("GRAPHICS DISABLED: 3 DEVICE LOSSES, relaunch to retry"),
    ).not.toHaveClass("request-status__text--fault");
  });

  it("states a withdrawn adapter", async () => {
    renderPanel("vulkan", { kind: "adapter-outcome", outcome: await adapter() }, LOST, {
      kind: "adapter-withdrawn",
    });
    expect(
      screen.getByText("GRAPHICS DISABLED: adapter withdrawn, relaunch to retry"),
    ).toBeInTheDocument();
  });

  it("reads the default mode", () => {
    renderPanel("default");
    expect(reading("Mode")).toHaveTextContent("DEFAULT");
  });
});
