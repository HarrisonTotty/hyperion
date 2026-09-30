import { graphicsModeAnnunciation, useGraphicsStatus } from "../view/engine/status";

/**
 * The header strip's banner for a graphics mode that ends only with a relaunch: the safe mode or
 * WebGPU disabled for the session. Nothing otherwise.
 *
 * @remarks
 * The guide reserves the header strip's banner for a simulation, training or replay mode; extending
 * it to these two is drafted for the owner (R01 Design note 10, R01.T5.c). It is the console's
 * statement of its own condition, in plain text, and never counts as an alert.
 */
export function GraphicsModeBanner() {
  const annunciation = graphicsModeAnnunciation(useGraphicsStatus());
  if (annunciation === null) {
    return null;
  }
  return (
    <output className="console__banner" aria-label="Graphics mode">
      {annunciation.text}
    </output>
  );
}
