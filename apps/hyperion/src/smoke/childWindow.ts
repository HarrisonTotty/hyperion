/**
 * The main process's side of R07.T21, the child window on a second monitor: which display the
 * child goes on, the refusal when there is none, the one window the page may open and the record
 * the run writes.
 *
 * @remarks
 * R01.T13's prototype was a scratch branch that no longer exists, so this harness rebuilds it in
 * the smoke harness, outside the client, whose every window stays denied: an opener whose full
 * window canvas stands for the main view, and a same-origin `window.open` child on another display
 * whose canvas is a view of the opener's engine, drawn from the opener's animation frames. The page
 * keeps R01.T13's rule (it drops the child's view on the child's `pagehide`) and records both
 * windows' frame intervals, each view's GPU time and what follows the child's closing. A hidden
 * variant (`--smoke-child-hidden=1`) puts an offscreen child on the same display, to prove the
 * harness on a machine with one display; its pacing is offscreen rendering's, not a display's.
 */

import { hostname } from "node:os";

import type { MachineDescription } from "../main/results";

/** The child's `window.open` target name, the one the opener's window-open handler allows. */
export const CHILD_FRAME_NAME = "hyperion-t21-child";

/** The child's content size, DIP: an instrument's 4:3, larger than a slot's. */
export const CHILD_SIZE = { width: 640, height: 480 } as const;

/** A display as the harness reads it from Electron's `screen`. */
export interface DisplayInfo {
  readonly id: number;
  readonly label: string;
  readonly bounds: {
    readonly x: number;
    readonly y: number;
    readonly width: number;
    readonly height: number;
  };
  readonly workArea: {
    readonly x: number;
    readonly y: number;
    readonly width: number;
    readonly height: number;
  };
  /** Its refresh rate, Hz (`Display.displayFrequency`); 0 when unknown. */
  readonly displayFrequency: number;
  readonly scaleFactor: number;
}

/** A display in one line: its ID, label, size and rate. */
export function displayLine(display: DisplayInfo): string {
  const { width, height, x, y } = display.bounds;
  const label = display.label.length > 0 ? ` ${display.label}` : "";
  return `display ${String(display.id)}${label} ${String(width)} × ${String(height)} at (${String(x)}, ${String(y)}), ${display.displayFrequency.toFixed(2)} Hz, scale ${String(display.scaleFactor)}`;
}

/** The display the child goes on: the first that is not `mainId`'s, or `null` with one display. */
export function childDisplayOf(
  displays: ReadonlyArray<DisplayInfo>,
  mainId: number,
): DisplayInfo | null {
  return displays.find((display) => display.id !== mainId) ?? null;
}

/** The refusal printed when the run needs a second display and Electron sees none. */
export function refusalLine(displays: ReadonlyArray<DisplayInfo>): string {
  return `SETUP R07.T21 needs a second display, and Electron sees ${String(displays.length)}: ${displays.map(displayLine).join("; ")}. Connect a second display (or extend the desktop onto one) and run again; no window was opened.`;
}

/** What `setWindowOpenHandler` is given, as far as the harness reads it. */
export interface OpenRequest {
  readonly url: string;
  readonly frameName: string;
}

/** The child window's options, as `overrideBrowserWindowOptions` takes them. */
export interface ChildWindowOptions {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly useContentSize: true;
  readonly show: boolean;
  readonly title: string;
  readonly webPreferences: {
    readonly sandbox: true;
    readonly contextIsolation: true;
    readonly nodeIntegration: false;
    readonly offscreen: boolean;
    readonly backgroundThrottling: false;
  };
}

/**
 * The opener's answer to a window it asks for: the child, centred in `display`'s work area, and
 * nothing else (any other URL or target name is denied).
 *
 * @param hidden - Whether the child is offscreen and never shown (the harness's own proof).
 */
export function childOpenResponse(
  request: OpenRequest,
  display: DisplayInfo,
  hidden: boolean,
):
  | { readonly action: "deny" }
  | { readonly action: "allow"; readonly overrideBrowserWindowOptions: ChildWindowOptions } {
  if (request.frameName !== CHILD_FRAME_NAME || request.url !== "about:blank") {
    return { action: "deny" };
  }
  const { workArea } = display;
  return {
    action: "allow",
    overrideBrowserWindowOptions: {
      x: Math.round(workArea.x + (workArea.width - CHILD_SIZE.width) / 2),
      y: Math.round(workArea.y + (workArea.height - CHILD_SIZE.height) / 2),
      width: CHILD_SIZE.width,
      height: CHILD_SIZE.height,
      useContentSize: true,
      show: !hidden,
      title: "HYPERION R07.T21 child window",
      webPreferences: {
        sandbox: true,
        contextIsolation: true,
        nodeIntegration: false,
        offscreen: hidden,
        backgroundThrottling: false,
      },
    },
  };
}

