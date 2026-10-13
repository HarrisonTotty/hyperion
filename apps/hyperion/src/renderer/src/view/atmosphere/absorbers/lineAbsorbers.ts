/**
 * The line absorbers' tables `absorbers.ts` takes when its caller gives none (plan R08, R08.T4.d).
 */

import type { KDistributionTable } from "./kDistribution";

/**
 * The k-distribution tables `absorberSpecs` takes by default: none. The committed tables are
 * imported per species on demand ({@link import("./kDistribution").loadKDistributions}, the
 * species {@link import("./kDistribution").committedLineSpecies} lists), by the optics worker,
 * never by the render thread's bundle (`decision-r08-t4d-kdist.md` §4), which passes them to
 * `absorberSpecs`.
 */
export const ABSORBER_K_DISTRIBUTIONS: ReadonlyArray<KDistributionTable> = [];
