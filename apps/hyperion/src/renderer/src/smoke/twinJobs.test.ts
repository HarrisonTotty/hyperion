import { describe, expect, it } from "vitest";

import { lineAbsorberMedium } from "../test/atmosphereFixtures";
import { quaternion } from "../view/camera/quaternion";
import { R05_EARTH_REFERENCE } from "../view/atmosphere/earth";
import { type AtmosphereMedium, tabulatedDensity } from "../view/atmosphere/medium";
import { kernelMedium, NO_ABSORBER_CURVES } from "../view/atmosphere/tables";
import {
  multiScatteringTwin,
  type PlanetTwin,
  type RayMarchSettings,
  rayMarchTwin,
  skyViewTwin,
  type TwinTable,
  type TwinTransmittance,
  twinView,
} from "../view/atmosphere/tablesCpu";
import { replyTo, runTwinJob, type TwinJob } from "./twinJobs";

const RADIUS_M = 6_371_000;

/** A table of `width` × `height` texels whose values vary smoothly from `scale` up. */
function table(width: number, height: number, scale: number): TwinTable {
  const texels = Float64Array.from(
    { length: width * height * 4 },
    (_, i) => scale * (1 + 0.1 * (i % 7) + 0.01 * Math.floor(i / 4)),
  );
  return { widthTexels: width, heightTexels: height, texels };
}

/**
 * A small planet over R05's Earth, its tables made up rather than twinned, so that a per-frame
 * twin over it is quick: what the job's crossing is tested on is the arithmetic, not the tables.
 */
const PLANET: PlanetTwin = {
  medium: R05_EARTH_REFERENCE,
  bottomRadiusM: RADIUS_M,
  transmittance: table(16, 8, 0.05),
  absorberColumns: { ...table(16, 8, 0), texels: new Float64Array(16 * 8 * 4) },
  absorberPressureColumns: { ...table(16, 8, 0), texels: new Float64Array(16 * 8 * 4) },
  lastAbsorberPressureColumn: { ...table(1, 1, 0), texels: new Float64Array(4) },
  curves: NO_ABSORBER_CURVES,
  multiScattering: table(8, 8, 1e-3),
};

const VIEW = twinView(
  {
    positionM: { x: RADIUS_M + 2_000, y: 0, z: 0 },
    orientation: quaternion(0, Math.SQRT1_2, Math.SQRT1_2, 0),
    fovXRad: 2 * Math.atan(2),
    viewport: { widthPx: 8, heightPx: 4 },
  },
  {
    directionBodyFixed: { x: 0.5, y: 0, z: Math.sqrt(0.75) },
    illuminanceLx: [1, 1, 1],
    angularRadiusRad: 0.005,
  },
  { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M },
  0.1,
);

/** Whether two tables are the same, texel for texel and bit for bit, and hold some light. */
function identical(a: TwinTable, b: TwinTable): boolean {
  return (
    b.texels.some((value) => Number.isFinite(value) && value > 0) &&
    a.widthTexels === b.widthTexels &&
    a.heightTexels === b.heightTexels &&
    a.texels.length === b.texels.length &&
    a.texels.every((value, i) => Object.is(value, b.texels[i]))
  );
}

describe("the twins as jobs for the twin workers", () => {
  it("answers a sky-view job, cloned as a worker receives it, with the twin bit for bit", () => {
    const job: TwinJob = {
      kind: "skyView",
      planet: PLANET,
      view: VIEW,
      size: { widthTexels: 8, heightTexels: 4 },
      samples: 8,
    };
    const answer = runTwinJob(structuredClone(job));
    const direct = skyViewTwin(PLANET, VIEW, { widthTexels: 8, heightTexels: 4 }, 8);
    expect(answer.kind).toBe("skyView");
    expect(answer.kind === "skyView" && identical(answer.twin, direct)).toBe(true);
  });

  it("answers a ray-march job over one depth at every pixel with the twin bit for bit", () => {
    const settings: RayMarchSettings = {
      rayMarchScale: 1,
      rayMarchSamples: 4,
      aerialPerspectiveReachM: 32_000,
      marchBand: { lowerShareOfTop: 0.3, upperShareOfTop: 0.5 },
    };
    const depth = 0.1 / 60_000;
    const answer = runTwinJob(
      structuredClone({ kind: "rayMarch", planet: PLANET, view: VIEW, settings, depth }),
    );
    const direct = rayMarchTwin(PLANET, VIEW, settings, () => depth);
    expect(answer.kind === "rayMarch" && identical(answer.twin, direct)).toBe(true);
  });

  it("carries a line absorber's medium, curves and pressure columns to a worker intact", () => {
    // R08.T4.d's medium crosses as the smoke's R08.T6.b group sends it: its curves' pressure rows,
    // its line species' k-distributions and its pressure profile, with the pressure-column tables.
    const { medium, curves } = lineAbsorberMedium();
    const read = kernelMedium(medium);
    const transmittance: TwinTransmittance = {
      transmittance: table(16, 8, 0.05),
      absorberColumns: table(16, 8, 0.01),
      absorberPressureColumns: table(16, 8, 0.005),
      lastAbsorberPressureColumn: { ...table(1, 1, 0), texels: new Float64Array(4) },
      curves,
    };
    const job: TwinJob = {
      kind: "multiScattering",
      medium: read,
      bottomRadiusM: RADIUS_M,
      transmittance,
      curveSlot: 0,
    };
    const cloned = structuredClone(job);
    expect(cloned).toStrictEqual(job);
    // One texel of the twin over the clone, as a worker forms it, against the page's own.
    const texel = [[16, 16]] as const;
    const across = multiScatteringTwin(
      cloned.medium,
      RADIUS_M,
      cloned.transmittance,
      undefined,
      texel,
    );
    const direct = multiScatteringTwin(read, RADIUS_M, transmittance, undefined, texel);
    expect(identical(across, direct)).toBe(true);
  });

  it("replies with the name and message of what the twin throws", () => {
    // A tabulated term that ends below the top at a density other than 0, which the twin refuses.
    const [gas, ...others] = R05_EARTH_REFERENCE.terms;
    if (gas === undefined) {
      throw new Error("R05's Earth has its gas");
    }
    const refused: AtmosphereMedium = {
      ...R05_EARTH_REFERENCE,
      name: "refused",
      terms: [
        {
          ...gas,
          density: tabulatedDensity(Float64Array.of(0, 1_000), Float64Array.of(1, 1)),
        },
        ...others,
      ],
    };
    const reply = replyTo({
      id: 4,
      job: { kind: "transmittance", medium: refused, bottomRadiusM: RADIUS_M },
    });
    expect(reply).toMatchObject({ kind: "error", id: 4, name: "RangeError" });
  });
});
