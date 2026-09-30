import type { ReactNode } from "react";

import { useServerConnection } from "../lib/connection";
import { ServerLinkContext, useServerLinkValue } from "../lib/serverLink";

interface ServerLinkHarnessProps {
  readonly children: ReactNode;
}

/**
 * Provides the server link to `children` as `App` does, over a socket that tests stub with
 * `FakeWebSocket`.
 *
 * @remarks
 * `children` are created by the test, not by the harness, so a pong re-renders the harness alone
 * and a consumer re-renders only when the provided value changes.
 */
export function ServerLinkHarness({ children }: ServerLinkHarnessProps) {
  const connection = useServerConnection("ws://ship/ws", "1.2.3");
  const link = useServerLinkValue(connection);
  return <ServerLinkContext value={link}>{children}</ServerLinkContext>;
}
