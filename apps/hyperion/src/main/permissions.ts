/**
 * The client's answer to the page's permission requests: always no.
 *
 * @remarks
 * `.claude/rules/typescript-dev.md` (Electron security) denies permission requests by default.
 * The bridge needs none (no camera, microphone, notifications, geolocation or MIDI), so every
 * request, whatever its kind and from whichever page, is refused, as the smoke harness's are.
 */

import type { Session } from "electron";

/** A permission-request handler that refuses whatever is asked. */
export function refusePermission(
  _contents: unknown,
  _permission: unknown,
  decide: (granted: boolean) => void,
): void {
  decide(false);
}

/** A permission-check handler that answers no, whatever is checked. */
export function refusePermissionCheck(): boolean {
  return false;
}

/**
 * Makes `session` refuse every permission a page requests, and answer no to every check, which
 * Electron otherwise grants (`navigator.permissions.query` would read `granted`).
 *
 * @remarks
 * Set once, before the first window is created, on `session.defaultSession`.
 */
export function denyPermissionRequests(
  session: Pick<Session, "setPermissionRequestHandler" | "setPermissionCheckHandler">,
): void {
  session.setPermissionRequestHandler(refusePermission);
  session.setPermissionCheckHandler(refusePermissionCheck);
}
