import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import { SpikeApp } from "./view/spike/SpikeApp";
import "@fontsource/b612/400.css";
import "@fontsource/b612/700.css";
import "@fontsource/b612-mono/400.css";
import "@fontsource/b612-mono/700.css";
import "./styles.css";

const container = document.getElementById("root");
if (container === null) {
  throw new Error("missing #root element");
}

// A `--descent-spike` launch draws the spike in place of the consoles (plan R05, T13.c).
const spike = window.hyperion.spike;
createRoot(container).render(
  <StrictMode>
    {spike === undefined ? <App /> : <SpikeApp spike={spike} graphics={window.hyperion.graphics} />}
  </StrictMode>,
);
