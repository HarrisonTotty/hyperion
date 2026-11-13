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

/**
 * A page's URL without its query or fragment, which a reload or the smoke page's options vary, in
 * the WHATWG URL parser's form: so the development server's `http://localhost:5173`, as
 * electron-vite gives it, and the `http://localhost:5173/` its frame reports are one page (plan R11,
 * R11.T10.c's security review). A URL the parser refuses is cut at its query as written.
 */
export function pageOf(url: string): string {
  let parsed: URL | null;
  try {
    parsed = new URL(url);
  } catch {
    parsed = null;
  }
  if (parsed === null) {
    const cut = url.search(/[?#]/);
    return cut < 0 ? url : url.slice(0, cut);
  }
  parsed.search = "";
  parsed.hash = "";
  return parsed.href;
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

/**
 * Whether `frame` is the main frame of the window it came from, at the page `pageUrl`: the check of
 * a handler every window of the app shares, such as the stills' (plan R11, R11.T10.c), which takes
 * the window from the event's sender.
 *
 * @param frame - `event.senderFrame`, `null` once the frame has gone.
 * @param owner - The main frame of the window whose page sent the event, or `null` where no window
 *   owns its sender (`BrowserWindow.fromWebContents` found none).
 */
export function isWindowsOwnPage(
  frame: SenderFrame | null | undefined,
  pageUrl: string,
  owner: SenderFrame | null,
): boolean {
  return owner !== null && isOwnPage(frame, pageUrl, owner);
}
