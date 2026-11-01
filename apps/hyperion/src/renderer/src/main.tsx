import { lazy, StrictMode, Suspense } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import { watchStrokeProperties } from "./lib/strokes";
import { provideWholeTerrainField } from "./view/scenes/terrainScene";
import "@fontsource/b612/400.css";
import "@fontsource/b612/700.css";
import "@fontsource/b612-mono/400.css";
import "@fontsource/b612-mono/700.css";
import "./styles.css";

const container = document.getElementById("root");
if (container === null) {
  throw new Error("missing #root element");
}

// The SVG strokes' widths at the display's ratio, before the first render and on each change of
// it, so that none is drawn under 2 device px (R07.T16.f). It lasts as long as the page does.
watchStrokeProperties(document.documentElement);

// A `--descent-spike` launch draws the spike in place of the consoles (plan R05, T13.c). Its module
// loads only then, so that the consoles' bundle does not take its terrain and its atmosphere, whose
// reference Earth builds its ozone's curve of growth from the absorbers' cross-sections at load
// (plan R08, R08.T6.d).
const SpikeApp = lazy(async () => ({ default: (await import("./view/spike/SpikeApp")).SpikeApp }));
const spike = window.hyperion.spike;

// A `--terrain --whole` launch's field (plan R10, R10.T14.c), the native tool's file as the main
// process reads it, is provided before VIEW makes `TERRAIN TEST`'s scene on it; a field that cannot
// be had ends the run, naming why.
const viewsCheck = window.hyperion.viewsCheck;
if (viewsCheck?.launch.fixture === "terrain-test-whole") {
  try {
    provideWholeTerrainField(await viewsCheck.wholeField());
  } catch (error: unknown) {
    const reason = error instanceof Error ? error.message : String(error);
    console.error("views check: the whole body's field could not be had:", error);
    await viewsCheck.end({ status: "fail", reason });
  }
}

createRoot(container).render(
  <StrictMode>
    {spike === undefined ? (
      <App />
    ) : (
      <Suspense fallback={null}>
        <SpikeApp
          spike={spike}
          graphics={window.hyperion.graphics}
          serverUrl={window.hyperion.serverUrl}
        />
      </Suspense>
    )}
  </StrictMode>,
);
