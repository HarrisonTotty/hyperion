import { useId } from "react";

import { formatNumber, formatSignificant } from "../../lib/format";
import {
  controlEv100,
  type ExposureCommandResult,
  type ExposureControl,
  type ExposureTriple,
  enable,
  inhibit,
  programTriple,
  VIEW_CAMERA,
} from "../../view/photometry/exposure";
import { exposureReading } from "./viewRun";

/** Props of {@link ExposurePanel}. */
export interface ExposurePanelProps {
  /**
   * The view the panel acts on, its system designator on the title row (`PRIMARY`,
   * `INSTRUMENT 1`; R07.T19), or none.
   */
  readonly designator?: string | undefined;
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
  already_auto: "NOT AVAILABLE: the exposure is AUTO",
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
 * The camera's setting at the shown control: `MAN`'s own triple, else the view camera's at the
 * control's EV100 (decision-r07-exposure-camera).
 */
function shownTriple(exposure: ExposureControl): ExposureTriple {
  return exposure.kind === "manual"
    ? exposure.triple
    : programTriple(VIEW_CAMERA, controlEv100(exposure));
}

/** The guide's off-scale mark after a pegged value, `↑`, announced as "off scale high". */
function OffScaleHigh() {
  return (
    <>
      {" "}
      <span aria-hidden="true">↑</span>
      <span className="visually-hidden"> off scale high</span>
    </>
  );
}

interface CameraSettingProps {
  readonly triple: ExposureTriple;
}

/**
 * The camera's `APERTURE`, `SHUTTER`, `ND` and `ISO`, a member beyond the view camera's range
 * pegged at its end with the guide's off-scale `↑` (decision-r07-exposure-camera, item d).
 */
function CameraSetting({ triple }: CameraSettingProps) {
  const ndEv = triple.ndEv ?? 0;
  const ndPegged = ndEv > VIEW_CAMERA.maxNdEv;
  const isoPegged = triple.iso > VIEW_CAMERA.maxIso;
  return (
    <dl className="readout view-exposure__triple">
      <dt>APERTURE</dt>
      <dd>
        <output>f/{formatNumber(triple.aperture, 1)}</output>
      </dd>
      <dt>SHUTTER</dt>
      <dd>
        <output>{formatSignificant(triple.shutterS)} s</output>
      </dd>
      <dt>ND</dt>
      <dd>
        <output>
          {ndEv === 0 ? (
            "CLEAR"
          ) : (
            <>
              {formatNumber(ndPegged ? VIEW_CAMERA.maxNdEv : ndEv, 1)} EV
              {ndPegged ? <OffScaleHigh /> : null}
            </>
          )}
        </output>
      </dd>
      <dt>ISO</dt>
      <dd>
        <output>
          {formatNumber(isoPegged ? VIEW_CAMERA.maxIso : triple.iso, 0)}
          {isoPegged ? <OffScaleHigh /> : null}
        </output>
      </dd>
    </dl>
  );
}

/**
 * The view's exposure as an instrument (plan R02, R02.T15.c; Design note 11): the value with its
 * unit and automation level, `EV100 -1.0 MAN`, the camera's setting at every level (`APERTURE`,
 * `SHUTTER`, `ND`, `ISO`; R07.T13.c), and the congruent pair `ENABLE` and `INHIBIT`, in the guide's
 * order, each held back with its reason where it would be refused. There is no button named `AUTO`
 * (the guide's "Controls and commanding"); while there is no image to meter, `AUTO NOT AVAILABLE`
 * stands with `NO IMAGE TO METER`.
 *
 * @remarks
 * Display controls: the exposure is the view's own, not the ship's, so a command acts at once. The
 * setting is `MAN`'s triple, else the view camera's program at the shown EV100, so it changes only
 * with the published control.
 */
export function ExposurePanel({
  exposure,
  meteredEv100,
  onChange,
  designator,
}: ExposurePanelProps) {
  const titleId = useId();
  return (
    <section className="panel view-exposure" aria-labelledby={titleId}>
      <h2 className="panel__title" id={titleId}>
        Exposure
        {designator === undefined ? null : (
          <>
            {" "}
            <span className="panel__designator">{designator}</span>
          </>
        )}
      </h2>
      <p className="view-exposure__reading">
        <output>{exposureReading(exposure)}</output>
      </p>
      <CameraSetting triple={shownTriple(exposure)} />
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
