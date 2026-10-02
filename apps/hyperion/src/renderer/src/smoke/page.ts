/**
 * The headless smoke harness's page (R01.T9, Design note 17): it acquires the adapter through
 * `platform.ts`, loads the engine through `loadEngine.ts`, runs every check against read-back
 * frames and reports once to the harness's main process, which prints and judges the run.
 *
 * @remarks
 * Its variant comes in the query string: `default` runs the device as the adapter offers it,
 * `no-subgroups` requests it with subgroups withheld (T9.c). A fixture adds a broken shader to the
 * catalogue or makes external requests, either of which must fail the run.
 */

import { WGSL_CATALOGUE } from "../view/engine/catalogue";
import { loadRenderEngine } from "../view/engine/loadEngine";
import {
  type CapabilityOverrides,
  type GpuCapabilities,
  requestAdapterOutcome,
  styleAvailability,
} from "../view/engine/platform";
import { GraphicsStatusStore, initialGraphicsStatus } from "../view/engine/status";
import { checkBlendComputeCube, checkMaterialState, checkSplatRefused } from "./blending";
import { BROKEN_ENTRY, checkCatalogue, makeExternalRequests, type SmokeFixture } from "./catalogue";
import { addCanvas, checkClearAndTriangle, checkDepthCullBias, checkThreeCanvases } from "./frames";
import { Checks } from "./harness";
import { runSoak } from "./soak";
import { checkTwins } from "./twins";
import { checkWireframe } from "./wireframe";
import { checkForcedLoss, checkTargetsAsyncIndirectTiming } from "./work";

/** What the page reports, as `src/smoke/result.ts` reads it. */
interface Report {
  readonly variant: string;
  readonly adapter: string | null;
  readonly capabilities: string | null;
  readonly checks: Checks["list"];
  readonly setupError: string | null;
}

/** The page's variants and the capabilities each withholds. */
const VARIANTS: Readonly<Record<string, CapabilityOverrides | undefined>> = {
  default: undefined,
  "no-subgroups": { withholdSubgroups: true, withholdShaderF16: false },
};

/** Whether `value` is a known fixture. */
function isFixture(value: string): value is SmokeFixture {
  return value === "none" || value === "broken-wgsl" || value === "external-fetch";
}

/** The capability line: the path each feature puts the run on. */
function capabilityLine(capabilities: GpuCapabilities): string {
  return [
    `subgroups ${capabilities.subgroups ? `yes (min ${capabilities.subgroupMinSize ?? "?"})` : "no"}`,
    `shader-f16 ${capabilities.shaderF16 ? "yes" : "no"}`,
    `timestamp-query ${capabilities.timestampQuery ? "yes" : "no"}`,
    `float32-blendable ${capabilities.float32Blendable ? "yes" : "no"}`,
    `rg11b10 ${capabilities.rg11b10Renderable ? "yes" : "no"}`,
  ].join(", ");
}

/** Every device the page's adapters hand out, so that the loss check can destroy the engine's. */
const devices: GPUDevice[] = [];

/** Wraps `navigator.gpu.requestAdapter` so that each adapter's devices are recorded. */
function recordDevices(gpu: GPU): void {
  const request = gpu.requestAdapter.bind(gpu);
  gpu.requestAdapter = async (options?: GPURequestAdapterOptions): Promise<GPUAdapter | null> => {
    const adapter = await request(options);
    if (adapter !== null) {
      const requestDevice = adapter.requestDevice.bind(adapter);
      adapter.requestDevice = async (descriptor?: GPUDeviceDescriptor): Promise<GPUDevice> => {
        const device = await requestDevice(descriptor);
        devices.push(device);
        return device;
      };
    }
    return adapter;
  };
}

