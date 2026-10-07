import { describe, expect, it } from "vitest";

import { FakeAdapter, type FakeDevice, SWIFTSHADER_INFO } from "../../../test/fakeGpu";
import { TEXTURE_USAGE } from "../gpuFlags";
import { CanvasReadBackRefused } from "../types";
import { paddedBytesPerRow } from "./readback";
import { readCanvasTexture, srgbViewFormat, unpadRows } from "./view";

/** A fake device, and a 4 × 2 canvas texture made on it. */
async function canvasOnDevice(): Promise<{ device: FakeDevice; texture: GPUTexture }> {
  const adapter = new FakeAdapter({ info: SWIFTSHADER_INFO, features: [] });
  await adapter.requestDevice();
  const device = adapter.devices[0];
  if (device === undefined) {
    throw new Error("the adapter made no device");
  }
  const texture = device.createTexture({
    size: [4, 2],
    format: "rgba8unorm",
    usage: TEXTURE_USAGE.RENDER_ATTACHMENT | TEXTURE_USAGE.COPY_SRC,
  });
  return { device, texture };
}

/** The error the RTX 3080 raised at a late read of a 4 × 2 canvas (R07.T8.a's canvas check). */
const DESTROYED: GPUError = {
  message:
    "Destroyed texture [Texture (unlabeled 4x2 px, TextureFormat::RGBA8Unorm)] used in a submit.",
};

describe("a canvas's read-back", () => {
  it("reads every texel of a copy the device accepted", async () => {
    const { device, texture } = await canvasOnDevice();
    await expect(readCanvasTexture(device, texture, false, "primary")).resolves.toHaveLength(32);
  });

  it("rejects with CanvasReadBackRefused, not zeros, when the device refuses a late read's copy", async () => {
    const { device, texture } = await canvasOnDevice();
    device.scopeErrors.push(DESTROYED);
    await expect(readCanvasTexture(device, texture, false, "primary")).rejects.toBeInstanceOf(
      CanvasReadBackRefused,
    );
  });

  it("destroys its staging buffer when the copy is refused", async () => {
    const { device, texture } = await canvasOnDevice();
    device.scopeErrors.push(DESTROYED);
    await readCanvasTexture(device, texture, false, "primary").catch(() => undefined);
    expect(device.buffers.map((buffer) => buffer.destroyed)).toEqual([true]);
  });

  it("closes the error scope it opens", async () => {
    const { device, texture } = await canvasOnDevice();
    await readCanvasTexture(device, texture, false, "primary");
    expect(device.errorScopes).toEqual([]);
  });
});

describe("a view's canvas", () => {
  it("renders through the sRGB twin of its 8-bit format", () => {
    expect(srgbViewFormat("bgra8unorm")).toBe("bgra8unorm-srgb");
    expect(srgbViewFormat("rgba8unorm")).toBe("rgba8unorm-srgb");
  });

  it("refuses a format with no sRGB view", () => {
    expect(() => srgbViewFormat("rgba16float")).toThrow(/rgba16float/u);
  });
});

describe("a readback's texels", () => {
  it("lose their row padding", () => {
    const padded = new Uint8Array(512);
    padded.set([1, 2, 3, 4, 5, 6, 7, 8], 0);
    padded.set([9, 10, 11, 12, 13, 14, 15, 16], 256);
    expect([...unpadRows(padded, 2, 2, 256, false)]).toEqual([
      1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
    ]);
  });

  it("come out in RGBA order from a BGRA canvas", () => {
    const padded = new Uint8Array(256);
    padded.set([30, 20, 10, 255], 0);
    expect([...unpadRows(padded, 1, 1, 256, true)]).toEqual([10, 20, 30, 255]);
  });
});

describe("a readback's rows", () => {
  it("are padded to 256 bytes", () => {
    expect(paddedBytesPerRow(64, 4)).toBe(256);
    expect(paddedBytesPerRow(65, 4)).toBe(512);
    expect(paddedBytesPerRow(3, 8)).toBe(256);
    expect(paddedBytesPerRow(128, 8)).toBe(1024);
  });
});
