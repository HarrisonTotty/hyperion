import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it } from "vitest";

import {
  GraphicsStatusContext,
  GraphicsStatusStore,
  initialGraphicsStatus,
  useGraphicsStatus,
} from "./status";

function wrapperFor(store: GraphicsStatusStore) {
  return function Wrapper({ children }: { readonly children: ReactNode }) {
    return <GraphicsStatusContext value={store}>{children}</GraphicsStatusContext>;
  };
}

describe("useGraphicsStatus", () => {
  it("re-renders on a dispatch", () => {
    const store = new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));
    const { result } = renderHook(() => useGraphicsStatus(), { wrapper: wrapperFor(store) });
    expect(result.current.gpuProcessCrashes).toBe(0);
    act(() => {
      store.dispatch({ kind: "gpu-process-gone", count: 1 });
    });
    expect(result.current.gpuProcessCrashes).toBe(1);
  });

  it("unsubscribes on unmount", () => {
    const store = new GraphicsStatusStore(initialGraphicsStatus("vulkan", false));
    const { unmount } = renderHook(() => useGraphicsStatus(), { wrapper: wrapperFor(store) });
    expect(store.listenerCount).toBe(1);
    unmount();
    expect(store.listenerCount).toBe(0);
  });
});
