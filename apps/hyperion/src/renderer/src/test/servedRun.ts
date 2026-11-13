import type { SceneStateDto } from "@hyperion/protocol";

import { commandRun, startServerRun, type ViewRun } from "../displays/view/viewRun";
import type { CameraPreset } from "../view/camera/state";
import { serverSceneAtPush } from "../view/scene/fromServer";
import { SCENE_CHARTED_PLACE, sceneModelOf } from "./sceneFixture";

/**
 * A run of the server's scene of `state` at its push (R02.T17), its camera at the ship's seat, then
 * cut to `preset` with no eased move, for the tests of the own ship's frame (plan R10, R10.T12;
 * decision-r10-ship-frame).
 *
 * @throws Error when the scene cannot be drawn or the cut is refused.
 */
export function servedRun(state: SceneStateDto, preset: CameraPreset): ViewRun {
  const scene = serverSceneAtPush(sceneModelOf(state), SCENE_CHARTED_PLACE);
  if (scene === null) {
    throw new Error("the fixture's served scene can be drawn");
  }
  const run = startServerRun(scene);
  if (preset === "seat") {
    return run;
  }
  const cut = commandRun(
    run,
    { kind: "preset", preset },
    { easedMoves: false, reducedMotion: false },
  );
  if (cut.kind !== "done") {
    throw new Error(`the cut to ${preset} was refused: ${cut.reason}`);
  }
  return cut.run;
}
