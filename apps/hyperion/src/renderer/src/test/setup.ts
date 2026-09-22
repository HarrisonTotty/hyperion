import "@testing-library/jest-dom/vitest";

import { cleanup } from "@testing-library/react";
import { afterEach, beforeEach, vi } from "vitest";

import { applyColourTokens } from "./colourTokens";
import { FakeResizeObserver } from "./FakeResizeObserver";
import { FakeWebSocket } from "./FakeWebSocket";
import { installPointerCapture } from "./pointerCapture";
import { stubHyperionApi } from "./stubHyperionApi";
import { stubMatchMedia } from "./stubMatchMedia";

// jsdom has no ResizeObserver; every component that measures its layout observes through this.
// Nor does it apply the stylesheet, whose colour tokens canvases read, capture pointers, answer
// media queries, or run the preload that installs `window.hyperion`.
beforeEach(() => {
  FakeResizeObserver.instances = [];
  // The sockets are the one piece of state the fakes keep statically, and `isolate: false` shares
  // the module registry across files, so clearing it here is what makes `FakeWebSocket.latest()`
  // throw for a test that opened no socket rather than hand back an earlier file's.
  FakeWebSocket.instances = [];
  vi.stubGlobal("ResizeObserver", FakeResizeObserver);
  applyColourTokens();
  installPointerCapture();
  stubMatchMedia(false);
  stubHyperionApi();
});

afterEach(() => {
  cleanup();
  // Also for `isolate: false`: a file that installs fake timers must not leave them running for
  // the next file in the worker. This is a no-op when the timers are already real.
  vi.useRealTimers();
});
