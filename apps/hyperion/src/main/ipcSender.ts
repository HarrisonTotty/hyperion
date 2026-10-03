/**
 * The check every `ipcMain` handler makes of its sender (the TypeScript rules' Electron security):
 * that the message comes from the window's own page, not a frame inside it or another page.
 *
 * @remarks
 * Factored out of the smoke harness's inline check (`src/smoke/main.ts`) for the descent spike's
 * handlers (plan R05, T13.c), which are the client's first.
 */

/** The parts of an IPC event's sending frame the check reads. */
export interface SenderFrame {
  readonly url: string;
}

/** A page's URL without its query or fragment, which a reload or the smoke page's options vary. */
export function pageOf(url: string): string {
  const cut = url.search(/[?#]/);
  return cut < 0 ? url : url.slice(0, cut);
}

/**
 * Whether `frame` is the page at `pageUrl` and, where `mainFrame` is given, the window's own main
 * frame rather than a subframe at the same URL.
 *
 * @param frame - `event.senderFrame`, which Electron gives as `null` once the frame has gone.
 * @param pageUrl - The page's URL, as loaded; its query and fragment are ignored.
 * @param mainFrame - The window's `webContents.mainFrame`.
 */
export function isOwnPage(
  frame: SenderFrame | null | undefined,
  pageUrl: string,
  mainFrame?: SenderFrame,
): boolean {
  if (frame === null || frame === undefined) {
    return false;
  }
  if (mainFrame !== undefined && frame !== mainFrame) {
    return false;
  }
  return pageOf(frame.url) === pageOf(pageUrl);
}
