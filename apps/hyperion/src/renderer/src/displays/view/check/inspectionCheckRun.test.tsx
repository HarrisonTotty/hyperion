import { RequestClient } from "@hyperion/protocol";
import { describe, expect, it, vi } from "vitest";

import {
  INSPECTION_CHECK_TIMING,
  inspectionCaptureLabel,
  recordInspectionCheck,
  runInspectionCheck,
} from "./inspectionCheckRun";

/** A body the launch names. */
const BODY = "61ffd967fe000023.0100";

/** The main process's rule for a capture's label (`viewsCheckRecord.ts`' `CAPTURE_LABEL`). */
const CAPTURE_LABEL = /^[a-z0-9-]{1,48}$/u;

/** A request client whose server answers every request with a list of no universe. */
function noUniverses(): RequestClient {
  const client: RequestClient = new RequestClient((message) => {
    if (message.type === "request") {
      queueMicrotask(() => {
        client.handleServerMessage({
          type: "response",
          id: message.id,
          body: { kind: "list_universes", universes: [], server_generator_version: 24 },
        });
      });
    }
    return true;
  });
  return client;
}

describe("the inspection run (the owner's request)", () => {
  it("names its capture for the body's system and index, as a capture may be named", () => {
    const label = inspectionCaptureLabel(BODY);
    expect(label).toBe("inspection-61ffd967fe000023-0100");
    expect(CAPTURE_LABEL.test(label)).toBe(true);
  });

  it("fails naming the universe where the server lists none of that ID", async () => {
    const openSystem = vi.fn<() => void>();
    await expect(
      runInspectionCheck({
        api: { capture: () => Promise.resolve() },
        views: { drawTimes: () => [] },
        requests: noUniverses(),
        served: { universe: "3504954ff4feabf4", body: BODY },
        openSystem,
        document,
        nowMs: () => 0,
        sleep: () => Promise.resolve(),
        signal: new AbortController().signal,
        timing: INSPECTION_CHECK_TIMING,
      }),
    ).rejects.toThrow("views check: the server lists no universe 3504954ff4feabf4");
    expect(openSystem).not.toHaveBeenCalled();
  });

  it("ends passed once its capture is taken, and failed with the reason it threw", async () => {
    const end = vi.fn<() => Promise<void>>(() => Promise.resolve());
    const signal = new AbortController().signal;
    vi.spyOn(console, "error").mockImplementation(() => undefined);

    await recordInspectionCheck(
      { end },
      () => Promise.resolve({ label: "a", frame: null, exposure: null, statements: [] }),
      signal,
    );
    await recordInspectionCheck(
      { end },
      () => Promise.reject(new Error("views check: VIEW BODY was held back")),
      signal,
    );

    expect(end.mock.calls).toEqual([
      [{ status: "pass" }],
      [{ status: "fail", reason: "views check: VIEW BODY was held back" }],
    ]);
  });
});
