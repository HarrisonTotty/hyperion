import { useId } from "react";

import type { ExposureReading } from "../../view/post/autoExposure";
import type { MeterMode } from "../../view/post/meter";
import { viewDisplayName } from "./viewNames";
import { exposureReading } from "./viewRun";

/** Props of {@link MeterControl}. */
export interface MeterControlProps {
  /**
   * The view the panel acts on, its system designator on the title row (`PRIMARY`,
   * `INSTRUMENT 1`; R07.T19), or none.
   */
  readonly designator?: string | undefined;
  /** The operator's meter, which stands whether or not an image is metered now. */
  readonly meter: MeterMode;
  /** The metered exposure, or `null` while no photorealistic view meters an image. */
  readonly reading: ExposureReading | null;
  /** Called with the operator's choice of meter; a display control, so it acts at once. */
  readonly onMeter: (mode: MeterMode) => void;
}

/** The meters in Design note 10's order, with their labels, for T16's guide draft (`METER AVG`, …). */
const METERS: ReadonlyArray<{ readonly mode: MeterMode; readonly label: string }> = [
  { mode: "average", label: "AVG" },
  { mode: "lit", label: "LIT" },
  { mode: "dark", label: "DARK" },
];

/** A meter's label, as the label block and this control show it. */
export function meterLabel(mode: MeterMode): string {
  return METERS.find((meter) => meter.mode === mode)?.label ?? mode.toUpperCase();
}

/**
 * The exposure meter beside R02's `ExposurePanel` (plan R07, T13.b; Design notes 10–11): the
 * applied EV100 with its automation level, the meter, the photorealistic view it meters, and the
 * choice of `AVG`, `LIT` or `DARK`, each a keyboard-operable button, the chosen one underlined.
 *
 * @remarks
 * `AVG` meters everything but a star's disc, `LIT` only bodies' sunlit sides and `DARK` only their
 * night sides. The meter is the view's own, not the ship's: a display control that acts at once.
 * While nothing is metered the meter still shows with the reason, which describes the chosen
 * meter's button only, and its buttons still act, so that a meter with nothing to weigh can be
 * left by choosing another (mounted by R07.T8.a beside a drawn image only).
 */
export function MeterControl({ meter, reading, onMeter, designator }: MeterControlProps) {
  const titleId = useId();
  const meterId = useId();
  const sourceId = useId();
  const reasonId = useId();
  const held = reading === null;
  return (
    <section className="panel view-meter" aria-labelledby={titleId}>
      <h2 className="panel__title" id={titleId}>
        Exposure meter
        {designator === undefined ? null : (
          <>
            {" "}
            <span className="panel__designator">{designator}</span>
          </>
        )}
      </h2>
      {reading === null ? (
        <p className="view-meter__reason" id={reasonId}>
          NO IMAGE TO METER
        </p>
      ) : (
        <>
          <p className="view-meter__reading">
            <output>{exposureReading(reading.control)}</output>
          </p>
          <p className="field">
            <span className="field__label" id={sourceId}>
              SOURCE
            </span>{" "}
            <output className="view-meter__value" aria-labelledby={sourceId}>
              {viewDisplayName(reading.source)}
            </output>
          </p>
        </>
      )}
      <p className="field">
        <span className="field__label" id={meterId}>
          METER
        </span>{" "}
        <output className="view-meter__value" aria-labelledby={meterId}>
          {meterLabel(meter)}
        </output>
      </p>
      <fieldset className="view-meter__modes">
        <legend className="field__label">SELECT</legend>
        {METERS.map(({ mode, label }) => (
          <button
            key={mode}
            type="button"
            className="control"
            aria-pressed={meter === mode}
            // Never held back: the control stands only beside a drawn image, and a meter with
            // nothing to weigh (LIT with no lit body) must be left by choosing another. The reason
            // describes the chosen meter only: choosing another is the remedy, not refused.
            aria-describedby={held && meter === mode ? reasonId : undefined}
            onClick={() => {
              onMeter(mode);
            }}
          >
            {label}
          </button>
        ))}
      </fieldset>
    </section>
  );
}
