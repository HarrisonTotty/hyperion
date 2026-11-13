import { describe, expect, it } from "vitest";

import type { SavedStill } from "../../../../preload/api";

import { FakeStillsApi } from "../../test/fakeStills";
import { rgbaPng } from "../../test/pngFixtures";
import { testStillLabel } from "../../test/stillFixtures";
import { readStillLabel, withStillLabel } from "./pngLabel";
import { StillStore } from "./stillStore";

describe("the client's saved stills (R11.T10.c)", () => {
  it("is listing until the first listing answers", () => {
    expect(new StillStore(new FakeStillsApi()).getSnapshot()).toEqual({ kind: "listing" });
  });

  it("saves a still with its label embedded in the PNG and as its sidecar", async () => {
    const api = new FakeStillsApi();
    const label = testStillLabel();
    const answer = await new StillStore(api).save(rgbaPng(16, 8), label);
    const saved = api.saved[0];
    expect([
      answer.kind,
      saved?.label,
      saved === undefined ? null : readStillLabel(saved.png),
    ]).toEqual(["saved", label, expect.objectContaining({ kind: "label", label })]);
  });

  it("lists the saved stills again once one is saved, newest first", async () => {
    const api = new FakeStillsApi();
    const store = new StillStore(api);
    await store.save(rgbaPng(16, 8), testStillLabel({ setting: "low" }));
    await store.save(rgbaPng(16, 8), testStillLabel());
    const listing = store.getSnapshot();
    expect(
      listing.kind === "listed" ? listing.stills.map(({ label }) => label.setting) : listing,
    ).toEqual(["high", "low"]);
  });

  it("tells its listeners of each listing", async () => {
    const store = new StillStore(new FakeStillsApi());
    let told = 0;
    store.subscribe(() => {
      told += 1;
    });
    await store.refresh();
    expect([told, store.getSnapshot()]).toEqual([1, { kind: "listed", stills: [] }]);
  });

  it("states a failed listing with the main process's words", async () => {
    const api = new FakeStillsApi();
    api.failListing = true;
    const store = new StillStore(api);
    await store.refresh();
    expect(store.getSnapshot()).toEqual({ kind: "failed", reason: "EACCES" });
  });

  it("keeps the latest listing's answer, not one that answers after it", async () => {
    // Two listings whose answers the test gives, the later first.
    const answers: Array<(stills: ReadonlyArray<SavedStill>) => void> = [];
    const api = new FakeStillsApi();
    api.listStills = () =>
      new Promise((resolve) => {
        answers.push(resolve);
      });
    const store = new StillStore(api);
    const first = store.refresh();
    const second = store.refresh();
    const [answerFirst, answerSecond] = answers;
    answerSecond?.([{ id: "20261113T000000000Z-00000002", label: testStillLabel() }]);
    await second;
    answerFirst?.([]);
    await first;
    const listing = store.getSnapshot();
    expect(listing.kind === "listed" ? listing.stills.map(({ id }) => id) : listing).toEqual([
      "20261113T000000000Z-00000002",
    ]);
  });

  it("does not list again after a refused save", async () => {
    const api = new FakeStillsApi();
    api.answer = { kind: "refused", reason: "oversized" };
    const answer = await new StillStore(api).save(rgbaPng(16, 8), testStillLabel());
    expect([answer, api.calls]).toEqual([{ kind: "refused", reason: "oversized" }, ["saveStill"]]);
  });

  it("reads a still back, its PNG's label equal to its sidecar's", async () => {
    const api = new FakeStillsApi();
    const store = new StillStore(api);
    const label = testStillLabel();
    const answer = await store.save(rgbaPng(16, 8), label);
    const id = answer.kind === "saved" ? answer.still.id : "";
    expect(await store.read(id)).toMatchObject({ kind: "read", still: { id, label } });
  });

  it("finds a PNG whose label is not its sidecar's", async () => {
    const api = new FakeStillsApi();
    api.saved.push({
      id: "20261113T000000000Z-00000001",
      png: withStillLabel(rgbaPng(16, 8), testStillLabel({ setting: "low" })),
      label: testStillLabel(),
    });
    expect(await new StillStore(api).read("20261113T000000000Z-00000001")).toEqual({
      kind: "mismatch",
      reason: "the PNG's label is not the sidecar's",
    });
  });

  it("finds a PNG with no label", async () => {
    const api = new FakeStillsApi();
    api.saved.push({ id: "a", png: rgbaPng(16, 8), label: testStillLabel() });
    expect(await new StillStore(api).read("a")).toEqual({
      kind: "mismatch",
      reason: "the PNG holds no label",
    });
  });

  it("reads a still that is not saved as gone", async () => {
    expect(await new StillStore(new FakeStillsApi()).read("missing")).toEqual({ kind: "gone" });
  });
});
