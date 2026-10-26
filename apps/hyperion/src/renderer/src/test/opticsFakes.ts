import {
  type KDistributionTable,
  readKDistribution,
} from "../view/atmosphere/absorbers/kDistribution";
import type { AssembledMedium } from "../view/atmosphere/assemble";
import type { OpticsJobsDeps } from "../view/atmosphere/opticsJobs";
import { toyPart, toySidecar } from "./kDistributionToys";

/**
 * The toy line tables' broadening pressures, Pa: the stored pressures' first span, 4 a decade from
 * 10 Pa to 10 MPa (plan R08, R08.T10.a's optics tests).
 */
export const PRESSURES_PA: ReadonlyArray<number> = Array.from(
  { length: 25 },
  (_, k) => 10 * 10 ** (k / 4),
);

/** The toy line tables' temperatures, K. */
export const TEMPERATURES_K: ReadonlyArray<number> = [150, 200, 250, 300, 350];

/**
 * A toy line species' σ at a node and g-point, m²: a weak point, and a strong core falling as
 * 1 ÷ √p past 1 kPa, each rising with T, strong enough to absorb in an Earth's column.
 */
export function toySigma(temperatureK: number, pressurePa: number, g: number): number {
  const scale = temperatureK / 250;
  return g === 0 ? 2e-30 * scale : (6e-27 * scale) / Math.sqrt(1 + pressurePa / 1e3);
}

/** A toy line table for `species`: cells 690–694 nm, each a weak and a strong g-point. */
export function toyTable(species: string): KDistributionTable {
  const sigmasM2 = TEMPERATURES_K.flatMap((temperatureK) =>
    PRESSURES_PA.flatMap((pressurePa) => [
      toySigma(temperatureK, pressurePa, 0),
      toySigma(temperatureK, pressurePa, 1),
    ]),
  );
  const cells = [690, 691, 692, 693, 694].map((nm) => ({ nm, weights: [0.8, 0.2], sigmasM2 }));
  const sidecar = toySidecar(TEMPERATURES_K, PRESSURES_PA, cells.length, {
    species,
    measuredRangeK: [150, 296],
    broadening: { self: 1.5 },
  });
  const part = toyPart(cells, {
    variants: 1,
    temperatures: TEMPERATURES_K.length,
    pressures: PRESSURES_PA.length,
  });
  return readKDistribution(sidecar, [part], `toy ${species}`);
}

/**
 * An optics worker's k-table reader of toy tables, recording each species it is asked for in
 * `asked`.
 */
export function toyLoader(asked: string[] = []): OpticsJobsDeps {
  return {
    loadKDistributions: async (species) => {
      asked.push(...species);
      return Promise.resolve(species.map((key) => toyTable(key)));
    },
  };
}

/** Resolves after `hops` turns of the microtask queue, so that promise chains settle. */
export async function microtasks(hops = 20): Promise<void> {
  return hops === 0 ? Promise.resolve() : Promise.resolve().then(async () => microtasks(hops - 1));
}

/** Takes nothing: a resolver's or a spy's stand-in. */
export function ignore(): void {}

/** An assembly that draws no medium and says `ATMOSPHERE: NOT YET MODELLED`. */
export const NOT_MODELLED: AssembledMedium = {
  medium: undefined,
  drawn: undefined,
  labels: ["atmosphereNotYetModelled"],
  reasons: [],
  negligible: [],
};
