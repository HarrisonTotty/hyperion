import { describe, expect, it } from "vitest";

import { DEFAULT_EXPOSURE } from "../../view/photometry/exposure";
import type { ViewBody } from "../../view/scene/model";
import type { KeptScene } from "../../view/scenes/kept";
import { frameChangeScene } from "../../view/scenes/frameChange";
import { precisionScene } from "../../view/scenes/precision";
import {
  commandRun,
  frameName,
  labelLines,
  labelStatements,
  markRows,
  type ViewRun,
  startRun,
  stepRun,
} from "./viewRun";

const STILL = { dtS: 0, held: new Set<string>(), reducedMotion: false };
const CUT = { easedMoves: false, reducedMotion: false };

function done(result: ReturnType<typeof commandRun>): ViewRun {
  if (result.kind !== "done") {
    throw new Error(`the command was refused: ${result.reason}`);
  }
  return result.run;
}

/** Whether the label block says the camera is off the hull. */
function offHull(run: ViewRun): boolean {
  return labelStatements(run).includes("POSITIONS AS SEEN FROM SHIP");
}

/** A kept scene whose bodies' rotation is not modelled. */
function unrotated(kept: KeptScene): KeptScene {
  return {
    ...kept,
    sceneAt: (tS) => {
      const scene = kept.sceneAt(tS);
      const bodies: ViewBody[] = [];
      for (const body of scene.bodies) {
        bodies.push({ ...body, rotation: null });
      }
      return { ...scene, bodies };
    },
  };
}

describe("a view's run", () => {
  it("starts at the seat of the scene's own ship", () => {
    expect(startRun(precisionScene()).camera.preset).toBe("seat");
  });

  it("runs the script in real time and from its start again at its end", () => {
    const kept = precisionScene();
    const run = stepRun(stepRun(startRun(kept), { ...STILL, dtS: 0.25 }), { ...STILL, dtS: 0.25 });
    const wrapped = Array.from({ length: Math.ceil(kept.durationS / 0.25) }).reduce<ViewRun>(
      (each) => stepRun(each, { ...STILL, dtS: 0.25 }),
      startRun(kept),
    );
    expect([run.tS, wrapped.tS < kept.durationS]).toEqual([0.5, true]);
  });

  it("flies a free camera by the held keys", () => {
    const free = done(
      commandRun(startRun(precisionScene()), { kind: "preset", preset: "free" }, CUT),
    );
    const flown = stepRun(free, { dtS: 0.1, held: new Set(["w"]), reducedMotion: true });
    expect(flown.camera.pose.positionM).not.toEqual(free.camera.pose.positionM);
  });

  it("cuts to a preset at once under reduced motion, even with eased moves on", () => {
    const run = startRun(precisionScene());
    const eased = done(
      commandRun(
        run,
        { kind: "preset", preset: "chase" },
        { easedMoves: true, reducedMotion: false },
      ),
    );
    const reduced = done(
      commandRun(
        run,
        { kind: "preset", preset: "chase" },
        { easedMoves: true, reducedMotion: true },
      ),
    );
    expect([eased.camera.move === null, reduced.camera.move === null]).toEqual([false, true]);
  });

  it("steps the field of view", () => {
    const narrower = done(commandRun(startRun(precisionScene()), { kind: "fov", step: -1 }, CUT));
    expect(narrower.camera.fovDeg).toBe(45);
  });

  it("aims the camera at the next target", () => {
    const targeted = done(commandRun(startRun(precisionScene()), { kind: "target", step: 1 }, CUT));
    expect(targeted.camera.target).not.toBeNull();
  });
});

