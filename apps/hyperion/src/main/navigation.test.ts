import { describe, expect, it } from "vitest";

import { isSafeExternalUrl, isSameDocument } from "./navigation";

describe("isSafeExternalUrl", () => {
  it.each(["https://example.com/docs", "http://example.com"])("allows the web link %s", (url) => {
    expect(isSafeExternalUrl(url)).toBe(true);
  });

  it.each(["file:///etc/passwd", "javascript:alert(1)", "smb://host/share", "not a url", ""])(
    "rejects %s",
    (url) => {
      expect(isSafeExternalUrl(url)).toBe(false);
    },
  );
});

describe("isSameDocument", () => {
  const page = "file:///opt/hyperion/renderer/index.html";

  it("allows a reload of the current page", () => {
    expect(isSameDocument(page, page)).toBe(true);
  });

  it("allows a fragment change", () => {
    expect(isSameDocument(`${page}#helm`, page)).toBe(true);
  });

  it("rejects another local file", () => {
    expect(isSameDocument("file:///etc/passwd", page)).toBe(false);
  });

  it("rejects a remote page", () => {
    expect(isSameDocument("https://example.com/", "http://localhost:5173/")).toBe(false);
  });

  it("rejects an unparseable target", () => {
    expect(isSameDocument("not a url", page)).toBe(false);
  });
});
