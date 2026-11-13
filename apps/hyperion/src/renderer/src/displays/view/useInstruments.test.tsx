import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it } from "vitest";

import { nominalStore } from "../../test/viewDisplayHarness";
import { GraphicsStatusContext } from "../../view/engine/status";
import { DEFAULT_EXPOSURE } from "../../view/photometry/exposure";
import { phaseScene } from "../../view/scenes/phaseScene";
import { type InstrumentsInput, useInstruments } from "./useInstruments";
import { startRun } from "./viewRun";

const NOTE = "ATMOSPHERE: APPROXIMATE";

/** The instruments over no engine, opening from PHASE TEST's start. */
async function instruments(): Promise<
  ReturnType<typeof renderHook<ReturnType<typeof useInstruments>, unknown>>
> {
  const store = await nominalStore();
  const run = startRun(phaseScene());
  const input: InstrumentsInput = {
    engineState: { kind: "pending" },
    primaryRun: () => run,
    removeCamera: null,
    easedMoves: false,
    reducedMotion: false,
    sky: null,
    exposure: DEFAULT_EXPOSURE,
    setting: "high",
    modifier: "ctrl",
    ground: { current: null },
  };
  return renderHook(() => useInstruments(input), {
    wrapper: ({ children }: { readonly children: ReactNode }) => (
      <GraphicsStatusContext value={store}>{children}</GraphicsStatusContext>
    ),
  });
}

describe("an instrument's folds of its notes' reasons (the owner's request)", () => {
  it("folds every note's reasons as it opens", async () => {
    const { result } = await instruments();

    act(() => {
      result.current.open(1);
    });

    expect(result.current.slots[0]?.reasonsOpened.size).toBe(0);
  });

  it("keeps a note's reasons opened in its slot once pressed, whatever its view does", async () => {
    const { result } = await instruments();
    act(() => {
      result.current.open(1);
    });

    act(() => {
      result.current.foldReasons(1, NOTE);
    });

    expect([...(result.current.slots[0]?.reasonsOpened ?? [])]).toEqual([NOTE]);
  });

  it("folds them on another press", async () => {
    const { result } = await instruments();
    act(() => {
      result.current.open(1);
      result.current.foldReasons(1, NOTE);
    });

    act(() => {
      result.current.foldReasons(1, NOTE);
    });

    expect(result.current.slots[0]?.reasonsOpened.size).toBe(0);
  });

  it("folds them again when it is closed and opened anew", async () => {
    const { result } = await instruments();
    act(() => {
      result.current.open(1);
      result.current.foldReasons(1, NOTE);
    });

    act(() => {
      result.current.close(1);
    });
    act(() => {
      result.current.open(1);
    });

    expect(result.current.slots[0]?.reasonsOpened.size).toBe(0);
  });
});
