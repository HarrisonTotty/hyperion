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

/**
 * The camera panel's readings (plan R02, R02.T15.c; R07.T19.f): the free camera's `RATE`, with
 * where the camera looks, `POINTING`, beside it on its line, and where it is, `POSITION`, on the
 * next, for whichever view `CONTROLS` names, so that an instrument's place, which its slot has no
 * room to state, is on show.
 *
 * @remarks
 * Its own component and layout rules (`.view-camera__rate`, `.view-camera__position`), so that the
 * place's readings can move or take a shorter form in one place. A place held to a craft is muted
 * with its `S` while the server's scene is stale (`placeStale`). The readings change with the
 * readouts, so none is announced.
 */
export function CameraReadings({ rateStep, place, sceneStale }: CameraReadingsProps) {
  const stale = placeStale(place, sceneStale);
  const valueClass = stale ? "field__value stale" : "field__value";
  return (
    <>
      <p className="view-camera__rate">
        <output className="view-camera__rate-reading" aria-label="Free camera rate">
          {freeRateReading(rateStep)}
        </output>
        <span className="field view-camera__place">
          <span className="field__label">POINTING</span>
          <output className={valueClass} aria-label="Camera pointing" aria-live="off">
            {readingParts(place.pointing)}
          </output>
          {stale ? <StaleMark /> : null}
        </span>
      </p>
      <p className="field view-camera__place view-camera__position">
        <span className="field__label">POSITION</span>
        <output className={valueClass} aria-label="Camera position" aria-live="off">
          {readingParts(place.position)}
        </output>
        {stale ? <StaleMark /> : null}
      </p>
    </>
  );
}
