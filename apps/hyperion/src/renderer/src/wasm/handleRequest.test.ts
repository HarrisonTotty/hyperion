import { describe, expect, it } from "vitest";

import versionSource from "../../../../../../crates/hyperion-base/src/version.rs?raw";
import { generatorVersion, initSync } from "../generated/surface/hyperion_surface";
import wasmDataUrl from "../generated/surface/hyperion_surface_bg.wasm?inline";
import { describeLoadFailure, handleRequest } from "./handleRequest";

/** The `GENERATOR_VERSION` the sim and the server report, read from its one source. */
function sourceGeneratorVersion(): number {
  const match = /GENERATOR_VERSION: GeneratorVersion = GeneratorVersion::new\((\d+)\);/.exec(
    versionSource,
  );
  if (match?.[1] === undefined) {
    throw new Error("crates/hyperion-base/src/version.rs no longer defines GENERATOR_VERSION");
  }
  return Number(match[1]);
}

/** The module's bytes, from the `data:` URL Vite inlines them as. */
function wasmBytes(): Uint8Array {
  const comma = wasmDataUrl.indexOf(",");
  return Uint8Array.from(atob(wasmDataUrl.slice(comma + 1)), (c) => c.charCodeAt(0));
}

/**
 * V8's words for a compile the page's policy refuses, as captured on Electron 44.4.3 by R04.T10.a's
 * experiment (a sandboxed page under the renderer's own meta-tag policy, `file://`), 2026-09-30.
 */
const CSP_REFUSAL =
  "WebAssembly.instantiate(): Compiling or instantiating WebAssembly module violates the following " +
  "Content Security policy directive because 'unsafe-eval' is not an allowed source of script in " +
  "the following Content Security Policy directive: \"script-src 'self'\".";

describe("the surface worker's answers", () => {
  it("reports the module's generator version, which is the sim's", () => {
    initSync({ module: wasmBytes() });
    const reply = handleRequest({ generatorVersion }, { kind: "generator-version" });
    expect(reply).toEqual({ kind: "generator-version", version: sourceGeneratorVersion() });
  });

  it("names a compile refused by a Content Security Policy as its own failure", () => {
    expect(describeLoadFailure(new WebAssembly.CompileError(CSP_REFUSAL))).toEqual({
      kind: "csp-refused",
      message: CSP_REFUSAL,
    });
  });

  it("reports any other load failure as such", () => {
    const corrupt = new WebAssembly.CompileError("expected magic word 00 61 73 6d");
    expect(describeLoadFailure(corrupt)).toEqual({
      kind: "failed",
      message: "expected magic word 00 61 73 6d",
    });
    expect(describeLoadFailure(new TypeError("Failed to fetch"))).toEqual({
      kind: "failed",
      message: "Failed to fetch",
    });
    expect(describeLoadFailure("gone")).toEqual({ kind: "failed", message: "gone" });
  });
});