/**
 * The opener's window-open handler: {@link childOpenResponse}'s answer to the first request it
 * allows, and a refusal of every request after it, so that the page has its one child once.
 */
export function childOpenHandler(
  display: DisplayInfo,
  hidden: boolean,
): (request: OpenRequest) => ReturnType<typeof childOpenResponse> {
  let opened = false;
  return (request) => {
    const response = opened
      ? ({ action: "deny" } as const)
      : childOpenResponse(request, display, hidden);
    opened ||= response.action === "allow";
    return response;
  };
}

/** The line a run's GPU-process exits give, and the exit code with them: one exit fails a pass. */
export function withGpuExits(
  exitCode: number,
  exits: number,
  codes: { readonly pass: number; readonly failed: number },
): { readonly line: string; readonly exitCode: number } {
  return {
    line: `${exits === 0 ? "PASS" : "FAIL"} T21 no GPU-process exit: ${String(exits)}`,
    exitCode: exitCode === codes.pass && exits > 0 ? codes.failed : exitCode,
  };
}

/** What heads the run's record. */
export interface ChildWindowRun {
  readonly startedAt: Date;
  readonly hidden: boolean;
  readonly displays: ReadonlyArray<DisplayInfo>;
  readonly mainDisplayId: number;
  readonly childDisplayId: number;
  readonly versions: { readonly electron: string; readonly chromium: string };
  readonly switches: ReadonlyArray<string>;
  /** The machine as the descent spike's records describe it, read at the run's start. */
  readonly machine: MachineDescription;
  readonly exitCode: number;
}

/** The machine's name in a record's file name, as the descent spike's results take it. */
export function machineName(): string {
  const name = hostname()
    .toLowerCase()
    .replaceAll(/[^a-z0-9-]/g, "");
  return name.length > 0 ? name : "machine";
}

/** The record's file name: `<date>-<machine>-child-window[-hidden].md`. */
export function recordName(run: ChildWindowRun, machine: string): string {
  return `${run.startedAt.toISOString().slice(0, 10)}-${machine}-child-window${run.hidden ? "-hidden" : ""}.md`;
}

/** The run's record: what it ran on, then the harness's own lines, each check with its figures. */
export function childWindowMarkdown(
  run: ChildWindowRun,
  machine: string,
  lines: ReadonlyArray<string>,
): string {
  const role = (display: DisplayInfo): string =>
    display.id === run.mainDisplayId
      ? " (the opener's)"
      : display.id === run.childDisplayId
        ? " (the child's)"
        : "";
  return [
    `# A child window on a second monitor (R07.T21): ${machine}, ${run.startedAt.toISOString().slice(0, 10)}`,
    "",
    `- **Run:** ${run.hidden ? "hidden, offscreen, on one display: the harness's own proof, not T21's record" : "shown, the child on the second display"}; exit ${String(run.exitCode)} (${run.exitCode === 0 ? "every check passed" : "a check failed or the run could not be set up"})`,
    `- **Versions:** Electron ${run.versions.electron}, Chromium ${run.versions.chromium}`,
    `- **Machine:** ${run.machine.cpu}, ${String(run.machine.logicalCores)} threads; GPU ${run.machine.gpu.value === null ? `unknown (${run.machine.gpu.reason})` : `${run.machine.gpu.value.description ?? `${String(run.machine.gpu.value.vendorId)}:${String(run.machine.gpu.value.deviceId)}`}, driver ${run.machine.gpu.value.driverVersion ?? "unknown"}`}; governor ${run.machine.governor.value ?? `unknown (${run.machine.governor.reason})`}`,
    `- **Load average at the start:** ${run.machine.loadAverage.map((value) => value.toFixed(2)).join(", ")}${run.machine.loadAverage[0] >= 1 ? " (provisional: under 1 asked)" : ""}`,
    `- **Switches:** ${run.switches.length === 0 ? "none" : run.switches.map((each) => `\`${each}\``).join(" ")}`,
    "",
    "## Displays",
    "",
    ...run.displays.map((display) => `- ${displayLine(display)}${role(display)}`),
    "",
    "## The harness's lines",
    "",
    "```text",
    ...lines,
    "```",
    "",
    "## For the owner, by eye",
    "",
    "- [ ] The child window stood on the second display and drew (a green marker in its top left on a dark ground).",
    "- [ ] The main window drew the whole time, before and after the child closed.",
    "- [ ] No GPU-process exit (the run's `T21 no GPU-process exit` line): this shown run is R07's on-screen check of a child window under the Vulkan surface (R07's Risks, \"Hidden-window resizes restart the GPU process\").",
    "",
  ].join("\n");
}
