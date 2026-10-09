import { describe, expect, it } from "vitest";

import { isOwnPage, pageOf } from "./ipcSender";

const PAGE = "file:///opt/hyperion/out/renderer/index.html";

describe("the IPC sender check", () => {
  it("accepts the page itself, whatever its query and fragment", () => {
    expect(isOwnPage({ url: PAGE }, PAGE)).toBe(true);
    expect(isOwnPage({ url: `${PAGE}?variant=x#y` }, PAGE)).toBe(true);
  });

  it("refuses another page, a gone frame and a subframe at the same URL", () => {
    expect(isOwnPage({ url: "file:///tmp/evil.html" }, PAGE)).toBe(false);
    expect(isOwnPage({ url: "https://example.com/" }, PAGE)).toBe(false);
    expect(isOwnPage(null, PAGE)).toBe(false);
    expect(isOwnPage(undefined, PAGE)).toBe(false);
    const main = { url: PAGE };
    expect(isOwnPage({ url: PAGE }, PAGE, main)).toBe(false);
    expect(isOwnPage(main, PAGE, main)).toBe(true);
  });

  it("strips a query or fragment from a page's URL", () => {
    expect(pageOf(`${PAGE}?a=1`)).toBe(PAGE);
    expect(pageOf(`${PAGE}#top`)).toBe(PAGE);
    expect(pageOf(PAGE)).toBe(PAGE);
  });
});
