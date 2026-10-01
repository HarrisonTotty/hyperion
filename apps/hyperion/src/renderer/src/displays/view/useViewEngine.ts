import { useContext, useEffect, useState } from "react";

import { loadRenderEngine } from "../../view/engine/loadEngine";
import { type AdapterOutcome, requestAdapterOutcome } from "../../view/engine/platform";
import {
  type GraphicsStatus,
  GraphicsStatusContext,
  type GraphicsStatusStore,
  navigatorGpu,
  useGraphicsStatus,
} from "../../view/engine/status";
import type { RenderEngine } from "../../view/engine/types";

/** Where the view's engine stands: being made, made, or not to be had. */
export type ViewEngineState =
  | { readonly kind: "pending" }
  | { readonly kind: "ready"; readonly engine: RenderEngine }
  /** No adapter or no engine; the graphics status says why (R01's annunciation). */
  | { readonly kind: "unavailable" };

/** How the hook makes its engine: R01's by default, a fake in a test. */
export interface ViewEngineSource {
  readonly requestAdapter: () => Promise<AdapterOutcome>;
  readonly load: (
    outcome: AdapterOutcome & { readonly kind: "adapter" },
    status: GraphicsStatusStore,
  ) => Promise<RenderEngine>;
}

/** R01's own: `navigator.gpu`'s adapter, and the engine by its lazy import. */
export const DEFAULT_ENGINE_SOURCE: ViewEngineSource = {
  requestAdapter: () => requestAdapterOutcome(navigatorGpu()),
  load: (outcome, status) => loadRenderEngine(outcome, status),
};

/** Whether the graphics' standing condition rules out a view, so that no adapter is asked for. */
function ruledOut({ condition }: GraphicsStatus): boolean {
  return (
    condition.kind === "safe-mode" ||
    condition.kind === "disabled" ||
    condition.kind === "no-webgpu" ||
    condition.kind === "no-adapter"
  );
}

/**
 * The view's engine, made when the display mounts and disposed when it unmounts (plan R02,
 * R02.T15.a).
 *
 * @remarks
 * The engine is loaded through R01's `loadRenderEngine`, whose dynamic import keeps it out of the
 * entry chunk until a view first mounts. A fresh adapter is asked for, since an adapter is consumed
 * by its first device (R01 Design note 7), and the engine reports its faults to the client's one
 * graphics status store. The safe mode, a disabled session and a missing WebGPU or adapter ask
 * for nothing.
 *
 * @throws Error when no `GraphicsStatusContext` provider is above the caller, a wiring bug.
 */
export function useViewEngine(source: ViewEngineSource = DEFAULT_ENGINE_SOURCE): ViewEngineState {
  const store = useContext(GraphicsStatusContext);
  if (store === null) {
    throw new Error("useViewEngine needs a GraphicsStatusContext provider");
  }
  const unavailable = ruledOut(useGraphicsStatus());
  const [state, setState] = useState<ViewEngineState>({ kind: "pending" });
  useEffect(() => {
    if (unavailable) {
      return undefined;
    }
    const life = { ended: false };
    // Read through a call, since the awaits between the reads are where it changes.
    const ended = (): boolean => life.ended;
    const made: { engine: RenderEngine | null } = { engine: null };
    const make = async (): Promise<void> => {
      const outcome = await source.requestAdapter();
      if (ended()) {
        return;
      }
      if (outcome.kind !== "adapter") {
        setState({ kind: "unavailable" });
        return;
      }
      const engine = await source.load(outcome, store);
      if (ended()) {
        engine.dispose();
        return;
      }
      made.engine = engine;
      setState({ kind: "ready", engine });
    };
    make().catch((error: unknown) => {
      console.error("the view's engine could not be made:", error);
      if (!ended()) {
        setState({ kind: "unavailable" });
      }
    });
    return () => {
      life.ended = true;
      made.engine?.dispose();
      setState({ kind: "pending" });
    };
  }, [store, source, unavailable]);
  if (unavailable) {
    return { kind: "unavailable" };
  }
  return state;
}
