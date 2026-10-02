import { useId } from "react";

import { formatNumber, formatSignificant } from "../../lib/format";
import {
  type ExposureCommandResult,
  type ExposureControl,
  enable,
  inhibit,
} from "../../view/photometry/exposure";
import { exposureReading } from "./viewRun";

/** Props of {@link ExposurePanel}. */
export interface ExposurePanelProps {
  readonly exposure: ExposureControl;
  /**
   * The metering source's value, EV100, or `null` where there is none: always `null` for a
   * wireframe view until R07's photorealistic view accompanies it.
   */
  readonly meteredEv100: number | null;
  /** Called with the control after an accepted command. */
  readonly onChange: (exposure: ExposureControl) => void;
}

/** Why a command is held back, in the guide's `STATUS: clause` form. */
const REFUSAL_WORDS: Readonly<
  Record<Extract<ExposureCommandResult, { kind: "refused" }>["reason"], string>
> = {
  no_image_to_meter: "NO IMAGE TO METER",
  not_automatic: "NOT AVAILABLE: the exposure is MAN",
  not_inhibited: "NOT AVAILABLE: the exposure is not INHIBITED",
  invalid_triple: "NOT AVAILABLE: the triple is not valid",
};

interface ExposureCommandProps {
  readonly label: string;
  readonly result: ExposureCommandResult;
  readonly onChange: (exposure: ExposureControl) => void;
}

/** A command button, held back with its reason where the command would be refused. */
function ExposureCommand({ label, result, onChange }: ExposureCommandProps) {
  const reasonId = useId();
  const refused = result.kind === "refused";
  return (
    <div className="view-exposure__command">
      <button
        type="button"
        className="control"
        // Held back rather than disabled, so that it keeps focus and can say why.
        aria-disabled={refused ? "true" : undefined}
        aria-describedby={refused ? reasonId : undefined}
        onClick={() => {
          if (result.kind === "accepted") {
            onChange(result.control);
          }
        }}
      >
        {label}
      </button>
      {refused ? (
        <span className="view-exposure__reason" id={reasonId}>
          {REFUSAL_WORDS[result.reason]}
        </span>
      ) : null}
    </div>
  );
}

/**
 * The view's exposure as an instrument (plan R02, R02.T15.c; Design note 11): the value with its
 * unit and automation level, `EV100 -1.0 MAN`, the triple under `MAN`, and the congruent pair
 * `ENABLE` and `INHIBIT`, in the guide's order, each held back with its reason where it would be refused. There is no
 * button named `AUTO` (the guide's "Controls and commanding"); while there is no image to meter,
 * `AUTO NOT AVAILABLE` stands with `NO IMAGE TO METER`.
 *
 * @remarks
 * Display controls: the exposure is the view's own, not the ship's, so a command acts at once.
 */
export function ExposurePanel({ exposure, meteredEv100, onChange }: ExposurePanelProps) {
  const titleId = useId();
  return (
    <section className="panel view-exposure" aria-labelledby={titleId}>
      <h2 className="panel__title" id={titleId}>
        Exposure
      </h2>
      <p className="view-exposure__reading">
        <output>{exposureReading(exposure)}</output>
      </p>
      {exposure.kind === "manual" ? (
        <dl className="view-exposure__triple">
          <div className="field">
            <dt className="field__label">APERTURE</dt>
            <dd>
              <output>f/{formatNumber(exposure.triple.aperture, 1)}</output>
            </dd>
          </div>
          <div className="field">
            <dt className="field__label">SHUTTER</dt>
            <dd>
              <output>{formatSignificant(exposure.triple.shutterS)} s</output>
            </dd>
          </div>
          <div className="field">
            <dt className="field__label">ISO</dt>
            <dd>
              <output>{formatNumber(exposure.triple.iso, 0)}</output>
            </dd>
          </div>
        </dl>
      ) : null}
      {meteredEv100 === null ? (
        <p className="view-exposure__reason">AUTO NOT AVAILABLE: NO IMAGE TO METER</p>
      ) : null}
      <div className="view-exposure__commands">
        <ExposureCommand
          label="ENABLE"
          result={enable(exposure, meteredEv100)}
          onChange={onChange}
        />
        <ExposureCommand label="INHIBIT" result={inhibit(exposure)} onChange={onChange} />
      </div>
    </section>
  );
}
