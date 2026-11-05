/**
 * The frame profile's React commit hook (`--inspection --timing`; lane perf-render-thread-2), set on
 * the page's global on that launch alone, and before `react-dom` loads, which takes it then or
 * never: `main.tsx` imports this module first.
 */

import { installReactCommitHook } from "./displays/view/check/reactCommits";

if (window.hyperion.viewsCheck?.launch.timing === true) {
  installReactCommitHook(globalThis);
}
