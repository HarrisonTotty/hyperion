import { describe, expect, it } from "vitest";

import { DEFAULT_EXPOSURE } from "../../view/photometry/exposure";
import { normalise, scale, vec3 } from "../../geometry/vec3";
import { differenceM } from "../../view/coords/position";
import { sceneOrigins, type ViewBody } from "../../view/scene/model";
import { KEPT_BARYCENTRE, type KeptScene } from "../../view/scenes/kept";
import { frameChangeScene } from "../../view/scenes/frameChange";
import { precisionScene } from "../../view/scenes/precision";
import { phaseScene } from "../../view/scenes/phaseScene";
import type { SystemIdHex } from "@hyperion/protocol";

import {
  cameraReading,
  commandRun,
  followRun,
  photorealStatements,
  frameName,
  freeRateReading,
  labelLines,
  labelStatements,
  markRows,
  rangesFromCamera,
  type ViewRun,
  startInstrumentRun,
  startRun,
  stepRun,
} from "./viewRun";

const STILL = { serverScene: null, dtS: 0, held: new Set<string>(), reducedMotion: false };
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
    const flown = stepRun(free, {
      serverScene: null,
      dtS: 0.1,
      held: new Set(["w"]),
      reducedMotion: true,
    });
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

  it("reads the free camera's rate in m/s below 1 km/s and in km/s from there", () => {
    expect(freeRateReading(0)).toBe("RATE 1.00 m/s");
    expect(freeRateReading(5)).toBe("RATE 316 m/s");
    expect(freeRateReading(6)).toBe("RATE 1.00 km/s");
    expect(freeRateReading(7)).toBe("RATE 3.16 km/s");
  });

  it("steps the free camera's rate in FREE (R07.T19.b)", () => {
    const free = done(
      commandRun(startRun(precisionScene()), { kind: "preset", preset: "free" }, CUT),
    );
    const faster = done(commandRun(free, { kind: "rate", step: 1 }, CUT));
    expect(faster.camera.free.rateStep).toBe(free.camera.free.rateStep + 1);
  });

  it("refuses PAGE UP and PAGE DOWN outside FREE, so that the rate never changes unseen", () => {
    const run = startRun(precisionScene());
    expect([
      commandRun(run, { kind: "rate", step: 1 }, CUT),
      commandRun(run, { kind: "rate", step: -1 }, CUT),
    ]).toEqual([
      { kind: "refused", reason: "not_free" },
      { kind: "refused", reason: "not_free" },
    ]);
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

  it("reads the camera's preset, and in FREE its rate after a middle dot (R07.T19.b)", () => {
    const seat = startRun(precisionScene());
    const free = done(commandRun(seat, { kind: "preset", preset: "free" }, CUT));
    expect([
      cameraReading(seat.camera),
      labelLines(free, DEFAULT_EXPOSURE).find((line) => line.label === "CAMERA")?.value,
    ]).toEqual(["SEAT", "FREE · RATE 1.00 km/s"]);
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

  it("reads the view's style from its camera (R07.T7)", () => {
    const run = startRun(precisionScene());
    const photorealistic = { ...run, camera: { ...run.camera, style: "photorealistic" as const } };
    expect(labelLines(photorealistic, DEFAULT_EXPOSURE)).toContainEqual({
      label: "STYLE",
      value: "PHOTOREALISTIC",
    });
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

  it("adds the terrain annunciation after the other statements while it is shown", () => {
    const run = startRun(unrotated(frameChangeScene()));
    expect(labelStatements(run, "TERRAIN: STREAMING")).toEqual([
      "ROTATION NOT YET MODELLED",
      "TERRAIN: STREAMING",
    ]);
    expect(labelStatements(run, null)).toEqual(["ROTATION NOT YET MODELLED"]);
  });

  it("names the system and galactic frames", () => {
    const { scene } = startRun(precisionScene());
    expect([
      frameName({ kind: "system", system: scene.system }, scene),
      frameName({ kind: "galactic", origin: KEPT_BARYCENTRE }, scene),
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

  it("names each body's kind as the nomenclature does", () => {
    const kept = frameChangeScene();
    const kinds: ReadonlyArray<ViewBody["kind"]> = ["dwarf_planet", "unresolved"];
    const renamed: KeptScene = {
      ...kept,
      sceneAt: (tS) => {
        const scene = kept.sceneAt(tS);
        const bodies = scene.bodies.map((body, index) => ({
          ...body,
          kind: kinds[index % kinds.length] ?? body.kind,
        }));
        return { ...scene, bodies };
      },
    };
    const bodyKinds = markRows(startRun(renamed))
      .filter((row) => row.target.kind === "body")
      .map((row) => row.kind);
    expect(new Set(bodyKinds)).toEqual(new Set(["DWARF PLANET", "UNRESOLVED CONTACT"]));
  });

  it("gives each craft its closure rate on the own ship, and a body none", () => {
    // Each other craft closing on the resting own ship at 3.4 m/s along the line between them.
    const kept = frameChangeScene();
    const closing: KeptScene = {
      ...kept,
      sceneAt: (tS) => {
        const scene = kept.sceneAt(tS);
        const origins = sceneOrigins(scene);
        const own = scene.craft.find((c) => c.id === scene.ownShip);
        if (own === undefined) {
          throw new Error("the frame-change scene has no own ship");
        }
        const craft = scene.craft.map((c) =>
          c.id === own.id
            ? { ...c, velocityMPerS: vec3(0, 0, 0) }
            : {
                ...c,
                velocityMPerS: scale(
                  normalise(differenceM(c.pose.position, own.pose.position, origins)),
                  -3.4,
                ),
              },
        );
        return { ...scene, craft };
      },
    };
    const rows = markRows(startRun(closing));
    const craft = rows.filter((row) => row.target.kind === "craft");
    expect([
      craft.length > 0 &&
        craft.every((row) => row.closure.kind === "known" && row.closure.text === "+3.40 m/s"),
      rows.filter((row) => row.target.kind === "body").every((row) => row.closure.kind === "none"),
    ]).toEqual([true, true]);
  });

  it("shows a closure rate as missing where a velocity is not known", () => {
    // The kept scenes give their craft no velocity.
    const craft = markRows(startRun(frameChangeScene())).filter(
      (row) => row.target.kind === "craft",
    );
    expect(craft.length > 0 && craft.every((row) => row.closure.kind === "unknown")).toBe(true);
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
    expect([
      rangesFromCamera(startRun(shipless).scene),
      markRows(startRun(shipless)).every((row) => row.fromCamera),
      rangesFromCamera(startRun(kept).scene),
    ]).toEqual([true, true, false]);
  });
});

describe("the style", () => {
  const BOTH = { wireframe: true, photorealistic: true };

  it("toggles to the photorealistic style and back where the adapter offers it", () => {
    const run = startRun(phaseScene());
    const photoreal = done(commandRun(run, { kind: "style", style: "toggle" }, CUT, BOTH));
    expect(photoreal.camera.style).toBe("photorealistic");
    expect(photoreal.camera.pose).toEqual(run.camera.pose);
    const back = done(commandRun(photoreal, { kind: "style", style: "toggle" }, CUT, BOTH));
    expect(back.camera.style).toBe("wireframe");
  });

  it("stays in the wireframe on a software adapter", () => {
    const run = startRun(phaseScene());
    const refused = done(
      commandRun(run, { kind: "style", style: "photorealistic" }, CUT, {
        wireframe: true,
        photorealistic: false,
      }),
    );
    expect(refused.camera.style).toBe("wireframe");
  });

  it("states the lighting and the bodies' labels in the photorealistic style only", () => {
    const run = startRun(phaseScene());
    const albedo = ["BODY PHOTOMETRY: NOT YET MODELLED"] as const;
    expect(photorealStatements(run, "hosts-not-received", "wireframe", albedo)).toEqual([]);
    const photoreal = done(commandRun(run, { kind: "style", style: "toggle" }, CUT, BOTH));
    expect(photorealStatements(photoreal, "lit", "photorealistic", albedo)).toEqual(albedo);
    expect(photorealStatements(photoreal, "pending", "photorealistic", albedo)).toEqual([
      "LIGHTING: PENDING",
      ...albedo,
    ]);
    expect(photorealStatements(photoreal, "hosts-not-received", "photorealistic", [])).toEqual([
      "LIGHTING: STAR DISCS NOT RECEIVED",
    ]);
  });

  it("says the photorealistic style is pending while the view still draws its wireframe", () => {
    const photoreal = done(
      commandRun(startRun(phaseScene()), { kind: "style", style: "toggle" }, CUT, BOTH),
    );
    expect(photorealStatements(photoreal, "lit", "wireframe", [])).toEqual([
      "PHOTOREALISTIC: PREPARING",
    ]);
  });
});

describe("an instrument's run (R07.T19)", () => {
  it("opens as a camera at the chase preset of the primary's scene", () => {
    const run = startInstrumentRun(startRun(precisionScene()));
    expect([run.camera.role, run.camera.preset]).toEqual(["camera", "chase"]);
  });

  it("follows the scene its primary drew, at the primary's time", () => {
    const primary = startRun(precisionScene());
    const instrument = startInstrumentRun(primary);
    const stepped = stepRun(primary, { ...STILL, dtS: 0.5 });
    const followed = followRun(instrument, stepped, {
      dtS: 0.5,
      held: new Set(),
      reducedMotion: true,
    });
    expect([followed.scene === stepped.scene, followed.tS]).toEqual([true, stepped.tS]);
  });

  it("moves a free instrument as on a jump when its primary's scene is in another system", () => {
    const primary = startRun(precisionScene());
    const free = done(
      commandRun(startInstrumentRun(primary), { kind: "preset", preset: "free" }, CUT),
    );
    const elsewhere: SystemIdHex = "0200080020000005";
    const moved = followRun(
      free,
      { ...primary, scene: { ...primary.scene, system: elsewhere } },
      { dtS: 0, held: new Set(), reducedMotion: true },
    );
    expect([moved.camera.preset, moved.camera.target]).toEqual(["chase", null]);
  });
});
