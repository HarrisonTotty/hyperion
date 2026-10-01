/**
 * What the surface worker answers, as a pure function of the loaded module and a request.
 *
 * @remarks
 * The worker (`surface.worker.ts`) holds no logic of its own: it loads the module and hands each
 * message here, so a Node-environment test can drive the same answers from the module's bytes
 * (R04 Design note 16). R05's height workers add their requests beside this one.
 */

/** The part of the surface crate's WebAssembly module that the worker calls. */
export interface SurfaceModule {
  /** The generator version the module computes surfaces for. */
  readonly generatorVersion: () => number;
}

/** A message the render thread sends to the surface worker. */
export interface SurfaceRequest {
  readonly kind: "generator-version";
}

/**
 * The worker's answer to a request, its report that the module did not load, or its report that
 * answering threw.
 */
export type SurfaceReply =
  | { readonly kind: "generator-version"; readonly version: number }
  | { readonly kind: "load-failed"; readonly cause: SurfaceLoadFailure }
  | { readonly kind: "answer-failed"; readonly message: string };

/** Why the module did not load, as the worker saw it. */
export type SurfaceLoadFailure =
  /**
   * The compile was refused by a Content Security Policy: the module was loaded on a thread whose
   * policy refuses WebAssembly, as the page's does (R04.T10.a's ruling keeps it so).
   */
  | { readonly kind: "csp-refused"; readonly message: string }
  /** Anything else: a missing or corrupt module, or a failed fetch. */
  | { readonly kind: "failed"; readonly message: string };

/** Answers `request` from the loaded `module`. There is one request so far. */
export function handleRequest(module: SurfaceModule, request: SurfaceRequest): SurfaceReply {
  return { kind: request.kind, version: module.generatorVersion() };
}

/** Matches V8's message for a WebAssembly compile that a Content Security Policy refused. */
const CSP_REFUSAL = /Content Security Policy|wasm-unsafe-eval/i;

/**
 * Classifies an error thrown while the module loaded.
 *
 * @remarks
 * A `CompileError` that cites a Content Security Policy is its own fault, so that the module run on
 * the wrong thread, or a policy later sent with worker scripts, says what happened instead of
 * reading as a corrupt module (R04.T10.a's ruling).
 */
export function describeLoadFailure(error: unknown): SurfaceLoadFailure {
  const message = error instanceof Error ? error.message : String(error);
  if (error instanceof WebAssembly.CompileError && CSP_REFUSAL.test(message)) {
    return { kind: "csp-refused", message };
  }
  return { kind: "failed", message };
}
