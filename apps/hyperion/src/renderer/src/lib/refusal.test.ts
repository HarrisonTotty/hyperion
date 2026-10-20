import type { ErrorCode } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { isRefusal } from "./refusal";

describe("isRefusal", () => {
  it.each<ErrorCode>([
    "bad_request",
    "name_taken",
    "unknown_universe",
    "unknown_system",
    "unknown_body",
    "generator_version_mismatch",
  ])("reads %s as a refusal of what the request asked", (code) => {
    expect(isRefusal(code)).toBe(true);
  });

  it.each<string>(["internal", "queue_full", "storage_failed", "a_code_the_protocol_adds_later"])(
    "reads %s as a fault, a failed or overloaded system",
    (code) => {
      expect(isRefusal(code)).toBe(false);
    },
  );
});
