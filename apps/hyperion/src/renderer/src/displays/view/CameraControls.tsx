import { useId } from "react";

import type { ViewKeyAction } from "../../view/camera/keys";
import { FOV_STEPS_DEG } from "../../view/camera/projection";
import type { CameraPreset } from "../../view/camera/state";
import { freeRateReading, PRESET_NAMES } from "./viewRun";

/** Props of {@link CameraControls}. */
export interface CameraControlsProps {
  readonly preset: CameraPreset;
  /** The presets the scene offers: `FREE` alone where it has no own ship. */
  readonly offered: ReadonlyArray<CameraPreset>;
  /** The horizontal field of view, degrees. */
  readonly fovDeg: number;
  /** The free camera's commanded rate step (`freeRateMPerS`). */
  readonly rateStep: number;
  /** The highest rate step the scene allows (`maxFreeRateStep`). */
  readonly maxRateStep: number;
  /** The `EASED CAMERA MOVES` setting. */
  readonly easedMoves: boolean;
  /** Whether the operator asked for reduced motion, under which eased moves are not applied. */
  readonly reducedMotion: boolean;
  /** Called with the action a control asks for, as its key would. */
  readonly onAction: (action: ViewKeyAction) => void;
  readonly onEasedMovesChange: (easedMoves: boolean) => void;
}

/** The presets in their order, each with its single key (`keys.ts`' `VIEW_SINGLE_KEYS`). */
const PRESET_KEYS: ReadonlyArray<{ readonly preset: CameraPreset; readonly key: string }> = [
  { preset: "seat", key: "1" },
  { preset: "chase", key: "2" },
  { preset: "free", key: "3" },
];

/**
 * The view's camera controls (plan R02, R02.T15.c): the presets `SEAT`, `CHASE` and `FREE`, the
 * previous and next target, the field of view a step narrower or wider with its reading, and the
 * `EASED CAMERA MOVES` setting, each a button reachable by keyboard and showing its key; the field
 * of view's buttons are held back at the ends of its steps, the free camera's commanded rate
 * (stepped by `PAGE UP` and `PAGE DOWN` on the canvas) with a statement at either end of its steps,
 * and the setting says when reduced motion stops it applying.
 *
 * @remarks
 * Display controls, which change only what the view shows (`.control`). A preset the scene does not
 * offer, `SEAT` and `CHASE` with no own ship, is held back and says why.
 */
export function CameraControls({
  preset,
  offered,
  fovDeg,
  rateStep,
  maxRateStep,
  easedMoves,
  reducedMotion,
  onAction,
  onEasedMovesChange,
}: CameraControlsProps) {
  const titleId = useId();
  const noShipId = useId();
  const fovLimitId = useId();
  const reducedId = useId();
  const narrowest = fovDeg <= Math.min(...FOV_STEPS_DEG);
  const widest = fovDeg >= Math.max(...FOV_STEPS_DEG);
  const slowest = rateStep <= 0;
  const fastest = rateStep >= maxRateStep;
  const anyHeldBack = PRESET_KEYS.some(({ preset: each }) => !offered.includes(each));
  return (
    <section className="panel view-camera" aria-labelledby={titleId}>
      <h2 className="panel__title" id={titleId}>
        Camera
      </h2>
      <fieldset className="preset-buttons" aria-label="Camera presets">
        {PRESET_KEYS.map(({ preset: each, key }) => {
          const heldBack = !offered.includes(each);
          return (
            <button
              key={each}
              type="button"
              className="control preset-buttons__button"
              aria-pressed={each === preset}
              aria-keyshortcuts={key}
              aria-disabled={heldBack ? "true" : undefined}
              aria-describedby={heldBack ? noShipId : undefined}
              onClick={() => {
                if (!heldBack) {
                  onAction({ kind: "preset", preset: each });
                }
              }}
            >
              <span className="control__key">{key}</span> {PRESET_NAMES[each]}
            </button>
          );
        })}
      </fieldset>
      {anyHeldBack ? (
        <p className="view-camera__reason" id={noShipId}>
          NO OWN SHIP: SEAT and CHASE need one
        </p>
      ) : null}
      <fieldset className="preset-buttons" aria-label="Target">
        <button
          type="button"
          className="control"
          aria-keyshortcuts="["
          onClick={() => {
            onAction({ kind: "target", step: -1 });
          }}
        >
          <span className="control__key">[</span> PREVIOUS TARGET
        </button>
        <button
          type="button"
          className="control"
          aria-keyshortcuts="]"
          onClick={() => {
            onAction({ kind: "target", step: 1 });
          }}
        >
          <span className="control__key">]</span> NEXT TARGET
        </button>
      </fieldset>
      <fieldset className="preset-buttons" aria-label="Field of view">
        <button
          type="button"
          className="control"
          aria-keyshortcuts="+"
          aria-label="Narrower field of view"
          aria-disabled={narrowest ? "true" : undefined}
          aria-describedby={narrowest ? fovLimitId : undefined}
          onClick={() => {
            if (!narrowest) {
              onAction({ kind: "fov", step: -1 });
            }
          }}
        >
          <span className="control__key">+</span> NARROWER
        </button>
        <output className="view-camera__fov" aria-label="Field of view">
          FOV {String(fovDeg)}°
        </output>
        <button
          type="button"
          className="control"
          aria-keyshortcuts="-"
          aria-label="Wider field of view"
          aria-disabled={widest ? "true" : undefined}
          aria-describedby={widest ? fovLimitId : undefined}
          onClick={() => {
            if (!widest) {
              onAction({ kind: "fov", step: 1 });
            }
          }}
        >
          {/* The key as it is pressed, the hyphen-minus, not a missing value. */}
          <span className="control__key">-</span> WIDER
        </button>
      </fieldset>
      {narrowest || widest ? (
        <p className="view-camera__reason" id={fovLimitId}>
          NOT AVAILABLE: FOV at its {narrowest ? "narrowest" : "widest"} step
        </p>
      ) : null}
      <p className="view-camera__rate">
        <output className="view-camera__rate-reading" aria-label="Free camera rate">
          {freeRateReading(rateStep)}
        </output>
      </p>
      {slowest || fastest ? (
        <p className="view-camera__reason">
          NOT AVAILABLE: RATE at its {fastest ? "highest" : "lowest"} step
        </p>
      ) : null}
      <div className="view-camera__setting">
        <button
          type="button"
          className="control preset-buttons__button"
          aria-pressed={easedMoves}
          aria-describedby={reducedMotion ? reducedId : undefined}
          onClick={() => {
            onEasedMovesChange(!easedMoves);
          }}
        >
          EASED CAMERA MOVES
        </button>
        <output aria-hidden="true">{easedMoves ? "ON" : "OFF"}</output>
        {reducedMotion ? (
          <span className="view-camera__reason" id={reducedId}>
            NOT APPLIED: reduced motion
          </span>
        ) : null}
      </div>
    </section>
  );
}
