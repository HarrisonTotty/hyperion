/**
 * The Babylon.js implementation of the engine-agnostic interface, the one module that
 * `loadEngine.ts` imports dynamically, and so the root of the lazily loaded `babylon` chunk.
 *
 * @remarks
 * `@babylonjs/core` is pinned exactly, at 9.28.0 with no range (R01 Design note 15). The case for
 * Babylon is that its visual changes are logged with a flag that restores the old look, so an
 * upgrade is a deliberate act: read the breaking-changes log, set any restoring flag, and run
 * `just test-render`. The pin's reason lives here because `package.json` holds no comments.
 *
 * Imports go to Babylon's `.pure` modules and the side-effect registrations they need, since
 * `Engines/webgpuEngine.js` drags in the audio engine and loaders (Design note 14).
 *
 * Until R01.T8 builds the engine, `createBabylonEngine` refuses, naming the task.
 */

import { WebGPUEngine } from "@babylonjs/core/Engines/webgpuEngine.pure";

import type { CreateBabylonEngine } from "../types";

/**
 * Creates the engine on the vetted adapter.
 *
 * @throws Error always, until R01.T8.
 */
export const createBabylonEngine: CreateBabylonEngine = () =>
  Promise.reject(new Error(`${WebGPUEngine.name} is wrapped by R01.T8, which is not built yet`));
