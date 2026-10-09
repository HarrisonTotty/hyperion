import { useId } from "react";

import { formatNumber } from "../../lib/format";
import type { ViewId } from "../../view/camera/state";
import type { ExposureControl } from "../../view/photometry/exposure";
import { type Metering, meterStatus } from "../../view/post/autoExposure";
import type { MeterMode } from "../../view/post/meter";
import { exposureNote } from "./ExposurePanel";
import { readingParts } from "./ViewLabelBlock";
import { viewDisplayName } from "./viewNames";
import { exposureReading } from "./viewRun";

/** Props of {@link MeterControl}. */
export interface MeterControlProps {
  /** The panel's ID, by which a disclosure button controls it (R07.T19.b), or none. */
  readonly id?: string | undefined;
  /** Whether the panel is folded behind its disclosure button in the compact layout (R07.T19.b). */
  readonly hidden?: boolean | undefined;
  /**
   * The view the panel acts on, its system designator on the title row (`PRIMARY`,
   * `INSTRUMENT 1`; R07.T19), or none.
   */
  readonly designator?: string | undefined;
  /** The operator's meter, which stands whether or not an image is metered now. */
  readonly meter: MeterMode;
  /** The exposure as the display holds it, the `Exposure` panel's, metered or not. */
  readonly exposure: ExposureControl;
  /** The photorealistic view whose image is metered. */
  readonly source: ViewId;
  /**
   * The meter as it stands: its own value, EV100, which `ENABLE` takes (R07.T13.d), or why it has
   * none (R07.T16.b).
   */
  readonly metering: Metering;
  /** Called with the operator's choice of meter; a display control, so it acts at once. */
  readonly onMeter: (mode: MeterMode) => void;
}

/** The meters in Design note 10's order, with their labels, for T16's guide draft (`METER AVG`, …). */
const METERS: ReadonlyArray<{ readonly mode: MeterMode; readonly label: string }> = [
  { mode: "average", label: "AVG" },
  { mode: "lit", label: "LIT" },
  { mode: "dark", label: "DARK" },
];

/**
 * Whether the exposure leaves the meter's value unapplied: under `MAN`, or held by the operator's
 * `INHIBIT`. A system inhibit resumes `AUTO` by itself when the meter reads.
 */
function meterNotFollowed(control: ExposureControl): boolean {
  return (
    control.kind === "manual" || (control.kind === "inhibited" && control.reason === "operator")
  );
}

/** A meter's label, as the label block and this control show it. */
export function meterLabel(mode: MeterMode): string {
  return METERS.find((meter) => meter.mode === mode)?.label ?? mode.toUpperCase();
}

/**
 * What the operator can do where a meter weighs nothing in the drawn image, which follows its
 * status on this control alone (decision-r07-t8a-meter, item 1).
 */
const REMEDIES: Readonly<Record<MeterMode, string>> = {
  average: "widen the view or turn from the star",
  lit: "choose AVG, or bring a sunlit body into view",
  dark: "choose AVG, or bring a night side into view",
};

/**
 * The meter's status on this control while it has no value, or `null` where it has none to say:
 * `NO IMAGE TO METER`, or a meter's own status followed by what to do,
 * `NO LIT SIDE: choose AVG, or bring a sunlit body into view` (decision-r07-t8a-meter, item 1).
 */
export function meterStatusLine(metering: Metering): string | null {
  const status = meterStatus(metering);
  return status !== null && metering.kind === "nothing-weighed"
    ? `${status}: ${REMEDIES[metering.meter]}`
    : status;
}

/**
 * The exposure meter beside R02's `ExposurePanel` (plan R07, T13.b; Design notes 10–11): the
 * applied EV100 with its automation level, the meter's own value while the exposure does not follow
 * it (`METERED`, R07.T13.d), the meter, the photorealistic view it meters, and the choice of `AVG`,
 * `LIT` or `DARK`, each a keyboard-operable button, the chosen one underlined.
 *
 * @remarks
 * `METERED` stands under `MAN` and `INHIBITED · OPERATOR`: it is the value `ENABLE` would take, and
 * one the operator may enter in the `MAN` field; before the first histogram it is missing, `—`.
 * `AVG` meters everything but a star's disc, `LIT` only bodies' sunlit sides and `DARK` only their
 * night sides. The meter is the view's own, not the ship's: a display control that acts at once.
 * Without a value its status stands in the reading's place (`NO LIT SIDE: choose AVG, or bring a
 * sunlit body into view`), describing the chosen meter's button only, and its buttons still act,
 * so that a meter with nothing to weigh can be left by choosing another (mounted by R07.T8.a beside
 * a drawn image only). In the window before the first histogram (`acquiring`) it shows its reading
 * as it stands, with no status (R07.T16.b). The status is a live region, since it comes by itself.
 */
export function MeterControl({
  meter,
  exposure,
  source,
  metering,
  onMeter,
  designator,
  id,
  hidden,
}: MeterControlProps) {
  const titleId = useId();
  const meteredId = useId();
  const meterId = useId();
  const sourceId = useId();
  const reasonId = useId();
  const status = meterStatusLine(metering);
  // The meter's own value while the exposure does not follow it, missing until the first histogram.
  const unfollowed = meterNotFollowed(exposure) ? metering : null;
  return (
    <section className="panel view-meter" aria-labelledby={titleId} id={id} hidden={hidden}>
      <h2 className="panel__title" id={titleId}>
        Exposure meter
        {designator === undefined ? null : (
          <>
            {" "}
            <span className="panel__designator">{designator}</span>
          </>
        )}
      </h2>
      {status === null ? (
        <>
          <p className="view-meter__reading">
            <output>{readingParts(exposureReading(exposure))}</output>
          </p>
          {unfollowed === null ? null : (
            <p className="field">
              <span className="field__label" id={meteredId}>
                METERED
              </span>{" "}
              <output className="view-meter__value view-meter__metered" aria-labelledby={meteredId}>
                {unfollowed.kind === "metered" ? (
                  `EV100 ${formatNumber(unfollowed.ev100, 1)}`
                ) : (
                  <span className="readout__missing">—</span>
                )}
              </output>
            </p>
          )}
          <p className="field">
            <span className="field__label" id={sourceId}>
              SOURCE
            </span>{" "}
            <output className="view-meter__value" aria-labelledby={sourceId}>
              {viewDisplayName(source)}
            </output>
          </p>
        </>
      ) : (
        <p className="view-meter__reason" id={reasonId}>
          <output>{exposureNote(status)}</output>
        </p>
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
            // nothing to weigh (LIT with no lit body) must be left by choosing another. The status
            // describes the chosen meter only: choosing another is the remedy, not refused.
            aria-describedby={status !== null && meter === mode ? reasonId : undefined}
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
