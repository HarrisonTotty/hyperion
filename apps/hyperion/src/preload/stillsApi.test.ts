import { describe, expect, it, vi } from "vitest";

import { STILLS_CHANNELS } from "../main/stills";
import { rgbaPng } from "../renderer/src/test/pngFixtures";
import { testStillLabel } from "../renderer/src/test/stillFixtures";
import { STILLS_CHANNEL_NAMES, stillsApi } from "./stillsApi";

const ID = "20261113T120000000Z-00000001";

/** The functions over an `invoke` that answers `answer` on every channel. */
function over(answer: unknown) {
  const invoke = vi.fn<(channel: string, ...args: unknown[]) => Promise<unknown>>(() =>
    Promise.resolve(answer),
  );
  return { api: stillsApi({ invoke }), invoke };
}

describe("the preload's still functions (R11.T10.c)", () => {
  it("use the main process's channels", () => {
    expect(STILLS_CHANNEL_NAMES).toEqual(STILLS_CHANNELS);
  });

  it("send a save on its own channel with the PNG and the label", async () => {
    const label = testStillLabel();
    const png = rgbaPng(16, 8);
    const { api, invoke } = over({ kind: "saved", still: { id: ID, label } });
    expect([await api.saveStill(png, label), invoke.mock.calls]).toEqual([
      { kind: "saved", still: { id: ID, label } },
      [[STILLS_CHANNEL_NAMES.save, png, label]],
    ]);
  });

  it("pass a refusal and a failure on as the main process answered them", async () => {
    const label = testStillLabel();
    expect([
      await over({ kind: "refused", reason: "oversized" }).api.saveStill(rgbaPng(16, 8), label),
      await over({ kind: "failed", reason: "ENOSPC" }).api.saveStill(rgbaPng(16, 8), label),
    ]).toEqual([
      { kind: "refused", reason: "oversized" },
      { kind: "failed", reason: "ENOSPC" },
    ]);
  });

  it("refuse an answer to a save that is none of the main process's", async () => {
    await expect(
      over({ kind: "refused", reason: "because" }).api.saveStill(rgbaPng(16, 8), testStillLabel()),
    ).rejects.toThrow("the main process gave no answer to a still's save");
  });

  it("list the stills whose labels are of the schema, passing over the rest", async () => {
    const label = testStillLabel();
    const { api, invoke } = over([
      { id: ID, label },
      { id: "x", label: { schema: "other" } },
    ]);
    expect([await api.listStills(), invoke.mock.calls]).toEqual([
      [{ id: ID, label }],
      [[STILLS_CHANNEL_NAMES.list]],
    ]);
  });

  it("read a still back, or nothing where the main process has none", async () => {
    const label = testStillLabel();
    const png = rgbaPng(16, 8);
    const read = over({ id: ID, png, label });
    expect([
      await read.api.readStill(ID),
      read.invoke.mock.calls,
      await over(null).api.readStill(ID),
    ]).toEqual([{ id: ID, png, label }, [[STILLS_CHANNEL_NAMES.read, ID]], null]);
  });

  it("refuse a still read back without its PNG", async () => {
    await expect(
      over({ id: ID, png: "png", label: testStillLabel() }).api.readStill(ID),
    ).rejects.toThrow("the main process read back no still");
  });
});
