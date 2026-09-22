import "@testing-library/jest-dom/vitest";

import { cleanup } from "@testing-library/react";
import { afterEach, beforeEach, vi } from "vitest";

import { applyColourTokens } from "./colourTokens";
import { FakeResizeObserver } from "./FakeResizeObserver";
import { installPointerCapture } from "./pointerCapture";
import { stubHyperionApi } from "./stubHyperionApi";
import { stubMatchMedia } from "./stubMatchMedia";

// jsdom has no ResizeObserver; every component that measures its layout observes through this.
// Nor does it apply the stylesheet, whose colour tokens canvases read, capture pointers, answer
// media queries, or run the preload that installs `window.hyperion`.
beforeEach(() => {
  FakeResizeObserver.instances = [];
  vi.stubGlobal("ResizeObserver", FakeResizeObserver);
  applyColourTokens();
  installPointerCapture();
  stubMatchMedia(false);
  stubHyperionApi();
});

afterEach(() => {
  cleanup();
});
