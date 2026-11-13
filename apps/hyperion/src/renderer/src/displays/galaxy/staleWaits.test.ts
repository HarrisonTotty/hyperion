import { describe, expect, it, vi } from "vitest";

import { StaleWaits, staleWaitText } from "./staleWaits";

describe("StaleWaits", () => {
  it("starts with no wait for either view", () => {
    expect(new StaleWaits().getSnapshot()).toEqual({ face_on: null, edge_on: null });
  });

  it("tells its subscribers when a view's wait changes, and keeps the other view's", () => {
    const waits = new StaleWaits();
    const listener = vi.fn<() => void>();
    waits.subscribe(listener);
    waits.report("edge_on", "extinction");

    waits.report("face_on", "map");

    expect(listener).toHaveBeenCalledTimes(2);
    expect(waits.getSnapshot()).toEqual({ face_on: "map", edge_on: "extinction" });
  });

  it("keeps its snapshot, and tells no one, when a report changes nothing", () => {
    const waits = new StaleWaits();
    waits.report("face_on", "map");
    const before = waits.getSnapshot();
    const listener = vi.fn<() => void>();
    waits.subscribe(listener);

    waits.report("face_on", "map");

    expect(waits.getSnapshot()).toBe(before);
    expect(listener).not.toHaveBeenCalled();
  });

  it("tells a listener nothing once it is removed", () => {
    const waits = new StaleWaits();
    const listener = vi.fn<() => void>();
    const remove = waits.subscribe(listener);

    remove();
    waits.report("face_on", "map");

    expect(listener).not.toHaveBeenCalled();
  });
});

describe("staleWaitText", () => {
  it("states nothing while neither view waits", () => {
    expect(staleWaitText({ face_on: null, edge_on: null })).toBeNull();
  });

  it("states PENDING while either view waits on a map of the quantity shown", () => {
    expect(staleWaitText({ face_on: null, edge_on: "map" })).toBe("PENDING");
  });

  it("states PENDING, plain, while one view waits on its own map and the other on the extinction", () => {
    expect(staleWaitText({ face_on: "extinction", edge_on: "map" })).toBe("PENDING");
  });

  it("states EXTINCTION PENDING while the views wait only on the extinction maps", () => {
    expect(staleWaitText({ face_on: "extinction", edge_on: null })).toBe("EXTINCTION PENDING");
  });
});
