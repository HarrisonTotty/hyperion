import { useId } from "react";

import type { PrimaryModifier } from "../../lib/platform";
import type { ViewKeyAction } from "../../view/camera/keys";
import { FOV_STEPS_DEG } from "../../view/camera/projection";
import type { CameraPreset } from "../../view/camera/state";
import type { CameraPlace } from "./cameraPlace";
import { CameraReadings } from "./CameraReadings";
import { RateChord } from "./RateChord";
import { PRESET_NAMES } from "./viewRun";

/** Props of {@link CameraControls}. */
export interface CameraControlsProps {
  /** The panel's ID, by which a disclosure button controls it (R07.T19.b), or none. */
  readonly id?: string | undefined;
  /** Whether the panel is folded behind its disclosure button in the compact layout (R07.T19.b). */
  readonly hidden?: boolean | undefined;
  /**
   * The view the panel acts on, its system designator on the title row (`PRIMARY`,
   * `INSTRUMENT 1`; R07.T19), or none.
   */
  readonly designator?: string | undefined;
  readonly preset: CameraPreset;
  /** The presets the scene offers: `FREE` alone where it has no own ship. */
  readonly offered: ReadonlyArray<CameraPreset>;
  /** The horizontal field of view, degrees. */
  readonly fovDeg: number;
  /** The free camera's rate step (`freeRateMPerS`). */
  readonly rateStep: number;
  /** The highest rate step the scene allows (`maxFreeRateStep`). */
  readonly maxRateStep: number;
  /**
   * Where the view's camera is and where it looks (R07.T19.f; `cameraPlace`): `POSITION`'s and
   * `POINTING`'s readings.
   */
  readonly place: CameraPlace;
  /** Whether the server's scene is stale: a place held to a craft is muted with its `S`. */
  readonly sceneStale: boolean;
  /**
   * The platform's primary modifier, whose chord with the arrows the rate's limit reason names
   * (decision-r07-t19f-position, item 5).
   */
  readonly modifier: PrimaryModifier;
  /** The `EASED CAMERA MOVES` setting. */
  readonly easedMoves: boolean;
  /** Whether the operator asked for reduced motion, under which eased moves are not applied. */
  readonly reducedMotion: boolean;
  /** Called with the action a control asks for, as its key would. */
  readonly onAction: (action: ViewKeyAction) => void;
  readonly onEasedMovesChange: (easedMoves: boolean) => void;
}

/**
 * Why `SEAT` and `CHASE` are held back where the scene has no own ship; it stands under the compact
 * layout's row while this panel is folded (R07.T19.b).
 */
export const NO_OWN_SHIP = "NO OWN SHIP: SEAT and CHASE need one";

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
 * of view's buttons are held back at the ends of its steps, the free camera's rate (stepped on the
 * canvas, in `FREE` only) with a statement at either end of its steps that names the platform's
 * chord, `NOT AVAILABLE: CTRL+↑, RATE at its highest step` (`⌘↑` on macOS;
 * decision-r07-t19f-position, item 5), and the setting says when reduced motion stops it applying.
 * Above and beside the rate stand where the camera is and where it looks, `POSITION` and
 * `POINTING` (R07.T19.f; `CameraReadings`), so that an instrument's place, which its slot has no
 * room to state, is on show while `CONTROLS` names it.
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
  place,
  sceneStale,
  modifier,
  easedMoves,
  reducedMotion,
  onAction,
  onEasedMovesChange,
  designator,
  id,
  hidden,
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
    <section className="panel view-camera" aria-labelledby={titleId} id={id} hidden={hidden}>
      <h2 className="panel__title" id={titleId}>
        Camera
        {designator === undefined ? null : (
          <>
            {" "}
            <span className="panel__designator">{designator}</span>
          </>
        )}
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
          {NO_OWN_SHIP}
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
      <CameraReadings rateStep={rateStep} place={place} sceneStale={sceneStale} />
      {slowest || fastest ? (
        <p className="view-camera__reason">
          NOT AVAILABLE: <RateChord modifier={modifier} arrows={fastest ? "↑" : "↓"} />, RATE at its{" "}
          {fastest ? "highest" : "lowest"} step
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
