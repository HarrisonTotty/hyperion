import type { ErrorCode } from "@hyperion/protocol";

/**
 * Codes by which the server refuses a request for what it asks, not because anything failed: the
 * operator's name is taken, the universe, system or body is not there, the universe is of another
 * generator version, the request was malformed. Every other code, and any the protocol adds,
 * reports a failed or overloaded system.
 */
const REFUSALS: ReadonlySet<string> = new Set<ErrorCode>([
  "bad_request",
  "name_taken",
  "unknown_universe",
  "unknown_system",
  "unknown_body",
  "generator_version_mismatch",
]);

/**
 * Whether the server refused a request for what it asked (a refusal, plain `--text`) rather than
 * because anything failed (a fault): the split every request status uses, a component's
 * (`RequestStatus`) or a store's (the terrain's `CoarseFieldStore`).
 */
export function isRefusal(code: string): boolean {
  return REFUSALS.has(code);
}
