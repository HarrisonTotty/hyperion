/**
 * Loads the surface crate's WebAssembly module in a worker and checks it against the server.
 *
 * @remarks
 * The render thread never compiles WebAssembly: the page's Content Security Policy refuses it
 * there, and R04.T10.a's ruling keeps it that way. The module is compiled in a same-origin module
 * worker, which has no policy of its own under `file://` or the dev server (R04 Design note 16).
 * This first module's one job is to report its generator version, which is compared with the
 * server's: a stale `just gen-surface` build would otherwise draw terrain the server does not
 * collide with, once there is terrain, so a mismatch is a fault of the client's module. R05's
 * height workers extend this loader.
 */

import type { SurfaceLoadFailure, SurfaceReply, SurfaceRequest } from "./handleRequest";

/** Why the client's surface module cannot be used. */
export type SurfaceModuleFault =
  /** The module failed to load in its worker; a `csp-refused` cause names a refusing policy. */
  | { readonly kind: "load-failed"; readonly cause: SurfaceLoadFailure }
  /** The worker itself failed: its script did not load, it threw, or its answer threw. */
  | { readonly kind: "worker-failed"; readonly message: string }
  /** The worker gave no answer within the time allowed. */
  | { readonly kind: "no-answer"; readonly afterMs: number }
  /** The module computes surfaces for another generator version than the server's. */
  | { readonly kind: "version-mismatch"; readonly module: number; readonly server: number };

/** The client's surface module, checked against the server. */
export type SurfaceModuleStatus =
  | { readonly kind: "ready"; readonly generatorVersion: number }
  | { readonly kind: "fault"; readonly fault: SurfaceModuleFault };

/**
 * The part of a `Worker` the loader uses, so that tests can stand in for one.
 *
 * @remarks
 * A message's data is a structured clone of a `SurfaceReply` posted by our own same-origin worker
 * script, so its type is trusted rather than checked. An `error` event is an `ErrorEvent` when the
 * script threw, and a plain `Event` when a module worker's script failed to load.
 */
export interface SurfaceWorker {
  addEventListener(type: "message", listener: (event: MessageEvent<SurfaceReply>) => void): void;
  addEventListener(type: "error", listener: (event: Event) => void): void;
  postMessage(message: SurfaceRequest, transfer: Transferable[]): void;
  terminate(): void;
}

/** Creates the surface worker, as a same-origin module file (the policy refuses a `blob:` one). */
export function createSurfaceWorker(): SurfaceWorker {
  return new Worker(new URL("./surface.worker.ts", import.meta.url), {
    type: "module",
    name: "surface",
  });
}

/** How long the worker may take to load the module and answer, in milliseconds. */
export const SURFACE_ANSWER_TIMEOUT_MS = 10_000;

/** What a load takes besides the server's version. */
export interface LoadSurfaceModuleOptions {
  readonly createWorker?: () => SurfaceWorker;
  readonly timeoutMs?: number;
  readonly signal?: AbortSignal;
}

/**
 * Compares the module's generator version with the server's.
 *
 * @param moduleVersion - What the module reported.
 * @param serverVersion - The server's `generator_version`, from its welcome.
 */
export function checkGeneratorVersion(
  moduleVersion: number,
  serverVersion: number,
): SurfaceModuleStatus {
  return moduleVersion === serverVersion
    ? { kind: "ready", generatorVersion: moduleVersion }
    : {
        kind: "fault",
        fault: { kind: "version-mismatch", module: moduleVersion, server: serverVersion },
      };
}

/** The status the worker's `reply` gives, against the server's version. */
function statusOf(reply: SurfaceReply, serverVersion: number): SurfaceModuleStatus {
  let status: SurfaceModuleStatus;
  switch (reply.kind) {
    case "generator-version":
      status = checkGeneratorVersion(reply.version, serverVersion);
      break;
    case "load-failed":
      status = { kind: "fault", fault: { kind: "load-failed", cause: reply.cause } };
      break;
    case "answer-failed":
      status = { kind: "fault", fault: { kind: "worker-failed", message: reply.message } };
      break;
  }
  return status;
}

/**
 * Starts a surface worker, asks for the module's generator version, and checks it against the
 * server's. The worker is terminated once it has answered, failed or been given up on, or when
 * `signal` aborts, in which case the promise never settles.
 */
export function loadSurfaceModule(
  serverVersion: number,
  options: LoadSurfaceModuleOptions = {},
): Promise<SurfaceModuleStatus> {
  const timeoutMs = options.timeoutMs ?? SURFACE_ANSWER_TIMEOUT_MS;
  const signal = options.signal;
  return new Promise((resolve) => {
    if (signal?.aborted === true) {
      return;
    }
    const worker = (options.createWorker ?? createSurfaceWorker)();
    let settled = false;
    const finish = (status: SurfaceModuleStatus | null): void => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      signal?.removeEventListener("abort", abort);
      worker.terminate();
      if (status !== null) {
        resolve(status);
      }
    };
    const abort = (): void => {
      finish(null);
    };
    const timer = setTimeout(() => {
      finish({ kind: "fault", fault: { kind: "no-answer", afterMs: timeoutMs } });
    }, timeoutMs);
    signal?.addEventListener("abort", abort);
    worker.addEventListener("message", (event) => {
      finish(statusOf(event.data, serverVersion));
    });
    worker.addEventListener("error", (event) => {
      const message = event instanceof ErrorEvent ? event.message : "its script did not load";
      finish({ kind: "fault", fault: { kind: "worker-failed", message } });
    });
    worker.postMessage({ kind: "generator-version" }, []);
  });
}

/** One line describing `status`, for the console's diagnostics. */
export function describeSurfaceModule(status: SurfaceModuleStatus): string {
  let line: string;
  switch (status.kind) {
    case "ready":
      line = `surface module ready: generator version ${status.generatorVersion}`;
      break;
    case "fault":
      line = `surface module fault: ${describeFault(status.fault)}`;
      break;
  }
  return line;
}

function describeFault(fault: SurfaceModuleFault): string {
  let words: string;
  switch (fault.kind) {
    case "load-failed":
      words =
        fault.cause.kind === "csp-refused"
          ? `a Content Security Policy refused the compile (${fault.cause.message})`
          : `the module did not load (${fault.cause.message})`;
      break;
    case "worker-failed":
      words = `the worker failed (${fault.message})`;
      break;
    case "no-answer":
      words = `the worker gave no answer in ${fault.afterMs} ms`;
      break;
    case "version-mismatch":
      words =
        `the module is for generator version ${fault.module}, the server's is ` +
        `${fault.server}: run just gen-surface`;
      break;
  }
  return words;
}