async function run(variant: string, fixture: SmokeFixture): Promise<Report> {
  const failed = (setupError: string): Report => ({
    variant,
    adapter: null,
    capabilities: null,
    checks: [],
    setupError,
  });
  const gpu: unknown = Reflect.get(navigator, "gpu");
  if (gpu === undefined) {
    return failed("navigator.gpu is absent: no WebGPU");
  }
  recordDevices(navigator.gpu);
  const outcome = await requestAdapterOutcome(navigator.gpu);
  if (outcome.kind !== "adapter") {
    return failed(`no adapter: ${outcome.kind}`);
  }
  const { summary } = outcome;
  const checks = new Checks();
  const status = new GraphicsStatusStore(initialGraphicsStatus("vulkan", gpuTiming));
  const engine = await loadRenderEngine(
    outcome,
    status,
    VARIANTS[variant] === undefined ? {} : { overrides: VARIANTS[variant] },
  );
  const device = devices.at(-1);
  const capabilities = capabilityLine(engine.capabilities);
  if (fixture === "external-fetch") {
    await makeExternalRequests();
  }

  // T9.c: the adapter summary, and the path the run is on.
  checks.check(
    "T9.c SwiftShader is a fallback adapter",
    summary.architecture !== "swiftshader" || summary.fallback,
    `${summary.vendor}/${summary.architecture}, fallback ${String(summary.fallback)}`,
  );
  checks.check(
    "T9.c the photorealistic style is refused on a fallback adapter",
    !summary.fallback || !styleAvailability(summary).photorealistic,
    `photorealistic ${String(styleAvailability(summary).photorealistic)}`,
  );
  checks.check(
    `T9.c the ${variant} run is on its path`,
    variant !== "no-subgroups" || !engine.capabilities.subgroups,
    capabilities,
  );
  const rounding = status.getSnapshot().targetRounding;
  checks.check(
    "T9.i the rounding probe settles rgba16float",
    rounding.rgba16float !== "unknown",
    `targetRounding ${JSON.stringify(rounding)}`,
  );

  const entries = fixture === "broken-wgsl" ? [...WGSL_CATALOGUE, BROKEN_ENTRY] : WGSL_CATALOGUE;
  await checks.group("T9.b catalogue", () => checkCatalogue(engine, status, entries, checks));
  await checks.group("T9.a clear and triangle", () => checkClearAndTriangle(engine, checks));
  await checks.group("T9.d three canvases", () => checkThreeCanvases(engine, checks));
  await checks.group("T9.f depth, culling and bias", () => checkDepthCullBias(engine, checks));
  await checks.group("T9.g blending, compute and the packed cube", () =>
    checkBlendComputeCube(engine, checks),
  );
  await checks.group("T9.h targets, asynchronous pipelines, indirect work and timing", () =>
    checkTargetsAsyncIndirectTiming(engine, checks),
  );
  await checks.group("T9.i material state and the splat", () => checkMaterialState(engine, checks));

  await checks.group("T10 subgroup twins", () => checkTwins(engine, checks));

  await checks.group("R02.T14.c the wireframe", () => checkWireframe(engine, checks));

  // T9.i's refusal, on a second engine with float32-blendable withheld.
  await checks.group("T9.i splat refused", async () => {
    const second = await requestAdapterOutcome(navigator.gpu);
    if (second.kind !== "adapter") {
      throw new Error(`no second adapter: ${second.kind}`);
    }
    const withheld = await loadRenderEngine(
      second,
      new GraphicsStatusStore(initialGraphicsStatus("vulkan", false)),
      {
        overrides: {
          withholdSubgroups: false,
          withholdShaderF16: false,
          withholdFloat32Blendable: true,
        },
      },
    );
    checkSplatRefused(withheld, checks);
    withheld.dispose();
  });

  // Last, since it rebuilds the engine: T9.f's forced loss.
  await checks.group("T9.f forced loss", async () => {
    if (device === undefined) {
      throw new Error("the engine's device was not recorded");
    }
    await checkForcedLoss(engine, status, device, checks, addCanvas());
  });
  engine.dispose();
  return {
    variant,
    adapter: `${summary.vendor}/${summary.architecture}, fallback ${String(summary.fallback)}`,
    capabilities,
    checks: checks.list,
    setupError: null,
  };
}

/** The bridge's one call, narrowed from the window. */
function smokeReport(): ((result: unknown) => Promise<void>) | null {
  const smoke: unknown = Reflect.get(window, "smoke");
  if (typeof smoke !== "object" || smoke === null) {
    return null;
  }
  const report: unknown = Reflect.get(smoke, "report");
  return typeof report === "function"
    ? (result: unknown) => Promise.resolve(Reflect.apply(report, smoke, [result]))
    : null;
}

const parameters = new URLSearchParams(window.location.search);
const variant = parameters.get("variant") ?? "default";
const fixtureName = parameters.get("fixture") ?? "none";
/** Whether the run lifted timestamp quantization, so that pass times read `full`. */
const gpuTiming = parameters.get("gpuTiming") === "1";
const report = smokeReport();
const fixture: SmokeFixture = isFixture(fixtureName) ? fixtureName : "none";
/** The by-hand soak of T11 and T12 instead of the checks, for `seconds`. */
const soakSeconds = Number(parameters.get("soak") ?? "0");

/** Runs the soak and reports its figures. */
async function soak(): Promise<Report> {
  const checks = new Checks();
  const status = new GraphicsStatusStore(initialGraphicsStatus("vulkan", gpuTiming));
  await runSoak(status, checks, soakSeconds, parameters.get("video"));
  return {
    variant: "soak",
    adapter: null,
    capabilities: null,
    checks: checks.list,
    setupError: null,
  };
}

void (soakSeconds > 0 ? soak() : run(variant, fixture))
  .catch((error: unknown): Report => ({
    variant,
    adapter: null,
    capabilities: null,
    checks: [],
    setupError: `the page threw: ${error instanceof Error ? `${error.message}\n${error.stack ?? ""}` : String(error)}`,
  }))
  .then((result) => report?.(result))
  .catch((error: unknown) => {
    console.error("the smoke page could not report:", error);
  });
