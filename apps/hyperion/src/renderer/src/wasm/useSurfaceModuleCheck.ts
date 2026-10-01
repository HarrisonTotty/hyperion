/**
 * Checks the client's surface module against the server once the server has said its generator
 * version.
 *
 * @remarks
 * Nothing draws terrain yet, so the check has no display: a fault is a diagnostic on the console's
 * error log, and the outcome line is written to the document element's `data-surface-module`
 * attribute, which the by-hand check of the client reads over the DevTools protocol (R04.T10.c) and
 * which R05 replaces with its height workers' own status. The attribute is removed when the check
 * is torn down, so it never outlives the server it was checked against. Where the platform has no
 * `Worker` (the jsdom of the component tests), it does nothing.
 */

import { useEffect } from "react";

import { describeSurfaceModule, loadSurfaceModule } from "./loadSurfaceModule";

/** The `dataset` key the check's outcome line is written to (`data-surface-module`). */
export const SURFACE_MODULE_ATTRIBUTE = "surfaceModule";

/**
 * Loads the surface module in its worker and compares its generator version with
 * `serverGeneratorVersion`, again whenever that changes; `null` (no server yet) checks nothing.
 */
export function useSurfaceModuleCheck(serverGeneratorVersion: number | null): void {
  useEffect(() => {
    if (serverGeneratorVersion === null || typeof Worker !== "function") {
      return undefined;
    }
    const abort = new AbortController();
    const check = async (): Promise<void> => {
      const status = await loadSurfaceModule(serverGeneratorVersion, { signal: abort.signal });
      const line = describeSurfaceModule(status);
      document.documentElement.dataset[SURFACE_MODULE_ATTRIBUTE] = line;
      if (status.kind === "fault") {
        console.error(line);
      }
    };
    void check().catch((error: unknown) => {
      console.error("surface module check failed", error);
    });
    return () => {
      abort.abort();
      delete document.documentElement.dataset[SURFACE_MODULE_ATTRIBUTE];
    };
  }, [serverGeneratorVersion]);
}
