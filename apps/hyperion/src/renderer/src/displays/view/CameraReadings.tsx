import { StaleMark } from "../../components/StaleMark";
import { type CameraPlace, placeStale } from "./cameraPlace";
import { readingParts } from "./ViewLabelBlock";
import { freeRateReading } from "./viewRun";

/** Props of {@link CameraReadings}. */
export interface CameraReadingsProps {
  /** The free camera's rate step (`freeRateMPerS`). */
  readonly rateStep: number;
  /** Where the view's camera is and where it looks (`cameraPlace`). */
  readonly place: CameraPlace;
  /** Whether the server's scene is stale, which a camera held to a craft reads with it. */
  readonly sceneStale: boolean;
}

/** A reading's value class: muted while it is stale. */
function valueClass(stale: boolean): string {
  return stale ? "field__value stale" : "field__value";
}

/**
 * The camera panel's readings (plan R02, R02.T15.c; R07.T19.f; decision-r07-t19f-position, item
 * 2): where the camera is, `POSITION`, on its own line, then the free camera's `RATE` with where
 * the camera looks, `POINTING`, beside it, for whichever view `CONTROLS` names, so that an
 * instrument's place, which its slot has no room to state, is on show. The rate's limit reason
 * follows them, under the rate it describes.
 *
 * @remarks
 * Its own component and layout rules (`.view-camera__position`, `.view-camera__rate`), so that the
 * place's readings can move or take a shorter form in one place. `POSITION` is muted with its `S`
 * while the server's scene is stale and the camera is held to a craft, and `POINTING` while it is
 * stale in `SEAT` and `CHASE` (`placeStale`). The readings change with the readouts, so none is
 * announced.
 */
export function CameraReadings({ rateStep, place, sceneStale }: CameraReadingsProps) {
  const stale = placeStale(place, sceneStale);
  return (
    <>
      <p className="field view-camera__place view-camera__position">
        <span className="field__label">POSITION</span>
        <output className={valueClass(stale.position)} aria-label="Camera position" aria-live="off">
          {readingParts(place.position)}
        </output>
        {stale.position ? <StaleMark /> : null}
      </p>
      <p className="view-camera__rate">
        <output className="view-camera__rate-reading" aria-label="Free camera rate">
          {freeRateReading(rateStep)}
        </output>
        <span className="field view-camera__place">
          <span className="field__label">POINTING</span>
          <output
            className={valueClass(stale.pointing)}
            aria-label="Camera pointing"
            aria-live="off"
          >
            {readingParts(place.pointing)}
          </output>
          {stale.pointing ? <StaleMark /> : null}
        </span>
      </p>
    </>
  );
}
