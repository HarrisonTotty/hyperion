import { describe, expect, it, vi } from "vitest";

import { countingRenderEngine } from "../../../test/countingRenderEngine";
import { BUFFER_USAGE } from "../../engine/gpuFlags";
import { textureBytes, type TextureSpec } from "../../engine/memory";
import { allocationTally } from "./allocationTally";

const NORMALS: TextureSpec = {
  name: "normals atlas",
  size: [67, 67],
  dimension: "2d",
  format: "rg16float",
  mips: 1,
  usage: 0,
  category: "height-cache",
};

describe("allocationTally", () => {
  it("counts each category's live bytes by the formats' sizes", async () => {
    const engine = await countingRenderEngine();
    const tally = allocationTally(engine);
    engine.createBuffer({
      name: "heights",
      bytes: 33_800,
      usage: BUFFER_USAGE.STORAGE,
      category: "height-cache",
    });
    engine.createTexture(NORMALS);
    expect(tally.liveBytes("height-cache")).toBe(33_800 + 67 * 67 * 4);
    expect(textureBytes(NORMALS)).toBe(67 * 67 * 4);
    expect(tally.liveBytes("atmosphere-view")).toBe(0);
  });

  it("lowers the live bytes on a destruction and keeps the peak until it is reset", async () => {
    const engine = await countingRenderEngine();
    const tally = allocationTally(engine);
    engine.createBuffer({ name: "a", bytes: 100, usage: 0, category: "height-cache" });
    engine.createBuffer({ name: "b", bytes: 50, usage: 0, category: "height-cache" });
    engine.destroy("a", 100, "height-cache");
    expect(tally.liveBytes("height-cache")).toBe(50);
    expect(tally.peakBytes("height-cache")).toBe(150);
    tally.resetPeaks();
    expect(tally.peakBytes("height-cache")).toBe(50);
  });

  it("ignores the destruction of an allocation it never saw made", async () => {
    const engine = await countingRenderEngine();
    const tally = allocationTally(engine);
    engine.destroy("frame uniforms", 256, "other");
    expect(tally.liveBytes("other")).toBe(0);
  });

  it("counts the bytes uploaded since the frame began", async () => {
    const engine = await countingRenderEngine();
    const tally = allocationTally(engine);
    const buffer = engine.createBuffer({
      name: "instances",
      bytes: 64,
      usage: 0,
      category: "other",
    });
    engine.writeBuffer(buffer, 0, new Float32Array(8));
    engine.writeTexture(
      { kind: "texture", name: "normals" },
      [0, 0, 0],
      [2, 2, 1],
      new Uint16Array(8),
    );
    expect(tally.uploadedBytesThisFrame()).toBe(32 + 16);
    tally.startFrame();
    expect(tally.uploadedBytesThisFrame()).toBe(0);
  });

  it("tells its listeners of each event, and an unsubscribed one hears nothing", async () => {
    const engine = await countingRenderEngine();
    const tally = allocationTally(engine);
    const heard = vi.fn<() => void>();
    const off = tally.subscribe(heard);
    engine.createBuffer({ name: "a", bytes: 1, usage: 0, category: "other" });
    off();
    engine.createBuffer({ name: "b", bytes: 1, usage: 0, category: "other" });
    expect(heard).toHaveBeenCalledTimes(1);
  });

  it("stops counting once disposed", async () => {
    const engine = await countingRenderEngine();
    const tally = allocationTally(engine);
    tally.dispose();
    engine.createBuffer({ name: "a", bytes: 1, usage: 0, category: "other" });
    expect(tally.liveBytes("other")).toBe(0);
  });
});
