import { vi } from "vitest";

import { REDUCED_MOTION_QUERY } from "../lib/usePrefersReducedMotion";

/** A stubbed `prefers-reduced-motion` setting that a test can change. */
export interface ReducedMotionStub {
  /** Changes the setting and tells every listener, as the browser does. */
  set(reduced: boolean): void;
}

function isReducedMotion(query: string): boolean {
  return query === REDUCED_MOTION_QUERY;
}

/**
 * Replaces `window.matchMedia`, which jsdom lacks, for the rest of the test: the query
 * {@link REDUCED_MOTION_QUERY} matches while `reduced` holds, and nothing else matches.
 *
 * @remarks
 * `test/setup.ts` stubs it with `reduced` false before every test; a test that needs reduced
 * motion calls it again. Every list the stub hands out shares one set of `change` listeners per
 * query. The global is restored by `unstubGlobals`.
 */
export function stubMatchMedia(reduced: boolean): ReducedMotionStub {
  let current = reduced;
  const listeners = new Set<(event: MediaQueryListEvent) => void>();
  vi.stubGlobal("matchMedia", (query: string): MediaQueryList => {
    const list = {
      media: query,
      get matches(): boolean {
        return isReducedMotion(query) && current;
      },
      onchange: null,
      addEventListener: (type: string, listener: (event: MediaQueryListEvent) => void): void => {
        if (type === "change" && isReducedMotion(query)) {
          listeners.add(listener);
        }
      },
      removeEventListener: (type: string, listener: (event: MediaQueryListEvent) => void): void => {
        if (type === "change") {
          listeners.delete(listener);
        }
      },
      addListener: (): void => undefined,
      removeListener: (): void => undefined,
      dispatchEvent: (): boolean => true,
    };
    // A stand-in for the browser's list at the boundary jsdom leaves out; it has every member the
    // code under test reads.
    // oxlint-disable-next-line typescript/no-unsafe-type-assertion
    return list as unknown as MediaQueryList;
  });
  return {
    set(next) {
      current = next;
      const event = Object.assign(new Event("change"), {
        matches: next,
        media: REDUCED_MOTION_QUERY,
      });
      for (const listener of listeners) {
        listener(event);
      }
    },
  };
}
