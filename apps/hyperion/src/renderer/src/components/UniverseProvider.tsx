import type { ReactNode } from "react";

import { UniverseContext, useUniverseSession } from "../lib/universe";

interface UniverseProviderProps {
  readonly children: ReactNode;
}

/**
 * Runs the universe session and hands it to every display through `UniverseContext`.
 *
 * @remarks
 * Sits below the server link's provider, whose requests it makes.
 */
export function UniverseProvider({ children }: UniverseProviderProps) {
  const session = useUniverseSession();
  return <UniverseContext value={session}>{children}</UniverseContext>;
}