describe("the label block", () => {
  it("states the frame, time, style, camera, field of view, exposure, stars and scene", () => {
    const lines = labelLines(startRun(precisionScene()), DEFAULT_EXPOSURE);
    expect(lines.map((line) => line.label)).toEqual([
      "FRAME",
      "TIME",
      "STYLE",
      "CAMERA",
      "FOV",
      "EXPOSURE",
      "STARS",
      "SCENE",
    ]);
  });

  it("reads the exposure as EV100 -1.0 MAN and the field of view in degrees", () => {
    const lines = labelLines(startRun(precisionScene()), DEFAULT_EXPOSURE);
    expect(
      lines.filter((line) => ["STYLE", "CAMERA", "FOV", "EXPOSURE", "SCENE"].includes(line.label)),
    ).toEqual([
      { label: "STYLE", value: "WIREFRAME" },
      { label: "CAMERA", value: "SEAT" },
      { label: "FOV", value: "60°" },
      { label: "EXPOSURE", value: "EV100 -1.0 MAN" },
      { label: "SCENE", value: "PRECISION TEST" },
    ]);
  });

  it("says POSITIONS AS SEEN FROM SHIP while the camera is off the hull, not at the seat", () => {
    const seat = startRun(precisionScene());
    const chase = done(commandRun(seat, { kind: "preset", preset: "chase" }, CUT));
    expect([offHull(seat), offHull(chase)]).toEqual([false, true]);
  });

  it("says ROTATION NOT YET MODELLED while a body's rotation is not modelled", () => {
    expect(labelStatements(startRun(unrotated(frameChangeScene())))).toEqual([
      "ROTATION NOT YET MODELLED",
    ]);
  });

  it("names the system and galactic frames", () => {
    const { scene } = startRun(precisionScene());
    expect([
      frameName({ kind: "system", system: scene.system }, scene),
      frameName({ kind: "galactic", origin: scene.barycentre }, scene),
    ]).toEqual(["SYSTEM BARYCENTRIC", "GALACTIC"]);
  });

  it("names a craft's frame by the frame its position is in, a body-fixed one its body's", () => {
    const { scene } = startRun(frameChangeScene());
    const grounded = scene.craft.find((craft) => craft.pose.position.kind === "body_fixed");
    const position = grounded?.pose.position;
    const body =
      position?.kind === "body_fixed"
        ? scene.bodies.find((each) => each.id === position.body)
        : undefined;
    expect(
      grounded === undefined ? null : frameName({ kind: "craft", craft: grounded.id }, scene),
    ).toBe(`BODY ${body?.designation ?? ""}`);
  });

  it("names the scene only in a kept scene", () => {
    const run = startRun(precisionScene());
    const server = { ...run, scene: { ...run.scene, provenance: { kind: "server" as const } } };
    expect(labelLines(server, DEFAULT_EXPOSURE).some((line) => line.label === "SCENE")).toBe(false);
  });

  it("names a body frame by its body's designation", () => {
    const run = startRun(frameChangeScene());
    const body = run.scene.bodies.find((each) => each.kind === "planet");
    expect(body === undefined ? null : frameName({ kind: "body", body: body.id }, run.scene)).toBe(
      `BODY ${body?.designation ?? ""}`,
    );
  });
});

describe("the list", () => {
  it("lists the bodies and the craft", () => {
    const run = startRun(frameChangeScene());
    expect(markRows(run)).toHaveLength(run.scene.bodies.length + run.scene.craft.length - 1);
  });

  it("leaves the own ship out", () => {
    const run = startRun(frameChangeScene());
    expect(
      markRows(run).some(
        (row) => row.target.kind === "craft" && row.target.craft === run.scene.ownShip,
      ),
    ).toBe(false);
  });

  it("gives each range from the own ship, with its unit", () => {
    expect(
      markRows(startRun(frameChangeScene())).every(
        (row) => !row.fromCamera && /^[\d.,]+(E-?\d+)? (km|Mm|Gm|AU)$/.test(row.range),
      ),
    ).toBe(true);
  });

  it("keeps a range's unit within its hysteresis", () => {
    const run = startRun(frameChangeScene());
    const first = markRows(run)[0];
    if (first === undefined) {
      throw new Error("the frame-change scene has no rows");
    }
    const other = first.unit === "AU" ? "Gm" : "AU";
    const again = markRows(run, new Map([[first.key, other]]))[0];
    // A unit far from the range's own is left; the same unit is kept.
    const kept = markRows(run, new Map([[first.key, first.unit]]))[0];
    expect([again?.unit === other, kept?.unit]).toEqual([false, first.unit]);
  });

  it("labels its ranges FROM CAMERA where there is no own ship", () => {
    const kept = precisionScene();
    const shipless: KeptScene = {
      ...kept,
      sceneAt: (tS) => {
        const scene = kept.sceneAt(tS);
        return { ...scene, ownShip: null };
      },
    };
    expect(markRows(startRun(shipless)).every((row) => row.fromCamera)).toBe(true);
  });
});
