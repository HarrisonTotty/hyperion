import { type ReactNode, useEffect, useState } from "react";

import type { GraphicsApi } from "../../../../preload/api";
import {
  feedGraphicsStatus,
  GraphicsStatusContext,
  GraphicsStatusStore,
  initialGraphicsStatus,
} from "./status";

interface GraphicsStatusProviderProps {
  /** `window.hyperion.graphics`: the launch mode, the timing flag and the crash reports. */
  readonly graphics: GraphicsApi;
  /** `navigator.gpu`, or `undefined` where WebGPU is absent. */
  readonly gpu: GPU | undefined;
  readonly children: ReactNode;
}

/**
 * Owns the client's one graphics status store, feeds it from the preload and the first adapter
 * request, and hands it to every display.
 */
export function GraphicsStatusProvider({ graphics, gpu, children }: GraphicsStatusProviderProps) {
  const [store] = useState(
    () => new GraphicsStatusStore(initialGraphicsStatus(graphics.launchMode, graphics.gpuTiming)),
  );
  useEffect(() => feedGraphicsStatus(store, graphics, gpu), [store, graphics, gpu]);
  return <GraphicsStatusContext value={store}>{children}</GraphicsStatusContext>;
}
