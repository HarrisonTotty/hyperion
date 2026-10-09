/** Exposure controls for tests: `AUTO` and `MAN` at an EV100. */

import { type ExposureControl, setManualEv100 } from "../view/photometry/exposure";

/** `AUTO` standing at an EV100. */
export function autoAt(ev100: number): ExposureControl {
  return { kind: "auto", ev100 };
}

/** `MAN` entered at an EV100, as the exposure panel's field enters it (R07.T13.d). */
export function manualAt(ev100: number): ExposureControl {
  const result = setManualEv100(ev100);
  if (result.kind !== "accepted") {
    throw new Error(`MAN at EV100 ${ev100} was refused`);
  }
  return result.control;
}
