import { graphicsModeAnnunciation, useGraphicsStatus } from "../view/engine/status";

/**
 * The header strip's banner for a graphics mode that ends only with a relaunch: the safe mode or
 * WebGPU disabled for the session. Nothing otherwise.
 *
 * @remarks
 * The guide reserves the header strip's banner for a simulation, training or replay mode; extending
 * it to these two was signed off on 2026-09-30 (R01 Design note 10, R01.T5.c). It is the console's
 * statement of its own condition, in plain `--text` inside a `--text-muted` rule, and never counts
 * as an alert. It names the mode alone, the words before the colon, so that the strip fits at
 * 1280 × 720; the `GRAPHICS` panel carries the whole sentence.
 */
export function GraphicsModeBanner() {
  const annunciation = graphicsModeAnnunciation(useGraphicsStatus());
  if (annunciation === null) {
    return null;
  }
  return (
    <output className="console__banner" aria-label="Graphics mode">
      {annunciation.text.split(":", 1)[0]}
    </output>
  );
}
