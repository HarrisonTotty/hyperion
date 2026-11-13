import { describe, expect, it } from "vitest";

import { isOwnPage, isWindowsOwnPage, pageOf } from "./ipcSender";

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

  it("takes the development server's URL for the page its frame reports (R11.T10.c)", () => {
    // electron-vite gives the server without its path's slash; Chromium reports the frame with it.
    expect(isOwnPage({ url: "http://localhost:5173/" }, "http://localhost:5173")).toBe(true);
    expect(isOwnPage({ url: "http://localhost:5173/?a=1" }, "http://localhost:5173")).toBe(true);
    expect(isOwnPage({ url: "http://localhost:5174/" }, "http://localhost:5173")).toBe(false);
  });

  it("keeps a URL the parser refuses as written, cut at its query", () => {
    expect(pageOf("not a url?x")).toBe("not a url");
  });
});

describe("the check a window's shared handler makes (R11.T10.c)", () => {
  it("accepts the main frame of the window that sent it, at its page", () => {
    const main = { url: PAGE };
    expect(isWindowsOwnPage(main, PAGE, main)).toBe(true);
  });

  it("refuses a sender no window owns, a gone frame, a subframe and another page", () => {
    const main = { url: PAGE };
    const other = { url: "file:///tmp/evil.html" };
    expect([
      isWindowsOwnPage(main, PAGE, null),
      isWindowsOwnPage(null, PAGE, main),
      isWindowsOwnPage({ url: PAGE }, PAGE, main),
      isWindowsOwnPage(other, PAGE, other),
    ]).toEqual([false, false, false, false]);
  });
});
