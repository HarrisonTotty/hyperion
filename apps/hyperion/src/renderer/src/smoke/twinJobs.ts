/**
 * The atmosphere's CPU twins as jobs for the smoke page's twin workers (`twinWorker.ts`): each job
 * names one of `tablesCpu.ts`'s twins and carries its arguments as data, so that it crosses to a
 * worker and back by structured clone.
 *
 * @remarks
 * A job's answer is the twin's own, bit for bit: structured clone copies every number and typed
 * array exactly, and a worker runs the same module's arithmetic as the page would. The smoke's
 * twins ran on the page's thread, one after the GPU's work they compare, and with the GPU's they
 * passed the harness's 180 s watchdog (perf-smoke, 2026-10-13); on the workers they run beside it.
 * Each job takes the twin's defaults where the smoke's callers take them.
 */

import type { VolumeSize } from "../view/atmosphere/hillaire";
import type { AtmosphereMedium } from "../view/atmosphere/medium";
import type { AbsorberCurveSlots, TableSize } from "../view/atmosphere/tables";
import {
  aerialPerspectiveTwin,
  MULTI_SCATTERING_KERNEL_OPTIONS,
  multiScatteringTwin,
  type PlanetTwin,
  planetTwin,
  type RayMarchSettings,
  rayMarchTwin,
  skyViewTwin,
  type TwinTable,
  type TwinTransmittance,
  type TwinView,
  type TwinVolume,
  transmittanceTablesTwin,
  transmittanceTwin,
} from "../view/atmosphere/tablesCpu";

/** One twin to compute, `tablesCpu.ts`'s function of the same name with its arguments. */
export type TwinJob =
  | {
      /** `planetTwin`: both per-planet tables of the medium, with no GPU table read. */
      readonly kind: "planet";
      readonly medium: AtmosphereMedium;
      readonly bottomRadiusM: number;
      readonly curves: AbsorberCurveSlots;
    }
  | {
      /** `transmittanceTablesTwin`: the transmittance and absorber-column tables. */
      readonly kind: "transmittanceTables";
      readonly medium: AtmosphereMedium;
      readonly bottomRadiusM: number;
      readonly curves: AbsorberCurveSlots;
    }
  | {
      /** `transmittanceTwin`: the transmittance table alone. */
      readonly kind: "transmittance";
      readonly medium: AtmosphereMedium;
      readonly bottomRadiusM: number;
    }
  | {
      /**
       * `multiScatteringTwin` over the transmittance tables given, the GPU's or the twin's, its
       * layer of light slot `curveSlot` (R08.T7).
       */
      readonly kind: "multiScattering";
      readonly medium: AtmosphereMedium;
      readonly bottomRadiusM: number;
      readonly transmittance: TwinTransmittance;
      readonly curveSlot: number;
    }
  | {
      readonly kind: "skyView";
      readonly planet: PlanetTwin;
      readonly view: TwinView;
      readonly size: TableSize;
      readonly samples: number;
    }
  | {
      readonly kind: "aerialPerspective";
      readonly planet: PlanetTwin;
      readonly view: TwinView;
      readonly size: VolumeSize;
      readonly samplesPerSlice: number;
      readonly reachM: number;
    }
  | {
      /** `rayMarchTwin` over a scene whose depth is `depth` at every pixel (0 for none drawn). */
      readonly kind: "rayMarch";
      readonly planet: PlanetTwin;
      readonly view: TwinView;
      readonly settings: RayMarchSettings;
      readonly depth: number;
    };

/** A job's answer, tagged with the job's kind. */
export type TwinAnswer =
  | { readonly kind: "planet"; readonly twin: PlanetTwin }
  | { readonly kind: "transmittanceTables"; readonly twin: TwinTransmittance }
  | { readonly kind: "transmittance"; readonly twin: TwinTable }
  | { readonly kind: "multiScattering"; readonly twin: TwinTable }
  | { readonly kind: "skyView"; readonly twin: TwinTable }
  | { readonly kind: "aerialPerspective"; readonly twin: TwinVolume }
  | { readonly kind: "rayMarch"; readonly twin: TwinTable };

/** What the page posts to a twin worker: a job and the number its reply is matched by. */
export interface TwinRequest {
  readonly id: number;
  readonly job: TwinJob;
}

/** What a twin worker posts back: the job's answer, or what the twin threw. */
export type TwinReply =
  | { readonly kind: "answer"; readonly id: number; readonly answer: TwinAnswer }
  | {
      readonly kind: "error";
      readonly id: number;
      readonly name: string;
      readonly message: string;
    };

/**
 * Computes one job's twin.
 *
 * @throws What the twin throws: a `RangeError` for a medium it refuses (`tablesCpu.ts`).
 */
export function runTwinJob(job: TwinJob): TwinAnswer {
  let answer: TwinAnswer;
  switch (job.kind) {
    case "planet":
      answer = { kind: job.kind, twin: planetTwin(job.medium, job.bottomRadiusM, job.curves) };
      break;
    case "transmittanceTables":
      answer = {
        kind: job.kind,
        twin: transmittanceTablesTwin(job.medium, job.bottomRadiusM, job.curves),
      };
      break;
    case "transmittance":
      answer = { kind: job.kind, twin: transmittanceTwin(job.medium, job.bottomRadiusM) };
      break;
    case "multiScattering":
      answer = {
        kind: job.kind,
        twin: multiScatteringTwin(
          job.medium,
          job.bottomRadiusM,
          job.transmittance,
          MULTI_SCATTERING_KERNEL_OPTIONS,
          undefined,
          job.curveSlot,
        ),
      };
      break;
    case "skyView":
      answer = { kind: job.kind, twin: skyViewTwin(job.planet, job.view, job.size, job.samples) };
      break;
    case "aerialPerspective":
      answer = {
        kind: job.kind,
        twin: aerialPerspectiveTwin(
          job.planet,
          job.view,
          job.size,
          job.samplesPerSlice,
          job.reachM,
        ),
      };
      break;
    case "rayMarch": {
      const { depth } = job;
      answer = {
        kind: job.kind,
        twin: rayMarchTwin(job.planet, job.view, job.settings, () => depth),
      };
      break;
    }
  }
  return answer;
}

/** Answers one request, a twin's throw included, as the worker posts it. */
export function replyTo(request: TwinRequest): TwinReply {
  try {
    return { kind: "answer", id: request.id, answer: runTwinJob(request.job) };
  } catch (error: unknown) {
    return error instanceof Error
      ? { kind: "error", id: request.id, name: error.name, message: error.message }
      : { kind: "error", id: request.id, name: "Error", message: String(error) };
  }
}
