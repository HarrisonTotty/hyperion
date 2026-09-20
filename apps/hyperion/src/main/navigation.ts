const EXTERNAL_PROTOCOLS: ReadonlySet<string> = new Set(["https:", "http:"]);

function parseUrl(url: string): URL | undefined {
  return URL.canParse(url) ? new URL(url) : undefined;
}

/**
 * Whether `url` may be handed to the operating system's browser.
 *
 * @remarks
 * `shell.openExternal` can launch arbitrary protocol handlers, so only web links are allowed.
 */
export function isSafeExternalUrl(url: string): boolean {
  const parsed = parseUrl(url);
  return parsed !== undefined && EXTERNAL_PROTOCOLS.has(parsed.protocol);
}

/**
 * Whether navigating from `currentUrl` to `targetUrl` stays on the same document.
 *
 * @remarks
 * The bridge is a single page, so the only legitimate navigations are reloads and fragment
 * changes. Anything else would replace the application with foreign content.
 */
export function isSameDocument(targetUrl: string, currentUrl: string): boolean {
  const target = parseUrl(targetUrl);
  const current = parseUrl(currentUrl);
  if (target === undefined || current === undefined) {
    return false;
  }
  target.hash = "";
  current.hash = "";
  return target.href === current.href;
}
