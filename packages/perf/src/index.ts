/**
 * `@hyperion/perf`: HYPERION's performance record (plan R12). Pure TypeScript with no Node,
 * Electron or DOM import, shared by the client's main process and its renderer; the command-line
 * tools over `docs/measurements/rendering/runs.v1.jsonl` are under `src/cli/`.
 */

export * from "./record.ts";
export * from "./rows.ts";
export * from "./scenes.ts";
