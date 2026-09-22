import "@testing-library/jest-dom/vitest";

import { cleanup } from "@testing-library/react";
import { afterEach, beforeEach, vi } from "vitest";

import { applyColourTokens } from "./colourTokens";
import { FakeResizeObserver } from "./FakeResizeObserver";

// jsdom has no ResizeObserver; every component that measures its layout observes through this.
// Nor does it apply the stylesheet, whose colour tokens canvases read.
beforeEach(() => {
  FakeResizeObserver.instances = [];
  vi.stubGlobal("ResizeObserver", FakeResizeObserver);
  applyColourTokens();
});

afterEach(() => {
  cleanup();
});
